//! Optional on-disk hints for `FlattenedModel::array_sizes` keyed by model inputs (see `RUSTMODLICA_FLATTEN_CACHE_DIR`).

use super::ArraySizePolicy;
use crate::loader::ModelLoader;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

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
    Ok(())
}
