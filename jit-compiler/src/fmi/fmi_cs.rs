/// Emit fmi2_cs.c: FMI 2.0 Co-Simulation API that calls residual() from model.c.
/// Uses minimal inline FMI2 types so no external FMI SDK is required to compile.
pub fn emit_fmi2_cs_c(
    out: &mut dyn Write,
    n_states: usize,
    n_params: usize,
    n_outputs: usize,
) -> Result<(), String> {
    writeln!(
        out,
        "/* FMI 2.0 Co-Simulation wrapper for rustmodlica-generated model.c */"
    )
    .map_err(|e| e.to_string())?;
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
    writeln!(
        out,
        "typedef enum {{ fmi2ModelExchange, fmi2CoSimulation }} fmi2Type;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "typedef struct {{ void* logger; void* allocateMemory; void* freeMemory; void* stepFinished; }} fmi2CallbackFunctions;").map_err(|e| e.to_string())?;
    writeln!(out, "typedef struct {{").map_err(|e| e.to_string())?;
    writeln!(out, "  double t;").map_err(|e| e.to_string())?;
    writeln!(out, "  double *x;").map_err(|e| e.to_string())?;
    writeln!(out, "  double *xdot;").map_err(|e| e.to_string())?;
    writeln!(out, "  double *p;").map_err(|e| e.to_string())?;
    writeln!(out, "  double *y;").map_err(|e| e.to_string())?;
    writeln!(out, "}} Instance;").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Component fmi2Instantiate(fmi2String instanceName, fmi2Type fmuType, fmi2String fmuGUID, fmi2String fmuResourceLocation, const fmi2CallbackFunctions *functions, fmi2Boolean visible, fmi2Boolean loggingOn) {{").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  (void)fmuResourceLocation; (void)visible; (void)loggingOn;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  if (fmuType != fmi2CoSimulation) return NULL;").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  Instance *inst = (Instance*)calloc(1, sizeof(Instance));"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  if (!inst) return NULL;").map_err(|e| e.to_string())?;
    writeln!(out, "  inst->t = 0.0;").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  inst->x = (double*)calloc({}, sizeof(double));",
        n_states
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  inst->xdot = (double*)calloc({}, sizeof(double));",
        n_states
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  inst->p = (double*)calloc({}, sizeof(double));",
        n_params
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  inst->y = (double*)calloc({}, sizeof(double));",
        n_outputs
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  if (!inst->x || !inst->xdot || !inst->p || !inst->y) {{ free(inst); return NULL; }}"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  return (fmi2Component)inst;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "void fmi2FreeInstance(fmi2Component c) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)c; if (!inst) return;")
        .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  free(inst->x); free(inst->xdot); free(inst->p); free(inst->y); free(inst);"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "fmi2Status fmi2SetContinuousStates(fmi2Component c, const fmi2Real x[], size_t nx) {{"
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  Instance *inst = (Instance*)c; if (!inst || nx != {}) return fmi2Error;",
        n_states
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  memcpy(inst->x, x, nx * sizeof(fmi2Real)); return fmi2OK;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "fmi2Status fmi2GetContinuousStates(fmi2Component c, fmi2Real x[], size_t nx) {{"
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  Instance *inst = (Instance*)c; if (!inst || nx != {}) return fmi2Error;",
        n_states
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  memcpy(x, inst->x, nx * sizeof(fmi2Real)); return fmi2OK;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "fmi2Status fmi2GetDerivatives(fmi2Component c, fmi2Real derivatives[], size_t nx) {{"
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  Instance *inst = (Instance*)c; if (!inst || nx != {}) return fmi2Error;",
        n_states
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  residual(inst->t, inst->x, inst->xdot, inst->p, inst->y);"
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  memcpy(derivatives, inst->xdot, nx * sizeof(fmi2Real)); return fmi2OK;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2SetReal(fmi2Component c, const fmi2ValueReference vr[], size_t nvr, const fmi2Real value[]) {{").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  Instance *inst = (Instance*)c; if (!inst) return fmi2Error;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  for (size_t i = 0; i < nvr; i++) {{").map_err(|e| e.to_string())?;
    writeln!(out, "    if (vr[i] == 0) inst->t = value[i];").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "    else if (vr[i] >= 1 && vr[i] <= {}) inst->x[vr[i]-1] = value[i];",
        n_states
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "    else if (vr[i] >= {} && vr[i] < {} + {}) inst->p[vr[i]-{}] = value[i];",
        1 + n_states,
        1 + n_states,
        n_params,
        1 + n_states
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  }} return fmi2OK;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2GetReal(fmi2Component c, const fmi2ValueReference vr[], size_t nvr, fmi2Real value[]) {{").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  Instance *inst = (Instance*)c; if (!inst) return fmi2Error;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  residual(inst->t, inst->x, inst->xdot, inst->p, inst->y);"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  for (size_t i = 0; i < nvr; i++) {{").map_err(|e| e.to_string())?;
    writeln!(out, "    if (vr[i] == 0) value[i] = inst->t;").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "    else if (vr[i] >= 1 && vr[i] <= {}) value[i] = inst->x[vr[i]-1];",
        n_states
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "    else if (vr[i] >= {} && vr[i] < {} + {}) value[i] = inst->p[vr[i]-{}];",
        1 + n_states,
        1 + n_states,
        n_params,
        1 + n_states
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "    else value[i] = inst->y[vr[i]-1u-{}u-{}u]; }}",
        n_states, n_params
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  return fmi2OK;").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    writeln!(out, "fmi2Status fmi2DoStep(fmi2Component c, fmi2Real currentCommunicationPoint, fmi2Real communicationStepSize, fmi2Boolean noSetFMUStatePriorToCurrentPoint) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  (void)noSetFMUStatePriorToCurrentPoint;").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  Instance *inst = (Instance*)c; if (!inst) return fmi2Error;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  inst->t = currentCommunicationPoint + communicationStepSize;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  residual(inst->t, inst->x, inst->xdot, inst->p, inst->y);"
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "  for (size_t i = 0; i < {}; i++) inst->x[i] += communicationStepSize * inst->xdot[i];",
        n_states
    )
    .map_err(|e| e.to_string())?;
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

