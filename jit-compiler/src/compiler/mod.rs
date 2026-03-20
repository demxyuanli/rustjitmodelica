mod c_codegen;
mod equation_convert;
mod initial_conditions;
pub(crate) mod inline;
mod jacobian;
mod pipeline;
mod compile_model;

use std::collections::{HashMap, HashSet};

use crate::ast::{AlgorithmStatement, Equation, Expression};
use crate::backend_dae::ClockPartition as BackendClockPartition;
use crate::diag::WarningInfo;
use crate::equation_graph;
use crate::expr_eval;
use crate::flatten::Flattener;
use crate::jit::{CalcDerivsFunc, Jit};
use crate::loader::ModelLoader;
use pipeline::{flatten_and_inline, stage_trace_enabled};

#[derive(Clone, Debug)]
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
            | Expression::ShiftSample(c, n) => {
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
            | Expression::ShiftSample(c, n) => {
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
    for name in names {
        if inline::is_builtin_function(&name) {
            continue;
        }
        if let Ok(model) = loader.load_model_silent(&name, true) {
            if model.is_function {
                if let Some(ref ext) = model.external_info {
                    let c_name = ext.c_name.as_deref().unwrap_or(&name).to_string();
                    let lib_hint = parse_annotation_library(model.annotation.as_ref());
                    out.push((name, c_name, lib_hint));
                }
            }
        }
    }
    out
}

/// Result of compilation: either simulation artifacts or a single function result (F3-1).
pub enum CompileOutput {
    Simulation(Artifacts),
    FunctionRun(f64),
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
    pub state_var_index: HashMap<String, usize>,
    /// SYNC-2: Clock partitions for event/solver (e.g. clocked state handling).
    #[allow(dead_code)]
    pub clock_partitions: Vec<BackendClockPartition>,
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
        }
    }

    pub fn take_warnings(&mut self) -> Vec<WarningInfo> {
        std::mem::take(&mut self.warnings)
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
        let stage_trace = stage_trace_enabled();
        let flat_model = flatten_and_inline(
            &mut root_model,
            model_name,
            &mut self.loader,
            self.options.quiet,
            stage_trace,
        )?
        .flat_model;
        Ok(equation_graph::build_equation_graph(&flat_model))
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
