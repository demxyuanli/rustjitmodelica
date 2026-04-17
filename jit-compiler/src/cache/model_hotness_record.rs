//! Persistent per-model compile counters for warmup candidate ranking.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "model_hotness_v1.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModelHotnessFile {
    pub version: u32,
    pub compile_count: BTreeMap<String, u64>,
    pub flat_full_hits: BTreeMap<String, u64>,
}

fn file_path(cache_root: &Path) -> PathBuf {
    cache_root.join(FILE_NAME)
}

fn load_disk(cache_root: &Path) -> ModelHotnessFile {
    let p = file_path(cache_root);
    if let Ok(text) = std::fs::read_to_string(&p) {
        if let Ok(mut f) = serde_json::from_str::<ModelHotnessFile>(&text) {
            if f.version == 0 {
                f.version = 1;
            }
            return f;
        }
    }
    ModelHotnessFile {
        version: 1,
        compile_count: BTreeMap::new(),
        flat_full_hits: BTreeMap::new(),
    }
}

fn save_disk(cache_root: &Path, f: &ModelHotnessFile) {
    if let Ok(json) = serde_json::to_string_pretty(f) {
        let _ = std::fs::write(file_path(cache_root), json);
    }
}

/// Score for ranking: higher = warm up first.
pub fn score_for_model(cache_root: Option<&Path>, model_name: &str) -> f64 {
    let Some(root) = cache_root else {
        return 0.0;
    };
    let file = load_disk(root);
    let c = *file.compile_count.get(model_name).unwrap_or(&0) as f64;
    let h = *file.flat_full_hits.get(model_name).unwrap_or(&0) as f64;
    c * 10.0 + h
}

/// Record a successful compile (best-effort).
pub fn record_compile(cache_root: Option<&Path>, model_name: &str, flat_full_hits: u64) {
    let Some(root) = cache_root else {
        return;
    };
    let mut file = load_disk(root);
    *file
        .compile_count
        .entry(model_name.to_string())
        .or_insert(0) += 1;
    if flat_full_hits > 0 {
        *file
            .flat_full_hits
            .entry(model_name.to_string())
            .or_insert(0) += flat_full_hits;
    }
    file.version = 1;
    save_disk(root, &file);
}
