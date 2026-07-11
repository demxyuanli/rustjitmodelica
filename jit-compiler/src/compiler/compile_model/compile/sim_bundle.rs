//! Sim-bundle cache lookup + AOT archive native code loading.
//! Extracted from `entry.rs` for module size management.

use crate::compiler::CompilePerfReport;

/// Result of sim-bundle + AOT archive lookup.
pub(crate) struct SimBundleAotResult {
    pub cached_fn: Option<crate::jit::codegen_cache::CachedFunction>,
    pub bundle: Option<crate::cache::sim_bundle_cache::CompiledSimBundle>,
    pub artifact_bundle_cache_status: String,
    pub aot_native_load_status: Option<String>,
    pub aot_native_load_detail: Option<String>,
    pub compile_tier: Option<String>,
}

/// Attempt to load compiled native code via the sim-bundle cache index
/// and AOT archive. Returns `None` if disabled, skipped, or not found.
pub(crate) fn try_load_sim_bundle_aot(
    model_name: &str,
    sim_bundle_key: &Option<String>,
    layout_fp: &str,
    codegen_ck: &crate::jit::codegen_cache::CodegenCacheKey,
    all_symbols: &std::collections::HashMap<String, *const u8>,
    perf_report: &mut CompilePerfReport,
    user_stub_count: usize,
) -> Option<SimBundleAotResult> {
    if !crate::cache::sim_bundle_cache::sim_bundle_cache_enabled() {
        perf_report.artifact_bundle_cache_status = "disabled".to_string();
        return None;
    }
    if user_stub_count > 0 {
        perf_report.artifact_bundle_cache_status = "skipped_stubs".to_string();
        return None;
    }
    if !crate::jit::codegen_cache::codegen_cache_enabled() {
        perf_report.artifact_bundle_cache_status = "skipped_codegen_disk".to_string();
        return None;
    }

    perf_report.artifact_bundle_cache_status = "miss".to_string();
    let sk = sim_bundle_key.as_ref()?;
    let cache_root_sb = crate::flatten::flatten_cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let bundle = crate::cache::sim_bundle_cache::try_load(&cache_root_sb, sk)?;

    if bundle.var_layout_fingerprint != layout_fp
        || bundle.codegen_key.stable_hash() != codegen_ck.stable_hash()
    {
        return None;
    }

    let mut result = SimBundleAotResult {
        cached_fn: None,
        bundle: Some(bundle.clone()),
        artifact_bundle_cache_status: "hit".to_string(),
        aot_native_load_status: None,
        aot_native_load_detail: None,
        compile_tier: None,
    };

    #[cfg(any(windows, target_os = "linux"))]
    {
        if crate::jit::aot_archive::aot_default_archive_native_load_enabled() {
            if let Some(archives) = crate::jit::aot_archive::AotArchiveSet::load_tiered() {
                let key_h = bundle.codegen_key.stable_hash();
                if let Some((blob, ent)) = archives.lookup_entry_with_blob(model_name, &key_h) {
                    if ent.when_count as usize != bundle.when_count
                        || ent.crossings_count as usize != bundle.crossings_count
                    {
                        result.aot_native_load_status = Some(
                            CompilePerfReport::AOT_NATIVE_STATUS_WRONG_KEY.to_string(),
                        );
                        result.aot_native_load_detail =
                            Some("toc_when_crossing_mismatch_vs_bundle".to_string());
                    } else if !blob.is_empty() {
                        result.cached_fn = crate::jit::codegen_cache::load_aot_code_blob(
                            blob,
                            &ent.import_symbols,
                            all_symbols,
                            ent.when_count as usize,
                            ent.crossings_count as usize,
                        );
                        if result.cached_fn.is_some() {
                            result.aot_native_load_status = Some(
                                CompilePerfReport::AOT_NATIVE_STATUS_LOADED.to_string(),
                            );
                            result.aot_native_load_detail = Some(
                                CompilePerfReport::AOT_NATIVE_DETAIL_TOC_MATCH.to_string(),
                            );
                            result.compile_tier = Some("aot_native_loaded".to_string());
                        } else {
                            result.aot_native_load_status = Some(
                                CompilePerfReport::AOT_NATIVE_STATUS_LOAD_FAILED.to_string(),
                            );
                            result.aot_native_load_detail = Some(
                                CompilePerfReport::AOT_NATIVE_DETAIL_TOC_MATCH.to_string(),
                            );
                        }
                    } else {
                        result.aot_native_load_status = Some(
                            CompilePerfReport::AOT_NATIVE_STATUS_NO_BLOB.to_string(),
                        );
                    }
                } else if let Some(blob) = archives.get_code(&key_h) {
                    if !blob.is_empty() {
                        result.cached_fn = crate::jit::codegen_cache::load_aot_code_blob(
                            blob,
                            &Vec::new(),
                            all_symbols,
                            bundle.when_count,
                            bundle.crossings_count,
                        );
                        if result.cached_fn.is_some() {
                            result.aot_native_load_status = Some(
                                CompilePerfReport::AOT_NATIVE_STATUS_LOADED.to_string(),
                            );
                            result.aot_native_load_detail = Some(
                                CompilePerfReport::AOT_NATIVE_DETAIL_KEY_FALLBACK.to_string(),
                            );
                            result.compile_tier = Some("aot_native_loaded".to_string());
                        } else {
                            result.aot_native_load_status = Some(
                                CompilePerfReport::AOT_NATIVE_STATUS_LOAD_FAILED.to_string(),
                            );
                            result.aot_native_load_detail = Some(
                                CompilePerfReport::AOT_NATIVE_DETAIL_KEY_FALLBACK.to_string(),
                            );
                        }
                    } else {
                        result.aot_native_load_status = Some(
                            CompilePerfReport::AOT_NATIVE_STATUS_NO_BLOB.to_string(),
                        );
                    }
                } else {
                    result.aot_native_load_status = Some(
                        CompilePerfReport::AOT_NATIVE_STATUS_MODEL_NOT_IN_ARCHIVE.to_string(),
                    );
                }
            }
        }
    }

    Some(result)
}
