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

    if let Some(h) = crate::cache::path_hash_index::lookup(None, path, modified, len) {
            if let Ok(mut g) = hash_cache().write() {
                g.insert(
                    path.to_path_buf(),
                    FileHashEntry {
                        modified,
                        len,
                        hash: h.clone(),
                    },
                );
            }
            return Some(h);
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
    crate::cache::path_hash_index::store(None, path, modified, len, &hash);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dep(path: &str, hash: &str) -> DepHashEntry {
        DepHashEntry {
            path: path.to_string(),
            content_hash: hash.to_string(),
        }
    }

    #[test]
    fn test_closure_fingerprint_compute_stable_ordering() {
        let deps = vec![
            make_dep("/d/LibC.mo", "c"),
            make_dep("/d/LibA.mo", "a"),
            make_dep("/d/LibB.mo", "b"),
        ];
        let fp = ClosureFingerprint::compute(&deps);
        // Sorted order
        assert_eq!(fp.deps.len(), 3);
        assert_eq!(fp.deps[0].path, "/d/LibA.mo");
        assert_eq!(fp.deps[1].path, "/d/LibB.mo");
        assert_eq!(fp.deps[2].path, "/d/LibC.mo");
        assert_eq!(fp.topo_hash.len(), 16);
        assert!(fp.topo_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_closure_fingerprint_dedup_same_path() {
        let deps = vec![
            make_dep("/d/LibA.mo", "a"),
            make_dep("/d/LibA.mo", "a"), // duplicate
            make_dep("/d/LibB.mo", "b"),
        ];
        let fp = ClosureFingerprint::compute(&deps);
        assert_eq!(fp.deps.len(), 2);
        assert_eq!(fp.deps[0].path, "/d/LibA.mo");
        assert_eq!(fp.deps[1].path, "/d/LibB.mo");
    }

    #[test]
    fn test_closure_fingerprint_compute_empty() {
        let fp = ClosureFingerprint::compute(&[]);
        assert!(fp.deps.is_empty());
        assert_eq!(fp.topo_hash.len(), 16);
        assert_eq!(fp.depth, 0);
    }

    #[test]
    fn test_closure_fingerprint_different_hash_produces_different_topo() {
        let deps_a = vec![make_dep("/d/Lib.mo", "hash_a")];
        let deps_b = vec![make_dep("/d/Lib.mo", "hash_b")];
        let fp_a = ClosureFingerprint::compute(&deps_a);
        let fp_b = ClosureFingerprint::compute(&deps_b);
        assert_ne!(fp_a.topo_hash, fp_b.topo_hash);
    }

    #[test]
    fn test_closure_fingerprint_different_path_same_hash() {
        let deps_a = vec![make_dep("/d/A.mo", "h")];
        let deps_b = vec![make_dep("/d/B.mo", "h")];
        let fp_a = ClosureFingerprint::compute(&deps_a);
        let fp_b = ClosureFingerprint::compute(&deps_b);
        assert_ne!(fp_a.topo_hash, fp_b.topo_hash);
    }

    #[test]
    fn test_closure_fingerprint_deterministic() {
        let deps = vec![
            make_dep("/d/LibB.mo", "b"),
            make_dep("/d/LibA.mo", "a"),
        ];
        let fp1 = ClosureFingerprint::compute(&deps);
        let fp2 = ClosureFingerprint::compute(&deps);
        assert_eq!(fp1.topo_hash, fp2.topo_hash);
    }

    #[test]
    fn test_closure_fingerprint_additional_dep_changes_hash() {
        let deps_small = vec![make_dep("/d/A.mo", "a")];
        let deps_large = vec![make_dep("/d/A.mo", "a"), make_dep("/d/B.mo", "b")];
        let fp_small = ClosureFingerprint::compute(&deps_small);
        let fp_large = ClosureFingerprint::compute(&deps_large);
        assert_ne!(fp_small.topo_hash, fp_large.topo_hash);
    }

    #[test]
    fn test_unified_file_hash_testlib_mo_exists() {
        // A known file that exists in the repo
        let path = std::path::Path::new("jit-compiler/TestLib/BouncingBall.mo");
        if path.is_file() {
            let hash = unified_file_hash(path);
            assert!(hash.is_some());
            let hash = hash.unwrap();
            assert!(!hash.is_empty());
            assert_eq!(hash.len(), 16);
        }
    }

    #[test]
    fn test_unified_file_hash_non_existent_is_none() {
        let hash = unified_file_hash(std::path::Path::new("__nonexistent_file__.xyz"));
        assert!(hash.is_none());
    }
}
