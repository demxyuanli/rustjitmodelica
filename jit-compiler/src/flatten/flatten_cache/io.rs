use super::keys::{
    array_sizes_cache_v2_key, deps_match, full_cache_enabled, ArraySizesCacheV2,
    ARRAY_SIZES_CACHE_SCHEMA_V2,
};
use super::mem::{mem_cache_get, mem_cache_put};
use crate::cache::cache_scope::scope_from_storage_key;
use crate::cache::closure_hash;
use crate::flatten::cache_shm;
use crate::flatten::cache_sqlite;
use crate::flatten::flat_cache_v1::{DepHashEntry, FlatCacheV1, FLAT_CACHE_SCHEMA_V1};
use crate::flatten::flat_cache_v2::{self, FlatCacheV2, FLAT_CACHE_SCHEMA_V2};
use crate::loader::ModelLoader;
use crate::query_db::{record_cache_event, CacheEvent};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

pub fn try_read_flat_cache_v1(
    dir: &Path,
    key: &str,
    loader: &ModelLoader,
    model_name: &str,
) -> Option<crate::flatten::FlattenedModel> {
    if !full_cache_enabled() {
        return None;
    }
    let scope_pf = scope_from_storage_key(key).prefix();
    // Tier 2: cross-process shared memory (fastest cross-process path).
    if let Some(bytes) = cache_shm::shm_get(key) {
        crate::query_db::perf_record_us("cache_get_us", 0);
        let t_deser = std::time::Instant::now();
        if let Ok(cache) = bincode::deserialize::<FlatCacheV1>(&bytes) {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
            let deps_ok = deps_match(&cache.deps)
                || crate::cache::msl_pack::context::flat_cache_relax_deps_for(model_name);
            if cache.schema == FLAT_CACHE_SCHEMA_V1 && cache.key == key && deps_ok {
                let _ = loader;
                record_cache_event(scope_pf, "flat_full", CacheEvent::Hit);
                return Some(cache.into_flat_model());
            }
        } else {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
        }
    }
    // Tier 3: SQLite persistent store (optional): tier chain across project/user/std roots.
    let primary = scope_from_storage_key(key);
    for cfg in cache_sqlite::sqlite_read_try_configs(primary) {
        let t_get = std::time::Instant::now();
        let Ok(Some(bytes)) = cache_sqlite::sqlite_get(&cfg.path, key, "flat_cache_v1") else {
            continue;
        };
        crate::query_db::perf_record_us("cache_get_us", t_get.elapsed().as_micros() as u64);
        let t_deser = std::time::Instant::now();
        if let Ok(cache) = bincode::deserialize::<FlatCacheV1>(&bytes) {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
            let deps_ok = deps_match(&cache.deps)
                || crate::cache::msl_pack::context::flat_cache_relax_deps_for(model_name);
            if cache.schema == FLAT_CACHE_SCHEMA_V1 && cache.key == key && deps_ok {
                let _ = loader;
                let _ = cache_shm::shm_put(key, &bytes);
                record_cache_event(scope_pf, "flat_full", CacheEvent::Hit);
                return Some(cache.into_flat_model());
            }
        } else {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
        }
    }
    // Legacy compatibility: read old JSON file and migrate to SHM/SQLite.
    let path = dir.join(format!("{}.flat-cache.json", key));
    if path.is_file() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(cache) = serde_json::from_str::<FlatCacheV1>(&text) {
                let deps_ok = deps_match(&cache.deps)
                    || crate::cache::msl_pack::context::flat_cache_relax_deps_for(model_name);
                if cache.schema == FLAT_CACHE_SCHEMA_V1 && cache.key == key && deps_ok {
                    if let Ok(bytes) = bincode::serialize(&cache) {
                        let _ = cache_shm::shm_put(key, &bytes);
                        if let Some(cfg) =
                            cache_sqlite::sqlite_write_config_for_scope(scope_from_storage_key(key))
                        {
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
                    record_cache_event(scope_pf, "flat_full", CacheEvent::Hit);
                    return Some(cache.into_flat_model());
                }
            }
        }
    }
    record_cache_event(scope_pf, "flat_full", CacheEvent::Miss);
    None
}

