use std::collections::HashSet;
use std::sync::Arc;
use std::sync::{OnceLock, RwLock};
use std::collections::HashMap;

use crate::ast::Model;
use crate::flatten::FlattenError;
use crate::flatten::Flattener;
use crate::loader::ModelLoader;

thread_local! {
    static LOCAL_CONSTRAINEDBY_CACHE: std::cell::RefCell<HashMap<(String, String, String, String), bool>> =
        std::cell::RefCell::new(HashMap::new());
}

fn global_constrainedby_cache() -> &'static RwLock<HashMap<(String, String, String, String), bool>> {
    static GLOBAL: OnceLock<RwLock<HashMap<(String, String, String, String), bool>>> = OnceLock::new();
    GLOBAL.get_or_init(|| RwLock::new(HashMap::new()))
}

fn cache_get(key: &(String, String, String, String)) -> Option<bool> {
    if let Some(v) = LOCAL_CONSTRAINEDBY_CACHE.with(|c| c.borrow().get(key).copied()) {
        return Some(v);
    }
    if let Ok(g) = global_constrainedby_cache().read() {
        if let Some(v) = g.get(key).copied() {
            LOCAL_CONSTRAINEDBY_CACHE.with(|c| {
                c.borrow_mut().insert(key.clone(), v);
            });
            return Some(v);
        }
    }
    None
}

fn cache_put(key: (String, String, String, String), value: bool) {
    LOCAL_CONSTRAINEDBY_CACHE.with(|c| {
        c.borrow_mut().insert(key.clone(), value);
    });
    if let Ok(mut g) = global_constrainedby_cache().write() {
        const MAX_GLOBAL: usize = 10000;
        if g.len() >= MAX_GLOBAL && !g.contains_key(&key) {
            if let Some(k) = g.keys().next().cloned() {
                g.remove(&k);
            }
        }
        g.insert(key, value);
    }
}

fn load_class_for_redeclare(
    loader: &mut ModelLoader,
    scope_model: &Model,
    import_scope: &str,
    msl_context: &str,
    raw: &str,
) -> Result<Arc<Model>, FlattenError> {
    let raw = raw.trim();
    let short = raw.rsplit('.').next().unwrap_or(raw);
    if let Some(idx) = scope_model.inner_class_index.get(short) {
        return Ok(Arc::new(scope_model.inner_classes[*idx].clone()));
    }
    let resolved =
        Flattener::resolve_import_scoped_type(scope_model, raw, import_scope, msl_context);
    if resolved != raw {
        if let Some(idx) = scope_model.inner_class_index.get(resolved.as_str()) {
            return Ok(Arc::new(scope_model.inner_classes[*idx].clone()));
        }
    }
    if let Ok(m) = loader.load_model_silent(&resolved, true) {
        return Ok(m);
    }
    let q = Flattener::qualify_in_scope(import_scope, short);
    if let Ok(m) = loader.load_model_silent(&q, true) {
        return Ok(m);
    }
    let q2 = format!("{}.{}", import_scope, short);
    loader
        .load_model_silent(&q2, true)
        .map_err(FlattenError::Load)
}

fn resolve_extend_target(
    loader: &mut ModelLoader,
    scope_model: &Model,
    import_scope: &str,
    msl_context: &str,
    current: &Model,
    extend_name: &str,
) -> Result<Arc<Model>, FlattenError> {
    let raw = extend_name.trim();
    let short = raw.rsplit('.').next().unwrap_or(raw);
    if let Some(idx) = current.inner_class_index.get(short) {
        return Ok(Arc::new(current.inner_classes[*idx].clone()));
    }
    if let Some(idx) = scope_model.inner_class_index.get(short) {
        return Ok(Arc::new(scope_model.inner_classes[*idx].clone()));
    }
    let resolved =
        Flattener::resolve_import_scoped_type(current, raw, import_scope, msl_context);
    if let Ok(m) = loader.load_model_silent(&resolved, true) {
        return Ok(m);
    }
    let q = Flattener::qualify_in_scope(import_scope, short);
    if let Ok(m) = loader.load_model_silent(&q, true) {
        return Ok(m);
    }
    let q2 = format!("{}.{}", import_scope, short);
    loader
        .load_model_silent(&q2, true)
        .map_err(FlattenError::Load)
}

/// True iff `new_type` is the same class as `constraint` or inherits from it (extends closure).
pub fn constrainedby_holds_extends(
    loader: &mut ModelLoader,
    scope_model: &Model,
    import_scope: &str,
    msl_context: &str,
    new_type_raw: &str,
    constraint_raw: &str,
) -> Result<bool, FlattenError> {
    let new_type_raw = new_type_raw.trim();
    let constraint_raw = constraint_raw.trim();
    if new_type_raw.is_empty() || constraint_raw.is_empty() {
        return Ok(true);
    }

    // Cache key includes scope context because import resolution is scope-dependent.
    let key = (
        new_type_raw.to_string(),
        constraint_raw.to_string(),
        import_scope.to_string(),
        msl_context.to_string(),
    );
    if let Some(v) = cache_get(&key) {
        return Ok(v);
    }

    let target = load_class_for_redeclare(
        loader,
        scope_model,
        import_scope,
        msl_context,
        constraint_raw,
    )?;
    let start = load_class_for_redeclare(
        loader,
        scope_model,
        import_scope,
        msl_context,
        new_type_raw,
    )?;

    if start.name == target.name {
        return Ok(true);
    }

    let mut queue: Vec<Arc<Model>> = vec![start];
    let mut seen: HashSet<usize> = HashSet::new();

    while let Some(m) = queue.pop() {
        let p = Arc::as_ptr(&m) as usize;
        if !seen.insert(p) {
            continue;
        }
        for ext in &m.extends {
            let child = resolve_extend_target(
                loader,
                scope_model,
                import_scope,
                msl_context,
                m.as_ref(),
                &ext.model_name,
            )?;
            if child.name == target.name {
                cache_put(key, true);
                return Ok(true);
            }
            queue.push(child);
        }
    }

    cache_put(key, false);
    Ok(false)
}
