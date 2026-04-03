use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::query_db::semantic_hash_text;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use xxhash_rust::xxh64::Xxh64;

#[derive(Debug, Clone)]
struct FileHashEntry {
    modified: Option<std::time::SystemTime>,
    len: u64,
    hash: String,
}

fn hash_cache() -> &'static RwLock<HashMap<PathBuf, FileHashEntry>> {
    static CACHE: OnceLock<RwLock<HashMap<PathBuf, FileHashEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn unified_file_hash(path: &Path) -> Option<String> {
    let md = std::fs::metadata(path).ok()?;
    let modified = md.modified().ok();
    let len = md.len();
    if let Ok(g) = hash_cache().read() {
        if let Some(cached) = g.get(path) {
            if cached.modified == modified && cached.len == len {
                return Some(cached.hash.clone());
            }
        }
    }

    let is_modelica = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("mo"))
        .unwrap_or(false);
    let hash = if is_modelica {
        let text = std::fs::read_to_string(path).ok()?;
        semantic_hash_text(&text)
    } else {
        let data = std::fs::read(path).ok()?;
        let mut h = Xxh64::new(0);
        h.update(&data);
        format!("{:016x}", h.digest())
    };

    if let Ok(mut g) = hash_cache().write() {
        g.insert(
            path.to_path_buf(),
            FileHashEntry {
                modified,
                len,
                hash: hash.clone(),
            },
        );
    }
    Some(hash)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosureFingerprint {
    pub topo_hash: String,
    pub deps: Vec<DepHashEntry>,
    pub depth: usize,
}

impl ClosureFingerprint {
    pub fn compute(deps: &[DepHashEntry]) -> Self {
        let mut sorted = deps.to_vec();
        sorted.sort_by(|a, b| a.path.cmp(&b.path));
        sorted.dedup_by(|a, b| a.path == b.path);
        let mut h = Xxh64::new(0);
        for dep in &sorted {
            h.update(dep.path.as_bytes());
            h.update(dep.content_hash.as_bytes());
        }
        Self {
            topo_hash: format!("{:016x}", h.digest()),
            deps: sorted,
            depth: 0,
        }
    }

    pub fn matches_disk(&self) -> bool {
        for dep in &self.deps {
            let p = PathBuf::from(dep.path.as_str());
            let Some(actual) = unified_file_hash(&p) else {
                return false;
            };
            if actual != dep.content_hash {
                return false;
            }
        }
        true
    }
}

pub fn deps_match(deps: &[DepHashEntry]) -> bool {
    ClosureFingerprint::compute(deps).matches_disk()
}
