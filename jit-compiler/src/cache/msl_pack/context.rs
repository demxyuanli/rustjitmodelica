//! Thread-local session for stable cross-machine cache keys and relaxed deps checks
//! while an MSL tree matches a pre-baked pack (see `hydrate`).

use std::cell::RefCell;

thread_local! {
    static ACTIVE: RefCell<bool> = const { RefCell::new(false) };
    static VERSION_LABEL: RefCell<String> = const { RefCell::new(String::new()) };
    static TREE_DIGEST: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Stable fingerprint for [`super::super::cache_key::CacheKeyV2::libs_closure_hash`].
pub fn pack_libs_closure_digest() -> Option<String> {
    if !is_active() {
        return None;
    }
    let v = VERSION_LABEL.with(|c| c.borrow().clone());
    let t = TREE_DIGEST.with(|c| c.borrow().clone());
    if v.is_empty() || t.is_empty() {
        return None;
    }
    let mut h = xxhash_rust::xxh64::Xxh64::new(0);
    h.update(b"mslpack|");
    h.update(v.as_bytes());
    h.update(b"|");
    h.update(t.as_bytes());
    Some(format!("mslpack_{:016x}", h.digest()))
}

pub fn is_active() -> bool {
    ACTIVE.with(|c| *c.borrow())
}

pub fn version_label() -> String {
    VERSION_LABEL.with(|c| c.borrow().clone())
}

pub fn tree_digest() -> String {
    TREE_DIGEST.with(|c| c.borrow().clone())
}

/// Enable stable keys + relaxed query-cache deps for parse/model_ast.
pub fn session_activate(version_label: &str, tree_digest: &str) {
    VERSION_LABEL.with(|c| *c.borrow_mut() = version_label.to_string());
    TREE_DIGEST.with(|c| *c.borrow_mut() = tree_digest.to_string());
    ACTIVE.with(|c| *c.borrow_mut() = true);
}

pub fn session_deactivate() {
    ACTIVE.with(|c| *c.borrow_mut() = false);
    VERSION_LABEL.with(|c| c.borrow_mut().clear());
    TREE_DIGEST.with(|c| c.borrow_mut().clear());
}

pub fn relax_query_deps_for_stage(stage: &str, model_name: &str) -> bool {
    is_active()
        && model_name.starts_with("Modelica.")
        && (stage == "parse" || stage == "model_ast")
}

pub fn flat_cache_relax_deps_for(model_name: &str) -> bool {
    is_active() && model_name.starts_with("Modelica.")
}
