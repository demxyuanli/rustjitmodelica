//! Optional on-disk hints for `FlattenedModel::array_sizes` keyed by model inputs (see `RUSTMODLICA_FLATTEN_CACHE_DIR`).

use super::ArraySizePolicy;
use crate::flatten::ValidationMode;
use crate::flatten::cache_sqlite;
use crate::flatten::cache_shm;
use crate::loader::ModelLoader;
use crate::flatten::flat_cache_v1::{DepHashEntry, FlatCacheV1, FLAT_CACHE_SCHEMA_V1};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
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

fn mem_cache_get(key: &str) -> Option<Arc<HashMap<String, usize>>> {
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

fn mem_cache_put(key: &str, sizes: Arc<HashMap<String, usize>>) {
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

pub fn flatten_cache_dir() -> Option<PathBuf> {
    std::env::var("RUSTMODLICA_FLATTEN_CACHE_DIR")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

pub const ARRAY_SIZES_CACHE_SCHEMA_V2: &str = "rustmodlica_array_sizes_cache_v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArraySizesCacheV2 {
    pub schema: String,
    pub key: String,
    pub sizes: HashMap<String, usize>,
    pub deps: Vec<DepHashEntry>,
}

fn array_sizes_cache_v2_key(key: &str) -> String {
    format!("array_sizes_v2:{}", key)
}

pub fn flatten_array_sizes_cache_key(
    model_name: &str,
    loader: &ModelLoader,
    array_sizes_json: Option<&Path>,
    array_size_policy: ArraySizePolicy,
    warnings_level: &str,
) -> String {
    let mut h = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut h);
    model_name.hash(&mut h);
    match array_size_policy {
        ArraySizePolicy::Legacy => 0u8.hash(&mut h),
        ArraySizePolicy::Strict => 1u8.hash(&mut h),
    }
    warnings_level.hash(&mut h);
    if let Some(p) = loader.get_path_for_model(model_name) {
        if let Ok(data) = std::fs::read(p) {
            data.hash(&mut h);
        }
    }
    if let Some(jp) = array_sizes_json {
        if let Ok(data) = std::fs::read(jp) {
            data.hash(&mut h);
        }
    }
    format!("{:016x}", h.finish())
}

fn full_cache_enabled() -> bool {
    std::env::var("RUSTMODLICA_FLATTEN_FULL_CACHE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

pub fn flatten_full_cache_key(
    model_name: &str,
    loader: &ModelLoader,
    validation_mode: ValidationMode,
    compile_stop: &str,
    coarse_constrainedby_only: bool,
    array_sizes_json: Option<&Path>,
    array_size_policy: ArraySizePolicy,
    warnings_level: &str,
) -> String {
    let mut h = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut h);
    model_name.hash(&mut h);
    format!("{:?}", validation_mode).hash(&mut h);
    compile_stop.hash(&mut h);
    coarse_constrainedby_only.hash(&mut h);
    match array_size_policy {
        ArraySizePolicy::Legacy => 0u8.hash(&mut h),
        ArraySizePolicy::Strict => 1u8.hash(&mut h),
    }
    warnings_level.hash(&mut h);
    let mut libs: Vec<String> = loader.library_paths.iter().map(|p| {
        let mut s = p.display().to_string();
        // Normalize for Windows: stable separators and casing.
        s = s.replace('\\', "/");
        s = s.to_ascii_lowercase();
        s
    }).collect();
    libs.sort();
    libs.hash(&mut h);
    if let Some(p) = loader.get_path_for_model(model_name) {
        if let Ok(data) = std::fs::read(p) {
            data.hash(&mut h);
        }
    }
    if let Some(jp) = array_sizes_json {
        if let Ok(data) = std::fs::read(jp) {
            data.hash(&mut h);
        }
    }
    format!("{:016x}", h.finish())
}

fn file_hash_hex(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let mut h = DefaultHasher::new();
    data.hash(&mut h);
    Some(format!("{:016x}", h.finish()))
}

fn deps_match(deps: &[DepHashEntry]) -> bool {
    let t0 = std::time::Instant::now();
    for dep in deps {
        let p = PathBuf::from(dep.path.as_str());
        let Some(actual) = file_hash_hex(&p) else {
            crate::query_db::perf_record_us(
                "cache_deps_match_us",
                t0.elapsed().as_micros() as u64,
            );
            return false;
        };
        if actual != dep.content_hash {
            crate::query_db::perf_record_us(
                "cache_deps_match_us",
                t0.elapsed().as_micros() as u64,
            );
            return false;
        }
    }
    crate::query_db::perf_record_us("cache_deps_match_us", t0.elapsed().as_micros() as u64);
    true
}

pub fn try_read_flat_cache_v1(
    dir: &Path,
    key: &str,
    loader: &ModelLoader,
) -> Option<crate::flatten::FlattenedModel> {
    if !full_cache_enabled() {
        return None;
    }
    // Tier 2: cross-process shared memory (fastest cross-process path).
    if let Some(bytes) = cache_shm::shm_get(key) {
        crate::query_db::perf_record_us("cache_get_us", 0);
        let t_deser = std::time::Instant::now();
        if let Ok(cache) = bincode::deserialize::<FlatCacheV1>(&bytes) {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
            if cache.schema == FLAT_CACHE_SCHEMA_V1
                && cache.key == key
                && deps_match(&cache.deps)
            {
                let _ = loader;
                return Some(cache.into_flat_model());
            }
        } else {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
        }
    }
    // Tier 3: SQLite persistent store (optional).
    if let Some(cfg) = cache_sqlite::sqlite_config(Some(dir)) {
        let t_get = std::time::Instant::now();
        if let Ok(Some(bytes)) = cache_sqlite::sqlite_get(&cfg.path, key, "flat_cache_v1") {
            crate::query_db::perf_record_us("cache_get_us", t_get.elapsed().as_micros() as u64);
            let t_deser = std::time::Instant::now();
            if let Ok(cache) = bincode::deserialize::<FlatCacheV1>(&bytes) {
                crate::query_db::perf_record_us(
                    "cache_deserialize_us",
                    t_deser.elapsed().as_micros() as u64,
                );
                if cache.schema == FLAT_CACHE_SCHEMA_V1
                    && cache.key == key
                    && deps_match(&cache.deps)
                {
                    let _ = loader;
                    // Promote into shared memory for subsequent processes.
                    let _ = cache_shm::shm_put(key, &bytes);
                    return Some(cache.into_flat_model());
                }
            } else {
                crate::query_db::perf_record_us(
                    "cache_deserialize_us",
                    t_deser.elapsed().as_micros() as u64,
                );
            }
        }
    }
    // Legacy compatibility: read old JSON file and migrate to SHM/SQLite.
    let path = dir.join(format!("{}.flat-cache.json", key));
    if path.is_file() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(cache) = serde_json::from_str::<FlatCacheV1>(&text) {
                if cache.schema == FLAT_CACHE_SCHEMA_V1
                    && cache.key == key
                    && deps_match(&cache.deps)
                {
                    if let Ok(bytes) = bincode::serialize(&cache) {
                        let _ = cache_shm::shm_put(key, &bytes);
                        if let Some(cfg) = cache_sqlite::sqlite_config(Some(dir)) {
                            let deps_json = serde_json::to_string(&cache.deps).ok();
                            let _ = cache_sqlite::sqlite_put(
                                &cfg.path,
                                key,
                                FLAT_CACHE_SCHEMA_V1,
                                "flat_cache_v1",
                                &bytes,
                                deps_json.as_deref(),
                            );
                        }
                    }
                    return Some(cache.into_flat_model());
                }
            }
        }
    }
    None
}

