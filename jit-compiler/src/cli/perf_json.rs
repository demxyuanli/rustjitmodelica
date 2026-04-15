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
    let dual_ok = compile_perf
        .get("dual_compile_ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let dual_req = compile_perf
        .get("dual_compile_requested")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let dual_spec = compile_perf
        .get("dual_compile_speculation_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let dual_status = compile_perf
        .get("dual_compile_status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let dual_error = compile_perf.get("dual_compile_error").cloned();
    let dual_error = match dual_error {
        Some(serde_json::Value::Object(_)) => dual_error,
        Some(serde_json::Value::Null) | None => {
            let code = compile_perf.get("dual_compile_error_code").cloned();
            let detail = compile_perf.get("dual_compile_error_detail").cloned();
            let poisoned = compile_perf.get("dual_compile_error_registry_poisoned").cloned();
            let phase = compile_perf.get("dual_compile_error_cranelift_phase").cloned();
            let sym = compile_perf.get("dual_compile_error_symbol_name").cloned();
            if code.is_none()
                && detail.is_none()
                && poisoned.is_none()
                && phase.is_none()
                && sym.is_none()
            {
                None
            } else {
                let mut m = serde_json::Map::new();
                if let Some(v) = code {
                    m.insert("code".to_string(), v);
                }
                if let Some(v) = poisoned {
                    m.insert("registry_poisoned".to_string(), v);
                }
                if let Some(v) = phase {
                    m.insert("cranelift_phase".to_string(), v);
                }
                if let Some(v) = sym {
                    m.insert("symbol_name".to_string(), v);
                }
                if let Some(v) = detail {
                    m.insert("detail".to_string(), v);
                }
                Some(serde_json::Value::Object(m))
            }
        }
        _ => None,
    }
    .unwrap_or(serde_json::Value::Null);
    let aot_native = compile_perf
        .get("aot_native_load_status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let aot_native_detail = compile_perf
        .get("aot_native_load_detail")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "backend_dae_cache_status": backend_dae,
        "param_only_update": param_only,
        "dual_compile_requested": dual_req,
        "dual_compile_ok": dual_ok,
        "dual_compile_speculation_count": dual_spec,
        "dual_compile_status": dual_status,
        "dual_compile_error": dual_error,
        "aot_native_load_status": aot_native,
        "aot_native_load_detail": aot_native_detail,
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
