use rustmodlica::{CompileStopPhase, WarningInfo};

use super::RunError;

pub(crate) fn emit_validate_json(
    success: bool,
    warnings: &[WarningInfo],
    errors: &[String],
    state_vars: &[String],
    output_vars: &[String],
    validation_stop_phase: Option<&str>,
    validation_partial: bool,
    compile_export: Option<serde_json::Value>,
) {
    let warnings_json: Vec<serde_json::Value> = warnings
        .iter()
        .map(|w| {
            serde_json::json!({
                "path": w.path,
                "line": w.line,
                "column": w.column,
                "message": w.message
            })
        })
        .collect();
    let mut out = serde_json::json!({
        "success": success,
        "warnings": warnings_json,
        "errors": errors,
        "state_vars": state_vars,
        "output_vars": output_vars,
        "validationStopPhase": validation_stop_phase,
        "validationPartial": validation_partial,
    });
    if let Some(serde_json::Value::Object(extra)) = compile_export {
        if let Some(out_obj) = out.as_object_mut() {
            for (k, v) in extra {
                out_obj.insert(k.clone(), v.clone());
            }
        }
    }
    println!("{}", serde_json::to_string(&out).unwrap_or_default());
}

pub(crate) fn parse_validate_tier(s: &str) -> Result<CompileStopPhase, RunError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "full" => Ok(CompileStopPhase::Full),
        "parse" => Ok(CompileStopPhase::Parse),
        "flatten" => Ok(CompileStopPhase::Flatten),
        "analyze" => Ok(CompileStopPhase::Analyze),
        _ => Err(RunError::Message(format!(
            "unknown --validate-tier={} (use full|parse|flatten|analyze)",
            s.trim()
        ))),
    }
}
