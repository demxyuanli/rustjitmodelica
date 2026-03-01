// FMI-1 / FMI-2: Minimal FMI 2.0 Co-Simulation export.
// Generates modelDescription.xml and fmi2_cs.c that wraps model.c residual(); user compiles and zips as FMU.

use std::io::Write;

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Emit modelDescription.xml for FMI 2.0 Co-Simulation.
pub fn emit_model_description(
    out: &mut dyn Write,
    model_name: &str,
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
        "<fmiModelDescription modelName=\"{}\" guid=\"{}\" generationTool=\"rustmodlica\" version=\"1.0\" fmiVersion=\"2.0\">",
        escape_xml(model_name),
        escape_xml(guid)
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  <CoSimulation modelIdentifier=\"{}\"/>", escape_xml(model_name)).map_err(|e| e.to_string())?;
    writeln!(out, "  <ModelVariables>").map_err(|e| e.to_string())?;
    let mut vr = 0u32;
    writeln!(
        out,
        "    <ScalarVariable name=\"time\" valueReference=\"{}\" causality=\"independent\" variability=\"continuous\"/>",
        vr
    )
    .map_err(|e| e.to_string())?;
    vr += 1;
    for name in state_vars {
        writeln!(
            out,
            "    <ScalarVariable name=\"{}\" valueReference=\"{}\" causality=\"local\" variability=\"continuous\" initial=\"exact\"/>",
            escape_xml(name),
            vr
        )
        .map_err(|e| e.to_string())?;
        vr += 1;
    }
    for name in param_vars {
        writeln!(
            out,
            "    <ScalarVariable name=\"{}\" valueReference=\"{}\" causality=\"parameter\" variability=\"fixed\" initial=\"exact\"/>",
            escape_xml(name),
            vr
        )
        .map_err(|e| e.to_string())?;
        vr += 1;
    }
    for name in output_vars {
        writeln!(
            out,
            "    <ScalarVariable name=\"{}\" valueReference=\"{}\" causality=\"output\" variability=\"continuous\" initial=\"calculated\"/>",
            escape_xml(name),
            vr
        )
        .map_err(|e| e.to_string())?;
        vr += 1;
    }
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

