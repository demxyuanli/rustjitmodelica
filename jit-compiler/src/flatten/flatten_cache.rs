//! Optional on-disk hints for `FlattenedModel::array_sizes` keyed by model inputs (see `RUSTMODLICA_FLATTEN_CACHE_DIR`).

use super::ArraySizePolicy;
use crate::loader::ModelLoader;
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

fn mem_cache_get(key: &str) -> Option<Arc<HashMap<String, usize>>> {
    if let Some(v) = LOCAL_ARRAY_SIZES_CACHE.with(|c| c.borrow().get(key).cloned()) {
        return Some(v);
    }
    if let Ok(g) = global_array_sizes_cache().read() {
        if let Some(v) = g.get(key).cloned() {
            // Promote into local cache.
            LOCAL_ARRAY_SIZES_CACHE.with(|c| {
                c.borrow_mut().insert(key.to_string(), v.clone());
            });
            return Some(v);
        }
    }
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
