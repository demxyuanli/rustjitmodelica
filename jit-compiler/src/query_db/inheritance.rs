use crate::ast::{ExtendsClause, Model};
use crate::flatten::apply_redeclare_extends_blocks;
use crate::flatten::utils::{is_primitive, merge_models, qualify_short_type_names};
use crate::flatten::{apply_modification_to_model, FlattenError, ModifyContext};
use crate::query_db::QueryDb;
use std::sync::Arc;

pub(super) fn flatten_inheritance_pure(
    db: &dyn QueryDb,
    arc: &mut Arc<Model>,
    current_qualified: &str,
    deps: &mut Vec<crate::flatten::flat_cache_v1::DepHashEntry>,
    seen_paths: &mut std::collections::HashSet<String>,
) -> Result<(), FlattenError> {
    let model = Arc::make_mut(arc);
    apply_redeclare_extends_blocks(model);
    let extends = std::mem::take(&mut model.extends);
    if extends.is_empty() {
        return Ok(());
    }

    let mut models: Vec<Arc<Model>> = Vec::new();
    models.push(Arc::clone(arc)); // index 0 = root

    type Frame = (Option<usize>, usize, String, Vec<ExtendsClause>, usize);
    let mut stack: Vec<Frame> =
        vec![(None, 0, current_qualified.to_string(), extends, 0)];

    while let Some((parent_idx, current_idx, qual, ext, idx)) = stack.pop() {
        if idx >= ext.len() {
            if let Some(pidx) = parent_idx {
                let current_snapshot = models[current_idx].clone();
                let parent = Arc::make_mut(&mut models[pidx]);
                merge_models(parent, current_snapshot.as_ref());
            }
            continue;
        }
        let clause = &ext[idx];
        let current = models[current_idx].clone();
        let raw_extends = clause.model_name.trim();
        let (mut base_name, base_from_inner) = if !raw_extends.contains('.') {
            if let Some(ic) = current.as_ref().find_inner_class(raw_extends) {
                let bn = if qual.is_empty() {
                    raw_extends.to_string()
                } else {
                    format!("{}.{}", qual, raw_extends)
                };
                (bn, Some(Arc::new(ic.clone())))
            } else {
                let mut bn =
                    crate::flatten::Flattener::resolve_import_prefix(current.as_ref(), &clause.model_name, &qual);
                bn = crate::flatten::Flattener::qualify_in_scope(&qual, &bn);
                (bn, None)
            }
        } else {
            let mut bn =
                crate::flatten::Flattener::resolve_import_prefix(current.as_ref(), &clause.model_name, &qual);
            bn = crate::flatten::Flattener::qualify_in_scope(&qual, &bn);
            (bn, None)
        };
        if base_name.ends_with("ExternalObject") {
            stack.push((parent_idx, current_idx, qual, ext, idx + 1));
            continue;
        }

        let mut base_model = if let Some(m) = base_from_inner {
            m
        } else {
            let st = db.source_text(base_name.clone());
            if !st.path.is_empty() && seen_paths.insert(st.path.to_string()) {
                let h = super::semantic_hash_text(st.text.as_str());
                deps.push(crate::flatten::flat_cache_v1::DepHashEntry {
                    path: st.path.to_string(),
                    content_hash: h,
                });
            }
            let loaded = Arc::clone(&db.model_ast(base_name.clone()).model);
            if loaded.as_ref().declarations.is_empty()
                && loaded.as_ref().equations.is_empty()
                && loaded.as_ref().extends.is_empty()
                && loaded.as_ref().inner_classes.is_empty()
                && loaded.as_ref().imports.is_empty()
                && loaded.as_ref().type_aliases.is_empty()
            {
                let orig = crate::flatten::Flattener::resolve_import_prefix(current.as_ref(), &clause.model_name, &qual);
                let bare = if orig.contains('.') {
                    orig.split('.').next().unwrap_or(&orig).to_string()
                } else {
                    orig.clone()
                };
                let suffix = if bare != orig { &orig[bare.len()..] } else { "" };
                let mut found: Option<Arc<Model>> = None;
                let mut scope = qual.clone();
                while let Some((p, _)) = scope.rsplit_once('.') {
                    let candidate = format!("{}.{}{}", p, bare, suffix);
                    let st = db.source_text(candidate.clone());
                    if !st.path.is_empty() && seen_paths.insert(st.path.to_string()) {
                        let h = super::semantic_hash_text(st.text.as_str());
                        deps.push(crate::flatten::flat_cache_v1::DepHashEntry {
                            path: st.path.to_string(),
                            content_hash: h,
                        });
                    }
                    let cand = Arc::clone(&db.model_ast(candidate.clone()).model);
                    if !(cand.as_ref().declarations.is_empty()
                        && cand.as_ref().equations.is_empty()
                        && cand.as_ref().extends.is_empty()
                        && cand.as_ref().inner_classes.is_empty()
                        && cand.as_ref().imports.is_empty()
                        && cand.as_ref().type_aliases.is_empty())
                    {
                        base_name = candidate;
                        found = Some(cand);
                        break;
                    }
                    scope = p.to_string();
                }
                found.unwrap_or(loaded)
            } else {
                loaded
            }
        };

        // Use coarse_constrainedby_only=true because no ModelLoader is available
        // in the query path. The full constrainedby check with extends-chain
        // traversal requires a loader; without one apply_modification_to_model
        // would use the coarse string check anyway but with the original
        // (potentially too strict) setting. Force coarse here to avoid false
        // negatives from the string heuristic rejecting valid redeclarations.
        let mod_ctx = ModifyContext::for_extends_scope(
            &qual,
            true,
            crate::flatten::ValidationMode::parse(db.validation_mode().as_str()),
            db.compile_stop().as_ref().as_str(),
        );
        for modification in &clause.modifications {
            apply_modification_to_model(
                Arc::make_mut(&mut base_model),
                modification,
                &mod_ctx,
                None,
            )?;
        }
        apply_redeclare_extends_blocks(Arc::make_mut(&mut base_model));

        if let Some((base_pkg, _)) = base_name.rsplit_once('.') {
            qualify_short_types_query(db, Arc::make_mut(&mut base_model), base_pkg);
        }

        let base_extends = std::mem::take(&mut Arc::make_mut(&mut base_model).extends);
        let new_idx = models.len();
        models.push(base_model);
        stack.push((parent_idx, current_idx, qual, ext, idx + 1));
        stack.push((Some(current_idx), new_idx, base_name, base_extends, 0));
    }

    *arc = models.swap_remove(0);
    Ok(())
}

fn qualify_short_types_query(db: &dyn QueryDb, model: &mut Model, base_pkg: &str) {
    qualify_short_type_names(model, base_pkg, &mut |candidate: &str| {
        let ast = db.model_ast(candidate.to_string());
        !ast.model.declarations.is_empty()
            || !ast.model.equations.is_empty()
            || !ast.model.extends.is_empty()
            || !ast.model.inner_classes.is_empty()
    });
}

