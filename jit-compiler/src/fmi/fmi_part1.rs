fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn fmi_generation_tool() -> String {
    std::env::var("RUSTMODLICA_FMI_GENERATION_TOOL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "rustmodlica".to_string())
}

fn sanitize_c_identifier(raw: &str) -> String {
    let mut out = String::new();
    let mut prev_us = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
            prev_us = false;
        } else if !out.is_empty() && !prev_us {
            out.push('_');
            prev_us = true;
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        return String::new();
    }
    let first = out.chars().next().unwrap();
    if first.is_ascii_digit() {
        format!("m_{}", out)
    } else {
        out
    }
}

/// FMI 2.0 `modelIdentifier` must be a portable identifier (C symbol style). Qualified Modelica
/// names like `TestLib/SimpleTest` or `Pkg.Inner` are reduced to the last path/segment, then
/// non-alphanumeric characters become underscores.
///
/// Full override: `RUSTMODLICA_FMI_MODEL_ID` (trimmed) replaces the derived id; still sanitized.
/// Ignores `RUSTMODLICA_FMI_MODEL_ID_PREFIX` when set.
///
/// Optional prefix (when override unset): `RUSTMODLICA_FMI_MODEL_ID_PREFIX` prepends
/// `prefix_SimpleTest` after sanitization.
fn fmi_model_identifier(model_name: &str) -> String {
    if let Ok(override_id) = env::var("RUSTMODLICA_FMI_MODEL_ID") {
        let o = override_id.trim();
        if !o.is_empty() {
            let id = sanitize_c_identifier(o);
            if !id.is_empty() {
                return id;
            }
        }
    }

    let s = model_name.trim();
    let after_slash = s.rsplit('/').next().unwrap_or(s).trim();
    let leaf = after_slash.rsplit('.').next().unwrap_or(after_slash).trim();
    let mut id = sanitize_c_identifier(leaf);
    if id.is_empty() {
        id = "rustmodlica_model".to_string();
    }
    if let Ok(prefix) = env::var("RUSTMODLICA_FMI_MODEL_ID_PREFIX") {
        let p = prefix.trim();
        if !p.is_empty() {
            let p_s = sanitize_c_identifier(p);
            if !p_s.is_empty() {
                id = format!("{}_{}", p_s, id);
            }
        }
    }
    id
}

/// When set, CLI / API overrides take precedence over `RUSTMODLICA_FMI_MODEL_ID` and derived names.
#[derive(Clone, Default, Debug)]
pub struct FmiExportOptions {
    pub model_identifier_override: Option<String>,
    pub guid_override: Option<String>,
}

/// `model_identifier_override` (if non-empty after sanitize) wins; otherwise `RUSTMODLICA_FMI_MODEL_ID`,
/// optional `RUSTMODLICA_FMI_MODEL_ID_PREFIX`, then sanitized last path segment of the qualified name.
pub fn resolve_model_identifier(qualified_model_name: &str, model_identifier_override: Option<&str>) -> String {
    if let Some(o) = model_identifier_override {
        let t = o.trim();
        if !t.is_empty() {
            let id = sanitize_c_identifier(t);
            if !id.is_empty() {
                return id;
            }
        }
    }
    fmi_model_identifier(qualified_model_name)
}

fn normalize_guid_candidate(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.len() > 256 {
        return None;
    }
    if t.len() == 36 {
        let b = t.as_bytes();
        if b.len() == 36
            && b[8] == b'-'
            && b[13] == b'-'
            && b[18] == b'-'
            && b[23] == b'-'
            && t.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        {
            return Some(t.to_ascii_lowercase());
        }
    }
    if t.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Some(t.to_string());
    }
    None
}

fn resolve_fmi_guid(options: &FmiExportOptions) -> Result<String, String> {
    if let Some(ref g) = options.guid_override {
        return normalize_guid_candidate(g).ok_or_else(|| {
            format!(
                "invalid FMI guid override (use UUID or ASCII alnum with -/_ only): {:?}",
                g
            )
        });
    }
    if let Ok(env_g) = env::var("RUSTMODLICA_FMI_GUID") {
        let t = env_g.trim();
        if !t.is_empty() {
            return normalize_guid_candidate(t).ok_or_else(|| {
                format!(
                    "invalid RUSTMODLICA_FMI_GUID (use UUID or ASCII alnum with -/_ only): {:?}",
                    t
                )
            });
        }
    }
    Ok(generate_random_fmi_guid())
}

fn generate_random_fmi_guid() -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        rand_guid_part(),
        rand_guid_part() & 0x0fff | 0x4000,
        rand_guid_part() & 0x3fff | 0x8000,
        rand_guid_part(),
        rand_guid_part_48()
    )
}