pub fn write_flat_cache_v1(
    dir: &Path,
    key: &str,
    model_name: &str,
    flat: &crate::flatten::FlattenedModel,
    deps: &[PathBuf],
) -> Result<(), String> {
    if !full_cache_enabled() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut entries: Vec<DepHashEntry> = Vec::with_capacity(deps.len());
    for p in deps {
        let Some(h) = file_hash_hex(p.as_path()) else {
            continue;
        };
        entries.push(DepHashEntry {
            path: p.display().to_string(),
            content_hash: h,
        });
    }
    let cache = FlatCacheV1::from_flat_model(key.to_string(), model_name, flat, entries);
    if let Some(cfg) = cache_sqlite::sqlite_config(Some(dir)) {
        let bytes = bincode::serialize(&cache).map_err(|e| e.to_string())?;
        let _ = cache_shm::shm_put(key, &bytes);
        let deps_json = serde_json::to_string(&cache.deps).map_err(|e| e.to_string())?;
        let _ = cache_sqlite::sqlite_put(
            &cfg.path,
            key,
            FLAT_CACHE_SCHEMA_V1,
            "flat_cache_v1",
            &bytes,
            Some(deps_json.as_str()),
        );
        return Ok(());
    }
    // No SQLite config available; persist only in shared memory.
    let bytes = bincode::serialize(&cache).map_err(|e| e.to_string())?;
    let _ = cache_shm::shm_put(key, &bytes);
    Ok(())
}