/// Emit fmi2_cs.c: FMI 2.0 Co-Simulation API that calls residual() from model.c.
/// Uses minimal inline FMI2 types so no external FMI SDK is required to compile.
pub fn emit_fmi2_cs_c(
    out: &mut dyn Write,
    n_states: usize,
    n_params: usize,
    n_outputs: usize,
) -> Result<(), String> {
    writeln!(out, "/* FMI 2.0 Co-Simulation wrapper for rustmodlica-generated model.c */").map_err(|e| e.to_string())?;
    writeln!(out, "#include \"model.h\"").map_err(|e| e.to_string())?;
    writeln!(out, "#include <stdlib.h>").map_err(|e| e.to_string())?;
    writeln!(out, "#include <string.h>").map_err(|e| e.to_string())?;
    writeln!(out, "typedef void* fmi2Component;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef double fmi2Real;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef unsigned int fmi2ValueReference;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef unsigned char fmi2Boolean;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef const char* fmi2String;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef void* fmi2FMUstate;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef unsigned char fmi2Byte;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef enum {{ fmi2OK, fmi2Warning, fmi2Discard, fmi2Error, fmi2Fatal, fmi2Pending }} fmi2Status;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef enum {{ fmi2ModelExchange, fmi2CoSimulation }} fmi2Type;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef struct {{ void* logger; void* allocateMemory; void* freeMemory; void* stepFinished; }} fmi2CallbackFunctions;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef struct {{").map_err(|e| e.to_string())?;
    writeln!(out, "  double t;").map_err(|e| e.to_string())?;
    writeln!(out, "  double *x;").map_err(|e| e.to_string())?;
    writeln!(out, "  double *xdot;").map_err(|e| e.to_string())?;
    writeln!(out, "  double *p;").map_err(|e| e.to_string())?;
    writeln!(out, "  double *y;").map_err(|e| e.to_string())?;
    writeln!(out, "}} Instance;").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Component fmi2Instantiate(fmi2String instanceName, fmi2Type fmuType, fmi2String fmuGUID, fmi2String fmuResourceLocation, const fmi2CallbackFunctions *functions, fmi2Boolean visible, fmi2Boolean loggingOn) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  (void)fmuResourceLocation; (void)visible; (void)loggingOn;").map_err(|e| e.to_string())?;
    writeln!(out, "  if (fmuType != fmi2CoSimulation) return NULL;").map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)calloc(1, sizeof(Instance));").map_err(|e| e.to_string())?;
    writeln!(out, "  if (!inst) return NULL;").map_err(|e| e.to_string())?;
    writeln!(out, "  inst->t = 0.0;").map_err(|e| e.to_string())?;
    writeln!(out, "  inst->x = (double*)calloc({}, sizeof(double));", n_states).map_err(|e| e.to_string())?;
    writeln!(out, "  inst->xdot = (double*)calloc({}, sizeof(double));", n_states).map_err(|e| e.to_string())?;
    writeln!(out, "  inst->p = (double*)calloc({}, sizeof(double));", n_params).map_err(|e| e.to_string())?;
    writeln!(out, "  inst->y = (double*)calloc({}, sizeof(double));", n_outputs).map_err(|e| e.to_string())?;
    writeln!(out, "  if (!inst->x || !inst->xdot || !inst->p || !inst->y) {{ free(inst); return NULL; }}").map_err(|e| e.to_string())?;
    writeln!(out, "  return (fmi2Component)inst;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "void fmi2FreeInstance(fmi2Component c) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)c; if (!inst) return;").map_err(|e| e.to_string())?;
    writeln!(out, "  free(inst->x); free(inst->xdot); free(inst->p); free(inst->y); free(inst);").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2SetContinuousStates(fmi2Component c, const fmi2Real x[], size_t nx) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)c; if (!inst || nx != {}) return fmi2Error;", n_states).map_err(|e| e.to_string())?;
    writeln!(out, "  memcpy(inst->x, x, nx * sizeof(fmi2Real)); return fmi2OK;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2GetContinuousStates(fmi2Component c, fmi2Real x[], size_t nx) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)c; if (!inst || nx != {}) return fmi2Error;", n_states).map_err(|e| e.to_string())?;
    writeln!(out, "  memcpy(x, inst->x, nx * sizeof(fmi2Real)); return fmi2OK;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2GetDerivatives(fmi2Component c, fmi2Real derivatives[], size_t nx) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)c; if (!inst || nx != {}) return fmi2Error;", n_states).map_err(|e| e.to_string())?;
    writeln!(out, "  residual(inst->t, inst->x, inst->xdot, inst->p, inst->y);").map_err(|e| e.to_string())?;
    writeln!(out, "  memcpy(derivatives, inst->xdot, nx * sizeof(fmi2Real)); return fmi2OK;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2SetReal(fmi2Component c, const fmi2ValueReference vr[], size_t nvr, const fmi2Real value[]) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)c; if (!inst) return fmi2Error;").map_err(|e| e.to_string())?;
    writeln!(out, "  for (size_t i = 0; i < nvr; i++) {{").map_err(|e| e.to_string())?;
    writeln!(out, "    if (vr[i] == 0) inst->t = value[i];").map_err(|e| e.to_string())?;
    writeln!(out, "    else if (vr[i] >= 1 && vr[i] <= {}) inst->x[vr[i]-1] = value[i];", n_states).map_err(|e| e.to_string())?;
    writeln!(out, "    else if (vr[i] >= {} && vr[i] < {} + {}) inst->p[vr[i]-{}] = value[i];", 1 + n_states, 1 + n_states, n_params, 1 + n_states).map_err(|e| e.to_string())?;
    writeln!(out, "  }} return fmi2OK;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2GetReal(fmi2Component c, const fmi2ValueReference vr[], size_t nvr, fmi2Real value[]) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)c; if (!inst) return fmi2Error;").map_err(|e| e.to_string())?;
    writeln!(out, "  residual(inst->t, inst->x, inst->xdot, inst->p, inst->y);").map_err(|e| e.to_string())?;
    writeln!(out, "  for (size_t i = 0; i < nvr; i++) {{").map_err(|e| e.to_string())?;
    writeln!(out, "    if (vr[i] == 0) value[i] = inst->t;").map_err(|e| e.to_string())?;
    writeln!(out, "    else if (vr[i] >= 1 && vr[i] <= {}) value[i] = inst->x[vr[i]-1];", n_states).map_err(|e| e.to_string())?;
    writeln!(out, "    else if (vr[i] >= {} && vr[i] < {} + {}) value[i] = inst->p[vr[i]-{}];", 1 + n_states, 1 + n_states, n_params, 1 + n_states).map_err(|e| e.to_string())?;
    writeln!(out, "    else value[i] = inst->y[vr[i]-1u-{}u-{}u]; }}", n_states, n_params).map_err(|e| e.to_string())?;
    writeln!(out, "  return fmi2OK;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2DoStep(fmi2Component c, fmi2Real currentCommunicationPoint, fmi2Real communicationStepSize, fmi2Boolean noSetFMUStatePriorToCurrentPoint) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  (void)noSetFMUStatePriorToCurrentPoint;").map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)c; if (!inst) return fmi2Error;").map_err(|e| e.to_string())?;
    writeln!(out, "  inst->t = currentCommunicationPoint + communicationStepSize;").map_err(|e| e.to_string())?;
    writeln!(out, "  residual(inst->t, inst->x, inst->xdot, inst->p, inst->y);").map_err(|e| e.to_string())?;
    writeln!(out, "  for (size_t i = 0; i < {}; i++) inst->x[i] += communicationStepSize * inst->xdot[i];", n_states).map_err(|e| e.to_string())?;
    writeln!(out, "  return fmi2OK;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2GetFMUstate(fmi2Component c, fmi2FMUstate* s) {{ (void)c; *s = NULL; return fmi2OK; }}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2SetFMUstate(fmi2Component c, fmi2FMUstate s) {{ (void)c; (void)s; return fmi2OK; }}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2FreeFMUstate(fmi2Component c, fmi2FMUstate* s) {{ (void)c; *s = NULL; return fmi2OK; }}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2SerializedFMUstateSize(fmi2Component c, fmi2FMUstate s, size_t* n) {{ (void)c; (void)s; *n = 0; return fmi2OK; }}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2SerializeFMUstate(fmi2Component c, fmi2FMUstate s, fmi2Byte serializedState[], size_t n) {{ (void)c; (void)s; (void)serializedState; (void)n; return fmi2OK; }}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2DeSerializeFMUstate(fmi2Component c, const fmi2Byte s[], size_t n, fmi2FMUstate* state) {{ (void)c; (void)s; (void)n; *state = NULL; return fmi2OK; }}").map_err(|e| e.to_string())?;
    Ok(())
}

