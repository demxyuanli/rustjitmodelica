mod c_codegen;
mod equation_convert;
mod initial_conditions;
pub(crate) mod inline;
mod jacobian;
mod pipeline;
mod compile_model;
mod solvable_scale_warn;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::ast::{AlgorithmStatement, Equation, Expression, Model};
use crate::backend_dae::ClockPartition as BackendClockPartition;
use crate::diag::WarningInfo;
use crate::equation_graph;
use crate::equation_graph::EquationGraphMode;
use crate::expr_eval;
use crate::flatten::{ArraySizePolicy, Flattener};
use crate::jit::{CalcDerivsFunc, Jit};
use crate::loader::ModelLoader;
use pipeline::{flatten_and_inline, stage_trace_enabled};
pub use pipeline::geometric_default_for_name;

/// Stops compilation after the named phase when not `Full`. Used for tiered IDE validation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompileStopPhase {
    #[default]
    Full,
    Parse,
    Flatten,
    Analyze,
}

/// Summary returned when compilation stops after `Analyze` (no JIT).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationAnalyzedSummary {
    pub state_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub total_equations: usize,
    pub total_declarations: usize,
    pub alg_equation_count: usize,
    pub diff_equation_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompilerOptions {
    pub backend_dae_info: bool,
    pub index_reduction_method: String,
    pub tearing_method: String,
    pub generate_dynamic_jacobian: String,
    pub t_end: f64,
    pub dt: f64,
    pub atol: f64,
    pub rtol: f64,
    /// When running a function as entry: optional argument values (by input order). Missing => 0.0.
    pub function_args: Option<Vec<f64>>,
    /// RT1-3: Solver choice: "rk4" (fixed step) or "rk45" (adaptive when no when/zero-crossing).
    pub solver: String,
    /// RT1-5: Output/print interval for simulation results.
    pub output_interval: f64,
    /// RT1-5: Optional CSV result file path; when set, simulation writes time series to this file.
    pub result_file: Option<String>,
    /// DBG-3: Warnings level: "all" | "none" | "error" (none = suppress, error = treat as error).
    pub warnings_level: String,
    /// CG1-1: When set, emit C source (model.c, model.h) to this directory.
    pub emit_c_dir: Option<String>,
    /// EXT-1: Paths to shared libraries for external function symbols (e.g. .dll, .so).
    pub external_libs: Vec<String>,
    /// When true, suppress progress messages so only JSON (e.g. validate) is on stdout.
    pub quiet: bool,
    /// CLI `--validate`: relax function-as-root entry when scalar `expr_eval` cannot run the body.
    pub validate_only: bool,
    /// Tier S: write canonical flat JSON after flatten (before inline) to this path.
    pub emit_flat_snapshot: Option<String>,
    /// Stop compilation after writing `emit_flat_snapshot` (no JIT/simulation).
    pub flat_snapshot_only: bool,
    /// Use legacy string `constrainedby` check instead of extends-closure (see `instantiate` module).
    pub coarse_constrainedby_only: bool,
    /// `legacy` (default): unevaluated array dims fall back to scalar with optional warning. `strict`: error unless overridden.
    pub array_size_policy: String,
    /// Optional JSON file: `{"array_sizes":{"<flat_base_name>": N, ...}}` merged during flatten.
    pub array_sizes_json: Option<String>,
    /// When set and `RUSTMODLICA_JIT_POLICY_JSON` is unset, JIT loads this policy overlay path before the first JIT compile in the process.
    pub jit_policy_json: Option<String>,
    /// Validation-only speed/accuracy trade-off: "full" | "quick" | "superfast".
    /// Default is "full". This is intentionally a string to keep CLI/API wiring simple.
    #[serde(default)]
    pub validation_mode: String,
    /// Tiered validation: stop after parse, flatten, or analysis instead of running JIT.
    pub compile_stop: CompileStopPhase,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CompilePerfReport {
    pub model_name: String,
    pub load_model_ms: u64,
    pub flatten_inline_ms: u64,
    pub analyze_ms: u64,
    pub backend_dae_ms: u64,
    pub external_resolve_ms: u64,
    pub jit_ms: u64,
    pub state_count: usize,
    pub discrete_count: usize,
    pub param_count: usize,
    pub alg_eq_count: usize,
    pub diff_eq_count: usize,
    pub blt_degrade_guard_triggered: bool,
    pub blt_degrade_guard_limit: Option<usize>,
    pub blt_degrade_guard_equation_count: Option<usize>,
    pub symbolic_index_signal_count: usize,
    pub implicit_derivative_constraint_count: usize,
    pub aot_cache_status: String,
    pub jit_compile_ok: bool,
    pub jit_error: Option<String>,
    pub fallback_total: u64,
    pub fallback_jit_builtin: u64,
    pub fallback_jit_variable: u64,
    pub fallback_jit_derivative: u64,
    pub fallback_jit_equation_skip: u64,
    pub fallback_jit_multi_assign: u64,
    pub fallback_newton_init_accept: u64,
    pub fallback_newton_event_accept: u64,
    pub fallback_clock_degrade: u64,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        CompilerOptions {
            backend_dae_info: false,
            index_reduction_method: "none".to_string(),
            tearing_method: "first".to_string(),
            generate_dynamic_jacobian: "none".to_string(),
            t_end: 10.0,
            dt: 0.01,
            atol: 1e-6,
            rtol: 1e-3,
            function_args: None,
            solver: "rk45".to_string(),
            output_interval: 0.05,
            result_file: None,
            warnings_level: "all".to_string(),
            emit_c_dir: None,
            external_libs: Vec::new(),
            quiet: false,
            validate_only: false,
            emit_flat_snapshot: None,
            flat_snapshot_only: false,
            coarse_constrainedby_only: false,
            array_size_policy: "legacy".to_string(),
            array_sizes_json: None,
            jit_policy_json: None,
            validation_mode: "full".to_string(),
            compile_stop: CompileStopPhase::Full,
        }
    }
}

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

