mod c_codegen;
pub use c_codegen::{write_coder_package, CoderPackageInfo};
mod equation_convert;
mod initial_conditions;
pub(crate) mod inline;
pub(crate) mod adaptive;
mod jacobian;
mod pipeline;
mod compile_model;
mod solvable_scale_warn;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::analysis::ProvenanceIndex;
use crate::ast::{AlgorithmStatement, Equation, Expression, Model};
use crate::backend_dae::ClockPartition as BackendClockPartition;
use crate::diag::WarningInfo;
use crate::equation_graph;
use crate::equation_graph::EquationGraphMode;
use crate::expr_eval;
use crate::flatten::{ArraySizePolicy, Flattener};
use crate::jit::{CalcDerivsFunc, Jit};
use crate::loader::ModelLoader;
pub(crate) use pipeline::flatten_and_inline;
use pipeline::stage_trace_enabled;
pub use pipeline::geometric_default_for_name;

mod options;
pub use options::{
    CacheMissEvent, CompilePerfReport, CompileStopPhase, CompilerOptions,
    DualCompileErrorReport, ValidationAnalyzedSummary,
};

/// EXT-1/EXT-2: Keeps loaded libraries alive; resolved symbol pointers for JIT.
pub(crate) struct ExternalLibs(pub Vec<libloading::Library>);

pub struct Compiler {
    pub loader: ModelLoader,
    pub options: CompilerOptions,
    pub(crate) warnings: Vec<WarningInfo>,
    pub(crate) external_libraries: ExternalLibs,
    /// EXT-2: Resolved external function symbols (modelica_name -> ptr); valid while external_libraries is alive.
    pub(crate) external_symbol_ptrs: HashMap<String, *const u8>,
    /// Global string interner for variable name deduplication across compilation stages.
    pub interner: crate::string_intern::StringInterner,
    /// Structured compile-time performance report for the last compile call.
    pub last_compile_perf: Option<CompilePerfReport>,
    /// Equation/component provenance for the last successful flatten+inline (same flat as last compile).
    pub last_provenance_index: Option<Arc<ProvenanceIndex>>,
    /// Deferred strict-mode artifact write to flush on explicit confirmation.
    pub(crate) deferred_artifact: Option<DeferredArtifactWrite>,
}

impl Default for ExternalLibs {
    fn default() -> Self {
        ExternalLibs(Vec::new())
    }
}

/// EXT-4: Parse annotation string for Library="..." (e.g. annotation(Library="mylib")).
fn parse_annotation_library(annotation: Option<&String>) -> Option<String> {
    let s = annotation.as_ref()?.as_str();
    let s = s.trim();
    let idx = s.find("Library")?;
    let rest = s[idx + 7..].trim_start();
    let rest = rest
        .strip_prefix('=')
        .map(|r| r.trim_start())
        .unwrap_or(rest);
    let start = rest.find('"')?;
    let rest = &rest[start + 1..];
    let end = rest.find('"')?;
    let lib = rest[..end].trim();
    if lib.is_empty() {
        None
    } else {
        Some(lib.to_string())
    }
}

