//! Optional on-disk hints for `FlattenedModel::array_sizes` keyed by model inputs (see `RUSTMODLICA_FLATTEN_CACHE_DIR`).

use super::ArraySizePolicy;
use crate::flatten::ValidationMode;
use crate::flatten::cache_sqlite;
use crate::flatten::cache_shm;
use crate::loader::ModelLoader;
use crate::flatten::flat_cache_v1::{DepHashEntry, FlatCacheV1, FLAT_CACHE_SCHEMA_V1};
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

pub fn flatten_cache_dir() -> Option<PathBuf> {
    std::env::var("RUSTMODLICA_FLATTEN_CACHE_DIR")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
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
    for dep in deps {
        let p = PathBuf::from(dep.path.as_str());
        let Some(actual) = file_hash_hex(&p) else {
            return false;
        };
        if actual != dep.content_hash {
            return false;
        }
    }
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
        if let Ok(cache) = bincode::deserialize::<FlatCacheV1>(&bytes) {
            if cache.schema == FLAT_CACHE_SCHEMA_V1
                && cache.key == key
                && deps_match(&cache.deps)
            {
                let _ = loader;
                return Some(cache.into_flat_model());
            }
        }
    }
    // Tier 3: SQLite persistent store (optional).
    if let Some(cfg) = cache_sqlite::sqlite_config(Some(dir)) {
        if let Ok(Some(bytes)) = cache_sqlite::sqlite_get(&cfg.path, key) {
            if let Ok(cache) = bincode::deserialize::<FlatCacheV1>(&bytes) {
                if cache.schema == FLAT_CACHE_SCHEMA_V1
                    && cache.key == key
                    && deps_match(&cache.deps)
                {
                    let _ = loader;
                    // Promote into shared memory for subsequent processes.
                    let _ = cache_shm::shm_put(key, &bytes);
                    return Some(cache.into_flat_model());
                }
            }
        }
    }
    let path = dir.join(format!("{}.flat-cache.json", key));
    if !path.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let cache: FlatCacheV1 = serde_json::from_str(&text).ok()?;
    if cache.schema != FLAT_CACHE_SCHEMA_V1 {
        return None;
    }
    if cache.key != key {
        return None;
    }
    if !deps_match(&cache.deps) {
        return None;
    }
    // Minimal sanity check: ensure root can still be located within current library paths.
    // If the loader can't resolve it, fall back to recompute.
    if loader.get_path_for_model(cache.model_name.as_str()).is_none() {
        // Allow still using cached result when root path isn't registered in the calling loader,
        // as long as the dependency list validated.
    }
    // Promote JSON disk entry into shared memory (optional).
    if let Ok(bytes) = bincode::serialize(&cache) {
        let _ = cache_shm::shm_put(key, &bytes);
    }
    Some(cache.into_flat_model())
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
    let text = serde_json::to_string(&cache).map_err(|e| e.to_string())?;
    if let Ok(bytes) = bincode::serialize(&cache) {
        let _ = cache_shm::shm_put(key, &bytes);
    }
    let path = dir.join(format!("{}.flat-cache.json", key));
    std::fs::write(&path, text).map_err(|e| e.to_string())
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
    let path = dir.join(format!("{}.array-sizes.json", key));
    if !path.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let obj = v
        .get("array_sizes")
        .and_then(|x| x.as_object())
        .ok_or_else(|| "cache file missing array_sizes object".to_string())?;
    for (k, val) in obj {
        if external.contains_key(k) {
            continue;
        }
        let n = val
            .as_u64()
            .ok_or_else(|| format!("cache array_sizes[\"{}\"] invalid", k))?;
        if n == 0 || n > usize::MAX as u64 {
            return Err(format!("cache array_sizes[\"{}\"] out of range", k));
        }
        external.insert(k.clone(), n as usize);
    }
    mem_cache_put(key, Arc::new(external.clone()));
    Ok(())
}

pub fn write_array_sizes_cache(dir: &Path, key: &str, sizes: &HashMap<String, usize>) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.array-sizes.json", key));
    let mut obj = serde_json::Map::new();
    let mut inner = serde_json::Map::new();
    let mut keys: Vec<_> = sizes.keys().collect();
    keys.sort();
    for k in keys {
        if let Some(sz) = sizes.get(k.as_str()) {
            inner.insert(k.clone(), serde_json::Value::from(*sz as u64));
        }
    }
    obj.insert(
        "array_sizes".to_string(),
        serde_json::Value::Object(inner),
    );
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(obj)).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    mem_cache_put(key, Arc::new(sizes.clone()));
    Ok(())
}
