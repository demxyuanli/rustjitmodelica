use crate::flatten::cache_sqlite;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

thread_local! {
    // Session-local (thread-local) cache: fastest path for repeated validates in a long-lived process.
    static LOCAL_ARRAY_SIZES_CACHE: std::cell::RefCell<HashMap<String, Arc<HashMap<String, usize>>>> =
        std::cell::RefCell::new(HashMap::new());
}

fn global_array_sizes_cache() -> &'static RwLock<HashMap<String, Arc<HashMap<String, usize>>>> {
    static GLOBAL: OnceLock<RwLock<HashMap<String, Arc<HashMap<String, usize>>>>> = OnceLock::new();
    GLOBAL.get_or_init(|| RwLock::new(HashMap::new()))
}

fn global_analyze_input_cache() -> &'static RwLock<HashMap<String, Arc<crate::flatten::FlattenedModel>>> {
    static GLOBAL: OnceLock<RwLock<HashMap<String, Arc<crate::flatten::FlattenedModel>>>> = OnceLock::new();
    GLOBAL.get_or_init(|| RwLock::new(HashMap::new()))
}

fn global_inline_result_cache() -> &'static RwLock<HashMap<String, Arc<crate::flatten::FlattenedModel>>> {
    static GLOBAL: OnceLock<RwLock<HashMap<String, Arc<crate::flatten::FlattenedModel>>>> = OnceLock::new();
    GLOBAL.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn analyze_input_mem_get(key: &str) -> Option<Arc<crate::flatten::FlattenedModel>> {
    if let Ok(g) = global_analyze_input_cache().read() {
        return g.get(key).cloned();
    }
    None
}

pub fn analyze_input_mem_put(key: &str, v: Arc<crate::flatten::FlattenedModel>) {
    if let Ok(mut g) = global_analyze_input_cache().write() {
        const MAX_ENTRIES: usize = 256;
        if g.len() >= MAX_ENTRIES && !g.contains_key(key) {
            if let Some(k) = g.keys().next().cloned() {
                g.remove(&k);
            }
        }
        g.insert(key.to_string(), v);
    }
}

pub fn inline_result_mem_get(key: &str) -> Option<Arc<crate::flatten::FlattenedModel>> {
    if let Ok(g) = global_inline_result_cache().read() {
        return g.get(key).cloned();
    }
    None
}

pub fn inline_result_mem_put(key: &str, v: Arc<crate::flatten::FlattenedModel>) {
    if let Ok(mut g) = global_inline_result_cache().write() {
        const MAX_ENTRIES: usize = 256;
        if g.len() >= MAX_ENTRIES && !g.contains_key(key) {
            if let Some(k) = g.keys().next().cloned() {
                g.remove(&k);
            }
        }
        g.insert(key.to_string(), v);
    }
}

