//! Optional JSON append of frequently flattened `Modelica.*` classes (LRU cap).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const DEFAULT_CAP: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HotnessFileV1 {
    pub models: Vec<String>,
}

/// If `RUSTMODLICA_MSL_HOTNESS_JSON` points to a path, record `model_name` (Modelica.* only).
pub fn on_flatten_success(model_name: &str) {
    if !model_name.starts_with("Modelica.") {
        return;
    }
    let Ok(path_s) = std::env::var("RUSTMODLICA_MSL_HOTNESS_JSON") else {
        return;
    };
    let path = PathBuf::from(path_s.trim());
    if path.as_os_str().is_empty() {
        return;
    }
    let cap = std::env::var("RUSTMODLICA_MSL_HOTNESS_CAP")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_CAP);
    let mut file = HotnessFileV1::default();
    if let Ok(txt) = fs::read_to_string(&path) {
        if let Ok(parsed) = serde_json::from_str::<HotnessFileV1>(&txt) {
            file = parsed;
        }
    }
    file.models.retain(|m| m != model_name);
    file.models.insert(0, model_name.to_string());
    file.models.truncate(cap);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&file) {
        let _ = fs::write(&path, bytes);
    }
}

pub fn read_hot_models(path: &std::path::Path) -> Vec<String> {
    let Ok(txt) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(f) = serde_json::from_str::<HotnessFileV1>(&txt) else {
        return Vec::new();
    };
    f.models
}
