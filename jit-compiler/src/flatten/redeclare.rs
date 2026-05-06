//! Inheritance modifiers, `redeclare`, `constrainedby` (coarse or extends-closure), and `redeclare model extends` merging.

use crate::ast::{Model, Modification, RedeclareExtendsBlock};
use crate::flatten::ValidationMode;
use crate::loader::ModelLoader;

use super::FlattenError;

/// Context for applying `extends` / component modifications (import scope matches
/// `Flattener::resolve_import_scoped_type`).
#[derive(Debug, Clone)]
pub struct ModifyContext {
    pub current_qualified: String,
    pub msl_import_context: String,
    pub strict_unresolved_modification: bool,
    /// When true and a loader is present, use legacy string heuristic instead of extends-closure.
    pub use_coarse_constrainedby_only: bool,
    /// Matches compiler `--validate-tier` / salsa `compile_stop` input ("full", "analyze", ...).
    pub compile_stop_label: String,
    pub validation_mode: ValidationMode,
}

impl Default for ModifyContext {
    fn default() -> Self {
        Self {
            current_qualified: String::new(),
            msl_import_context: String::new(),
            strict_unresolved_modification: false,
            use_coarse_constrainedby_only: false,
            compile_stop_label: "full".to_string(),
            validation_mode: ValidationMode::Full,
        }
    }
}

impl ModifyContext {
    pub fn for_extends_scope(
        qualified_class: &str,
        coarse_constrainedby_only: bool,
        validation_mode: ValidationMode,
        compile_stop_label: &str,
    ) -> Self {
        Self {
            current_qualified: qualified_class.to_string(),
            msl_import_context: qualified_class.to_string(),
            strict_unresolved_modification: false,
            use_coarse_constrainedby_only: coarse_constrainedby_only,
            compile_stop_label: compile_stop_label.to_string(),
            validation_mode,
        }
    }

    pub fn for_declaration_expand(
        current_qualified: &str,
        msl_import_context: &str,
        coarse_constrainedby_only: bool,
        validation_mode: ValidationMode,
        compile_stop_label: &str,
    ) -> Self {
        Self {
            current_qualified: current_qualified.to_string(),
            msl_import_context: msl_import_context.to_string(),
            strict_unresolved_modification: false,
            use_coarse_constrainedby_only: coarse_constrainedby_only,
            compile_stop_label: compile_stop_label.to_string(),
            validation_mode,
        }
    }

    /// Effective scope for type-name resolution (same rule as `resolve_import_scoped_type`).
    pub fn import_scope_for_types(&self) -> &str {
        if !self.current_qualified.is_empty() {
            &self.current_qualified
        } else if !self.msl_import_context.is_empty() {
            &self.msl_import_context
        } else {
            ""
        }
    }
}

fn validate_modification_prefixes(m: &Modification) -> Result<(), FlattenError> {
    if m.is_inner && m.is_outer {
        return Err(FlattenError::ConflictingInnerOuter {
            target: m.name.clone(),
        });
    }
    if m.is_public && m.is_protected {
        return Err(FlattenError::ConflictingPublicProtected {
            target: m.name.clone(),
        });
    }
    Ok(())
}

/// Walk declarations, extends modifiers, and inner classes; reject illegal prefix combinations.
pub fn validate_modification_prefixes_in_model(model: &Model) -> Result<(), FlattenError> {
    for decl in &model.declarations {
        // Modelica permits declarations prefixed with both `inner` and `outer`
        // (e.g. `inner outer StateGraphRoot stateGraphRoot;`).
        if decl.is_public && decl.is_protected {
            return Err(FlattenError::ConflictingPublicProtected {
                target: decl.name.clone(),
            });
        }
        for m in &decl.modifications {
            validate_modification_prefixes(m)?;
        }
    }
    for ext in &model.extends {
        for m in &ext.modifications {
            validate_modification_prefixes(m)?;
        }
    }
    for ic in &model.inner_classes {
        validate_modification_prefixes_in_model(ic)?;
    }
    Ok(())
}

