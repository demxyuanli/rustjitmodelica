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

/// Emit fmi2_me.c: FMI 2.0 Model Exchange API (FMI-2). Importer performs integration; FMU provides GetDerivatives, SetTime, Set/GetContinuousStates, Set/GetReal.
pub fn emit_fmi2_me_c(
    out: &mut dyn Write,
    n_states: usize,
    n_params: usize,
    n_outputs: usize,
) -> Result<(), String> {
    writeln!(
        out,
        "/* FMI 2.0 Model Exchange wrapper for rustmodlica-generated model.c */"
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
    writeln!(out, "  if (fmuType != fmi2ModelExchange) return NULL;").map_err(|e| e.to_string())?;
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
        "fmi2Status fmi2SetTime(fmi2Component c, fmi2Real t) {{"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  Instance *inst = (Instance*)c; if (!inst) return fmi2Error; inst->t = t; return fmi2OK; }}").map_err(|e| e.to_string())?;
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
    emit_fmu_artifacts_with_options(
        dir,
        model_name,
        state_vars,
        param_vars,
        output_vars,
        start_time,
        stop_time,
        step_size,
        &FmiExportOptions::default(),
    )
}

/// Same as [`emit_fmu_artifacts`] with optional `model_identifier` / `guid` overrides (CLI or embedder).
pub fn emit_fmu_artifacts_with_options(
    dir: &std::path::Path,
    model_display_name: &str,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    start_time: f64,
    stop_time: f64,
    step_size: f64,
    options: &FmiExportOptions,
) -> Result<Vec<std::path::PathBuf>, String> {
    let model_id = resolve_model_identifier(
        model_display_name,
        options.model_identifier_override.as_deref(),
    );
    let guid = resolve_fmi_guid(options)?;
    let xml_path = dir.join("modelDescription.xml");
    let mut xml_file = std::fs::File::create(&xml_path).map_err(|e| e.to_string())?;
    emit_model_description(
        &mut xml_file,
        model_display_name,
        &model_id,
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

/// Write modelDescription.xml (ME) and fmi2_me.c to dir for FMI 2.0 Model Exchange (FMI-2). Requires model.c/model.h already in dir (from --emit-c).
pub fn emit_fmu_me_artifacts(
    dir: &std::path::Path,
    model_name: &str,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    start_time: f64,
    stop_time: f64,
    step_size: f64,
) -> Result<Vec<std::path::PathBuf>, String> {
    emit_fmu_me_artifacts_with_options(
        dir,
        model_name,
        state_vars,
        param_vars,
        output_vars,
        start_time,
        stop_time,
        step_size,
        &FmiExportOptions::default(),
    )
}

/// Same as [`emit_fmu_me_artifacts`] with optional overrides.
pub fn emit_fmu_me_artifacts_with_options(
    dir: &std::path::Path,
    model_display_name: &str,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    start_time: f64,
    stop_time: f64,
    step_size: f64,
    options: &FmiExportOptions,
) -> Result<Vec<std::path::PathBuf>, String> {
    let model_id = resolve_model_identifier(
        model_display_name,
        options.model_identifier_override.as_deref(),
    );
    let guid = resolve_fmi_guid(options)?;
    let xml_path = dir.join("modelDescription.xml");
    let mut xml_file = std::fs::File::create(&xml_path).map_err(|e| e.to_string())?;
    emit_model_description_me(
        &mut xml_file,
        model_display_name,
        &model_id,
        state_vars,
        param_vars,
        output_vars,
        &guid,
        start_time,
        stop_time,
        step_size,
    )?;
    let me_path = dir.join("fmi2_me.c");
    let mut me_file = std::fs::File::create(&me_path).map_err(|e| e.to_string())?;
    emit_fmi2_me_c(
        &mut me_file,
        state_vars.len(),
        param_vars.len(),
        output_vars.len(),
    )?;
    Ok(vec![xml_path, me_path])
}

fn rand_guid_part() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    (t & 0xFFFF_FFFF) as u32
}
fn rand_guid_part_48() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    (t >> 16) as u64 & 0xFFFF_FFFF_FFFF
}

