use crate::ast::{Expression, Model};
use crate::loader::ModelLoader;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) fn extract_named_record_field(expr: &Expression, field: &str) -> Option<Expression> {
    let Expression::Call(_, args) = expr else {
        return None;
    };
    for a in args {
        let Expression::Call(op, items) = a else {
            continue;
        };
        if op != "named" || items.len() != 2 {
            continue;
        }
        let (Expression::StringLiteral(nm), val) = (&items[0], &items[1]) else {
            continue;
        };
        if nm == field {
            return Some(val.clone());
        }
    }
    None
}

pub(super) fn search_record_field(
    model: &Model,
    field: &str,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    depth: u32,
) -> Option<Expression> {
    if depth > 8 {
        return None;
    }
    for ext in &model.extends {
        for m in &ext.modifications {
            if m.name == field {
                if let Some(v) = &m.value {
                    return Some(v.clone());
                }
            }
        }
    }
    for d in &model.declarations {
        if d.name == field {
            if let Some(v) = &d.start_value {
                return Some(v.clone());
            }
            for m in &d.modifications {
                if m.name == field {
                    if let Some(v) = &m.value {
                        return Some(v.clone());
                    }
                }
            }
        }
    }
    for ext in &model.extends {
        for parent_cand in super::rewrite::function_resolution_candidates(&ext.model_name) {
            let parent = cache
                .get(&parent_cand)
                .cloned()
                .or_else(|| loader.load_model(&parent_cand).ok());
            if let Some(parent_model) = parent {
                cache.insert(parent_cand, Arc::clone(&parent_model));
                if !parent_model.is_function {
                    if let Some(v) =
                        search_record_field(&parent_model, field, loader, cache, depth + 1)
                    {
                        return Some(v);
                    }
                }
            }
        }
    }
    None
}

pub(super) fn try_extract_record_constructor_dot_field(
    ctor_name: &str,
    args: &[Expression],
    field: &str,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
) -> Option<Expression> {
    if !args.is_empty() {
        return None;
    }
    for cand in super::rewrite::function_resolution_candidates(ctor_name) {
        let Some(model) = cache
            .get(&cand)
            .cloned()
            .or_else(|| loader.load_model(&cand).ok())
        else {
            continue;
        };
        cache.insert(cand.clone(), Arc::clone(&model));
        if !model.is_record && model.is_function {
            continue;
        }
        if let Some(v) = search_record_field(&model, field, loader, cache, 0) {
            return Some(v);
        }
    }
    None
}