pub fn write_flat_cache_v1(
    dir: &Path,
    key: &str,
    model_name: &str,
    flat: &crate::flatten::FlattenedModel,
    deps: &[PathBuf],
    absent_deps: &[PathBuf],
) -> Result<(), String> {
    if !full_cache_enabled() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut entries: Vec<DepHashEntry> = Vec::with_capacity(deps.len() + absent_deps.len());
    for p in deps {
        let Some(h) = closure_hash::unified_file_hash(p.as_path()) else {
            continue;
        };
        entries.push(DepHashEntry {
            path: p.display().to_string(),
            content_hash: h,
        });
    }
    for p in absent_deps {
        entries.push(DepHashEntry {
            path: p.display().to_string(),
            content_hash: closure_hash::ABSENT_DEP_SENTINEL.to_string(),
        });
    }
    let scope = scope_from_storage_key(key);
    let scope_pf = scope.prefix();

    // Write V2 format (rkyv-based, zero-copy) for future reads
    let _ = write_flat_cache_v2(dir, key, model_name, flat, deps, absent_deps);

    // Also write V1 format for backward compatibility
    let cache = FlatCacheV1::from_flat_model(key.to_string(), model_name, flat, entries);
    if let Some(cfg) = cache_sqlite::sqlite_write_config_for_scope(scope) {
        let bytes = bincode::serialize(&cache).map_err(|e| e.to_string())?;
        let deps_json = serde_json::to_string(&cache.deps).map_err(|e| e.to_string())?;
        let _ = cache_sqlite::sqlite_put(
            &cfg.path,
            key,
            FLAT_CACHE_SCHEMA_V1,
            "flat_cache_v1",
            &bytes,
            Some(deps_json.as_str()),
        );
        record_cache_event(scope_pf, "flat_full", CacheEvent::Write);
        return Ok(());
    }
    // No SQLite config available; persist only in shared memory via V2.
    record_cache_event(scope_pf, "flat_full", CacheEvent::Write);
    Ok(())
}

/// Try reading V2 cache format (rkyv-based, zero-copy).
/// Returns None if V2 not found or on error; caller should fall back to V1.
pub fn try_read_flat_cache_v2(
    _dir: &Path,
    key: &str,
    loader: &ModelLoader,
    model_name: &str,
) -> Option<crate::flatten::FlattenedModel> {
    if !full_cache_enabled() {
        return None;
    }
    let scope_pf = scope_from_storage_key(key).prefix();

    // Tier 2: cross-process shared memory (fastest cross-process path).
    if let Some(bytes) = cache_shm::shm_get(key) {
        crate::query_db::perf_record_us("cache_get_us", 0);
        let t_deser = std::time::Instant::now();
        if let Ok(cache) = flat_cache_v2::deserialize_cache(&bytes) {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
            let deps_ok = deps_match(&cache.deps)
                || crate::cache::msl_pack::context::flat_cache_relax_deps_for(model_name);
            if cache.schema == FLAT_CACHE_SCHEMA_V2 && cache.key == key && deps_ok {
                let _ = loader;
                record_cache_event(scope_pf, "flat_full", CacheEvent::Hit);
                return cache.into_flat_model().ok();
            }
        } else {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
        }
    }

    // Tier 3: SQLite persistent store
    let primary = scope_from_storage_key(key);
    for cfg in cache_sqlite::sqlite_read_try_configs(primary) {
        let t_get = std::time::Instant::now();
        let Ok(Some(bytes)) = cache_sqlite::sqlite_get(&cfg.path, key, "flat_cache_v2") else {
            continue;
        };
        crate::query_db::perf_record_us("cache_get_us", t_get.elapsed().as_micros() as u64);
        let t_deser = std::time::Instant::now();
        if let Ok(cache) = flat_cache_v2::deserialize_cache(&bytes) {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
            let deps_ok = deps_match(&cache.deps)
                || crate::cache::msl_pack::context::flat_cache_relax_deps_for(model_name);
            if cache.schema == FLAT_CACHE_SCHEMA_V2 && cache.key == key && deps_ok {
                let _ = loader;
                let _ = cache_shm::shm_put(key, &bytes);
                record_cache_event(scope_pf, "flat_full", CacheEvent::Hit);
                return cache.into_flat_model().ok();
            }
        } else {
            crate::query_db::perf_record_us(
                "cache_deserialize_us",
                t_deser.elapsed().as_micros() as u64,
            );
        }
    }
    None
}