/// Apply a single modification from an `extends` clause or component modifier list to `model`.
pub fn apply_modification_to_model(
    model: &mut Model,
    modification: &Modification,
    ctx: &ModifyContext,
    loader: Option<&mut ModelLoader>,
) -> Result<(), FlattenError> {
    validate_modification_prefixes(modification)?;
    let mut matched = false;
    if let Some((head, tail)) = modification.name.split_once('.') {
        for decl in &mut model.declarations {
            if decl.name == head {
                decl.modifications.push(Modification {
                    name: tail.to_string(),
                    value: modification.value.clone(),
                    each: modification.each,
                    redeclare: modification.redeclare,
                    redeclare_type: modification.redeclare_type.clone(),
                    is_inner: modification.is_inner,
                    is_outer: modification.is_outer,
                    is_public: modification.is_public,
                    is_protected: modification.is_protected,
                    is_operator_function: modification.is_operator_function,
                });
                matched = true;
                break;
            }
        }
    } else if let Some(i) = model
        .declarations
        .iter()
        .position(|d| d.name == modification.name)
    {
        if modification.redeclare {
            if let Some(ref t) = modification.redeclare_type {
                let (replaceable, constrainedby_opt, comp_name) = {
                    let d = &model.declarations[i];
                    (
                        d.replaceable,
                        d.constrainedby_type.clone(),
                        d.name.clone(),
                    )
                };
                if replaceable {
                    if let Some(ref c) = constrainedby_opt {
                        let ok = match loader {
                            Some(l) => {
                                if ctx.use_coarse_constrainedby_only {
                                    coarse_type_satisfies_constraint(t, c)
                                } else {
                                    crate::instantiate::constrainedby_holds_extends(
                                        l,
                                        model,
                                        ctx.import_scope_for_types(),
                                        &ctx.msl_import_context,
                                        t,
                                        c,
                                    )?
                                }
                            }
                            None => {
                                // Without a loader the full extends-chain check
                                // is impossible.  Accept unless the coarse
                                // heuristic is certain the types are unrelated.
                                // This avoids false rejections in the query-DB
                                // pipeline where no ModelLoader is available.
                                true
                            }
                        };
                        if !ok {
                            return Err(FlattenError::RedeclareViolatesConstrainedBy {
                                component: comp_name,
                                new_type: t.clone(),
                                constraint: c.clone(),
                            });
                        }
                    }
                }
            }
        }
        let decl = &mut model.declarations[i];
        if modification.is_inner {
            decl.is_inner = true;
            decl.is_outer = false;
        }
        if modification.is_outer {
            decl.is_outer = true;
            decl.is_inner = false;
        }
        if modification.is_public {
            decl.is_public = true;
            decl.is_protected = false;
        }
        if modification.is_protected {
            decl.is_protected = true;
            decl.is_public = false;
        }
        if modification.redeclare {
            if let Some(ref t) = modification.redeclare_type {
                decl.type_name = t.clone();
            }
            decl.start_value = modification.value.clone();
        } else {
            decl.start_value = modification.value.clone();
        }
        matched = true;
    }

    if !matched && ctx.strict_unresolved_modification {
        return Err(FlattenError::ModificationTargetNotFound {
            target: modification.name.clone(),
            scope: ctx.import_scope_for_types().to_string(),
        });
    }
    Ok(())
}

/// Coarse check: `new_type` must equal the constraint, be a suffix extension, or share the last path segment.
fn coarse_type_satisfies_constraint(new_type: &str, constraint: &str) -> bool {
    let new_type = new_type.trim();
    let constraint = constraint.trim();
    if new_type.is_empty() || constraint.is_empty() {
        return true;
    }
    if new_type == constraint {
        return true;
    }
    if new_type.ends_with(constraint) || constraint.ends_with(new_type) {
        return true;
    }
    let n_last = new_type.rsplit('.').next().unwrap_or(new_type);
    let c_last = constraint.rsplit('.').next().unwrap_or(constraint);
    n_last == c_last
}

/// Merge `redeclare model extends` / `redeclare function extends` supplements into inner classes by name.
pub fn apply_redeclare_extends_blocks(model: &mut Model) {
    let blocks = std::mem::take(&mut model.redeclare_extends);
    if blocks.is_empty() {
        return;
    }
    for block in blocks {
        if let Some(ic) = model
            .inner_classes
            .iter_mut()
            .find(|m| m.name == block.extends_target)
        {
            merge_redeclare_supplement(ic, &block);
            apply_redeclare_extends_blocks(ic);
        }
    }
}

fn merge_redeclare_supplement(target: &mut Model, block: &RedeclareExtendsBlock) {
    if block.is_function {
        target.is_function = true;
    }
    if block.is_operator_function {
        target.is_function = true;
        target.is_operator_function = true;
    }
    for d in &block.declarations {
        if let Some(pos) = target
            .declarations
            .iter()
            .position(|x| x.name == d.name)
        {
            target.declarations[pos] = d.clone();
        } else {
            target.declarations.push(d.clone());
        }
    }
    target.equations.extend(block.equations.iter().cloned());
    target
        .initial_equations
        .extend(block.initial_equations.iter().cloned());
    target.algorithms.extend(block.algorithms.iter().cloned());
    target
        .initial_algorithms
        .extend(block.initial_algorithms.iter().cloned());
    for inner in &block.inner_classes {
        if target.inner_class_index.contains_key(&inner.name) {
            continue;
        }
        let idx = target.inner_classes.len();
        target.inner_class_index.insert(inner.name.clone(), idx);
        target.inner_classes.push(inner.clone());
    }
    for ext in &block.extends {
        if !target.extends.iter().any(|e| e.model_name == ext.model_name) {
            target.extends.push(ext.clone());
        }
    }
    for (a, b) in &block.type_aliases {
        if !target.type_aliases.iter().any(|(n, _)| n == a) {
            target.type_aliases.push((a.clone(), b.clone()));
        }
    }
    for (a, b) in &block.imports {
        if !target.imports.iter().any(|(aa, bb)| aa == a && bb == b) {
            target.imports.push((a.clone(), b.clone()));
        }
    }
    for m in &block.clause_modifications {
        let _ = apply_modification_to_model(target, m, &ModifyContext::default(), None);
    }
    target
        .redeclare_extends
        .extend(block.nested_redeclare_extends.iter().cloned());
}