fn perf_trace_enabled() -> bool {
    std::env::var("RUSTMODLICA_PERF_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

#[derive(Default, Clone)]
struct CacheCounters {
    hits: u64,
    misses: u64,
    evictions: u64,
}

static COUNTERS: OnceLock<RwLock<CacheCounters>> = OnceLock::new();

fn counters() -> &'static RwLock<CacheCounters> {
    COUNTERS.get_or_init(|| RwLock::new(CacheCounters::default()))
}

fn inc_hit() {
    if !perf_trace_enabled() {
        return;
    }
    if let Ok(mut c) = counters().write() {
        c.hits += 1;
    }
}

fn inc_miss() {
    if !perf_trace_enabled() {
        return;
    }
    if let Ok(mut c) = counters().write() {
        c.misses += 1;
    }
}

fn inc_evict() {
    if !perf_trace_enabled() {
        return;
    }
    if let Ok(mut c) = counters().write() {
        c.evictions += 1;
    }
}

pub(super) fn mem_cache_get(key: &str) -> Option<Arc<HashMap<String, usize>>> {
    // Optional TTL: when enabled, clear thread-local cache entries periodically to avoid staleness
    // in long-lived IDE processes. Default is off to keep behavior unchanged.
    const TTL_ENV: &str = "RUSTMODLICA_FLATTEN_CACHE_TTL_MS";
    thread_local! {
        static LOCAL_LAST_CLEAR_MS: std::cell::Cell<u128> = std::cell::Cell::new(0);
    }
    if let Ok(ttl_str) = std::env::var(TTL_ENV) {
        if let Ok(ttl_ms) = ttl_str.trim().parse::<u128>() {
            if ttl_ms > 0 {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                let last = LOCAL_LAST_CLEAR_MS.with(|c| c.get());
                if last == 0 || now.saturating_sub(last) >= ttl_ms {
                    LOCAL_LAST_CLEAR_MS.with(|c| c.set(now));
                    LOCAL_ARRAY_SIZES_CACHE.with(|c| c.borrow_mut().clear());
                }
            }
        }
    }
    if let Some(v) = LOCAL_ARRAY_SIZES_CACHE.with(|c| c.borrow().get(key).cloned()) {
        inc_hit();
        return Some(v);
    }
    if let Ok(g) = global_array_sizes_cache().read() {
        if let Some(v) = g.get(key).cloned() {
            // Promote into local cache.
            LOCAL_ARRAY_SIZES_CACHE.with(|c| {
                c.borrow_mut().insert(key.to_string(), v.clone());
            });
            inc_hit();
            return Some(v);
        }
    }
    inc_miss();
    None
}

pub(super) fn mem_cache_put(key: &str, sizes: Arc<HashMap<String, usize>>) {
    LOCAL_ARRAY_SIZES_CACHE.with(|c| {
        c.borrow_mut().insert(key.to_string(), sizes.clone());
    });
    if let Ok(mut g) = global_array_sizes_cache().write() {
        // Simple bounded growth guard (avoid unbounded memory in IDE sessions).
        const MAX_GLOBAL_ENTRIES: usize = 2048;
        if g.len() >= MAX_GLOBAL_ENTRIES && !g.contains_key(key) {
            // Remove an arbitrary key (HashMap iteration order is fine for a soft bound).
            if let Some(k) = g.keys().next().cloned() {
                g.remove(&k);
                inc_evict();
            }
        }
        g.insert(key.to_string(), sizes);
    }
}

pub fn array_sizes_cache_counters_snapshot_reset() -> Option<(u64, u64, u64)> {
    if !perf_trace_enabled() {
        return None;
    }
    if let Ok(mut c) = counters().write() {
        let out = (c.hits, c.misses, c.evictions);
        c.hits = 0;
        c.misses = 0;
        c.evictions = 0;
        return Some(out);
    }
    None
}

/// Software install root: `RUSTMODLICA_INSTALL_ROOT`, else directory of `current_exe`, stepping out of a final `bin` segment.
fn software_install_root() -> Option<PathBuf> {
    const ROOT_ENV: &str = "RUSTMODLICA_INSTALL_ROOT";
    if let Ok(s) = std::env::var(ROOT_ENV) {
        let t = s.trim();
        if !t.is_empty() {
            return Some(PathBuf::from(t));
        }
    }
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?.to_path_buf();
    if dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("bin"))
        .unwrap_or(false)
    {
        dir.pop();
    }
    Some(dir)
}

/// Detect a project-level cache directory: if CWD contains a `build/` directory
/// or project marker (e.g. `package.mo`, `.modelica-project`), use `<cwd>/build/cache`.
fn dev_project_cache_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let build_dir = cwd.join("build");
    let markers = ["package.mo", ".modelica-project", "modelica.toml"];
    let has_marker = markers.iter().any(|m| cwd.join(m).exists());
    if build_dir.is_dir() || has_marker {
        Some(build_dir.join("cache"))
    } else {
        None
    }
}

fn default_flatten_cache_dir() -> Option<PathBuf> {
    dev_project_cache_dir().or_else(|| software_install_root().map(|r| r.join("cache")))
}

