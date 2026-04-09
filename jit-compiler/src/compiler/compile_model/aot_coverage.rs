use std::hash::{Hash, Hasher};
use std::path::Path;

use crate::ast::Equation;

use super::env_perf::perf_trace_enabled;

#[derive(Debug, Clone, Copy)]
pub(crate) enum AotCacheStatus {
    DisabledNoEnv,
    DisabledEmptyDir,
    Hit,
    Store,
    WriteFailed,
    MkdirFailed,
}

impl AotCacheStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DisabledNoEnv => "disabled_no_env",
            Self::DisabledEmptyDir => "disabled_empty_dir",
            Self::Hit => "hit",
            Self::Store => "store",
            Self::WriteFailed => "write_failed",
            Self::MkdirFailed => "mkdir_failed",
        }
    }
}

pub(crate) fn maybe_write_aot_cache_marker(
    model_name: &str,
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    options: &crate::compiler::CompilerOptions,
) -> AotCacheStatus {
    let Ok(cache_dir) = std::env::var("RUSTMODLICA_AOT_CACHE_DIR") else {
        return AotCacheStatus::DisabledNoEnv;
    };
    if cache_dir.trim().is_empty() {
        return AotCacheStatus::DisabledEmptyDir;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    model_name.hash(&mut hasher);
    options.solver.hash(&mut hasher);
    options.index_reduction_method.hash(&mut hasher);
    options.tearing_method.hash(&mut hasher);
    options.generate_dynamic_jacobian.hash(&mut hasher);
    alg_equations.len().hash(&mut hasher);
    diff_equations.len().hash(&mut hasher);
    for eq in alg_equations.iter().chain(diff_equations.iter()) {
        format!("{:?}", eq).hash(&mut hasher);
    }
    let key = format!("{:016x}", hasher.finish());
    let cache_root = std::path::PathBuf::from(cache_dir);
    if std::fs::create_dir_all(&cache_root).is_err() {
        return AotCacheStatus::MkdirFailed;
    }
    let path = cache_root.join(format!("{}.aot-marker", key));
    if path.exists() {
        eprintln!("[aot-cache] hit {}", path.display());
        if perf_trace_enabled() {
            eprintln!("[perf] aot_cache=hit key={}", key);
        }
        return AotCacheStatus::Hit;
    }
    let payload = format!(
        "model={}\nkey={}\nsolver={}\nalg_eqs={}\ndiff_eqs={}\n",
        model_name,
        key,
        options.solver,
        alg_equations.len(),
        diff_equations.len()
    );
    if std::fs::write(&path, payload).is_ok() {
        eprintln!("[aot-cache] store {}", path.display());
        if perf_trace_enabled() {
            eprintln!("[perf] aot_cache=store key={}", key);
        }
        return AotCacheStatus::Store;
    }
    if perf_trace_enabled() {
        eprintln!("[perf] aot_cache=write_failed key={}", key);
    }
    AotCacheStatus::WriteFailed
}

pub(crate) fn parse_coverage_status() -> Option<(f64, f64, f64, f64, Vec<String>)> {
    let candidate_paths = [
        Path::new("scripts/coverage_status.json"),
        Path::new("jit-compiler/scripts/coverage_status.json"),
    ];
    let path = candidate_paths.iter().find(|p| p.exists())?;
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let sem_target = v.get("semantic_target_percent")?.as_f64()?;
    let sem_current = v.get("semantic_current_percent")?.as_f64()?;
    let m34_target = v.get("modelica34_target_percent")?.as_f64()?;
    let m34_current = v.get("modelica34_current_percent")?.as_f64()?;
    let gaps = v
        .get("gaps")
        .and_then(|g| g.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some((sem_target, sem_current, m34_target, m34_current, gaps))
}

pub(crate) fn maybe_coverage_target_warning_message() -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let Some((sem_target, sem_current, m34_target, m34_current, gaps)) = parse_coverage_status() else {
        return Ok(None);
    };
    let sem_ok = sem_current + f64::EPSILON >= sem_target;
    let m34_ok = m34_current + f64::EPSILON >= m34_target;
    if sem_ok && m34_ok {
        return Ok(None);
    }
    let gap_text = if gaps.is_empty() {
        "none listed".to_string()
    } else {
        gaps.join(", ")
    };
    let msg = format!(
        "coverage target not met: semantic {:.2}% / target {:.2}%, Modelica 3.4 {:.2}% / target {:.2}%. gaps: {}. Run `powershell -ExecutionPolicy Bypass -File scripts/run_mos_regression.ps1` and refresh `scripts/coverage_status.json`.",
        sem_current, sem_target, m34_current, m34_target, gap_text
    );
    let strict = std::env::var("RUSTMODLICA_COVERAGE_STRICT")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
        .unwrap_or(false);
    if strict {
        return Err(msg.into());
    }
    Ok(Some(msg))
}