fn hot_full_cache() -> &'static RwLock<HashMap<String, Arc<crate::flatten::FlattenedModel>>> {
    static HOT: OnceLock<RwLock<HashMap<String, Arc<crate::flatten::FlattenedModel>>>> = OnceLock::new();
    HOT.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn get_or_compute_flattened_model_v1<F>(
    dir: &Path,
    key: &str,
    loader: &ModelLoader,
    compute: F,
) -> Result<crate::flatten::FlattenedModel, crate::flatten::FlattenError>
where
    F: FnOnce() -> Result<crate::flatten::FlattenedModel, crate::flatten::FlattenError>,
{
    if let Ok(h) = hot_full_cache().read() {
        if let Some(v) = h.get(key) {
            return Ok((**v).clone());
        }
    }
    if let Some(v) = try_read_flat_cache_v1(dir, key, loader) {
        if let Ok(mut h) = hot_full_cache().write() {
            h.insert(key.to_string(), Arc::new(v.clone()));
        }
        return Ok(v);
    }
    let v = compute()?;
    if let Ok(mut h) = hot_full_cache().write() {
        h.insert(key.to_string(), Arc::new(v.clone()));
    }
    Ok(v)
}

pub fn merge_cached_array_sizes(
    dir: &Path,
    key: &str,
    external: &mut HashMap<String, usize>,
) -> Result<(), String> {
    if let Some(mem) = mem_cache_get(key) {
        for (k, v) in mem.as_ref() {
            external.entry(k.clone()).or_insert(*v);
        }
        return Ok(());
    }

    let k2 = array_sizes_cache_v2_key(key);
    if let Some(bytes) = cache_shm::shm_get(k2.as_str()) {
        if let Ok(cache) = bincode::deserialize::<ArraySizesCacheV2>(&bytes) {
            if cache.schema == ARRAY_SIZES_CACHE_SCHEMA_V2 && cache.key == k2 {
                for (k, v) in cache.sizes {
                    external.entry(k).or_insert(v);
                }
                mem_cache_put(key, Arc::new(external.clone()));
                return Ok(());
            }
        }
    }
    if let Some(cfg) = cache_sqlite::sqlite_config(Some(dir)) {
        if let Ok(Some(bytes)) = cache_sqlite::sqlite_get(&cfg.path, k2.as_str(), "array_sizes_v2") {
            if let Ok(cache) = bincode::deserialize::<ArraySizesCacheV2>(&bytes) {
                if cache.schema == ARRAY_SIZES_CACHE_SCHEMA_V2 && cache.key == k2 {
                    let _ = cache_shm::shm_put(k2.as_str(), &bytes);
                    for (k, v) in cache.sizes {
                        external.entry(k).or_insert(v);
                    }
                    mem_cache_put(key, Arc::new(external.clone()));
                    return Ok(());
                }
            }
        }
    }

    // Legacy compatibility: old per-entry JSON file cache.
    // Read once and migrate into sqlite/shm to avoid repeated small-file I/O.
    let legacy_path = dir.join(format!("{}.array-sizes.json", key));
    if legacy_path.is_file() {
        if let Ok(text) = std::fs::read_to_string(&legacy_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(obj) = v.get("array_sizes").and_then(|x| x.as_object()) {
                    let mut migrated: HashMap<String, usize> = HashMap::new();
                    for (k, val) in obj {
                        if let Some(n) = val.as_u64() {
                            if n > 0 && n <= usize::MAX as u64 {
                                migrated.insert(k.clone(), n as usize);
                            }
                        }
                    }
                    if !migrated.is_empty() {
                        for (k, v) in migrated.iter() {
                            external.entry(k.clone()).or_insert(*v);
                        }
                        let _ = write_array_sizes_cache(dir, key, &migrated);
                        mem_cache_put(key, Arc::new(external.clone()));
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn write_array_sizes_cache(dir: &Path, key: &str, sizes: &HashMap<String, usize>) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let k2 = array_sizes_cache_v2_key(key);
    let cache = ArraySizesCacheV2 {
        schema: ARRAY_SIZES_CACHE_SCHEMA_V2.to_string(),
        key: k2.clone(),
        sizes: sizes.clone(),
        deps: Vec::new(),
    };
    if let Ok(bytes) = bincode::serialize(&cache) {
        let _ = cache_shm::shm_put(k2.as_str(), &bytes);
        if let Some(cfg) = cache_sqlite::sqlite_config(Some(dir)) {
            let _ = cache_sqlite::sqlite_put(
                &cfg.path,
                k2.as_str(),
                ARRAY_SIZES_CACHE_SCHEMA_V2,
                "array_sizes_v2",
                &bytes,
                None,
            );
        }
    }
    mem_cache_put(key, Arc::new(sizes.clone()));
    Ok(())
}