/// FUNC-2: Collect all function names called from equations and algorithms (after inlining).
pub(super) fn collect_all_called_names(
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
) -> HashSet<String> {
    let mut names = HashSet::new();
    fn normalize_called_name(name: &str) -> Option<String> {
        match name {
            "valveCharacteristic" => {
                Some("Modelica.Fluid.Valves.BaseClasses.ValveCharacteristics.linear".to_string())
            }
            "regRoot" | "regRoot2" | "regSquare2" | "regFun3" | "regStep" | "spliceFunction" => {
                Some(format!("Modelica.Fluid.Utilities.{name}"))
            }
            "arg" => Some("Modelica.ComplexMath.arg".to_string()),
            "distribution" => Some("Modelica.Blocks.Noise.Interfaces.distribution".to_string()),
            "oneTrue" => Some("Modelica.Electrical.Batteries.Utilities.oneTrue".to_string()),
            "isPowerOf2" => Some("Modelica.Electrical.Polyphase.Functions.isPowerOf2".to_string()),
            "numberOfSymmetricBaseSystems" => {
                Some("Modelica.Electrical.Polyphase.Functions.numberOfSymmetricBaseSystems".to_string())
            }
            "factorY2DC" => Some("Modelica.Electrical.Polyphase.Functions.factorY2DC".to_string()),
            "exlin" => Some("Modelica.Electrical.Analog.Semiconductors.exlin".to_string()),
            "exlin2" => Some("Modelica.Electrical.Analog.Semiconductors.exlin2".to_string()),
            _ => None,
        }
    }
    fn collect_calls_expr(expr: &Expression, out: &mut HashSet<String>) {
        match expr {
            Expression::Call(name, _) => {
                out.insert(name.clone());
                if let Some(normalized) = normalize_called_name(name) {
                    out.insert(normalized);
                }
            }
            Expression::BinaryOp(l, _, r) => {
                collect_calls_expr(l, out);
                collect_calls_expr(r, out);
            }
            Expression::Der(inner) => collect_calls_expr(inner, out),
            Expression::If(c, t, e) => {
                collect_calls_expr(c, out);
                collect_calls_expr(t, out);
                collect_calls_expr(e, out);
            }
            Expression::ArrayAccess(a, i) => {
                collect_calls_expr(a, out);
                collect_calls_expr(i, out);
            }
            Expression::Sample(inner)
            | Expression::Interval(inner)
            | Expression::Hold(inner)
            | Expression::Previous(inner) => collect_calls_expr(inner, out),
            Expression::SubSample(c, n)
            | Expression::SuperSample(c, n)
            | Expression::ShiftSample(c, n)
            | Expression::BackSample(c, n) => {
                collect_calls_expr(c, out);
                collect_calls_expr(n, out);
            }
            _ => {}
        }
    }
    fn collect_calls_eq(eq: &Equation, out: &mut HashSet<String>) {
        match eq {
            Equation::Simple(lhs, rhs) => {
                collect_calls_expr(lhs, out);
                collect_calls_expr(rhs, out);
            }
            Equation::CallStmt(expr) => {
                collect_calls_expr(expr, out);
            }
            Equation::SolvableBlock {
                equations,
                residuals,
                ..
            } => {
                for e in equations {
                    if let Equation::Simple(l, r) = e {
                        collect_calls_expr(l, out);
                        collect_calls_expr(r, out);
                    }
                }
                for r in residuals {
                    collect_calls_expr(r, out);
                }
            }
            _ => {}
        }
    }
    fn collect_calls_alg(stmt: &AlgorithmStatement, out: &mut HashSet<String>) {
        match stmt {
            AlgorithmStatement::Assignment(_, rhs) => collect_calls_expr(rhs, out),
            AlgorithmStatement::MultiAssign(_, rhs) => collect_calls_expr(rhs, out),
            AlgorithmStatement::CallStmt(expr) => collect_calls_expr(expr, out),
            AlgorithmStatement::NoOp => {}
            AlgorithmStatement::Break => {}
            AlgorithmStatement::Return(v) => {
                if let Some(expr) = v {
                    collect_calls_expr(expr, out);
                }
            }
            AlgorithmStatement::If(cond, t, eifs, els) => {
                collect_calls_expr(cond, out);
                for s in t {
                    collect_calls_alg(s, out);
                }
                for (c, b) in eifs {
                    collect_calls_expr(c, out);
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
                if let Some(b) = els {
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::For(_, range, body) => {
                collect_calls_expr(range, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
            }
            AlgorithmStatement::When(cond, body, elses) => {
                collect_calls_expr(cond, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
                for (c, b) in elses {
                    collect_calls_expr(c, out);
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::Reinit(_, e) => collect_calls_expr(e, out),
            AlgorithmStatement::Assert(cond, msg) => {
                collect_calls_expr(cond, out);
                collect_calls_expr(msg, out);
            }
            AlgorithmStatement::Terminate(msg) => collect_calls_expr(msg, out),
            AlgorithmStatement::While(cond, body) => {
                collect_calls_expr(cond, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
            }
        }
    }
    for eq in alg_equations.iter().chain(diff_equations) {
        collect_calls_eq(eq, &mut names);
    }
    for stmt in algorithms {
        collect_calls_alg(stmt, &mut names);
    }
    names
}

/// Raw call-site names from equations/algorithms (before external resolution / loader work).
pub(crate) fn collect_external_raw_call_sites(
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
) -> HashSet<String> {
    let mut names = HashSet::new();
    fn collect_calls_expr(expr: &Expression, out: &mut HashSet<String>) {
        match expr {
            Expression::Call(name, _) => {
                out.insert(name.clone());
            }
            Expression::BinaryOp(l, _, r) => {
                collect_calls_expr(l, out);
                collect_calls_expr(r, out);
            }
            Expression::Der(inner) => collect_calls_expr(inner, out),
            Expression::If(c, t, e) => {
                collect_calls_expr(c, out);
                collect_calls_expr(t, out);
                collect_calls_expr(e, out);
            }
            Expression::ArrayAccess(a, i) => {
                collect_calls_expr(a, out);
                collect_calls_expr(i, out);
            }
            Expression::Sample(inner)
            | Expression::Interval(inner)
            | Expression::Hold(inner)
            | Expression::Previous(inner) => collect_calls_expr(inner, out),
            Expression::SubSample(c, n)
            | Expression::SuperSample(c, n)
            | Expression::ShiftSample(c, n)
            | Expression::BackSample(c, n) => {
                collect_calls_expr(c, out);
                collect_calls_expr(n, out);
            }
            _ => {}
        }
    }
    fn collect_calls_eq(eq: &Equation, out: &mut HashSet<String>) {
        match eq {
            Equation::Simple(lhs, rhs) => {
                collect_calls_expr(lhs, out);
                collect_calls_expr(rhs, out);
            }
            Equation::SolvableBlock {
                equations,
                residuals,
                ..
            } => {
                for e in equations {
                    if let Equation::Simple(l, r) = e {
                        collect_calls_expr(l, out);
                        collect_calls_expr(r, out);
                    }
                }
                for r in residuals {
                    collect_calls_expr(r, out);
                }
            }
            _ => {}
        }
    }
    for eq in alg_equations.iter().chain(diff_equations) {
        collect_calls_eq(eq, &mut names);
    }
    fn collect_calls_alg(stmt: &AlgorithmStatement, out: &mut HashSet<String>) {
        match stmt {
            AlgorithmStatement::Assignment(_, rhs) => collect_calls_expr(rhs, out),
            AlgorithmStatement::MultiAssign(_, rhs) => collect_calls_expr(rhs, out),
            AlgorithmStatement::CallStmt(expr) => collect_calls_expr(expr, out),
            AlgorithmStatement::NoOp => {}
            AlgorithmStatement::Break => {}
            AlgorithmStatement::Return(v) => {
                if let Some(expr) = v {
                    collect_calls_expr(expr, out);
                }
            }
            AlgorithmStatement::If(cond, t, eifs, els) => {
                collect_calls_expr(cond, out);
                for s in t {
                    collect_calls_alg(s, out);
                }
                for (c, b) in eifs {
                    collect_calls_expr(c, out);
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
                if let Some(b) = els {
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::For(_, range, body) => {
                collect_calls_expr(range, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
            }
            AlgorithmStatement::When(cond, body, elses) => {
                collect_calls_expr(cond, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
                for (c, b) in elses {
                    collect_calls_expr(c, out);
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::Reinit(_, e) => collect_calls_expr(e, out),
            AlgorithmStatement::Assert(cond, msg) => {
                collect_calls_expr(cond, out);
                collect_calls_expr(msg, out);
            }
            AlgorithmStatement::Terminate(msg) => collect_calls_expr(msg, out),
            AlgorithmStatement::While(cond, body) => {
                collect_calls_expr(cond, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
            }
        }
    }
    for stmt in algorithms {
        collect_calls_alg(stmt, &mut names);
    }
    names
}

/// EXT-2: Collect (modelica_name, c_name, library_hint) for functions that have external_info and are called from eqs/alg.
/// EXT-4: library_hint is from annotation(Library="...").
pub(super) fn collect_external_calls(
    loader: &mut ModelLoader,
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
) -> Vec<(String, String, Option<String>)> {
    let names = collect_external_raw_call_sites(alg_equations, diff_equations, algorithms);
    let mut out = Vec::new();
    for call_site in names {
        // Avoid mis-classifying builtin operators as external calls.
        // In particular, the unqualified TestLib.* fallback below would otherwise map
        // `sample(...)` / clock-derived operators to TestLib stubs (if present) and force
        // the JIT down the generic Import path, which can panic on missing symbols.
        let lower = call_site.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "sample"
                | "interval"
                | "subsample"
                | "supersample"
                | "shiftsample"
                | "backsample"
                | "hold"
                | "previous"
                | "ones"
                | "zeros"
                | "fill"
                | "pre"
                | "edge"
                | "change"
                | "assert"
                | "terminate"
                | "size"
        ) {
            continue;
        }
        // Do not skip on `is_builtin_function` here: that helper treats any
        // package-qualified name with an uppercase first segment (e.g. TestLib.*)
        // as a "builtin", which would drop real external declarations from EXT
        // collection and let JIT fall through to namespace passthrough (wrong).
        let mut resolved: Option<Arc<Model>> = None;
        if let Ok(m) = loader.load_model_silent(&call_site, true) {
            if m.is_function && m.external_info.is_some() {
                resolved = Some(m);
            }
        }
        if resolved.is_none() && !call_site.contains('.') {
            let q = format!("TestLib.{call_site}");
            if let Ok(m) = loader.load_model_silent(&q, true) {
                if m.is_function && m.external_info.is_some() {
                    resolved = Some(m);
                }
            }
        }
        let Some(model) = resolved else {
            continue;
        };
        if let Some(ref ext) = model.external_info {
            let default_c = call_site
                .rsplit_once('.')
                .map(|(_, t)| t)
                .unwrap_or(call_site.as_str());
            let c_name = ext.c_name.as_deref().unwrap_or(default_c).to_string();
            let lib_hint = parse_annotation_library(model.annotation.as_ref());
            out.push((call_site, c_name, lib_hint));
        }
    }
    out
}

/// Opaque handle to a compiled model (L3-T06). Wraps structural cache identity
/// and simulation layout so that parameter sweeps can reuse native code without
/// recompilation.
pub struct CompiledModel {
    pub(crate) artifacts: Artifacts,
    pub(crate) model_name: String,
}

impl CompiledModel {
    /// Create a simulation-ready artifact set with a new parameter vector.
    /// The compiled native code is reused; only the parameter array is swapped.
    pub fn with_params(&self, new_params: Vec<f64>) -> Artifacts {
        let mut a = self.artifacts.clone_layout_with_params(new_params);
        a.param_only_update = true;
        a
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    pub fn param_vars(&self) -> &[String] {
        &self.artifacts.param_vars
    }

    pub fn params(&self) -> &[f64] {
        &self.artifacts.params
    }

    pub fn state_vars(&self) -> &[String] {
        &self.artifacts.state_vars
    }
}

/// Result of compilation: either simulation artifacts or a single function result (F3-1).
pub enum CompileOutput {
    Simulation(Artifacts),
    FunctionRun(f64),
    /// Flat Tier S snapshot written; compilation stopped before analysis/JIT.
    FlatSnapshotDone,
    /// Stopped after load/parse; no flatten.
    ValidationParseOk,
    /// Stopped after flatten; no variable analysis or JIT.
    ValidationFlattenOk {
        total_equations: usize,
        total_declarations: usize,
    },
    /// Stopped after analysis; no external stub resolution or JIT.
    ValidationAnalyzed(ValidationAnalyzedSummary),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockPartitionTrigger {
    Always,
    Sample { start: f64, interval: f64 },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClockPartitionScheduleEntry {
    pub id: String,
    pub trigger: ClockPartitionTrigger,
    pub var_names: Vec<String>,
    pub algorithm_indices: Vec<usize>,
    pub alg_equation_indices: Vec<usize>,
    pub diff_equation_indices: Vec<usize>,
}

pub struct Artifacts {
    pub calc_derivs: CalcDerivsFunc,
    pub states: Vec<f64>,
    pub discrete_vals: Vec<f64>,
    pub params: Vec<f64>,
    pub state_vars: Vec<String>,
    pub param_vars: Vec<String>,
    pub discrete_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub output_start_vals: Vec<f64>,
    pub state_var_index: HashMap<String, usize>,
    /// SYNC-2: Clock partitions for event/solver (e.g. clocked state handling).
    #[allow(dead_code)]
    pub clock_partitions: Vec<BackendClockPartition>,
    pub clock_partition_schedule: Vec<ClockPartitionScheduleEntry>,
    pub when_count: usize,
    pub crossings_count: usize,
    pub t_end: f64,
    pub dt: f64,
    pub numeric_ode_jacobian: bool,
    pub symbolic_ode_jacobian: Option<Vec<Vec<Expression>>>,
    /// Tearing variable names per SolvableBlock (for Newton failure diagnostics).
    pub newton_tearing_var_names: Vec<String>,
    pub atol: f64,
    pub rtol: f64,
    /// Backend DAE differential index (for IDA / warnings).
    pub differential_index: u32,
    /// IDA `IDASetId` vector; same length as `states`, from `SimulationDae` layout.
    pub ida_component_id: Vec<f64>,
    pub solver: String,
    pub output_interval: f64,
    pub result_file: Option<String>,
    /// FUNC-2: Keep user function stub JITs alive so calc_derivs call targets remain valid.
    #[allow(dead_code)]
    pub user_stub_jits: Vec<Jit>,
    /// Keeps disk-backed `calc_derivs` memory valid when returning from sim-bundle fast path.
    #[allow(dead_code)]
    pub(crate) calc_derivs_codegen_keepalive: Option<Box<crate::jit::codegen_cache::CachedFunction>>,
    /// Keeps the owning `Jit` (and its `JITModule`) alive so the `calc_derivs` pointer
    /// into the module's executable memory remains valid.  Without this the `Jit` is
    /// dropped at the end of `compile_model::compile()`, freeing the machine code that
    /// `calc_derivs` points to and causing ACCESS_VIOLATION on Windows.
    #[allow(dead_code)]
    pub(crate) jit_module_keepalive: Option<Box<crate::jit::Jit>>,
    /// Set by `reuse_compiled_with_new_params` when only the parameter vector was replaced.
    pub param_only_update: bool,
    /// Leyden dual-compile: generic fallback function for deopt.
    pub dual_compile_generic: Option<CalcDerivsFunc>,
    /// Keep dual-compile JIT modules alive so function pointers remain valid.
    #[allow(dead_code)]
    pub(crate) dual_compile_keepalive: Option<Box<crate::jit::deopt::DualCompileResult>>,
}

impl Artifacts {
    /// Create a new Artifacts with a different parameter vector, reusing the compiled function.
    ///
    /// Initial states / discrete / output starts are copied from `self` so Fast Restart
    /// and parameter sweeps start from the same ICs. Keepalives stay on the parent
    /// `CompiledModel` — the returned Artifacts must not outlive that parent.
    pub(crate) fn clone_layout_with_params(&self, new_params: Vec<f64>) -> Artifacts {
        Artifacts {
            calc_derivs: self.calc_derivs,
            states: self.states.clone(),
            discrete_vals: self.discrete_vals.clone(),
            params: new_params,
            state_vars: self.state_vars.clone(),
            param_vars: self.param_vars.clone(),
            discrete_vars: self.discrete_vars.clone(),
            output_vars: self.output_vars.clone(),
            output_start_vals: self.output_start_vals.clone(),
            state_var_index: self.state_var_index.clone(),
            clock_partitions: Vec::new(),
            clock_partition_schedule: self.clock_partition_schedule.clone(),
            when_count: self.when_count,
            crossings_count: self.crossings_count,
            t_end: self.t_end,
            dt: self.dt,
            numeric_ode_jacobian: self.numeric_ode_jacobian,
            symbolic_ode_jacobian: self.symbolic_ode_jacobian.clone(),
            newton_tearing_var_names: self.newton_tearing_var_names.clone(),
            atol: self.atol,
            rtol: self.rtol,
            differential_index: self.differential_index,
            ida_component_id: self.ida_component_id.clone(),
            solver: self.solver.clone(),
            output_interval: self.output_interval,
            result_file: self.result_file.clone(),
            user_stub_jits: Vec::new(),
            calc_derivs_codegen_keepalive: None,
            jit_module_keepalive: None,
            param_only_update: true,
            dual_compile_generic: self.dual_compile_generic,
            dual_compile_keepalive: None,
        }
    }
}

include!("compiler_impl.rs");
