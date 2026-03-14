mod c_codegen;
mod equation_convert;
mod initial_conditions;
mod inline;
mod jacobian;

use std::collections::{HashMap, HashSet};

use crate::ast::{Equation, Expression, AlgorithmStatement};
use crate::backend_dae::{build_simulation_dae, SimulationDae, ClockPartition as BackendClockPartition};
use crate::loader::ModelLoader;
use crate::flatten::{Flattener, eval_const_expr};
use crate::analysis::{sort_algebraic_equations, collect_states_from_eq, analyze_initial_equations, AnalysisOptions};
use crate::diag::WarningInfo;
use crate::jit::{Jit, CalcDerivsFunc, ArrayInfo, ArrayType};
use crate::jit::native::builtin_jit_symbol_names;
use crate::equation_graph;
use crate::expr_eval;
use crate::i18n;

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
    let rest = rest.strip_prefix('=').map(|r| r.trim_start()).unwrap_or(rest);
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
            Expression::Sample(inner) | Expression::Interval(inner) | Expression::Hold(inner) | Expression::Previous(inner) => collect_calls_expr(inner, out),
            Expression::SubSample(c, n) | Expression::SuperSample(c, n) | Expression::ShiftSample(c, n) => {
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
            Equation::SolvableBlock { equations, residuals, .. } => {
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
            Expression::Sample(inner) | Expression::Interval(inner) | Expression::Hold(inner) | Expression::Previous(inner) => collect_calls_expr(inner, out),
            Expression::SubSample(c, n) | Expression::SuperSample(c, n) | Expression::ShiftSample(c, n) => {
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
            Equation::SolvableBlock { equations, residuals, .. } => {
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
        let mut flattener = Flattener::new();
        for path in &self.loader.library_paths {
            flattener.loader.add_path(path.clone());
        }
        if let Some(p) = self.loader.get_path_for_model(model_name) {
            flattener.loader.register_path(model_name, p);
        }
        let mut flat_model = flattener.flatten(&mut root_model, model_name)?;
        inline::inline_function_calls(&mut flat_model, &mut self.loader);
        Ok(equation_graph::build_equation_graph(&flat_model))
    }

    /// Run a function once with given inputs (or 0.0 per input if not provided) and return the output (F3-1).
    fn run_function_once(&mut self, model_name: &str) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        let root_model = self.loader.load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        if let Some((input_names, outputs)) = inline::get_function_body(root_model.as_ref()) {
            let body = outputs.first().ok_or("Function has no output expression.")?.1.clone();
            let args = self.options.function_args.as_deref().unwrap_or(&[]);
            let mut vars = HashMap::new();
            for (i, name) in input_names.iter().enumerate() {
                let val = args.get(i).copied().unwrap_or(0.0);
                vars.insert(name.clone(), val);
            }
            return expr_eval::eval_expr(&body, &vars).map_err(|e| e.into());
        }
        if root_model.external_info.is_some() {
            return Ok(0.0);
        }
        Err("Function must have at least one output and assignments in algorithm.".into())
    }

    pub fn compile(&mut self, model_name: &str) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
        self.warnings.clear();
        let opts = &self.options;
        let model_file_path = format!("{}.mo", model_name.replace('.', "/"));
        println!("{}", i18n::msg("loading_model", &[&model_name as &dyn std::fmt::Display]));
        let mut root_model = self.loader.load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        if root_model.as_ref().is_function {
            if self.options.function_args.is_some() {
                println!("{}", i18n::msg0("evaluating_function_args"));
            } else {
                println!("{}", i18n::msg0("evaluating_function_default"));
            }
            let value = self.run_function_once(model_name)?;
            return Ok(CompileOutput::FunctionRun(value));
        }

        println!("{}", i18n::msg0("flattening_model"));
        let mut flattener = Flattener::new();
        for path in &self.loader.library_paths {
            flattener.loader.add_path(path.clone());
        }
        if let Some(p) = self.loader.get_path_for_model(model_name) {
            flattener.loader.register_path(model_name, p);
        }
        let mut flat_model = flattener.flatten(&mut root_model, model_name)?;

        inline::inline_function_calls(&mut flat_model, &mut self.loader);

        // Output detailed statistics
        let total_equations = flat_model.equations.len();
        let total_declarations = flat_model.declarations.len();
        println!("{}", i18n::msg("flattened_equations", &[&total_equations]));
        println!("{}", i18n::msg("flattened_declarations", &[&total_declarations]));

        // 2. Identify Variables
        println!("{}", i18n::msg0("analyzing_variables"));
        let mut state_vars = HashSet::new();
        let mut discrete_vars = HashSet::new();
        let mut param_vars = Vec::new();
        let mut output_vars = Vec::new();
        let mut params = Vec::new();
        let mut states = Vec::new();
        let mut discrete_vals = Vec::new();
        
        // Collect states from equations first (der(x))
        for eq in &flat_model.equations {
            collect_states_from_eq(eq, &mut state_vars);
        }
        
        // Also check declarations
        for decl in &flat_model.declarations {
            if decl.is_parameter {
                param_vars.push(decl.name.clone());
                let val = decl.start_value.as_ref().and_then(|v| eval_const_expr(v)).unwrap_or(0.0);
                params.push(val);
            } else if decl.is_discrete {
                discrete_vars.insert(decl.name.clone());
            } else {
                // Algebraic
            }
        }
        
        // Sort vars for deterministic order
        let mut state_vars_sorted: Vec<String> = state_vars.iter().cloned().collect();
        state_vars_sorted.sort();
        
        let mut discrete_vars_sorted: Vec<String> = discrete_vars.iter().cloned().collect();
        discrete_vars_sorted.sort();

        let state_set: HashSet<&String> = state_vars_sorted.iter().collect();
        let discrete_set: HashSet<&String> = discrete_vars_sorted.iter().collect();

        let decl_index: HashMap<String, usize> = flat_model.declarations
            .iter()
            .enumerate()
            .map(|(i, d)| (d.name.clone(), i))
            .collect();

        for var in &state_vars_sorted {
            let val = decl_index.get(var)
                .and_then(|&idx| flat_model.declarations[idx].start_value.as_ref())
                .and_then(|v| eval_const_expr(v))
                .unwrap_or(0.0);
            states.push(val);
        }
        
        for var in &discrete_vars_sorted {
            let val = decl_index.get(var)
                .and_then(|&idx| flat_model.declarations[idx].start_value.as_ref())
                .and_then(|v| eval_const_expr(v))
                .unwrap_or(0.0);
            discrete_vals.push(val);
        }
        
        let mut algebraic_vars = HashSet::new();
        let mut output_vars_set = HashSet::new();
        for decl in &flat_model.declarations {
             if !decl.is_parameter && !discrete_set.contains(&decl.name) && !state_set.contains(&decl.name) {
                 algebraic_vars.insert(decl.name.clone());
                 output_vars.push(decl.name.clone());
                 output_vars_set.insert(decl.name.clone());
             }
        }
        
        for var in &state_vars_sorted {
            let der_var = format!("der_{}", var);
            if output_vars_set.insert(der_var.clone()) {
                output_vars.push(der_var.clone());
                algebraic_vars.insert(der_var);
            }
        }
        
        let input_var_names: Vec<String> = flat_model
            .declarations
            .iter()
            .filter(|d| d.is_input)
            .map(|d| d.name.clone())
            .collect();

        let state_var_index: HashMap<String, usize> = state_vars_sorted.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        let discrete_var_index: HashMap<String, usize> = discrete_vars_sorted.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        let param_var_index: HashMap<String, usize> = param_vars.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();

        // Apply simple constant initial equations / algorithms to override defaults
        initial_conditions::apply_initial_conditions(
            &flat_model,
            &mut states,
            &mut discrete_vals,
            &mut params,
            &state_var_index,
            &discrete_var_index,
            &param_var_index,
        );
        let mut output_var_index: HashMap<String, usize> = output_vars.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        
        let mut array_info = HashMap::new();
        for (name, size) in &flat_model.array_sizes {
            let first_elem = format!("{}_1", name);
            
            let (atype, start_idx) = if let Some(&pos) = state_var_index.get(&first_elem) {
                (ArrayType::State, pos)
            } else if let Some(&pos) = discrete_var_index.get(&first_elem) {
                (ArrayType::Discrete, pos)
            } else if let Some(&pos) = param_var_index.get(&first_elem) {
                (ArrayType::Parameter, pos)
            } else if let Some(&pos) = output_var_index.get(&first_elem) {
                (ArrayType::Output, pos)
            } else {
                 continue;
            };
            
            array_info.insert(name.clone(), ArrayInfo {
                array_type: atype,
                start_index: start_idx,
                size: *size,
            });
        }

        // 3. Normalize Derivatives (der(x) -> der_x)
        println!("{}", i18n::msg0("normalizing_derivatives"));
        let mut normalized_eqs = Vec::new();
        for eq in &flat_model.equations {
             match eq {
                 Equation::Simple(lhs, rhs) => {
                     normalized_eqs.push(Equation::Simple(crate::analysis::normalize_der(lhs), crate::analysis::normalize_der(rhs)));
                 }
                 Equation::For(v, s, e, b) => {
                     let norm_body = b.iter().map(|eq| {
                         if let Equation::Simple(l, r) = eq {
                             Equation::Simple(crate::analysis::normalize_der(l), crate::analysis::normalize_der(r))
                         } else {
                             eq.clone()
                         }
                     }).collect();
                     normalized_eqs.push(Equation::For(v.clone(), s.clone(), e.clone(), norm_body));
                 }
                 Equation::If(cond, then_eqs, elseif_list, else_eqs) => {
                     let norm_then = then_eqs.iter().map(|e| match e {
                         Equation::Simple(l, r) => Equation::Simple(crate::analysis::normalize_der(l), crate::analysis::normalize_der(r)),
                         _ => e.clone(),
                     }).collect();
                     let norm_elseif = elseif_list.iter().map(|(c, eb)| (
                         crate::analysis::normalize_der(c),
                         eb.iter().map(|e| match e {
                             Equation::Simple(l, r) => Equation::Simple(crate::analysis::normalize_der(l), crate::analysis::normalize_der(r)),
                             _ => e.clone(),
                         }).collect::<Vec<_>>(),
                     )).collect();
                     let norm_else = else_eqs.as_ref().map(|eqs| eqs.iter().map(|e| match e {
                         Equation::Simple(l, r) => Equation::Simple(crate::analysis::normalize_der(l), crate::analysis::normalize_der(r)),
                         _ => e.clone(),
                     }).collect());
                     normalized_eqs.push(Equation::If(
                         crate::analysis::normalize_der(cond),
                         norm_then,
                         norm_elseif,
                         norm_else,
                     ));
                 }
                 _ => normalized_eqs.push(eq.clone()),
             }
        }
        
        let alg_equations;
        let mut diff_equations = Vec::new();
        
        // Generate implicit diff equations: der(x) = der_x
        // We need to add der_x to outputs if they are not there, and update array info.
        
        for var in &state_vars_sorted {
            let der_var = format!("der_{}", var);
            if !output_var_index.contains_key(&der_var) {
                let pos = output_vars.len();
                output_vars.push(der_var.clone());
                output_var_index.insert(der_var.clone(), pos);
            }
            
            if let Some((base, _idx)) = equation_convert::parse_array_index(var) {
                if let Some(info) = array_info.get(&base).cloned() {
                    if let ArrayType::State = info.array_type {
                         let der_base = format!("der_{}", base);
                         if !array_info.contains_key(&der_base) {
                             let first_der = format!("der_{}_1", base);
                             if let Some(&pos) = output_var_index.get(&first_der) {
                                  array_info.insert(der_base, ArrayInfo {
                                     array_type: ArrayType::Output,
                                     start_index: pos,
                                     size: info.size
                                  });
                             }
                         }
                    }
                }
            }
        }
        
        for (name, info) in &array_info.clone() {
             if let ArrayType::State = info.array_type {
                 let der_name = format!("der_{}", name);
                 let first_elem = format!("der_{}_1", name);
                 if let Some(&pos) = output_var_index.get(&first_elem) {
                     array_info.insert(der_name, ArrayInfo {
                         array_type: ArrayType::Output, 
                         start_index: pos,
                         size: info.size,
                     });
                 }
             }
        }
        
        // Second pass: Generate diff equations
        for var in &state_vars_sorted {
            // Check if var is part of an array
             let rhs_expr = if let Some((base, idx)) = equation_convert::parse_array_index(var) {
                  // Check if base is in array_info (as State)
                  if array_info.contains_key(&base) {
                      let der_base = format!("der_{}", base);
                      // Check if der_base is in array_info (as Output/Algebraic)
                      if array_info.contains_key(&der_base) {
                          Expression::ArrayAccess(
                              Box::new(Expression::Variable(der_base)), 
                              Box::new(Expression::Number(idx as f64))
                          )
                      } else {
                          Expression::Variable(format!("der_{}", var))
                      }
                  } else {
                      Expression::Variable(format!("der_{}", var))
                  }
             } else {
                  // Use der_var name
                  Expression::Variable(format!("der_{}", var))
             };

            diff_equations.push(Equation::Simple(
                Expression::Der(Box::new(Expression::Variable(var.clone()))),
                rhs_expr
            ));
        }

        // 4. Structure Analysis (BLT)
        println!("{}", i18n::msg0("performing_structure_analysis"));
        
        let mut continuous_eqs = Vec::new();
        
        for eq in normalized_eqs {
             match eq {
                 Equation::When(_, _, _) | Equation::Reinit(_, _) | Equation::If(_, _, _, _)
                 | Equation::Assert(_, _) | Equation::Terminate(_) => {
                     // Converted to algorithms and handled later
                 }
                 _ => continuous_eqs.push(eq),
             }
        }
        
        // Sort continuous equations
        // We need to pass all known variables to sort_algebraic_equations.
        // Knowns = States + Discrete + Params + Time (added inside) + Inputs (if any)
        // Unknowns = Algebraic + Derivatives
        
        let mut known_vars = HashSet::new();
        for v in &state_vars_sorted { known_vars.insert(v.clone()); }
        for v in &discrete_vars_sorted { known_vars.insert(v.clone()); }
        // params are passed separately
        
        // IMPORTANT: Ensure der_x are NOT in known_vars!
        // (They are not in state_vars_sorted or discrete_vars_sorted, so we are good)
        
        let analysis_opts = AnalysisOptions {
            index_reduction_method: opts.index_reduction_method.clone(),
            tearing_method: opts.tearing_method.clone(),
        };
        let sort_result = sort_algebraic_equations(
            &continuous_eqs,
            &known_vars,
            &param_vars,
            &analysis_opts,
        );
        let mut alg_eqs = sort_result.sorted_equations;
        for out_var in &output_vars {
            if let Some(alias_expr) = sort_result.alias_map.get(out_var) {
                alg_eqs.push(Equation::Simple(
                    Expression::Variable(out_var.clone()),
                    alias_expr.clone(),
                ));
            }
        }
        alg_equations = alg_eqs;
        let differential_index = sort_result.differential_index;
        let constraint_equation_count = sort_result.constraint_equation_count;

        if differential_index > 1 && opts.warnings_level != "none" {
            let method_note = if opts.index_reduction_method == "none" {
                "index reduction not applied (use --index-reduction-method=dummyDerivative); simulation may be unreliable".to_string()
            } else {
                format!("{} constraint equation(s) before reduction; differential index {}", constraint_equation_count, differential_index)
            };
            self.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!("differential index is {}; {}", differential_index, method_note),
                source: None,
            });
        }

        let mut known_at_initial = HashSet::new();
        known_at_initial.insert("time".to_string());
        for p in &param_vars {
            known_at_initial.insert(p.clone());
        }
        let initial_info = analyze_initial_equations(&flat_model.initial_equations, &known_at_initial);
        if initial_info.is_underdetermined && initial_info.equation_count > 0 && opts.warnings_level != "none" {
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
        let algebraic_loops = alg_equations.iter().filter(|e| matches!(e, Equation::SolvableBlock { .. })).count();
        if algebraic_loops > 0 && opts.warnings_level != "none" {
            self.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!("{} algebraic loop(s) (strong component(s)) present, solved with tearing", algebraic_loops),
                source: None,
            });
        }

        let numeric_ode_jacobian =
            opts.generate_dynamic_jacobian == "numeric" || opts.generate_dynamic_jacobian == "both";
        let symbolic_ode_jacobian_enabled =
            opts.generate_dynamic_jacobian == "symbolic" || opts.generate_dynamic_jacobian == "both";
        let ode_jacobian_sparse = if symbolic_ode_jacobian_enabled {
            jacobian::build_ode_jacobian_sparse(&state_vars_sorted, &alg_equations, &state_var_index)
        } else {
            None
        };
        let symbolic_ode_jacobian_matrix = ode_jacobian_sparse
            .as_ref()
            .map(|s| s.to_dense())
            .or_else(|| {
                if symbolic_ode_jacobian_enabled {
                    jacobian::build_ode_jacobian_expressions(&state_vars_sorted, &alg_equations, &state_var_index)
                } else {
                    None
                }
            });
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

        let all_called = collect_all_called_names(&alg_equations, &diff_equations, &flat_model.algorithms);
        let external_names: HashSet<String> = external_list.iter().map(|(n, _, _)| n.clone()).collect();
        let mut user_stub_jits: Vec<Jit> = Vec::new();
        let mut user_stub_ptrs: HashMap<String, *const u8> = HashMap::new();
        let mut user_function_bodies: HashMap<String, (Vec<String>, Expression)> = HashMap::new();
        for name in &all_called {
            if inline::is_builtin_function(name) || external_names.contains(name) {
                continue;
            }
            let func_model = self.loader.load_model(name)
                .map_err(|e| format!("Cannot load function '{}': {}", name, e))?;
            if func_model.external_info.is_some() {
                continue;
            }
            let (input_names, outputs) = inline::get_function_body(func_model.as_ref())
                .ok_or_else(|| format!(
                    "Function '{}' cannot be used as JIT callable: not inlinable (side effects, multi-output, or no body). Use single-output pure function (FUNC-2).",
                    name
                ))?;
            if outputs.len() != 1 {
                return Err(format!(
                    "Function '{}' has {} outputs; JIT callable supports single-output only (FUNC-2).",
                    name, outputs.len()
                ).into());
            }
            let mut stub_jit = Jit::new();
            let ptr = stub_jit.compile_user_function_stub(name, &input_names, &outputs[0].1)
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
                &continuous_eqs,
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
                    state_var_index.get(&first).copied().map(|start| (name.clone(), start, size))
                })
                .collect();
            let output_array_layout: Vec<(String, usize, usize)> = flat_model
                .array_sizes
                .iter()
                .filter_map(|(name, &size)| {
                    let first = format!("{}_1", name);
                    output_var_index.get(&first).copied().map(|start| (name.clone(), start, size))
                })
                .collect();
            let param_array_layout: Vec<(String, usize, usize)> = flat_model
                .array_sizes
                .iter()
                .filter_map(|(name, &size)| {
                    let first = format!("{}_1", name);
                    param_var_index.get(&first).copied().map(|start| (name.clone(), start, size))
                })
                .collect();
            let state_layout_opt = if state_array_layout.is_empty() { None } else { Some(state_array_layout.as_slice()) };
            let output_layout_opt = if output_array_layout.is_empty() { None } else { Some(output_array_layout.as_slice()) };
            let param_layout_opt = if param_array_layout.is_empty() { None } else { Some(param_array_layout.as_slice()) };
            let external_c_names: HashMap<String, String> = external_list.iter().map(|(m, c, _)| (m.clone(), c.clone())).collect();
            let external_c_names_opt = if external_c_names.is_empty() { None } else { Some(external_c_names) };
            let external_names_set: HashSet<String> = external_list.iter().map(|(n, _, _)| n.clone()).collect();
            let user_fn_bodies_opt = if user_function_bodies.is_empty() { None } else { Some(&user_function_bodies) };
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
                    let paths: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
                    println!("{}", i18n::msg("c_codegen_emitted", &[&paths.join(", ")]));
                }
                Err(e) => {
                    return Err(format!("C codegen failed: {}{}", e, self.source_loc_suffix(model_name)).into());
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
        println!("{}", i18n::msg0("jit_compiling"));
        println!("{}", i18n::msg("equations_after_sorting", &[&alg_equations.len()]));
        println!("{}", i18n::msg("state_variables", &[&state_vars_sorted.len()]));
        println!("{}", i18n::msg("discrete_variables", &[&discrete_vars_sorted.len()]));
        println!("{}", i18n::msg("parameters_count", &[&param_vars.len()]));
        
        let mut algorithms = flat_model.algorithms.clone();
        
        // Convert When/Reinit to algorithms
        for eq in &flat_model.equations {
            match eq {
                 Equation::When(c, b, e) => {
                     let nb: Vec<Equation> = b.iter().map(|x| match x { Equation::Simple(l,r) => Equation::Simple(crate::analysis::normalize_der(l), crate::analysis::normalize_der(r)), _ => x.clone() }).collect();
                     let norm_eq = Equation::When(crate::analysis::normalize_der(c), nb, e.clone());
                     algorithms.push(equation_convert::convert_eq_to_alg_stmt(norm_eq));
                 },
                 Equation::Reinit(v, e) => {
                     let norm_eq = Equation::Reinit(v.clone(), crate::analysis::normalize_der(e));
                     algorithms.push(equation_convert::convert_eq_to_alg_stmt(norm_eq));
                 }
                 Equation::Assert(cond, msg) => {
                     algorithms.push(equation_convert::convert_eq_to_alg_stmt(Equation::Assert(
                         crate::analysis::normalize_der(cond),
                         crate::analysis::normalize_der(msg),
                     )));
                 }
                 Equation::Terminate(msg) => {
                     algorithms.push(equation_convert::convert_eq_to_alg_stmt(Equation::Terminate(
                         crate::analysis::normalize_der(msg),
                     )));
                 }
                 Equation::If(cond, then_eqs, elseif_list, else_eqs) => {
                     let norm_if = Equation::If(
                         crate::analysis::normalize_der(cond),
                         then_eqs.iter().map(|e| match e {
                             Equation::Simple(l, r) => Equation::Simple(crate::analysis::normalize_der(l), crate::analysis::normalize_der(r)),
                             _ => e.clone(),
                         }).collect(),
                         elseif_list.iter().map(|(c, eb)| (
                             crate::analysis::normalize_der(c),
                             eb.iter().map(|e| match e {
                                 Equation::Simple(l, r) => Equation::Simple(crate::analysis::normalize_der(l), crate::analysis::normalize_der(r)),
                                 _ => e.clone(),
                             }).collect(),
                         )).collect(),
                         else_eqs.as_ref().map(|eqs| eqs.iter().map(|e| match e {
                             Equation::Simple(l, r) => Equation::Simple(crate::analysis::normalize_der(l), crate::analysis::normalize_der(r)),
                             _ => e.clone(),
                         }).collect()),
                     );
                     algorithms.push(equation_convert::convert_eq_to_alg_stmt(norm_if));
                 }
                _ => {}
            }
        }

        let newton_tearing_var_names: Vec<String> = alg_equations
            .iter()
            .filter_map(|eq| {
                if let Equation::SolvableBlock { tearing_var: Some(ref t), residuals, .. } = eq {
                    if residuals.len() == 1 {
                        Some(t.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        let lib_paths: Vec<std::path::PathBuf> = if !self.options.external_libs.is_empty() {
            self.options.external_libs.iter().map(|p| std::path::PathBuf::from(p)).collect()
        } else {
            let mut from_annotation: Vec<std::path::PathBuf> = external_list.iter()
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
                let lib = unsafe { libloading::Library::new(path.as_path()) }
                    .map_err(|e| format!("Failed to load external lib '{}': {}", path.display(), e))?;
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
            Ok((calc_derivs, when_count, crossings_count)) => Ok(CompileOutput::Simulation(Artifacts {
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
            })),
            Err(e) => {
                Err(format!("JIT compilation failed: {}{}", e, self.source_loc_suffix(model_name)).into())
            }
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