/// Write modelDescription.xml and fmi2_cs.c to dir. Requires model.c/model.h already in dir (from --emit-c).
pub fn emit_fmu_artifacts(
    dir: &std::path::Path,
    model_name: &str,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    start_time: f64,
    stop_time: f64,
    step_size: f64,
) -> Result<Vec<std::path::PathBuf>, String> {
    let guid = format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        rand_guid_part(), rand_guid_part() & 0x0fff | 0x4000, rand_guid_part() & 0x3fff | 0x8000, rand_guid_part(), rand_guid_part_48());
    let xml_path = dir.join("modelDescription.xml");
    let mut xml_file = std::fs::File::create(&xml_path).map_err(|e| e.to_string())?;
    emit_model_description(
        &mut xml_file,
        model_name,
        state_vars,
        param_vars,
        output_vars,
        &guid,
        start_time,
        stop_time,
        step_size,
    )?;
    let cs_path = dir.join("fmi2_cs.c");
    let mut cs_file = std::fs::File::create(&cs_path).map_err(|e| e.to_string())?;
    emit_fmi2_cs_c(
        &mut cs_file,
        state_vars.len(),
        param_vars.len(),
        output_vars.len(),
    )?;
    Ok(vec![xml_path, cs_path])
}

fn rand_guid_part() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    (t & 0xFFFF_FFFF) as u32
}
fn rand_guid_part_48() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    (t >> 16) as u64 & 0xFFFF_FFFF_FFFF
}