/// Write V2 cache format (rkyv-based) for zero-copy deserialization.
pub fn write_flat_cache_v2(
    dir: &Path,
    key: &str,
    model_name: &str,
    flat: &crate::flatten::FlattenedModel,
    deps: &[PathBuf],
    absent_deps: &[PathBuf],
) -> Result<(), String> {
    if !full_cache_enabled() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut entries: Vec<DepHashEntry> = Vec::with_capacity(deps.len() + absent_deps.len());
    for p in deps {
        let Some(h) = closure_hash::unified_file_hash(p.as_path()) else {
            continue;
        };
        entries.push(DepHashEntry {
            path: p.display().to_string(),
            content_hash: h,
        });
    }
    for p in absent_deps {
        entries.push(DepHashEntry {
            path: p.display().to_string(),
            content_hash: closure_hash::ABSENT_DEP_SENTINEL.to_string(),
        });
    }
    let cache = FlatCacheV2::from_flat_model(key.to_string(), model_name, flat, entries);
    let scope = scope_from_storage_key(key);
    let scope_pf = scope.prefix();

    let bytes = flat_cache_v2::serialize_cache(&cache).map_err(|e| e.to_string())?;
    let _ = cache_shm::shm_put(key, &bytes);

    if let Some(cfg) = cache_sqlite::sqlite_write_config_for_scope(scope) {
        let deps_json = serde_json::to_string(&cache.deps).map_err(|e| e.to_string())?;
        let _ = cache_sqlite::sqlite_put(
            &cfg.path,
            key,
            FLAT_CACHE_SCHEMA_V2,
            "flat_cache_v2",
            &bytes,
            Some(deps_json.as_str()),
        );
    }
    record_cache_event(scope_pf, "flat_full", CacheEvent::Write);
    Ok(())
}

struct HotFullCacheState {
    map: HashMap<String, Arc<crate::flatten::FlattenedModel>>,
    lru: VecDeque<String>,
}

fn hot_full_cache_max_entries() -> usize {
    std::env::var("RUSTMODLICA_HOT_FULL_CACHE_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(512)
}

fn hot_full_cache_state() -> &'static RwLock<HotFullCacheState> {
    static HOT: OnceLock<RwLock<HotFullCacheState>> = OnceLock::new();
    HOT.get_or_init(|| {
        RwLock::new(HotFullCacheState {
            map: HashMap::new(),
            lru: VecDeque::new(),
        })
    })
}

fn hot_full_cache_touch(key: &str) {
    let Ok(mut g) = hot_full_cache_state().write() else {
        return;
    };
    if !g.map.contains_key(key) {
        return;
    }
    if let Some(i) = g.lru.iter().position(|k| k == key) {
        g.lru.remove(i);
    }
    g.lru.push_back(key.to_string());
}

/// Remove hot in-memory flatten entries whose qualified cache key contains any `needle`
/// (e.g. `":flat_full_v2:"` from stage tag).
pub fn hot_full_cache_evict_matching_needles(needles: &[String]) {
    let Ok(mut g) = hot_full_cache_state().write() else {
        return;
    };
    let keys: Vec<String> = g.map.keys().cloned().collect();
    for k in keys {
        if needles.iter().any(|n| k.contains(n.as_str())) {
            g.map.remove(&k);
            while let Some(i) = g.lru.iter().position(|x| x == &k) {
                g.lru.remove(i);
            }
        }
    }
}

fn hot_full_cache_put(key: String, value: Arc<crate::flatten::FlattenedModel>) {
    let max = hot_full_cache_max_entries();
    let Ok(mut g) = hot_full_cache_state().write() else {
        return;
    };
    if g.map.contains_key(&key) {
        g.map.insert(key.clone(), value);
        if let Some(i) = g.lru.iter().position(|k| k == &key) {
            g.lru.remove(i);
        }
        g.lru.push_back(key);
        return;
    }
    g.map.insert(key.clone(), value);
    g.lru.push_back(key.clone());
    while g.map.len() > max {
        let Some(oldest) = g.lru.pop_front() else {
            break;
        };
        g.map.remove(&oldest);
    }
}

