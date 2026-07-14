//! Same-thread reuse of [`super::Database`] between `flattened_model_q` calls (Julia-style
//! incremental compilation within a process). Enabled by default; set `RUSTMODLICA_SALSA_PROCESS_DB=0` to disable.
//!
//! Reuse is gated by the same qualified key as [`super::flat_model::flattened_model_q`] and
//! [`crate::cache::closure_hash::deps_match`] on the dependency list recorded for that query.
//! When a full compile completes, [`record_codegen_stable_hash`] stores the JIT codegen key on the
//! session so a later reuse with unchanged deps can skip redundant codegen work.
//!
//! ## Environment
//! - `RUSTMODLICA_SALSA_PROCESS_DB` — `0`/`false`/`no` disables reuse (default: on).
//! - `RUSTMODLICA_SALSA_PROCESS_DB_SLOTS` — max concurrent sessions per OS thread (default `4`, clamped to `1..=16`).
//! Use [`salsa_process_db_stats`] from tests or the IDE to read `(hits, misses, evictions)`.

use crate::cache::closure_hash;
use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::flatten::ValidationMode;
use crate::loader::ModelLoader;
use crate::query_db::QueryDb;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

thread_local! {
    static LAST_FLAT_MODEL_Q_DEPS: RefCell<Option<Vec<DepHashEntry>>> = const { RefCell::new(None) };
}

thread_local! {
    static SALSA_PROCESS_LRU: RefCell<Vec<SalsaProcessSession>> = const { RefCell::new(Vec::new()) };
}

thread_local! {
    static LAST_TAKE_HIT: RefCell<bool> = const { RefCell::new(false) };
}

thread_local! {
    static LAST_TAKE_CODEGEN_HASH: RefCell<Option<String>> = const { RefCell::new(None) };
}

static SALSA_DB_HIT: AtomicU64 = AtomicU64::new(0);
static SALSA_DB_MISS: AtomicU64 = AtomicU64::new(0);
static SALSA_DB_EVICTION: AtomicU64 = AtomicU64::new(0);

/// Counters: `(hits, misses, evictions)` for [`take_reusable_database`] / [`return_database`].
pub fn salsa_process_db_stats() -> (u64, u64, u64) {
    (
        SALSA_DB_HIT.load(Ordering::Relaxed),
        SALSA_DB_MISS.load(Ordering::Relaxed),
        SALSA_DB_EVICTION.load(Ordering::Relaxed),
    )
}

struct SalsaProcessSession {
    key: String,
    deps: Vec<DepHashEntry>,
    db: super::Database,
    codegen_stable_hash: Option<String>,
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

fn lru_capacity() -> usize {
    std::env::var("RUSTMODLICA_SALSA_PROCESS_DB_SLOTS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|n| n.clamp(1, 16))
        .unwrap_or(4)
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

/// True when the most recent [`take_reusable_database`] on this thread returned a cached session.
pub fn consume_salsa_process_db_hit() -> bool {
    LAST_TAKE_HIT.with(|c| {
        let hit = *c.borrow();
        *c.borrow_mut() = false;
        hit
    })
}

/// Codegen stable hash from the session returned by the last [`take_reusable_database`] hit.
pub fn take_salsa_codegen_stable_hash() -> Option<String> {
    LAST_TAKE_CODEGEN_HASH.with(|c| c.borrow_mut().take())
}

/// Associates `hash` with the in-LRU session for `model_name` after a successful codegen compile.
pub fn record_codegen_stable_hash(
    loader: &ModelLoader,
    model_name: &str,
    coarse_constrainedby_only: bool,
    compile_stop: &str,
    validation_mode: ValidationMode,
    hash: String,
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
    SALSA_PROCESS_LRU.with(|cell| {
        let mut slot = cell.borrow_mut();
        if let Some(sess) = slot.iter_mut().find(|s| s.key == key) {
            sess.codegen_stable_hash = Some(hash);
        }
    });
}

/// If process-local reuse is enabled and a stored session matches `key` and disk deps, returns
/// the cached [`super::Database`]. Otherwise returns `None` (caller should use `Database::default()`).
pub fn take_reusable_database(
    loader: &ModelLoader,
    model_name: &str,
    coarse_constrainedby_only: bool,
    compile_stop: &str,
    validation_mode: ValidationMode,
) -> Option<super::Database> {
    LAST_TAKE_HIT.with(|c| *c.borrow_mut() = false);
    LAST_TAKE_CODEGEN_HASH.with(|c| *c.borrow_mut() = None);
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
    SALSA_PROCESS_LRU.with(|cell| {
        let mut slot = cell.borrow_mut();
        if let Some(idx) = slot
            .iter()
            .position(|s| s.key == want_key && closure_hash::deps_match(&s.deps))
        {
            let sess = slot.remove(idx);
            SALSA_DB_HIT.fetch_add(1, Ordering::Relaxed);
            LAST_TAKE_HIT.with(|c| *c.borrow_mut() = true);
            LAST_TAKE_CODEGEN_HASH.with(|c| *c.borrow_mut() = sess.codegen_stable_hash);
            return Some(sess.db);
        }
        SALSA_DB_MISS.fetch_add(1, Ordering::Relaxed);
        None
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
    let cap = lru_capacity();
    SALSA_PROCESS_LRU.with(|cell| {
        let mut slot = cell.borrow_mut();
        let prev_codegen = slot
            .iter()
            .find(|s| s.key == key)
            .and_then(|s| s.codegen_stable_hash.clone());
        slot.retain(|s| s.key != key);
        slot.push(SalsaProcessSession {
            key,
            deps,
            db,
            codegen_stable_hash: prev_codegen,
        });
        while slot.len() > cap {
            slot.remove(0);
            SALSA_DB_EVICTION.fetch_add(1, Ordering::Relaxed);
        }
    });
}
