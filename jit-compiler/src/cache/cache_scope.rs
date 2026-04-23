use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
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

/// Path-based scope from [`classify_model_scope`], then qualified-name heuristics when still `Project`
/// (`Modelica.*` -> [`CacheScope::GlobalStd`], `ModelicaTest.*` -> [`CacheScope::UserExt`]).
/// Keeps flatten-disk keys aligned with Salsa query keys.
pub fn classify_model_scope_with_heuristics(
    lib_path: Option<&Path>,
    model_name: Option<&str>,
) -> CacheScope {
    let by_path = match lib_path {
        None => CacheScope::Project,
        Some(p) if p.as_os_str().is_empty() => CacheScope::Project,
        Some(p) => classify_model_scope(p),
    };
    if !matches!(by_path, CacheScope::Project) {
        return by_path;
    }
    let Some(name) = model_name.filter(|s| !s.is_empty()) else {
        return CacheScope::Project;
    };
    if name.starts_with("Modelica.") {
        return CacheScope::GlobalStd;
    }
    if name.starts_with("ModelicaTest.") {
        return CacheScope::UserExt;
    }
    CacheScope::Project
}

/// Resolves scope from a storage key produced by [`crate::cache::cache_key::CacheKeyV2::to_qualified_key`]
/// (and optional `RUSTMODLICA_QUERY_CACHE_NAMESPACE` prefix).
pub fn scope_from_storage_key(key: &str) -> CacheScope {
    fn from_token(t: &str) -> Option<CacheScope> {
        if t == CacheScope::GlobalStd.prefix() {
            Some(CacheScope::GlobalStd)
        } else if t == CacheScope::UserExt.prefix() {
            Some(CacheScope::UserExt)
        } else if t == CacheScope::Project.prefix() {
            Some(CacheScope::Project)
        } else {
            None
        }
    }
    let mut it = key.split(':');
    let Some(first) = it.next() else {
        return CacheScope::Project;
    };
    if let Some(s) = from_token(first) {
        return s;
    }
    if let Some(second) = it.next() {
        if let Some(s) = from_token(second) {
            return s;
        }
    }
    for t in key.split(':') {
        if let Some(s) = from_token(t) {
            return s;
        }
    }
    CacheScope::Project
}

/// SQLite tier lookup: preferred scope first, then outward (L2 -> L1 -> L0).
pub fn sqlite_scope_lookup_chain(primary: CacheScope) -> impl Iterator<Item = CacheScope> {
    match primary {
        CacheScope::Project => [
            Some(CacheScope::Project),
            Some(CacheScope::UserExt),
            Some(CacheScope::GlobalStd),
        ],
        CacheScope::UserExt => [Some(CacheScope::UserExt), Some(CacheScope::GlobalStd), None],
        CacheScope::GlobalStd => [Some(CacheScope::GlobalStd), None, None],
    }
    .into_iter()
    .flatten()
}
