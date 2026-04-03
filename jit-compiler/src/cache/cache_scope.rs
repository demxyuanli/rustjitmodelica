use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheScope {
    GlobalStd,
    UserExt,
    Project,
}

impl CacheScope {
    pub fn prefix(&self) -> &'static str {
        match self {
            CacheScope::GlobalStd => "L0",
            CacheScope::UserExt => "L1",
            CacheScope::Project => "L2",
        }
    }

    pub fn resolve_dir(&self, base: &Path) -> PathBuf {
        match self {
            CacheScope::GlobalStd => base.join("std"),
            CacheScope::UserExt => base.join("user"),
            CacheScope::Project => base.join("project"),
        }
    }

    pub fn sqlite_db_name(&self) -> &'static str {
        match self {
            CacheScope::GlobalStd => "cache-std.sqlite",
            CacheScope::UserExt => "cache-user.sqlite",
            CacheScope::Project => "cache-project.sqlite",
        }
    }
}

fn parse_roots_from_env(name: &str) -> Vec<PathBuf> {
    std::env::var(name)
        .ok()
        .map(|s| {
            s.split(';')
                .map(|p| p.trim())
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn normalize_for_match(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn starts_with_normalized(path: &Path, root: &Path) -> bool {
    let p = normalize_for_match(path);
    let r = normalize_for_match(root);
    if p == r {
        return true;
    }
    if r.is_empty() {
        return false;
    }
    p.starts_with(&(r + "/"))
}

pub fn classify_model_scope(lib_path: &Path) -> CacheScope {
    let stdlib_roots = parse_roots_from_env("RUSTMODLICA_STDLIB_ROOTS");
    for root in stdlib_roots {
        if starts_with_normalized(lib_path, root.as_path()) {
            return CacheScope::GlobalStd;
        }
    }
    let user_roots = parse_roots_from_env("RUSTMODLICA_USERLIB_ROOTS");
    for root in user_roots {
        if starts_with_normalized(lib_path, root.as_path()) {
            return CacheScope::UserExt;
        }
    }
    CacheScope::Project
}
