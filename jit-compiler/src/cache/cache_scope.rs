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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_scope_prefix() {
        assert_eq!(CacheScope::GlobalStd.prefix(), "L0");
        assert_eq!(CacheScope::UserExt.prefix(), "L1");
        assert_eq!(CacheScope::Project.prefix(), "L2");
    }

    #[test]
    fn test_cache_scope_resolve_dir() {
        let base = Path::new("/cache");
        assert_eq!(CacheScope::GlobalStd.resolve_dir(base), PathBuf::from("/cache/std"));
        assert_eq!(CacheScope::UserExt.resolve_dir(base), PathBuf::from("/cache/user"));
        assert_eq!(CacheScope::Project.resolve_dir(base), PathBuf::from("/cache/project"));
    }

    #[test]
    fn test_cache_scope_sqlite_db_name() {
        assert_eq!(CacheScope::GlobalStd.sqlite_db_name(), "cache-std.sqlite");
        assert_eq!(CacheScope::UserExt.sqlite_db_name(), "cache-user.sqlite");
        assert_eq!(CacheScope::Project.sqlite_db_name(), "cache-project.sqlite");
    }

    #[test]
    fn test_scope_from_storage_key_qualified_v2_format() {
        assert_eq!(
            scope_from_storage_key("L2:flat_full_v2:abc123"),
            CacheScope::Project
        );
        assert_eq!(
            scope_from_storage_key("L1:eq_expand_v2:def456"),
            CacheScope::UserExt
        );
        assert_eq!(
            scope_from_storage_key("L0:parse_v2:ghi789"),
            CacheScope::GlobalStd
        );
    }

    #[test]
    fn test_scope_from_storage_key_namespace_prefixed() {
        // QUERY_CACHE_NAMESPACE prefix then scope
        assert_eq!(
            scope_from_storage_key("ns1:L2:flat_full_v2:abc123"),
            CacheScope::Project
        );
        assert_eq!(
            scope_from_storage_key("ns2:L0:model_ast_v2:xyz"),
            CacheScope::GlobalStd
        );
    }

    #[test]
    fn test_scope_from_storage_key_unknown_falls_back_to_project() {
        assert_eq!(scope_from_storage_key("garbage"), CacheScope::Project);
        assert_eq!(scope_from_storage_key(""), CacheScope::Project);
        assert_eq!(scope_from_storage_key("L3:something"), CacheScope::Project);
    }

    #[test]
    fn test_scope_from_storage_key_scope_in_any_position() {
        // L0 in a later position
        assert_eq!(
            scope_from_storage_key("artifact_v1:SolvableBlock4Res:L0:something"),
            CacheScope::GlobalStd
        );
    }

    #[test]
    fn test_sqlite_scope_lookup_chain_project() {
        let chain: Vec<CacheScope> = sqlite_scope_lookup_chain(CacheScope::Project).collect();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], CacheScope::Project);
        assert_eq!(chain[1], CacheScope::UserExt);
        assert_eq!(chain[2], CacheScope::GlobalStd);
    }

    #[test]
    fn test_sqlite_scope_lookup_chain_user_ext() {
        let chain: Vec<CacheScope> = sqlite_scope_lookup_chain(CacheScope::UserExt).collect();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0], CacheScope::UserExt);
        assert_eq!(chain[1], CacheScope::GlobalStd);
    }

    #[test]
    fn test_sqlite_scope_lookup_chain_global_std() {
        let chain: Vec<CacheScope> = sqlite_scope_lookup_chain(CacheScope::GlobalStd).collect();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0], CacheScope::GlobalStd);
    }

    #[test]
    fn test_classify_model_scope_with_heuristics_modelica_std() {
        // Qualified name "Modelica.*" → GlobalStd regardless of path
        let scope = classify_model_scope_with_heuristics(None, Some("Modelica.Math.sin"));
        assert_eq!(scope, CacheScope::GlobalStd);
    }

    #[test]
    fn test_classify_model_scope_with_heuristics_modelica_test() {
        let scope = classify_model_scope_with_heuristics(None, Some("ModelicaTest.Fluid.BranchingDynamicPipes"));
        assert_eq!(scope, CacheScope::UserExt);
    }

    #[test]
    fn test_classify_model_scope_with_heuristics_user_project() {
        // Unknown name without known path roots → Project
        let scope = classify_model_scope_with_heuristics(None, Some("MyCustomModel"));
        assert_eq!(scope, CacheScope::Project);
    }

    #[test]
    fn test_classify_model_scope_with_heuristics_empty_inputs() {
        assert_eq!(
            classify_model_scope_with_heuristics(None, None),
            CacheScope::Project
        );
        assert_eq!(
            classify_model_scope_with_heuristics(None, Some("")),
            CacheScope::Project
        );
    }

    #[test]
    fn test_classify_model_scope_with_heuristics_empty_path() {
        let scope = classify_model_scope_with_heuristics(
            Some(Path::new("")),
            Some("SomeModel"),
        );
        assert_eq!(scope, CacheScope::Project);
    }

    #[test]
    fn test_scope_prefixes_are_unique() {
        // All three scopes must have different prefixes
        let prefixes: Vec<&str> = [
            CacheScope::GlobalStd,
            CacheScope::UserExt,
            CacheScope::Project,
        ]
        .iter()
        .map(|s| s.prefix())
        .collect();
        let mut deduped = prefixes.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), 3);
    }

    #[test]
    fn test_scope_db_names_are_unique() {
        let names: Vec<&str> = [
            CacheScope::GlobalStd,
            CacheScope::UserExt,
            CacheScope::Project,
        ]
        .iter()
        .map(|s| s.sqlite_db_name())
        .collect();
        let mut deduped = names.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), 3);
    }
}
