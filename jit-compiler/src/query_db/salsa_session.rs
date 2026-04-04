//! Same-thread reuse of [`super::Database`] between `flattened_model_q` calls (Julia-style
//! incremental compilation within a process). Enabled by default; set `RUSTMODLICA_SALSA_PROCESS_DB=0` to disable.
//!
//! Reuse is gated by the same qualified key as [`super::flat_model::flattened_model_q`] and
//! [`crate::cache::closure_hash::deps_match`] on the dependency list recorded for that query.

use crate::cache::closure_hash;
use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::flatten::ValidationMode;
use crate::loader::ModelLoader;
use crate::query_db::QueryDb;
use std::cell::RefCell;
use std::sync::Arc;

thread_local! {
    static LAST_FLAT_MODEL_Q_DEPS: RefCell<Option<Vec<DepHashEntry>>> = const { RefCell::new(None) };
}

thread_local! {
    static SALSA_PROCESS_SESSION: RefCell<Option<SalsaProcessSession>> = const { RefCell::new(None) };
}

struct SalsaProcessSession {
    key: String,
    deps: Vec<DepHashEntry>,
    db: super::Database,
}

fn process_db_enabled() -> bool {
    // Default ON for process-internal Salsa DB reuse (Julia-style incremental compilation).
    // Set RUSTMODLICA_SALSA_PROCESS_DB=0 to disable explicitly.
    std::env::var("RUSTMODLICA_SALSA_PROCESS_DB")
        .ok()
        .map(|v| {
            let t = v.trim();
            !(t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no"))
        })
        .unwrap_or(true)
}

fn flat_model_q_key_string(
    loader: &ModelLoader,
    model_name: &str,
    coarse_constrainedby_only: bool,
    compile_stop: &str,
    validation_mode: ValidationMode,
) -> String {
    let mut db = super::Database::default();
    db.set_library_paths(Arc::new(loader.library_paths.clone()));
    db.set_coarse_constrainedby_only(coarse_constrainedby_only);
    db.set_compile_stop(Arc::new(compile_stop.to_string()));
    db.set_validation_mode(Arc::new(format!("{:?}", validation_mode)));
    super::flat_model_q_cache_key(&db, model_name.to_string()).1
}

/// Clears the last recorded flat_model_q dependency list (call at the start of `flattened_model_q`).
pub fn clear_last_flat_model_q_deps() {
    LAST_FLAT_MODEL_Q_DEPS.with(|c| {
        *c.borrow_mut() = None;
    });
}

pub fn record_last_flat_model_q_deps(deps: Vec<DepHashEntry>) {
    LAST_FLAT_MODEL_Q_DEPS.with(|c| {
        *c.borrow_mut() = Some(deps);
    });
}

pub fn take_last_flat_model_q_deps() -> Option<Vec<DepHashEntry>> {
    LAST_FLAT_MODEL_Q_DEPS.with(|c| c.borrow_mut().take())
}

/// If process-local reuse is enabled and the stored session matches `key` and disk deps, returns
/// the cached [`super::Database`]. Otherwise returns `None` (caller should use `Database::default()`).
pub fn take_reusable_database(
    loader: &ModelLoader,
    model_name: &str,
    coarse_constrainedby_only: bool,
    compile_stop: &str,
    validation_mode: ValidationMode,
) -> Option<super::Database> {
    if !process_db_enabled() {
        return None;
    }
    let want_key = flat_model_q_key_string(
        loader,
        model_name,
        coarse_constrainedby_only,
        compile_stop,
        validation_mode,
    );
    SALSA_PROCESS_SESSION.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(sess) = slot.take() else {
            return None;
        };
        if sess.key != want_key {
            return None;
        }
        if !closure_hash::deps_match(&sess.deps) {
            return None;
        }
        Some(sess.db)
    })
}

/// Stores `db` for reuse on this thread when `RUSTMODLICA_SALSA_PROCESS_DB` is enabled.
pub fn return_database(
    loader: &ModelLoader,
    model_name: &str,
    coarse_constrainedby_only: bool,
    compile_stop: &str,
    validation_mode: ValidationMode,
    db: super::Database,
    deps: Vec<DepHashEntry>,
) {
    if !process_db_enabled() {
        return;
    }
    let key = flat_model_q_key_string(
        loader,
        model_name,
        coarse_constrainedby_only,
        compile_stop,
        validation_mode,
    );
    SALSA_PROCESS_SESSION.with(|cell| {
        *cell.borrow_mut() = Some(SalsaProcessSession { key, deps, db });
    });
}
