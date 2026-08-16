use crate::ast::{Declaration, Equation, Expression, Model};
use crate::string_intern::resolve_id;
use crate::diag::SourceLocation;
use crate::loader::LoadError;
use crate::flatten::utils::{
    effective_primitive_alias, is_primitive, resolve_inner_class_alias, resolve_type_alias,
    split_primitive_array_type,
};
use crate::flatten::{apply_modification_to_model, ModifyContext};
use crate::flatten::{eval_const_expr_with_param_exprs, index_expression, FlattenError};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod array_dim;
mod env;
mod param_pass;

#[derive(Clone)]
enum ExpandDeclTask {
    Process {
        model: Arc<Model>,
        prefix: String,
        current_model_name: Option<String>,
        msl_import_context: String,
    },
    ExpandEquations {
        model: Arc<Model>,
        prefix: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExpandDeclMode {
    /// SuperFast: Only top-level declarations, no recursive sub-model loading
    SuperFast,
    /// DeclOnly: Expand declarations but skip sub-equations (kept for callers that
    /// intentionally separate eq expansion; salsa path uses DeclAndSubEq).
    DeclOnly,
    /// DeclAndSubEq: Full expansion including sub-model equations
    DeclAndSubEq,
}

fn type_is_visualizer_leaf(resolved_type: &str) -> bool {
    let t = resolved_type;
    t.contains("Visualizers")
        || t.contains(".Shapes.")
        || t.ends_with(".Shape")
        || t.ends_with(".Surface")
        || t.contains("FixedShape")
        || t.contains("Internal.Lines")
        || t.contains("Internal.Surface")
}

/// Types that must expand declarations on every instance (StateSelect / DOFs).
fn type_must_expand_every_instance(resolved_type: &str) -> bool {
    let t = resolved_type;
    t.contains(".Parts.Body")
        || t.contains("BodyShape")
        || t.contains("BodyBox")
        || t.contains("BodyCylinder")
        || t.contains(".Joints.")
        || t.contains("Interfaces.Frame")
        || t.contains("Quaternions.Orientation")
        || t.ends_with(".Body")
        || t.contains("Rotational.Interfaces.Flange")
        || t.contains("Translational.Interfaces.Flange")
        || t.contains("Modelica.StateGraph")
}

/// InitialStep `initial equation active = true` plus `localActive = pre(newActive)`.
fn initial_step_start(
    current_model_name: Option<&str>,
    decl_name: &str,
    start: Option<Expression>,
) -> Option<Expression> {
    if start.is_some() {
        return start;
    }
    let ty = current_model_name.unwrap_or("");
    if !ty.contains("InitialStep") {
        return None;
    }
    match decl_name {
        "newActive" | "localActive" | "active" | "oldActive" => Some(Expression::Number(1.0)),
        _ => None,
    }
}

fn push_primitive_alias_components(
    flat: &mut crate::flatten::FlattenedModel,
    full_path: &str,
    prim: &str,
    arr_n: Option<usize>,
    decl: &Declaration,
    start_value: Option<Expression>,
    state_select: Option<crate::ast::StateSelect>,
    array_index: Option<usize>,
) {
    let count = arr_n.unwrap_or(1).max(1);
    if let Some(n) = arr_n {
        flat.array_sizes.insert(full_path.to_string(), n);
    }
    for idx in 1..=count {
        let name = if arr_n.is_some() {
            format!("{full_path}_{idx}")
        } else {
            full_path.to_string()
        };
        let elem_start = start_value.as_ref().map(|sv| {
            if arr_n.is_some() {
                index_expression(sv, idx)
            } else {
                sv.clone()
            }
        });
        flat.declarations.push(leaf_declaration(
            prim.to_string(),
            name.clone(),
            decl,
            elem_start,
            state_select,
            decl.is_discrete,
        ));
        flat.register_inst_path(name, prim.to_string(), array_index.or(Some(idx)));
    }
}

fn is_literal_start_guess(expr: &Expression) -> bool {
    matches!(expr, Expression::Number(_))
}

/// Numeric start already known for a bound RHS (e.g. `condition=true` -> 1).
fn numeric_start_from_rhs(
    flat: &crate::flatten::FlattenedModel,
    rhs: &Expression,
) -> Option<Expression> {
    match rhs {
        Expression::Number(_) => Some(rhs.clone()),
        Expression::Variable(id) => {
            let name = resolve_id(*id);
            flat.declarations.iter().rev().find(|d| d.name == name).and_then(|d| {
                match &d.start_value {
                    Some(Expression::Number(_)) => d.start_value.clone(),
                    Some(e) => crate::flatten::eval_const_expr(e).map(Expression::Number),
                    None => None,
                }
            })
        }
        _ => None,
    }
}

/// `comp(u=expr)` on an input is a binding, not a start guess. Emit `u = expr`.
/// Short names are qualified on the instance (`condition` -> `transition1_condition`).
/// Nested modifiers that refer to the owner (`internalAxis(tau=tau)`) fall back
/// to the parent prefix (`revolute_tau`) when the instance-qualified name is self.
/// Returns the qualified RHS when an equation was emitted.
fn maybe_emit_input_binding(
    flat: &mut crate::flatten::FlattenedModel,
    full_path: &str,
    prefix: &str,
    is_input: bool,
    resolved_type: &str,
    start_value: &Option<Expression>,
) -> Option<Expression> {
    if !is_input {
        return None;
    }
    let sv = start_value.as_ref()?;
    if is_literal_start_guess(sv) {
        return None;
    }
    let mut rhs = qualify_short_vars(sv, prefix);
    if let Expression::Variable(id) = &rhs {
        if resolve_id(*id) == full_path {
            let parent = prefix.rsplit_once('_').map(|(p, _)| p).unwrap_or("");
            if parent.is_empty() || parent == prefix {
                return None;
            }
            rhs = qualify_short_vars(sv, parent);
        }
    }
    if let Expression::Variable(id) = &rhs {
        if resolve_id(*id) == full_path {
            return None;
        }
    }
    if is_literal_start_guess(&rhs) {
        return None;
    }
    let emitted = rhs.clone();
    // Boolean input bindings (`final localCondition=condition`) must stay as real
    // assignments. A bare `u = v` alias is dropped and `enableFire` still reads `u`.
    let emit_rhs = if resolved_type.eq_ignore_ascii_case("Boolean") {
        Expression::If(
            Box::new(Expression::Number(1.0)),
            Box::new(rhs),
            Box::new(Expression::Number(0.0)),
        )
    } else {
        rhs
    };
    flat.equations
        .push(Equation::Simple(Expression::var(full_path), emit_rhs));
    Some(emitted)
}

fn qualify_short_vars(expr: &Expression, parent: &str) -> Expression {
    if parent.is_empty() {
        return expr.clone();
    }
    match expr {
        Expression::Variable(id) => {
            let name = resolve_id(*id);
            if name.contains('_') || name.starts_with(parent) {
                expr.clone()
            } else {
                Expression::var(&format!("{parent}_{name}"))
            }
        }
        Expression::BinaryOp(l, op, r) => Expression::BinaryOp(
            Box::new(qualify_short_vars(l, parent)),
            *op,
            Box::new(qualify_short_vars(r, parent)),
        ),
        Expression::Call(f, args) => Expression::Call(
            f.clone(),
            args.iter().map(|a| qualify_short_vars(a, parent)).collect(),
        ),
        Expression::ArrayAccess(a, i) => Expression::ArrayAccess(
            Box::new(qualify_short_vars(a, parent)),
            Box::new(qualify_short_vars(i, parent)),
        ),
        Expression::If(c, t, f) => Expression::If(
            Box::new(qualify_short_vars(c, parent)),
            Box::new(qualify_short_vars(t, parent)),
            Box::new(qualify_short_vars(f, parent)),
        ),
        Expression::ArrayLiteral(es) => {
            Expression::ArrayLiteral(es.iter().map(|e| qualify_short_vars(e, parent)).collect())
        }
        Expression::Der(inner) => Expression::Der(Box::new(qualify_short_vars(inner, parent))),
        other => other.clone(),
    }
}

fn leaf_declaration(
    type_name: String,
    name: String,
    decl: &Declaration,
    start_value: Option<Expression>,
    state_select: Option<crate::ast::StateSelect>,
    is_discrete: bool,
) -> Declaration {
    Declaration {
        type_name,
        name,
        replaceable: decl.replaceable,
        constrainedby_type: decl.constrainedby_type.clone(),
        is_parameter: decl.is_parameter,
        is_flow: decl.is_flow,
        is_stream: decl.is_stream,
        is_discrete,
        is_input: decl.is_input,
        is_output: decl.is_output,
        is_inner: decl.is_inner,
        is_outer: decl.is_outer,
        is_public: decl.is_public,
        is_protected: decl.is_protected,
        start_value,
        array_size: None,
        modifications: Vec::new(),
        is_rest: decl.is_rest,
        annotation: None,
        condition: None,
        state_select,
    }
}

fn push_primitive_leaf(
    flat: &mut crate::flatten::FlattenedModel,
    full_path: &str,
    prefix: &str,
    resolved_type: &str,
    decl: &Declaration,
    start_value: Option<Expression>,
    state_select: Option<crate::ast::StateSelect>,
    array_index: Option<usize>,
) {
    let bound_rhs = maybe_emit_input_binding(
        flat,
        full_path,
        prefix,
        decl.is_input,
        resolved_type,
        &start_value,
    );
    let mut leaf_start = start_value;
    if let Some(rhs) = &bound_rhs {
        if let Some(n) = numeric_start_from_rhs(flat, rhs) {
            leaf_start = Some(n);
        }
    }
    flat.declarations.push(leaf_declaration(
        resolved_type.to_string(),
        full_path.to_string(),
        decl,
        leaf_start,
        state_select,
        decl.is_discrete,
    ));
    if bound_rhs.is_some() {
        if let Some(d) = flat.declarations.last_mut() {
            d.is_input = false;
        }
    }
    flat.register_inst_path(
        full_path.to_string(),
        resolved_type.to_string(),
        array_index,
    );
}

include!("flattener_impl.rs");
include!("flattener_load.rs");
include!("flattener_instance.rs");
