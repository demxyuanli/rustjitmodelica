use std::io::{self, BufRead, Write};

use rustmodlica::{CompileOutput, Compiler};

use super::perf_json::{compile_export_sidebar_json, maybe_write_perf_json};
use super::validate_json::emit_validate_json;
use super::RunError;

#[derive(Debug, serde::Deserialize)]
struct ValidateStdioRequest {
    #[serde(default)]
    quit: bool,
    #[serde(default)]
    model: String,
    /// When set, load this source before compile (IDE in-memory buffer).
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    perf_json: Option<String>,
    #[serde(default)]
    cache_stats_json: Option<String>,
    #[serde(default)]
    dep_graph_json: Option<String>,
    /// When true, embed full `compilePerf` in the stdout JSON response.
    #[serde(default)]
    embed_perf: bool,
}

fn apply_request_env(req: &ValidateStdioRequest) {
    if let Some(p) = &req.cache_stats_json {
        std::env::set_var("RUSTMODLICA_CACHE_STATS_JSON", p);
    }
    if let Some(p) = &req.dep_graph_json {
        std::env::set_var("RUSTMODLICA_DEP_GRAPH_JSON", p);
    }
}

fn validate_one(
    compiler: &mut Compiler,
    model: &str,
    code: Option<&str>,
    perf_json: &Option<String>,
    embed_perf: bool,
) -> bool {
    eprintln!("[validate-stdio] begin model={}", model);
    let perf_enabled = std::env::var("RUSTMODLICA_PERF_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false);
    let compile_t0 = if perf_enabled {
        Some(std::time::Instant::now())
    } else {
        None
    };

    let out = if let Some(src) = code {
        if let Err(e) = compiler.loader.load_model_from_source(model, src) {
            let warnings = compiler.take_warnings();
            emit_validate_json(
                false,
                &warnings,
                &[e.to_string()],
                &[],
                &[],
                None,
                false,
                None,
                None,
            );
            eprintln!("[validate-stdio] end model={} success=false", model);
            return false;
        }
        match compiler.compile(model) {
            Ok(o) => o,
            Err(e) => {
                let warnings = compiler.take_warnings();
                emit_validate_json(
                    false,
                    &warnings,
                    &[e.to_string()],
                    &[],
                    &[],
                    None,
                    false,
                    None,
                    None,
                );
                eprintln!("[validate-stdio] end model={} success=false", model);
                return false;
            }
        }
    } else {
        match compiler.compile(model) {
            Ok(o) => o,
            Err(e) => {
                let warnings = compiler.take_warnings();
                emit_validate_json(
                    false,
                    &warnings,
                    &[e.to_string()],
                    &[],
                    &[],
                    None,
                    false,
                    None,
                    None,
                );
                eprintln!("[validate-stdio] end model={} success=false", model);
                return false;
            }
        }
    };

    if let Some(t0) = compile_t0 {
        eprintln!("[perf] compile_ms={}", t0.elapsed().as_millis());
    }
    let warnings = compiler.take_warnings();
    let compile_perf = serde_json::to_value(compiler.take_compile_perf_report())
        .unwrap_or(serde_json::Value::Null);
    let compile_perf_export = if embed_perf {
        Some(compile_perf.clone())
    } else {
        None
    };
    let sidebar = |artifacts: Option<&rustmodlica::Artifacts>| {
        Some(compile_export_sidebar_json(&compile_perf, artifacts))
    };

    let success = match &out {
        CompileOutput::FunctionRun(_) => {
            let _ = maybe_write_perf_json(perf_json, model, warnings.len(), Some(compile_perf.clone()), None);
            emit_validate_json(
                true,
                &warnings,
                &[],
                &[],
                &[],
                Some("full"),
                false,
                sidebar(None),
                compile_perf_export.clone(),
            );
            true
        }
        CompileOutput::Simulation(artifacts) => {
            let _ = maybe_write_perf_json(
                perf_json,
                model,
                warnings.len(),
                Some(compile_perf.clone()),
                None,
            );
            emit_validate_json(
                true,
                &warnings,
                &[],
                &artifacts.state_vars,
                &artifacts.output_vars,
                Some("full"),
                false,
                sidebar(Some(artifacts)),
                compile_perf_export.clone(),
            );
            true
        }
        CompileOutput::FlatSnapshotDone => {
            let _ = maybe_write_perf_json(perf_json, model, warnings.len(), Some(compile_perf.clone()), None);
            emit_validate_json(
                true,
                &warnings,
                &[],
                &[],
                &[],
                Some("full"),
                false,
                sidebar(None),
                compile_perf_export.clone(),
            );
            true
        }
        CompileOutput::ValidationParseOk => {
            let _ = maybe_write_perf_json(perf_json, model, warnings.len(), Some(compile_perf.clone()), None);
            emit_validate_json(
                true,
                &warnings,
                &[],
                &[],
                &[],
                Some("parse"),
                true,
                sidebar(None),
                compile_perf_export.clone(),
            );
            true
        }
        CompileOutput::ValidationFlattenOk { .. } => {
            let _ = maybe_write_perf_json(perf_json, model, warnings.len(), Some(compile_perf.clone()), None);
            emit_validate_json(
                true,
                &warnings,
                &[],
                &[],
                &[],
                Some("flatten"),
                true,
                sidebar(None),
                compile_perf_export.clone(),
            );
            true
        }
        CompileOutput::ValidationAnalyzed(s) => {
            let _ = maybe_write_perf_json(perf_json, model, warnings.len(), Some(compile_perf.clone()), None);
            emit_validate_json(
                true,
                &warnings,
                &[],
                &s.state_vars,
                &s.output_vars,
                Some("analyze"),
                true,
                sidebar(None),
                compile_perf_export.clone(),
            );
            true
        }
    };
    eprintln!(
        "[validate-stdio] end model={} success={}",
        model, success
    );
    success
}

/// Long-lived validate worker: read JSON requests from stdin, emit one validate JSON per line on stdout.
/// Request: `{"model":"FQN","perf_json":"...","cache_stats_json":"...","dep_graph_json":"..."}`
/// Shutdown: `{"quit":true}` or empty line.
pub(crate) fn run_validate_stdio(compiler: &mut Compiler) -> Result<(), RunError> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| RunError::Message(format!("stdin read failed: {}", e)))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        let req: ValidateStdioRequest = serde_json::from_str(trimmed).map_err(|e| {
            RunError::Message(format!("invalid validate-stdio request JSON: {}", e))
        })?;
        if req.quit {
            break;
        }
        if req.model.is_empty() {
            return Err(RunError::Message(
                "validate-stdio request missing \"model\"".to_string(),
            ));
        }
        apply_request_env(&req);
        let code = req.code.as_deref();
        validate_one(
            compiler,
            &req.model,
            code,
            &req.perf_json,
            req.embed_perf,
        );
        stdout
            .flush()
            .map_err(|e| RunError::Message(format!("stdout flush failed: {}", e)))?;
    }
    Ok(())
}