/// Emit modelDescription.xml for FMI 2.0 Co-Simulation.
pub fn emit_model_description(
    out: &mut dyn Write,
    model_display_name: &str,
    model_identifier: &str,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    guid: &str,
    start_time: f64,
    stop_time: f64,
    step_size: f64,
) -> Result<(), String> {
    writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "<fmiModelDescription modelName=\"{}\" guid=\"{}\" generationTool=\"{}\" version=\"1.0\" fmiVersion=\"2.0\">",
        escape_xml(model_display_name),
        escape_xml(guid),
        escape_xml(&fmi_generation_tool())
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  <CoSimulation modelIdentifier=\"{}\" canBeInstantiatedOnlyOncePerProcess=\"false\" canNotUseMemoryManagementFunctions=\"false\" canHandleVariableCommunicationStepSize=\"true\" canInterpolateInputs=\"true\"/>",
        escape_xml(model_identifier)
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  <ModelVariables>").map_err(|e| e.to_string())?;
    emit_model_variables(out, state_vars, param_vars, output_vars)?;
    writeln!(out, "  </ModelVariables>").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  <DefaultExperiment startTime=\"{}\" stopTime=\"{}\" stepSize=\"{}\"/>",
        start_time, stop_time, step_size
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "</fmiModelDescription>").map_err(|e| e.to_string())?;
    Ok(())
}

/// Emit ModelVariables section (shared by CS and ME).
fn emit_model_variables(
    out: &mut dyn Write,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
) -> Result<(), String> {
    let mut vr = 0u32;
    writeln!(
        out,
        "    <ScalarVariable name=\"time\" valueReference=\"{}\" causality=\"independent\" variability=\"continuous\">",
        vr
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "      <Real/>").map_err(|e| e.to_string())?;
    writeln!(out, "    </ScalarVariable>").map_err(|e| e.to_string())?;
    vr += 1;
    for name in state_vars {
        writeln!(
            out,
            "    <ScalarVariable name=\"{}\" valueReference=\"{}\" causality=\"local\" variability=\"continuous\" initial=\"exact\">",
            escape_xml(name),
            vr
        )
        .map_err(|e| e.to_string())?;
        writeln!(out, "      <Real/>").map_err(|e| e.to_string())?;
        writeln!(out, "    </ScalarVariable>").map_err(|e| e.to_string())?;
        vr += 1;
    }
    for name in param_vars {
        writeln!(
            out,
            "    <ScalarVariable name=\"{}\" valueReference=\"{}\" causality=\"parameter\" variability=\"fixed\" initial=\"exact\">",
            escape_xml(name),
            vr
        )
        .map_err(|e| e.to_string())?;
        writeln!(out, "      <Real/>").map_err(|e| e.to_string())?;
        writeln!(out, "    </ScalarVariable>").map_err(|e| e.to_string())?;
        vr += 1;
    }
    for name in output_vars {
        writeln!(
            out,
            "    <ScalarVariable name=\"{}\" valueReference=\"{}\" causality=\"output\" variability=\"continuous\" initial=\"calculated\">",
            escape_xml(name),
            vr
        )
        .map_err(|e| e.to_string())?;
        writeln!(out, "      <Real/>").map_err(|e| e.to_string())?;
        writeln!(out, "    </ScalarVariable>").map_err(|e| e.to_string())?;
        vr += 1;
    }
    Ok(())
}

/// Emit modelDescription.xml for FMI 2.0 Model Exchange (FMI-2).
pub fn emit_model_description_me(
    out: &mut dyn Write,
    model_display_name: &str,
    model_identifier: &str,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    guid: &str,
    start_time: f64,
    stop_time: f64,
    step_size: f64,
) -> Result<(), String> {
    writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "<fmiModelDescription modelName=\"{}\" guid=\"{}\" generationTool=\"{}\" version=\"1.0\" fmiVersion=\"2.0\">",
        escape_xml(model_display_name),
        escape_xml(guid),
        escape_xml(&fmi_generation_tool())
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  <ModelExchange modelIdentifier=\"{}\" canBeInstantiatedOnlyOncePerProcess=\"false\" canNotUseMemoryManagementFunctions=\"false\"/>",
        escape_xml(model_identifier)
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  <ModelVariables>").map_err(|e| e.to_string())?;
    emit_model_variables(out, state_vars, param_vars, output_vars)?;
    writeln!(out, "  </ModelVariables>").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  <DefaultExperiment startTime=\"{}\" stopTime=\"{}\" stepSize=\"{}\"/>",
        start_time, stop_time, step_size
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "</fmiModelDescription>").map_err(|e| e.to_string())?;
    Ok(())
}
