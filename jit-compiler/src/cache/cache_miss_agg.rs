//! Aggregate cache miss reasons under the flatten cache root (JSON).

use crate::compiler::CompilePerfReport;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

const FILE_NAME: &str = "cache_miss_agg_v1.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MissAggFile {
    pub version: u32,
    pub by_reason: BTreeMap<String, u64>,
    pub by_layer: BTreeMap<String, u64>,
}

fn path(cache_root: &Path) -> std::path::PathBuf {
    cache_root.join(FILE_NAME)
}

fn load(cache_root: &Path) -> MissAggFile {
    if let Ok(t) = std::fs::read_to_string(path(cache_root)) {
        if let Ok(mut f) = serde_json::from_str::<MissAggFile>(&t) {
            if f.version == 0 {
                f.version = 1;
            }
            return f;
        }
    }
    MissAggFile {
        version: 1,
        by_reason: BTreeMap::new(),
        by_layer: BTreeMap::new(),
    }
}

fn save(cache_root: &Path, f: &MissAggFile) {
    if let Ok(s) = serde_json::to_string_pretty(f) {
        let _ = std::fs::write(path(cache_root), s);
    }
}

/// Append miss events from one compile perf report.
pub fn append_from_report(cache_root: &Path, report: &CompilePerfReport) {
    if report.cache_miss_events.is_empty() {
        return;
    }
    let mut f = load(cache_root);
    for ev in &report.cache_miss_events {
        *f.by_reason.entry(ev.reason.clone()).or_insert(0) += 1;
        *f.by_layer.entry(ev.layer.clone()).or_insert(0) += 1;
    }
    f.version = 1;
    save(cache_root, &f);
}

pub fn read_aggregate(cache_root: &Path) -> MissAggFile {
    load(cache_root)
}