pub fn get_or_compute_flattened_model_v1<F>(
    dir: &Path,
    key: &str,
    loader: &ModelLoader,
    model_name: &str,
    compute: F,
) -> Result<crate::flatten::FlattenedModel, crate::flatten::FlattenError>
where
    F: FnOnce() -> Result<crate::flatten::FlattenedModel, crate::flatten::FlattenError>,
{
    let scope_pf = scope_from_storage_key(key).prefix();
    if let Ok(h) = hot_full_cache_state().read() {
        if let Some(v) = h.map.get(key) {
            record_cache_event(scope_pf, "flat_full", CacheEvent::Hit);
            let out = (**v).clone();
            drop(h);
            hot_full_cache_touch(key);
            return Ok(out);
        }
    }
    // Try V2 first (rkyv-based, zero-copy, faster deserialization)
    if let Some(v) = try_read_flat_cache_v2(dir, key, loader, model_name) {
        hot_full_cache_put(key.to_string(), Arc::new(v.clone()));
        return Ok(v);
    }
    // Fall back to V1 (bincode-based)
    if let Some(v) = try_read_flat_cache_v1(dir, key, loader, model_name) {
        hot_full_cache_put(key.to_string(), Arc::new(v.clone()));
        return Ok(v);
    }
    let v = compute()?;
    hot_full_cache_put(key.to_string(), Arc::new(v.clone()));
    Ok(v)
}

pub fn merge_cached_array_sizes(
    dir: &Path,
    key: &str,
    external: &mut HashMap<String, usize>,
) -> Result<(), String> {
    let scope_pf = scope_from_storage_key(key).prefix();
    if let Some(mem) = mem_cache_get(key) {
        for (k, v) in mem.as_ref() {
            external.entry(k.clone()).or_insert(*v);
        }
        record_cache_event(scope_pf, "array_sizes", CacheEvent::Hit);
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
                record_cache_event(scope_pf, "array_sizes", CacheEvent::Hit);
                return Ok(());
            }
        }
    }
    let primary = scope_from_storage_key(k2.as_str());
    for cfg in cache_sqlite::sqlite_read_try_configs(primary) {
        let Ok(Some(bytes)) = cache_sqlite::sqlite_get(&cfg.path, k2.as_str(), "array_sizes_v2") else {
            continue;
        };
        if let Ok(cache) = bincode::deserialize::<ArraySizesCacheV2>(&bytes) {
            if cache.schema == ARRAY_SIZES_CACHE_SCHEMA_V2 && cache.key == k2 {
                let _ = cache_shm::shm_put(k2.as_str(), &bytes);
                for (k, v) in cache.sizes {
                    external.entry(k).or_insert(v);
                }
                mem_cache_put(key, Arc::new(external.clone()));
                record_cache_event(scope_pf, "array_sizes", CacheEvent::Hit);
                return Ok(());
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
                        record_cache_event(scope_pf, "array_sizes", CacheEvent::Hit);
                        return Ok(());
                    }
                }
            }
        }
    }

    record_cache_event(scope_pf, "array_sizes", CacheEvent::Miss);
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
        let scope_pf = scope_from_storage_key(k2.as_str()).prefix();
        let _ = cache_shm::shm_put(k2.as_str(), &bytes);
        if let Some(cfg) =
            cache_sqlite::sqlite_write_config_for_scope(scope_from_storage_key(k2.as_str()))
        {
            let _ = cache_sqlite::sqlite_put(
                &cfg.path,
                k2.as_str(),
                ARRAY_SIZES_CACHE_SCHEMA_V2,
                "array_sizes_v2",
                &bytes,
                None,
            );
        }
        record_cache_event(scope_pf, "array_sizes", CacheEvent::Write);
    }
    mem_cache_put(key, Arc::new(sizes.clone()));
    Ok(())
}