/// EXT-2: Collect (modelica_name, c_name, library_hint) for functions that have external_info and are called from eqs/alg.
/// EXT-4: library_hint is from annotation(Library="...").
pub(super) fn collect_external_calls(
    loader: &mut ModelLoader,
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
) -> Vec<(String, String, Option<String>)> {
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
    let mut out = Vec::new();
    for call_site in names {
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

#[derive(Clone, Debug)]
pub enum ClockPartitionTrigger {
    Always,
    Sample { start: f64, interval: f64 },
}

#[derive(Clone, Debug)]
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
}

impl Compiler {
    fn function_has_output_in_hierarchy(
        &mut self,
        model: &crate::ast::Model,
        current_qualified: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if model.declarations.iter().any(|d| d.is_output) {
            return Ok(true);
        }
        for clause in &model.extends {
            let base_name =
                Flattener::resolve_import_prefix(model, &clause.model_name, current_qualified);
            let base_name = Flattener::qualify_in_scope(current_qualified, &base_name);
            let base_model = self
                .loader
                .load_model(&base_name)
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            if self.function_has_output_in_hierarchy(base_model.as_ref(), &base_name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn new() -> Self {
        Compiler {
            loader: ModelLoader::new(),
            options: CompilerOptions::default(),
            warnings: Vec::new(),
            external_libraries: ExternalLibs::default(),
            external_symbol_ptrs: HashMap::new(),
            interner: crate::string_intern::StringInterner::new(),
            last_compile_perf: None,
        }
    }

    pub fn take_warnings(&mut self) -> Vec<WarningInfo> {
        std::mem::take(&mut self.warnings)
    }

    pub fn take_compile_perf_report(&mut self) -> Option<CompilePerfReport> {
        self.last_compile_perf.take()
    }

    /// Compile a model from source code in memory (for IDE / single-file). Caller may add_path
    /// for StandardLib/TestLib before this if the model has dependencies.
    pub fn compile_from_source(
        &mut self,
        model_name: &str,
        code: &str,
    ) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
        self.loader
            .load_model_from_source(model_name, code)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        self.compile(model_name)
    }

    /// Build equation/variable dependency graph from source (for analysis/debug). Does not run full compile.
    pub fn get_equation_graph_from_source(
        &mut self,
        model_name: &str,
        code: &str,
        mode: EquationGraphMode,
    ) -> Result<equation_graph::EquationGraph, Box<dyn std::error::Error + Send + Sync>> {
        self.loader
            .load_model_from_source(model_name, code)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        let mut root_model = self
            .loader
            .load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        if root_model.as_ref().is_function {
            return Err("Equation graph is not supported for functions.".into());
        }
        if matches!(mode, EquationGraphMode::Structural) {
            return Ok(equation_graph::build_structural_graph(root_model.as_ref()));
        }
        let stage_trace = stage_trace_enabled();
        let snap_path = self
            .options
            .emit_flat_snapshot
            .as_deref()
            .map(std::path::Path::new);
        let array_sizes_path = self
            .options
            .array_sizes_json
            .as_deref()
            .map(std::path::Path::new);
        let array_size_policy = ArraySizePolicy::parse(self.options.array_size_policy.as_str());
        let flat_model = flatten_and_inline(
            &mut root_model,
            model_name,
            &mut self.loader,
            self.options.quiet,
            stage_trace,
            snap_path,
            self.options.coarse_constrainedby_only,
            crate::flatten::ValidationMode::parse(self.options.validation_mode.as_str()),
            array_size_policy,
            array_sizes_path,
            self.options.warnings_level.as_str(),
        )?
        .flat_model;
        Ok(equation_graph::build_equation_graph(&flat_model, mode))
    }

    /// Run a function once with given inputs (or 0.0 per input if not provided) and return the output (F3-1).
    fn run_function_once(
        &mut self,
        model_name: &str,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        let mut root_model = self
            .loader
            .load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        if !root_model.extends.is_empty() {
            let mut flattener = Flattener::new();
            flattener.loader.library_paths = self.loader.library_paths.clone();
            if let Some(p) = self.loader.get_path_for_model(model_name) {
                flattener.loader.register_path(model_name, p);
            }
            flattener
                .flatten_inheritance(&mut root_model, model_name)
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        }
        if let Some((input_names, outputs)) = inline::get_function_body(root_model.as_ref()) {
            let body = outputs
                .first()
                .ok_or("Function has no output expression.")?
                .1
                .clone();
            let args = self.options.function_args.as_deref().unwrap_or(&[]);
            let mut vars = HashMap::new();
            for (i, name) in input_names.iter().enumerate() {
                let val = args.get(i).copied().unwrap_or(0.0);
                vars.insert(name.clone(), val);
            }
            return expr_eval::eval_expr(&body, &vars).map_err(|e| e.into());
        }
        if self.options.quiet {
            return Ok(0.0);
        }
        if self.function_has_output_in_hierarchy(root_model.as_ref(), model_name)? {
            return Ok(0.0);
        }
        if root_model.external_info.is_some() {
            return Ok(0.0);
        }
        Err("Function must have at least one output and assignments in algorithm.".into())
    }

    pub fn compile(
        &mut self,
        model_name: &str,
    ) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
        compile_model::compile(self, model_name)
    }

    /// DBG-4: suffix for error messages (file path or model name).
    fn source_loc_suffix(&self, model_name: &str) -> String {
        self.loader
            .get_path_for_model(model_name)
            .map(|p| format!("\n  --> {}", p.display()))
            .unwrap_or_else(|| format!(" (model: {})", model_name))
    }
}
