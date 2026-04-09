use crate::cache::cache_key::{CacheKeyV2, CacheStage, CompileFlagsKey};
use crate::cache::cache_scope::{classify_model_scope, CacheScope};
use crate::cache::closure_hash;
use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::flatten::ArraySizePolicy;
use crate::flatten::ValidationMode;
use crate::loader::ModelLoader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const ARRAY_SIZES_CACHE_SCHEMA_V2: &str = "rustmodlica_array_sizes_cache_v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArraySizesCacheV2 {
    pub schema: String,
    pub key: String,
    pub sizes: HashMap<String, usize>,
    pub deps: Vec<DepHashEntry>,
}

pub(super) fn array_sizes_cache_v2_key(key: &str) -> String {
    format!("array_sizes_v2:{}", key)
}

pub fn flatten_array_sizes_cache_key(
    model_name: &str,
    loader: &ModelLoader,
    array_sizes_json: Option<&Path>,
    array_size_policy: ArraySizePolicy,
    warnings_level: &str,
) -> String {
    let source_path = loader.get_path_for_model(model_name);
    let scope = source_path
        .as_deref()
        .map(classify_model_scope)
        .unwrap_or(CacheScope::Project);
    let mut root_hash = String::new();
    if let Some(p) = source_path {
        if let Some(h) = closure_hash::unified_file_hash(&p) {
            root_hash = h;
        }
    }
    let mut flags = CompileFlagsKey::default();
    flags.array_size_policy = match array_size_policy {
        ArraySizePolicy::Legacy => 0,
        ArraySizePolicy::Strict => 1,
    };
    flags.warnings_level = warnings_level.to_string();
    flags.compile_stop = "flatten".to_string();
    if let Some(jp) = array_sizes_json {
        if let Some(h) = closure_hash::unified_file_hash(jp) {
            root_hash.push_str(h.as_str());
        }
    }
    CacheKeyV2::builder(CacheStage::ArraySizes, scope, model_name)
        .libs_from_path_bufs(loader.library_paths.as_slice())
        .root_content_hash(root_hash)
        .compile_flags(flags)
        .build()
        .to_qualified_key()
}

pub(super) fn full_cache_enabled() -> bool {
    match std::env::var("RUSTMODLICA_FLATTEN_FULL_CACHE") {
        Ok(v) => {
            let t = v.trim();
            if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                return false;
            }
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        }
        Err(_) => true,
    }
}

pub fn flatten_full_cache_key(
    model_name: &str,
    loader: &ModelLoader,
    validation_mode: ValidationMode,
    compile_stop: &str,
    coarse_constrainedby_only: bool,
    array_sizes_json: Option<&Path>,
    array_size_policy: ArraySizePolicy,
    warnings_level: &str,
) -> String {
    flatten_full_cache_key_with_deps(
        model_name,
        loader,
        validation_mode,
        compile_stop,
        coarse_constrainedby_only,
        array_sizes_json,
        array_size_policy,
        warnings_level,
        None,
    )
}

/// Enhanced cache key with explicit dependency closure fingerprint.
pub fn flatten_full_cache_key_with_deps(
    model_name: &str,
    loader: &ModelLoader,
    validation_mode: ValidationMode,
    compile_stop: &str,
    coarse_constrainedby_only: bool,
    array_sizes_json: Option<&Path>,
    array_size_policy: ArraySizePolicy,
    warnings_level: &str,
    loaded_paths: Option<&[PathBuf]>,
) -> String {
    let source_path = loader.get_path_for_model(model_name);
    let scope = source_path
        .as_deref()
        .map(classify_model_scope)
        .unwrap_or(CacheScope::Project);
    let mut root_hash = String::new();
    if let Some(p) = source_path {
        if let Some(h) = closure_hash::unified_file_hash(&p) {
            root_hash = h;
        }
    }
    if let Some(jp) = array_sizes_json {
        if let Some(h) = closure_hash::unified_file_hash(jp) {
            root_hash.push_str(h.as_str());
        }
    }

    // Compute libs epoch for cache invalidation (enabled by default)
    let libs_epoch_enabled = std::env::var("RUSTMODLICA_LIBS_EPOCH_CACHE")
        .ok()
        .map(|v| {
            let t = v.trim();
            !(t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no"))
        })
        .unwrap_or(true); // Enabled by default

    let libs_fingerprint = if libs_epoch_enabled {
        // Use loaded paths if available (after compilation), otherwise library directories (sorted for stable keys).
        let paths: Vec<PathBuf> = loaded_paths
            .map(|p| p.to_vec())
            .unwrap_or_else(|| {
                let mut v = loader.library_paths.clone();
                v.sort_by_key(|p| p.to_string_lossy().to_string());
                v.dedup();
                v
            });
        let mut lib_dirs = loader.library_paths.clone();
        lib_dirs.sort_by_key(|p| p.to_string_lossy().to_string());
        lib_dirs.dedup();
        crate::cache::lib_epoch::DepClosureFingerprint::compute(&paths, &lib_dirs)
    } else {
        // Fallback: empty fingerprint
        crate::cache::lib_epoch::DepClosureFingerprint {
            libs_epoch: String::new(),
            deps_hash: String::new(),
            deps_count: 0,
        }
    };

    // Include libs fingerprint in root hash
    root_hash.push_str(&libs_fingerprint.combined_hash());

    let flags = CompileFlagsKey {
        validation_mode: format!("{validation_mode:?}"),
        compile_stop: compile_stop.to_string(),
        coarse_constrainedby_only,
        array_size_policy: match array_size_policy {
            ArraySizePolicy::Legacy => 0,
            ArraySizePolicy::Strict => 1,
        },
        warnings_level: warnings_level.to_string(),
        target_platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
    };
    CacheKeyV2::builder(CacheStage::FlatFull, scope, model_name)
        .libs_from_path_bufs(loader.library_paths.as_slice())
        .root_content_hash(root_hash)
        .compile_flags(flags)
        .build()
        .to_qualified_key()
}

pub(super) fn deps_match(deps: &[DepHashEntry]) -> bool {
    let t0 = std::time::Instant::now();
    let ok = closure_hash::deps_match(deps);
    if !ok {
        crate::query_db::perf_record_add("cache_deps_mismatch", 1);
    }
    crate::query_db::perf_record_us("cache_deps_match_us", t0.elapsed().as_micros() as u64);
    ok
}
