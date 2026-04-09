use std::env;
use std::fs;

use rustmodlica::Artifacts;

use super::RunError;

pub(crate) fn perf_salsa_stats_enabled() -> bool {
    env::var("RUSTMODLICA_PERF_SALSA_STATS")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

pub(crate) fn merge_salsa_process_db_stats_into_compile_perf(compile_perf: &mut serde_json::Value) {
    if !perf_salsa_stats_enabled() {
        return;
    }
    let (hits, misses, evictions) = rustmodlica::salsa_process_db_stats();
    let Some(obj) = compile_perf.as_object_mut() else {
        return;
    };
    obj.insert(
        "salsa_process_db_hits".to_string(),
        serde_json::json!(hits),
    );
    obj.insert(
        "salsa_process_db_misses".to_string(),
        serde_json::json!(misses),
    );
    obj.insert(
        "salsa_process_db_evictions".to_string(),
        serde_json::json!(evictions),
    );
}

/// Fields for stdout JSON and validate-json: mirrors `CompilePerfReport::backend_dae_cache_status`
/// and `Artifacts::param_only_update` (fallback: `CompilePerfReport::param_only_update` when no artifacts).
pub(crate) fn compile_export_sidebar_json(
    compile_perf: &serde_json::Value,
    artifacts: Option<&Artifacts>,
) -> serde_json::Value {
    let backend_dae = compile_perf
        .get("backend_dae_cache_status")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let param_only = artifacts
        .map(|a| a.param_only_update)
        .or_else(|| {
            compile_perf
                .get("param_only_update")
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(false);
    serde_json::json!({
        "backend_dae_cache_status": backend_dae,
        "param_only_update": param_only,
    })
}

pub(crate) fn maybe_write_perf_json(
    perf_json_path: &Option<String>,
    model_name: &str,
    warnings_count: usize,
    mut compile_perf: Option<serde_json::Value>,
    sim_perf: Option<serde_json::Value>,
) -> Result<(), RunError> {
    let Some(path) = perf_json_path.as_ref() else {
        return Ok(());
    };
    if let Some(ref mut cp) = compile_perf {
        merge_salsa_process_db_stats_into_compile_perf(cp);
    }
    let payload = serde_json::json!({
        "model": model_name,
        "warnings_count": warnings_count,
        "compile_perf": compile_perf,
        "sim_perf": sim_perf
    });
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|e| RunError::Message(format!("serialize perf json failed: {}", e)))?;
    fs::write(path, text)
        .map_err(|e| RunError::Message(format!("write perf json '{}' failed: {}", path, e)))?;
    Ok(())
}