// ── FMU packaging (ZIP) ──────────────────────────────────────────────

/// Pack a directory containing FMU artifacts into a .fmu (ZIP) file.
/// FMU spec: ZIP with modelDescription.xml at root, sources/ and binaries/<platform>/.
pub fn package_fmu(dir: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
    let entries = collect_fmu_entries(dir)?;
    if entries.is_empty() {
        return Err("No FMU artifacts found in directory".to_string());
    }
    let mut f = std::fs::File::create(output).map_err(|e| e.to_string())?;
    write_zip(&entries, &mut f)?;
    Ok(())
}

struct ZipEntry {
    name: String,       // relative path within FMU, e.g. "modelDescription.xml"
    data: Vec<u8>,      // file content
}

fn collect_fmu_entries(dir: &std::path::Path) -> Result<Vec<ZipEntry>, String> {
    let mut entries = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path.strip_prefix(dir).map_err(|e| e.to_string())?;
            let name = rel.to_string_lossy().replace('\\', "/");
            let data = std::fs::read(&path).map_err(|e| e.to_string())?;
            entries.push(ZipEntry { name, data });
        }
    }
    Ok(entries)
}

/// Minimal ZIP writer (no compression, store-only). Sufficient for FMU packaging.
fn write_zip(entries: &[ZipEntry], w: &mut dyn std::io::Write) -> Result<(), String> {
    use std::io::Write as IoWrite;
    let mut central: Vec<u8> = Vec::new();
    let mut offset: u32 = 0;
    for e in entries {
        let name_bytes = e.name.as_bytes();
        let crc = crc32(&e.data);
        // Local file header
        w.write_all(b"PK\x03\x04").map_err(|e| e.to_string())?; // signature
        w.write_all(&20u16.to_le_bytes()).map_err(|e| e.to_string())?; // version needed
        w.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // flags (no compression)
        w.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // method (store)
        w.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // mod time
        w.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // mod date
        w.write_all(&crc.to_le_bytes()).map_err(|e| e.to_string())?;
        let compressed_size = e.data.len() as u32;
        let uncompressed_size = compressed_size;
        w.write_all(&compressed_size.to_le_bytes()).map_err(|e| e.to_string())?;
        w.write_all(&uncompressed_size.to_le_bytes()).map_err(|e| e.to_string())?;
        let name_len = name_bytes.len() as u16;
        w.write_all(&name_len.to_le_bytes()).map_err(|e| e.to_string())?;
        w.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // extra field length
        w.write_all(name_bytes).map_err(|e| e.to_string())?;
        w.write_all(&e.data).map_err(|e| e.to_string())?;

        // Central directory entry
        central.write_all(b"PK\x01\x02").map_err(|e| e.to_string())?;
        central.write_all(&20u16.to_le_bytes()).map_err(|e| e.to_string())?; // version made by
        central.write_all(&20u16.to_le_bytes()).map_err(|e| e.to_string())?; // version needed
        central.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // flags
        central.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // method
        central.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // mod time
        central.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // mod date
        central.write_all(&crc.to_le_bytes()).map_err(|e| e.to_string())?;
        central.write_all(&compressed_size.to_le_bytes()).map_err(|e| e.to_string())?;
        central.write_all(&uncompressed_size.to_le_bytes()).map_err(|e| e.to_string())?;
        central.write_all(&name_len.to_le_bytes()).map_err(|e| e.to_string())?;
        central.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // extra field length
        central.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // file comment length
        central.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // disk number
        central.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // internal attrs
        central.write_all(&0u32.to_le_bytes()).map_err(|e| e.to_string())?; // external attrs
        central.write_all(&offset.to_le_bytes()).map_err(|e| e.to_string())?; // local header offset
        central.write_all(name_bytes).map_err(|e| e.to_string())?;

        // Update offset for next entry
        offset += 30 + name_len as u32 + compressed_size;
    }

    let central_start = offset;
    w.write_all(&central).map_err(|e| e.to_string())?;
    let central_size = central.len() as u32;
    // End of central directory record
    w.write_all(b"PK\x05\x06").map_err(|e| e.to_string())?;
    w.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // disk number
    w.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // disk with central dir
    w.write_all(&(entries.len() as u16).to_le_bytes()).map_err(|e| e.to_string())?; // entries on disk
    w.write_all(&(entries.len() as u16).to_le_bytes()).map_err(|e| e.to_string())?; // total entries
    w.write_all(&central_size.to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(&central_start.to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // comment length
    Ok(())
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Try to compile C sources in `dir` into a shared library. Returns the path to the compiled binary.
/// Requires a C compiler (cc/gcc/clang) on PATH.
pub fn compile_fmu_shared_lib(dir: &std::path::Path, model_name: &str) -> Result<std::path::PathBuf, String> {
    let os = std::env::consts::OS;
    let (dll_ext, platform_dir) = match os {
        "windows" => ("dll", "win64"),
        "linux" => ("so", "linux64"),
        "macos" => ("dylib", "darwin64"),
        _ => return Err(format!("FMU compile: unsupported OS '{}'", os)),
    };
    let cc = find_c_compiler()?;
    let model_c = dir.join("model.c");
    let fmi_c = if dir.join("fmi2_cs.c").exists() {
        dir.join("fmi2_cs.c")
    } else {
        dir.join("fmi2_me.c")
    };
    if !model_c.exists() {
        return Err("model.c not found — run --emit-c first".to_string());
    }
    let bin_dir = dir.join("binaries").join(platform_dir);
    std::fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
    let out_name = format!("{}.{}", model_name, dll_ext);
    let out_path = bin_dir.join(&out_name);

    let mut cmd = std::process::Command::new(&cc);
    if os == "windows" {
        cmd.arg("/LD").arg("/Fe:").arg(&out_path);
    } else {
        cmd.arg("-shared").arg("-fPIC").arg("-o").arg(&out_path);
    }
    if os == "macos" {
        cmd.arg("-dynamiclib");
    }
    cmd.arg(&model_c).arg(&fmi_c);
    if os == "linux" {
        cmd.arg("-lm");
    }

    let output = cmd.output().map_err(|e| format!("Failed to run {}: {}", cc, e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("C compilation failed:\n{}", stderr));
    }
    Ok(out_path)
}

fn find_c_compiler() -> Result<String, String> {
    for cc in &["cc", "gcc", "clang", "cl"] {
        if std::process::Command::new(cc).arg("--version").output().is_ok() {
            return Ok(cc.to_string());
        }
    }
    // On Windows, try MSVC via environment
    if let Ok(vs) = std::env::var("VCToolsInstallDir") {
        return Ok(format!("{}/bin/Hostx64/x64/cl.exe", vs));
    }
    Err("No C compiler found (tried cc, gcc, clang, cl). Install one or set CC env var.".to_string())
}

/// Complete FMU export: emit C sources + FMI artifacts, compile, and package.
pub fn emit_complete_fmu(
    dir: &std::path::Path,
    fmu_path: &std::path::Path,
    model_name: &str,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    start_time: f64,
    stop_time: f64,
    step_size: f64,
    options: &FmiExportOptions,
) -> Result<(), String> {
    // Emit FMI wrapper sources + modelDescription.xml
    emit_fmu_artifacts_with_options(
        dir, model_name, state_vars, param_vars, output_vars,
        start_time, stop_time, step_size, options,
    )?;
    // Try to compile the C sources
    let model_id = resolve_model_identifier(model_name, options.model_identifier_override.as_deref());
    let _compile_result = compile_fmu_shared_lib(dir, &model_id);
    // Package into .fmu even if compilation failed (sources are still useful)
    package_fmu(dir, fmu_path)?;
    Ok(())
}