/// On-disk cache root for flatten hints, SQLite tiers, and IR epoch stamp.
///
/// - If `RUSTMODLICA_FLATTEN_CACHE_DIR` is set to a non-empty path, that path is used.
/// - If it is unset or empty, uses `<software_install_root>/cache` (see [`software_install_root`]).
/// - Set `RUSTMODLICA_FLATTEN_CACHE_DIR` to `0`, `false`, `no`, or `none` to disable the disk root.
pub fn flatten_cache_dir() -> Option<PathBuf> {
    const ENV: &str = "RUSTMODLICA_FLATTEN_CACHE_DIR";
    match std::env::var(ENV) {
        Ok(s) => {
            let t = s.trim();
            if t.is_empty() {
                default_flatten_cache_dir()
            } else if t == "0"
                || t.eq_ignore_ascii_case("false")
                || t.eq_ignore_ascii_case("no")
                || t.eq_ignore_ascii_case("none")
            {
                None
            } else {
                Some(PathBuf::from(t))
            }
        }
        Err(_) => default_flatten_cache_dir(),
    }
}

fn env_path_or_disabled(var: &str, default_if_unset: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    match std::env::var(var) {
        Ok(s) => {
            let t = s.trim();
            if t.is_empty() {
                default_if_unset()
            } else if t == "0"
                || t.eq_ignore_ascii_case("false")
                || t.eq_ignore_ascii_case("no")
                || t.eq_ignore_ascii_case("none")
            {
                None
            } else {
                Some(PathBuf::from(t))
            }
        }
        Err(_) => default_if_unset(),
    }
}

fn default_std_cache_root() -> Option<PathBuf> {
    std::env::var("LOCALAPPDATA")
        .ok()
        .map(|p| PathBuf::from(p).join("rustmodlica").join("std-cache"))
}

fn default_user_cache_root() -> Option<PathBuf> {
    std::env::var("APPDATA")
        .ok()
        .map(|p| PathBuf::from(p).join("rustmodlica").join("user-cache"))
}

/// Global standard-library tier cache root (L0 SQLite + artifacts). Override with `RUSTMODLICA_STD_CACHE_ROOT`.
pub fn std_cache_root() -> Option<PathBuf> {
    env_path_or_disabled("RUSTMODLICA_STD_CACHE_ROOT", default_std_cache_root)
}

/// Shared user-extension tier cache root (L1). Override with `RUSTMODLICA_USER_CACHE_ROOT`.
pub fn user_cache_root() -> Option<PathBuf> {
    env_path_or_disabled("RUSTMODLICA_USER_CACHE_ROOT", default_user_cache_root)
}

/// All configured on-disk cache roots (project, user extension, global std) for purge / invalidation.
pub fn all_disk_cache_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let mut add = |p: Option<PathBuf>| {
        if let Some(p) = p {
            if !v.iter().any(|x| x == &p) {
                v.push(p);
            }
        }
    };
    add(flatten_cache_dir());
    add(user_cache_root());
    add(std_cache_root());
    v
}

const IR_SCHEMA_EPOCH_STAMP: &str = "ir_schema_epoch.txt";

fn clear_flatten_mem_caches_for_disk_purge() {
    LOCAL_ARRAY_SIZES_CACHE.with(|c| c.borrow_mut().clear());
    if let Ok(mut g) = global_array_sizes_cache().write() {
        g.clear();
    }
    if let Ok(mut g) = global_analyze_input_cache().write() {
        g.clear();
    }
    if let Ok(mut g) = global_inline_result_cache().write() {
        g.clear();
    }
}

/// If `ir_schema_epoch.txt` under the cache root disagrees with [`crate::cache::ir_epoch::IR_SCHEMA_EPOCH`],
/// drops the entire cache directory tree (SQLite + JSON hints), clears pooled DB handles and in-process
/// flatten mem caches, then writes the stamp. If the stamp is missing, creates the root and writes it
/// without deleting existing files (first use or pre-stamp caches).
pub fn sync_flatten_cache_root_ir_epoch(cache_root: &Path) {
    use crate::cache::ir_epoch::IR_SCHEMA_EPOCH;
    let stamp_path = cache_root.join(IR_SCHEMA_EPOCH_STAMP);
    let want = IR_SCHEMA_EPOCH.to_string();
    if let Ok(s) = std::fs::read_to_string(&stamp_path) {
        if s.trim() == want.as_str() {
            return;
        }
        clear_flatten_mem_caches_for_disk_purge();
        cache_sqlite::sqlite_connection_pool_clear();
        let _ = std::fs::remove_dir_all(cache_root);
    }
    let _ = std::fs::create_dir_all(cache_root);
    let _ = std::fs::write(&stamp_path, format!("{}\n", want));
}
