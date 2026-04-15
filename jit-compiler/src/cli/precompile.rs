use std::fs;
use std::path::PathBuf;

use super::RunError;

/// Built-in MSL models suitable for install-time precompilation (L4-T05).
/// These are high-value standard library entry points that benefit most from
/// warm caches on first user invocation.
const MSL_PRECOMPILE_MODELS: &[&str] = &[
    "Modelica.Blocks.Examples.PID_Controller",
    "Modelica.Mechanics.Rotational.Examples.First",
    "Modelica.Electrical.Analog.Examples.ChuaCircuit",
    "Modelica.Thermal.HeatTransfer.Examples.TwoMasses",
    "Modelica.Fluid.Examples.HeatingSystem",
];

fn precompile_stamp_path() -> Option<PathBuf> {
    rustmodlica::flatten::flatten_cache_dir()
        .map(|r| r.join(".msl_precompiled"))
}

/// Run MSL precompile if not already done (first-run detection via stamp file).
pub(crate) fn run_msl_precompile_if_needed(lib_paths: &[String]) -> Result<(), RunError> {
    let stamp = match precompile_stamp_path() {
        Some(p) => p,
        None => return Ok(()),
    };
    if stamp.exists() {
        return Ok(());
    }
    eprintln!(
        "[msl-precompile] first-run: precompiling {} MSL models (Leyden install-time condenser)",
        MSL_PRECOMPILE_MODELS.len()
    );

    let path_bufs: Vec<PathBuf> = lib_paths.iter().map(|s| PathBuf::from(s)).collect();
    let models: Vec<String> = MSL_PRECOMPILE_MODELS.iter().map(|s| s.to_string()).collect();

    let (ok_count, _err_count) =
        rustmodlica::cache::warmup::precompile_models_parallel(&models, &path_bufs, false);

    if let Some(parent) = stamp.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        &stamp,
        format!(
            "precompiled {} of {} models\n",
            ok_count,
            MSL_PRECOMPILE_MODELS.len()
        ),
    );
    eprintln!(
        "[msl-precompile] done ({}/{} succeeded)",
        ok_count,
        MSL_PRECOMPILE_MODELS.len()
    );
    Ok(())
}

pub(crate) fn run_precompile(list_path: &str, lib_paths: &[String]) -> Result<(), RunError> {
    let text = fs::read_to_string(list_path)
        .map_err(|e| RunError::Message(format!("read '{}': {}", list_path, e)))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| RunError::Message(format!("json: {}", e)))?;
    let models: Vec<String> = if let Some(a) = v.as_array() {
        a.iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect()
    } else if let Some(a) = v.get("models").and_then(|x| x.as_array()) {
        a.iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        return Err(RunError::Message(
            "precompile json must be [\"A.B\", ...] or {\"models\":[\"A.B\"]}".into(),
        ));
    };
    if models.is_empty() {
        return Err(RunError::Message("precompile list is empty".into()));
    }
    let mut compiler = rustmodlica::Compiler::new();
    for p in lib_paths {
        compiler.loader.add_path(p.into());
    }
    for m in models {
        eprintln!("[precompile] {}", m);
        compiler
            .compile(&m)
            .map_err(|e| RunError::Message(format!("compile {}: {}", m, e)))?;
    }
    Ok(())
}
