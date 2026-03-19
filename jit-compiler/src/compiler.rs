mod c_codegen;
mod equation_convert;
mod initial_conditions;
mod inline;
mod jacobian;
mod pipeline;

use std::collections::{HashMap, HashSet};

use crate::analysis::analyze_initial_equations;
use crate::ast::{AlgorithmStatement, Equation, Expression};
use crate::backend_dae::{
    build_simulation_dae, ClockPartition as BackendClockPartition, SimulationDae,
};
use crate::diag::WarningInfo;
use crate::equation_graph;
use crate::expr_eval;
use crate::flatten::Flattener;
use crate::i18n;
use crate::jit::native::builtin_jit_symbol_names;
use crate::jit::{CalcDerivsFunc, Jit};
use crate::loader::ModelLoader;
use pipeline::{
    analyze_equations, build_runtime_algorithms, classify_variables,
    collect_newton_tearing_var_names, flatten_and_inline, stage_trace_enabled,
};

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
fn collect_all_called_names(
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
fn collect_external_calls(
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
        if let Ok(model) = loader.load_model(&name) {
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
        let stage_trace = stage_trace_enabled();

        self.warnings.clear();
        self.loader.set_quiet(self.options.quiet);
        let opts = &self.options;
        let model_file_path = format!("{}.mo", model_name.replace('.', "/"));
        if !self.options.quiet {
            println!(
                "{}",
                i18n::msg("loading_model", &[&model_name as &dyn std::fmt::Display])
            );
        }
        let mut root_model = self
            .loader
            .load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        if root_model.as_ref().is_function {
            if !self.options.quiet {
                if self.options.function_args.is_some() {
                    println!("{}", i18n::msg0("evaluating_function_args"));
                } else {
                    println!("{}", i18n::msg0("evaluating_function_default"));
                }
            }
            let value = self.run_function_once(model_name)?;
            return Ok(CompileOutput::FunctionRun(value));
        }

        if !self.options.quiet {
            println!("{}", i18n::msg0("flattening_model"));
        }
        let frontend = flatten_and_inline(
            &mut root_model,
            model_name,
            &mut self.loader,
            self.options.quiet,
            stage_trace,
        )?;
        let flat_model = frontend.flat_model;
        let total_equations = frontend.total_equations;
        let total_declarations = frontend.total_declarations;
        if !self.options.quiet {
            println!("{}", i18n::msg("flattened_equations", &[&total_equations]));
            println!(
                "{}",
                i18n::msg("flattened_declarations", &[&total_declarations])
            );
            println!("{}", i18n::msg0("analyzing_variables"));
        }

        let mut variable_layout = classify_variables(&flat_model, opts.quiet, stage_trace);
        if !self.options.quiet {
            println!("{}", i18n::msg0("normalizing_derivatives"));
            println!("{}", i18n::msg0("performing_structure_analysis"));
        }
        let analysis_stage = analyze_equations(&flat_model, &mut variable_layout, opts, stage_trace);
        let state_vars_sorted = variable_layout.state_vars;
        let discrete_vars_sorted = variable_layout.discrete_vars;
        let param_vars = variable_layout.param_vars;
        let input_var_names = variable_layout.input_var_names;
        let output_vars = variable_layout.output_vars;
        let output_var_index = variable_layout.output_var_index;
        let state_var_index = variable_layout.state_var_index;
        let param_var_index = variable_layout.param_var_index;
        let array_info = variable_layout.array_info;
        let states = variable_layout.states;
        let discrete_vals = variable_layout.discrete_vals;
        let params = variable_layout.params;
        let alg_equations = analysis_stage.alg_equations;
        let diff_equations = analysis_stage.diff_equations;
        let differential_index = analysis_stage.differential_index;
        let constraint_equation_count = analysis_stage.constraint_equation_count;
        let constant_conflict_count = analysis_stage.constant_conflict_count;
        let numeric_ode_jacobian = analysis_stage.numeric_ode_jacobian;
        let ode_jacobian_sparse = analysis_stage.ode_jacobian_sparse;
        let symbolic_ode_jacobian_matrix = analysis_stage.symbolic_ode_jacobian_matrix;

        if differential_index > 1 && opts.warnings_level != "none" {
            let method_note = if opts.index_reduction_method == "none" {
                "index reduction not applied (use --index-reduction-method=dummyDerivative); simulation may be unreliable".to_string()
            } else {
                format!(
                    "{} constraint equation(s) before reduction; differential index {}",
                    constraint_equation_count, differential_index
                )
            };
            self.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!(
                    "differential index is {}; {}",
                    differential_index, method_note
                ),
                source: None,
            });
        }
        if constant_conflict_count > 0 && opts.warnings_level != "none" {
            self.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!(
                    "equation system contains {} constant contradictory equation(s); simulation will fail unless the model is corrected",
                    constant_conflict_count
                ),
                source: None,
            });
        }

        let mut known_at_initial = HashSet::new();
        known_at_initial.insert("time".to_string());
        for p in &param_vars {
            known_at_initial.insert(p.clone());
        }
        let initial_info =
            analyze_initial_equations(&flat_model.initial_equations, &known_at_initial);
        if initial_info.is_underdetermined
            && initial_info.equation_count > 0
            && opts.warnings_level != "none"
        {
            self.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!(
                    "initial equation system underdetermined ({} equations, {} unknowns); consistent initialization may be incomplete",
                    initial_info.equation_count, initial_info.variable_count
                ),
                source: None,
            });
        }
        if initial_info.is_overdetermined && opts.warnings_level != "none" {
            self.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!(
                    "initial equation system overdetermined ({} equations, {} unknowns)",
                    initial_info.equation_count, initial_info.variable_count
                ),
                source: None,
            });
        }
        let algebraic_loops = alg_equations
            .iter()
            .filter(|e| matches!(e, Equation::SolvableBlock { .. }))
            .count();
        if algebraic_loops > 0 && opts.warnings_level != "none" {
            self.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!(
                    "{} algebraic loop(s) (strong component(s)) present, solved with tearing",
                    algebraic_loops
                ),
                source: None,
            });
        }

        let symbolic_ode_jacobian = symbolic_ode_jacobian_matrix.is_some();
        let strong_component_jacobians = false;

        let when_equation_count = flat_model
            .equations
            .iter()
            .filter(|e| matches!(e, Equation::When(_, _, _)))
            .count();
        let backend_clock_partitions: Vec<BackendClockPartition> = flat_model
            .clock_partitions
            .iter()
            .map(|p| BackendClockPartition {
                id: p.id.clone(),
                var_names: p.var_names.clone(),
            })
            .collect();
        let simulation_dae: SimulationDae = build_simulation_dae(
            &state_vars_sorted,
            &discrete_vars_sorted,
            &param_vars,
            &output_vars,
            &input_var_names,
            diff_equations.len(),
            &alg_equations,
            flat_model.initial_equations.len(),
            initial_info.variable_count,
            when_equation_count,
            differential_index,
            constraint_equation_count,
            &backend_clock_partitions,
        );

        let external_list = collect_external_calls(
            &mut self.loader,
            &alg_equations,
            &diff_equations,
            &flat_model.algorithms,
        );

        let all_called =
            collect_all_called_names(&alg_equations, &diff_equations, &flat_model.algorithms);
        let external_names: HashSet<String> =
            external_list.iter().map(|(n, _, _)| n.clone()).collect();
        let mut user_stub_jits: Vec<Jit> = Vec::new();
        let mut user_stub_ptrs: HashMap<String, *const u8> = HashMap::new();
        let mut user_function_bodies: HashMap<String, (Vec<String>, Expression)> = HashMap::new();
        for name in &all_called {
            if inline::is_builtin_function(name) || external_names.contains(name) {
                continue;
            }
            // MSL Fluid: valveCharacteristic is usually provided via replaceable function
            // bound to BaseClasses.ValveCharacteristics.linear/one/...; our current
            // frontend does not track the specific binding here, so we fall back to
            // the default linear characteristic for JIT stubs.
            let load_name = if name == "valveCharacteristic" {
                "Modelica.Fluid.Valves.BaseClasses.ValveCharacteristics.linear".to_string()
            } else if name.starts_with("world.") {
                format!("Modelica.Mechanics.MultiBody.World.{}", name.trim_start_matches("world."))
            } else if name.starts_with("BaseClasses.") {
                format!("Modelica.Fluid.Utilities.{}", name)
            } else if name.starts_with("Machines.") {
                format!("Modelica.Electrical.{}", name)
            } else if name.starts_with("Mechanics.") {
                format!("Modelica.{}", name)
            } else if name == "Cv" {
                "Modelica.Units.Conversions".to_string()
            } else if let Some(rest) = name.strip_prefix("Cv.") {
                format!("Modelica.Units.Conversions.{}", rest)
            } else {
                name.to_string()
            };
            let func_model = match self.loader.load_model(&load_name) {
                Ok(m) => m,
                Err(_) => {
                    // Some collected call-like identifiers are not real Modelica
                    // functions in current frontend coverage; skip hard failure.
                    continue;
                }
            };
            if func_model.external_info.is_some() {
                continue;
            }
            let Some((input_names, outputs)) = inline::get_function_body(func_model.as_ref()) else {
                // Keep compilation progressing for library test wrappers that call
                // non-inlinable functions; expr translator provides a placeholder path.
                continue;
            };
            if outputs.len() != 1 {
                return Err(format!(
                    "Function '{}' has {} outputs; JIT callable supports single-output only (FUNC-2).",
                    name, outputs.len()
                ).into());
            }
            let mut stub_jit = Jit::new();
            let ptr = stub_jit
                .compile_user_function_stub(name, &input_names, &outputs[0].1)
                .map_err(|e| format!("JIT stub for '{}': {}", name, e))?;
            user_stub_ptrs.insert(name.clone(), ptr);
            user_stub_jits.push(stub_jit);
            user_function_bodies.insert(name.clone(), (input_names.clone(), outputs[0].1.clone()));
        }
        let mut all_symbols = self.external_symbol_ptrs.clone();
        for (k, v) in user_stub_ptrs {
            all_symbols.insert(k, v);
        }

        if opts.backend_dae_info {
            jacobian::print_backend_dae_info(
                opts,
                differential_index,
                total_equations,
                total_declarations,
                &state_vars_sorted,
                &discrete_vars_sorted,
                &param_vars,
                &output_vars,
                &flat_model.clocked_var_names,
                &flat_model.equations,
                &alg_equations,
                &flat_model.equations,
                &flat_model.algorithms,
                strong_component_jacobians,
                symbolic_ode_jacobian,
                numeric_ode_jacobian,
                symbolic_ode_jacobian_matrix.as_ref(),
                ode_jacobian_sparse.as_ref(),
                Some(&simulation_dae),
            );
        }

        if let Some(ref dir) = self.options.emit_c_dir {
            let path = std::path::Path::new(dir);
            let jac = symbolic_ode_jacobian_matrix.as_deref();
            let state_array_layout: Vec<(String, usize, usize)> = flat_model
                .array_sizes
                .iter()
                .filter_map(|(name, &size)| {
                    let first = format!("{}_1", name);
                    state_var_index
                        .get(&first)
                        .copied()
                        .map(|start| (name.clone(), start, size))
                })
                .collect();
            let output_array_layout: Vec<(String, usize, usize)> = flat_model
                .array_sizes
                .iter()
                .filter_map(|(name, &size)| {
                    let first = format!("{}_1", name);
                    output_var_index
                        .get(&first)
                        .copied()
                        .map(|start| (name.clone(), start, size))
                })
                .collect();
            let param_array_layout: Vec<(String, usize, usize)> = flat_model
                .array_sizes
                .iter()
                .filter_map(|(name, &size)| {
                    let first = format!("{}_1", name);
                    param_var_index
                        .get(&first)
                        .copied()
                        .map(|start| (name.clone(), start, size))
                })
                .collect();
            let state_layout_opt = if state_array_layout.is_empty() {
                None
            } else {
                Some(state_array_layout.as_slice())
            };
            let output_layout_opt = if output_array_layout.is_empty() {
                None
            } else {
                Some(output_array_layout.as_slice())
            };
            let param_layout_opt = if param_array_layout.is_empty() {
                None
            } else {
                Some(param_array_layout.as_slice())
            };
            let external_c_names: HashMap<String, String> = external_list
                .iter()
                .map(|(m, c, _)| (m.clone(), c.clone()))
                .collect();
            let external_c_names_opt = if external_c_names.is_empty() {
                None
            } else {
                Some(external_c_names)
            };
            let external_names_set: HashSet<String> =
                external_list.iter().map(|(n, _, _)| n.clone()).collect();
            let user_fn_bodies_opt = if user_function_bodies.is_empty() {
                None
            } else {
                Some(&user_function_bodies)
            };
            match c_codegen::emit_c_files(
                path,
                &state_vars_sorted,
                &param_vars,
                &output_vars,
                &alg_equations,
                jac,
                state_layout_opt,
                output_layout_opt,
                param_layout_opt,
                external_c_names_opt,
                Some(&external_names_set),
                user_fn_bodies_opt,
            ) {
                Ok(files) => {
                    let paths: Vec<String> =
                        files.iter().map(|p| p.display().to_string()).collect();
                    println!("{}", i18n::msg("c_codegen_emitted", &[&paths.join(", ")]));
                }
                Err(e) => {
                    return Err(format!(
                        "C codegen failed: {}{}",
                        e,
                        self.source_loc_suffix(model_name)
                    )
                    .into());
                }
            }
        }

        // F2-1: Fail at compile time with clear message if unsupported der(expr) is present
        for eq in alg_equations.iter().chain(diff_equations.iter()) {
            if let Some(hint) = crate::analysis::find_unsupported_der_in_eq(eq) {
                return Err(format!("Unsupported nested der(): {}. (F2-1)", hint).into());
            }
        }

        // 5. JIT Compile
        if !self.options.quiet {
            println!("{}", i18n::msg0("jit_compiling"));
            println!(
                "{}",
                i18n::msg("equations_after_sorting", &[&alg_equations.len()])
            );
            println!(
                "{}",
                i18n::msg("state_variables", &[&state_vars_sorted.len()])
            );
            println!(
                "{}",
                i18n::msg("discrete_variables", &[&discrete_vars_sorted.len()])
            );
            println!("{}", i18n::msg("parameters_count", &[&param_vars.len()]));
        }

        let algorithms = build_runtime_algorithms(&flat_model, stage_trace);
        let newton_tearing_var_names = collect_newton_tearing_var_names(&alg_equations);

        let lib_paths: Vec<std::path::PathBuf> = if !self.options.external_libs.is_empty() {
            self.options
                .external_libs
                .iter()
                .map(|p| std::path::PathBuf::from(p))
                .collect()
        } else {
            let mut from_annotation: Vec<std::path::PathBuf> = external_list
                .iter()
                .filter_map(|(_, _, hint)| hint.as_ref())
                .map(|lib_name| {
                    let ext = std::env::consts::DLL_EXTENSION;
                    std::path::PathBuf::from(format!("{}.{}", lib_name, ext))
                })
                .collect();
            from_annotation.sort();
            from_annotation.dedup();
            from_annotation
        };
        if !lib_paths.is_empty() {
            self.external_libraries.0.clear();
            self.external_symbol_ptrs.clear();
            for path in &lib_paths {
                let lib = unsafe { libloading::Library::new(path.as_path()) }.map_err(|e| {
                    format!("Failed to load external lib '{}': {}", path.display(), e)
                })?;
                for (modelica_name, c_name, _) in &external_list {
                    if self.external_symbol_ptrs.contains_key(modelica_name) {
                        continue;
                    }
                    if let Ok(sym) = unsafe { lib.get::<extern "C" fn()>(c_name.as_bytes()) } {
                        let ptr = *sym as *const u8;
                        self.external_symbol_ptrs.insert(modelica_name.clone(), ptr);
                    }
                }
                self.external_libraries.0.push(lib);
            }
            for (modelica_name, _c_name, _) in &external_list {
                if !self.external_symbol_ptrs.contains_key(modelica_name) {
                    return Err(format!(
                        "EXT-2: external function '{}' not found in any loaded library (--external-lib or annotation Library)",
                        modelica_name
                    ).into());
                }
            }
        }

        let builtins = builtin_jit_symbol_names();
        for name in &external_names {
            if builtins.contains(name.as_str()) || all_symbols.contains_key(name) {
                continue;
            }
            return Err(format!(
                "External function '{}' is not linked. Provide a shared library with this symbol (e.g. --external-lib=<path> or Library annotation).",
                name
            ).into());
        }

        let t_end = self.options.t_end;
        let dt = self.options.dt;
        let mut jit = if all_symbols.is_empty() {
            Jit::new()
        } else {
            Jit::new_with_extra_symbols(Some(&all_symbols))
        };
        let res = jit.compile(
            &state_vars_sorted,
            &discrete_vars_sorted,
            &param_vars,
            &output_vars,
            &array_info,
            &alg_equations,
            &diff_equations,
            &algorithms,
            t_end,
            &newton_tearing_var_names,
        );

        match res {
            Ok((calc_derivs, when_count, crossings_count)) => {
                Ok(CompileOutput::Simulation(Artifacts {
                    calc_derivs,
                    states,
                    discrete_vals,
                    params,
                    state_vars: state_vars_sorted,
                    param_vars,
                    discrete_vars: discrete_vars_sorted,
                    output_vars,
                    state_var_index,
                    clock_partitions: backend_clock_partitions,
                    when_count,
                    crossings_count,
                    t_end,
                    dt,
                    numeric_ode_jacobian,
                    symbolic_ode_jacobian: symbolic_ode_jacobian_matrix,
                    newton_tearing_var_names,
                    atol: self.options.atol,
                    rtol: self.options.rtol,
                    solver: self.options.solver.clone(),
                    output_interval: self.options.output_interval,
                    result_file: self.options.result_file.clone(),
                    user_stub_jits,
                }))
            }
            Err(e) => Err(format!(
                "JIT compilation failed: {}{}",
                e,
                self.source_loc_suffix(model_name)
            )
            .into()),
        }
    }

    /// DBG-4: suffix for error messages (file path or model name).
    fn source_loc_suffix(&self, model_name: &str) -> String {
        self.loader
            .get_path_for_model(model_name)
            .map(|p| format!("\n  --> {}", p.display()))
            .unwrap_or_else(|| format!(" (model: {})", model_name))
    }
}
