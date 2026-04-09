use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{OnceLock, RwLock};
use xxhash_rust::xxh64::Xxh64;

use crate::cache::artifact_key;
use crate::cache::cache_scope::CacheScope;
use crate::flatten::cache_sqlite;

use super::super::pipeline::{AnalysisStage, VariableLayout};
use super::super::{CompilerOptions, ValidationAnalyzedSummary};

#[derive(Clone)]
pub(crate) struct AnalyzeCacheEntry {
    pub(crate) summary: ValidationAnalyzedSummary,
    pub(crate) analyze_ms: u64,
}

pub(crate) fn analyze_cache() -> &'static RwLock<HashMap<u64, AnalyzeCacheEntry>> {
    static CACHE: OnceLock<RwLock<HashMap<u64, AnalyzeCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(crate) fn analysis_summary_disk_cache_enabled() -> bool {
    match std::env::var("RUSTMODLICA_ANALYSIS_SUMMARY_CACHE") {
        Ok(v) => {
            let t = v.trim();
            if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                return false;
            }
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") || t.is_empty()
        }
        Err(_) => true,
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AnalysisSummaryDiskV1 {
    summary: ValidationAnalyzedSummary,
    analyze_ms: u64,
}

pub(crate) fn analysis_summary_disk_key(model_name: &str, flat_h: u64, opts: &CompilerOptions) -> String {
    let mut h = Xxh64::new(0);
    h.update(model_name.as_bytes());
    h.update(&[0]);
    h.update(&flat_h.to_le_bytes());
    h.update(opts.index_reduction_method.as_bytes());
    h.update(&[0]);
    h.update(opts.tearing_method.as_bytes());
    h.update(&[0]);
    h.update(opts.generate_dynamic_jacobian.as_bytes());
    h.update(&[0]);
    h.update(opts.warnings_level.as_bytes());
    h.update(&[0]);
    h.update(opts.validation_mode.as_bytes());
    format!(
        "analysis_summary_v1:{}:{:016x}",
        model_name.replace('.', "_"),
        h.digest()
    )
}

pub(crate) fn try_read_analysis_summary_disk(
    cache_root: &Path,
    key: &str,
) -> Option<AnalyzeCacheEntry> {
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))?;
    let bytes = cache_sqlite::sqlite_get(&cfg.path, key, "analysis_summary_v1").ok()??;
    let v: AnalysisSummaryDiskV1 = bincode::deserialize(&bytes).ok()?;
    Some(AnalyzeCacheEntry {
        summary: v.summary,
        analyze_ms: v.analyze_ms,
    })
}

pub(crate) fn try_write_analysis_summary_disk(
    cache_root: &Path,
    key: &str,
    entry: &AnalyzeCacheEntry,
) {
    let Some(cfg) = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))
    else {
        return;
    };
    let v = AnalysisSummaryDiskV1 {
        summary: entry.summary.clone(),
        analyze_ms: entry.analyze_ms,
    };
    let Ok(bytes) = bincode::serialize(&v) else {
        return;
    };
    let _ = cache_sqlite::sqlite_put(
        &cfg.path,
        key,
        "asV1",
        "analysis_summary_v1",
        &bytes,
        None,
    );
}

pub(crate) fn pipeline_analysis_disk_cache_enabled() -> bool {
    match std::env::var("RUSTMODLICA_PIPELINE_ANALYSIS_CACHE") {
        Ok(v) => {
            let t = v.trim();
            if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                return false;
            }
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") || t.is_empty()
        }
        Err(_) => true,
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PipelineAnalysisDiskV1 {
    variable_layout: VariableLayout,
    analysis_stage: AnalysisStage,
}

pub(crate) fn sqlite_project_model_digest_key(
    model_name: &str,
    flat_h: u64,
    opts: &CompilerOptions,
) -> String {
    let flags = artifact_key::compile_flags_hash(opts);
    let mut h = Xxh64::new(0);
    h.update(model_name.as_bytes());
    h.update(&[0]);
    h.update(&flat_h.to_le_bytes());
    h.update(flags.as_bytes());
    format!(
        "{}:{:016x}",
        model_name.replace('.', "_"),
        h.digest()
    )
}

pub(crate) fn pipeline_analysis_disk_key(model_name: &str, flat_h: u64, opts: &CompilerOptions) -> String {
    format!(
        "pipeline_analysis_v1:{}",
        sqlite_project_model_digest_key(model_name, flat_h, opts)
    )
}

pub(crate) fn backend_dae_disk_cache_enabled() -> bool {
    match std::env::var("RUSTMODLICA_BACKEND_DAE_CACHE") {
        Ok(v) => {
            let t = v.trim();
            if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                return false;
            }
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") || t.is_empty()
        }
        Err(_) => true,
    }
}

pub(crate) fn backend_dae_disk_key(model_name: &str, flat_h: u64, opts: &CompilerOptions) -> String {
    format!(
        "backend_dae_v1:{}",
        sqlite_project_model_digest_key(model_name, flat_h, opts)
    )
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BackendDaeDiskV1 {
    simulation_dae: crate::backend_dae::SimulationDae,
}

pub(crate) fn try_read_backend_dae_disk(
    cache_root: &Path,
    key: &str,
) -> Option<crate::backend_dae::SimulationDae> {
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))?;
    let bytes = cache_sqlite::sqlite_get(&cfg.path, key, "backend_dae_v1").ok()??;
    let v: BackendDaeDiskV1 = bincode::deserialize(&bytes).ok()?;
    Some(v.simulation_dae)
}

pub(crate) fn try_write_backend_dae_disk(
    cache_root: &Path,
    key: &str,
    simulation_dae: &crate::backend_dae::SimulationDae,
) {
    let Some(cfg) = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))
    else {
        return;
    };
    let v = BackendDaeDiskV1 {
        simulation_dae: simulation_dae.clone(),
    };
    let Ok(bytes) = bincode::serialize(&v) else {
        return;
    };
    let _ = cache_sqlite::sqlite_put(
        &cfg.path,
        key,
        "bdV1",
        "backend_dae_v1",
        &bytes,
        None,
    );
}

pub(crate) fn try_read_pipeline_analysis_disk(
    cache_root: &Path,
    key: &str,
) -> Option<(VariableLayout, AnalysisStage)> {
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))?;
    let bytes = cache_sqlite::sqlite_get(&cfg.path, key, "pipeline_analysis_v1").ok()??;
    let v: PipelineAnalysisDiskV1 = bincode::deserialize(&bytes).ok()?;
    Some((v.variable_layout, v.analysis_stage))
}

pub(crate) fn try_write_pipeline_analysis_disk(
    cache_root: &Path,
    key: &str,
    variable_layout: &VariableLayout,
    analysis_stage: &AnalysisStage,
) {
    let Some(cfg) = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))
    else {
        return;
    };
    let v = PipelineAnalysisDiskV1 {
        variable_layout: variable_layout.clone(),
        analysis_stage: analysis_stage.clone(),
    };
    let Ok(bytes) = bincode::serialize(&v) else {
        return;
    };
    let _ = cache_sqlite::sqlite_put(
        &cfg.path,
        key,
        "paV1",
        "pipeline_analysis_v1",
        &bytes,
        None,
    );
}

pub(crate) fn flat_model_hash(flat_model: &crate::flatten::FlattenedModel) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", flat_model).hash(&mut h);
    h.finish()
}
