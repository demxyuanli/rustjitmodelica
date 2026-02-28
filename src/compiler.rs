mod inline;
mod jacobian;

use std::collections::{HashMap, HashSet};

use crate::ast::{Equation, Expression, AlgorithmStatement};
use crate::backend_dae::{build_simulation_dae, SimulationDae};
use crate::loader::ModelLoader;
use crate::flatten::{Flattener, eval_const_expr};
use crate::analysis::{sort_algebraic_equations, collect_states_from_eq, analyze_initial_equations, AnalysisOptions};
use crate::diag::WarningInfo;
use crate::jit::{Jit, CalcDerivsFunc, ArrayInfo, ArrayType};
use crate::expr_eval;
use crate::i18n;

#[derive(Clone)]
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
    /// DBG-3: Warnings level: "all" | "none" | "error" (none = suppress, error = treat as error).
    pub warnings_level: String,
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
            warnings_level: "all".to_string(),
        }
    }
}

pub struct Compiler {
    pub loader: ModelLoader,
    pub jit: Jit,
    pub options: CompilerOptions,
    pub(crate) warnings: Vec<WarningInfo>,
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
    pub discrete_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub state_var_index: HashMap<String, usize>,
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
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            loader: ModelLoader::new(),
            jit: Jit::new(),
            options: CompilerOptions::default(),
            warnings: Vec::new(),
        }
    }

    pub fn take_warnings(&mut self) -> Vec<WarningInfo> {
        std::mem::take(&mut self.warnings)
    }

    /// Run a function once with given inputs (or 0.0 per input if not provided) and return the output (F3-1).
    fn run_function_once(&mut self, model_name: &str) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        let root_model = self.loader.load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        let (input_names, body) = inline::get_function_body(root_model.as_ref())
            .ok_or("Function must have exactly one output and one assignment to it in algorithm.")?;
        let args = self.options.function_args.as_deref().unwrap_or(&[]);
        let mut vars = HashMap::new();
        for (i, name) in input_names.iter().enumerate() {
            let val = args.get(i).copied().unwrap_or(0.0);
            vars.insert(name.clone(), val);
        }
        expr_eval::eval_expr(&body, &vars).map_err(|e| e.into())
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
        let mut flat_model = flattener.flatten(&mut root_model)?;

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
        apply_initial_conditions(
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
            
            if let Some((base, _idx)) = parse_array_index(var) {
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
             let rhs_expr = if let Some((base, idx)) = parse_array_index(var) {
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
        alg_equations = sort_result.sorted_equations;
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
        let symbolic_ode_jacobian_matrix =
            if symbolic_ode_jacobian_enabled {
                jacobian::build_ode_jacobian_expressions(&state_vars_sorted, &alg_equations, &state_var_index)
            } else {
                None
            };
        let symbolic_ode_jacobian = symbolic_ode_jacobian_matrix.is_some();
        let strong_component_jacobians = false;

        let simulation_dae: SimulationDae = build_simulation_dae(
            &state_vars_sorted,
            &discrete_vars_sorted,
            &param_vars,
            &output_vars,
            &input_var_names,
            diff_equations.len(),
            &alg_equations,
            flat_model.initial_equations.len(),
            differential_index,
            constraint_equation_count,
        );

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
                &continuous_eqs,
                &alg_equations,
                &flat_model.equations,
                &flat_model.algorithms,
                strong_component_jacobians,
                symbolic_ode_jacobian,
                numeric_ode_jacobian,
                symbolic_ode_jacobian_matrix.as_ref(),
                Some(&simulation_dae),
            );
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
                     algorithms.push(convert_eq_to_alg_stmt(norm_eq));
                 },
                 Equation::Reinit(v, e) => {
                     let norm_eq = Equation::Reinit(v.clone(), crate::analysis::normalize_der(e));
                     algorithms.push(convert_eq_to_alg_stmt(norm_eq));
                 }
                 Equation::Assert(cond, msg) => {
                     algorithms.push(convert_eq_to_alg_stmt(Equation::Assert(
                         crate::analysis::normalize_der(cond),
                         crate::analysis::normalize_der(msg),
                     )));
                 }
                 Equation::Terminate(msg) => {
                     algorithms.push(convert_eq_to_alg_stmt(Equation::Terminate(
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
                     algorithms.push(convert_eq_to_alg_stmt(norm_if));
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

        let t_end = self.options.t_end;
        let dt = self.options.dt;
        let res = self.jit.compile(
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
                discrete_vars: discrete_vars_sorted,
                output_vars,
                state_var_index,
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
            })),
            Err(e) => Err(format!("JIT compilation failed: {}", e).into()),
        }
    }
}

fn convert_eq_to_alg_stmt(eq: Equation) -> AlgorithmStatement {
    match eq {
        Equation::Simple(lhs, rhs) => AlgorithmStatement::Assignment(lhs, rhs),
        Equation::Reinit(var, val) => AlgorithmStatement::Reinit(var, val),
        Equation::For(var, start, end, body) => {
            let alg_body = body.into_iter().map(convert_eq_to_alg_stmt).collect();
            let range = Expression::Range(start, Box::new(Expression::Number(1.0)), end);
            AlgorithmStatement::For(var, Box::new(range), alg_body)
        }
        Equation::When(cond, body, else_whens) => {
             let alg_body = body.into_iter().map(convert_eq_to_alg_stmt).collect();
             let alg_else = else_whens.into_iter().map(|(c, b)| (c, b.into_iter().map(convert_eq_to_alg_stmt).collect())).collect();
             AlgorithmStatement::When(cond, alg_body, alg_else)
        }
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => {
            let then_alg = then_eqs.into_iter().map(convert_eq_to_alg_stmt).collect();
            let elseif_alg = elseif_list.into_iter()
                .map(|(c, eb)| (c, eb.into_iter().map(convert_eq_to_alg_stmt).collect()))
                .collect();
            let else_alg = else_eqs.map(|eqs| eqs.into_iter().map(convert_eq_to_alg_stmt).collect());
            AlgorithmStatement::If(cond, then_alg, elseif_alg, else_alg)
        }
        Equation::Assert(cond, msg) => AlgorithmStatement::Assert(cond, msg),
        Equation::Terminate(msg) => AlgorithmStatement::Terminate(msg),
        _ => panic!("Unsupported equation in algorithm conversion: {:?}", eq),
    }
}

fn parse_array_index(name: &str) -> Option<(String, usize)> {
    if let Some((base, idx_str)) = name.rsplit_once('_') {
        if let Ok(idx) = idx_str.parse::<usize>() {
            return Some((base.to_string(), idx));
        }
    }
    None
}

/// Apply a very small subset of Modelica initialization semantics:
/// - Handle `initial equation` and `initial algorithm` that are simple assignments
///   to scalar variables with constant right-hand sides.
/// - RHS is evaluated with `eval_const_expr` and can depend only on literals
///   (no state/discrete/parameter references for now).
fn apply_initial_conditions(
    flat_model: &crate::flatten::FlattenedModel,
    states: &mut [f64],
    discrete_vals: &mut [f64],
    params: &mut [f64],
    state_var_index: &HashMap<String, usize>,
    discrete_var_index: &HashMap<String, usize>,
    param_var_index: &HashMap<String, usize>,
) {
    use crate::flatten::eval_const_expr;
    use crate::ast::Expression;

    // Helper: assign value to state / discrete / param vector if name matches.
    fn assign_var(
        name: &str,
        value: f64,
        states: &mut [f64],
        discrete_vals: &mut [f64],
        params: &mut [f64],
        state_var_index: &HashMap<String, usize>,
        discrete_var_index: &HashMap<String, usize>,
        param_var_index: &HashMap<String, usize>,
    ) {
        if let Some(&idx) = state_var_index.get(name) {
            if idx < states.len() {
                states[idx] = value;
                return;
            }
        }
        if let Some(&idx) = discrete_var_index.get(name) {
            if idx < discrete_vals.len() {
                discrete_vals[idx] = value;
                return;
            }
        }
        if let Some(&idx) = param_var_index.get(name) {
            if idx < params.len() {
                params[idx] = value;
                return;
            }
        }
    }

    // initial equation section: apply in dependency order by iteratively substituting known values (IR3-3).
    let mut applied = true;
    let mut pass_limit = 20;
    while applied && pass_limit > 0 {
        pass_limit -= 1;
        applied = false;
        for eq in &flat_model.initial_equations {
            if let Equation::Simple(lhs, rhs) = eq {
                if let Expression::Variable(name) = lhs {
                    let rhs_sub = substitute_initial_values(
                        rhs,
                        state_var_index,
                        discrete_var_index,
                        param_var_index,
                        states,
                        discrete_vals,
                        params,
                    );
                    if let Some(v) = eval_const_expr(&rhs_sub) {
                        let prev = state_var_index.get(name).and_then(|&i| Some(states[i]))
                            .or_else(|| discrete_var_index.get(name).and_then(|&i| Some(discrete_vals[i])))
                            .or_else(|| param_var_index.get(name).and_then(|&i| Some(params[i])));
                        let changed = prev.map(|p| (p - v).abs() > 1e-15).unwrap_or(true);
                        if changed {
                            assign_var(
                                name,
                                v,
                                states,
                                discrete_vals,
                                params,
                                state_var_index,
                                discrete_var_index,
                                param_var_index,
                            );
                            applied = true;
                        }
                    }
                }
            }
        }
    }

    // initial algorithm section: apply with substitution of state/discrete/param (consistent init).
    for stmt in &flat_model.initial_algorithms {
        if let AlgorithmStatement::Assignment(lhs, rhs) = stmt {
            if let Expression::Variable(name) = lhs {
                let rhs_sub = substitute_initial_values(
                    rhs,
                    state_var_index,
                    discrete_var_index,
                    param_var_index,
                    states,
                    discrete_vals,
                    params,
                );
                if let Some(v) = eval_const_expr(&rhs_sub) {
                    assign_var(
                        name,
                        v,
                        states,
                        discrete_vals,
                        params,
                        state_var_index,
                        discrete_var_index,
                        param_var_index,
                    );
                } else {
                    eprintln!(
                        "Warning: initial assignment for '{}' ignored (non-constant rhs: {:?})",
                        name,
                        rhs
                    );
                }
            }
        }
    }
}

/// Replace occurrences of state, discrete, and parameter variables with current values (for initial system).
fn substitute_initial_values(
    expr: &Expression,
    state_var_index: &HashMap<String, usize>,
    discrete_var_index: &HashMap<String, usize>,
    param_var_index: &HashMap<String, usize>,
    states: &[f64],
    discrete_vals: &[f64],
    params: &[f64],
) -> Expression {
    use Expression::*;
    match expr {
        Variable(name) => {
            if let Some(&idx) = state_var_index.get(name) {
                if idx < states.len() {
                    return Number(states[idx]);
                }
            }
            if let Some(&idx) = discrete_var_index.get(name) {
                if idx < discrete_vals.len() {
                    return Number(discrete_vals[idx]);
                }
            }
            if let Some(&idx) = param_var_index.get(name) {
                if idx < params.len() {
                    return Number(params[idx]);
                }
            }
            if name == "time" {
                return Number(0.0);
            }
            Variable(name.clone())
        }
        Number(n) => Number(*n),
        BinaryOp(lhs, op, rhs) => BinaryOp(
            Box::new(substitute_initial_values(
                lhs, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
            *op,
            Box::new(substitute_initial_values(
                rhs, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
        ),
        Call(func, args) => Call(
            func.clone(),
            args.iter()
                .map(|a| substitute_initial_values(
                    a, state_var_index, discrete_var_index, param_var_index,
                    states, discrete_vals, params,
                ))
                .collect(),
        ),
        Der(inner) => Der(Box::new(substitute_initial_values(
            inner, state_var_index, discrete_var_index, param_var_index,
            states, discrete_vals, params,
        ))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(substitute_initial_values(
                arr, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
            Box::new(substitute_initial_values(
                idx, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
        ),
        If(cond, t, f) => If(
            Box::new(substitute_initial_values(
                cond, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
            Box::new(substitute_initial_values(
                t, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
            Box::new(substitute_initial_values(
                f, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
        ),
        Range(start, step, end) => Range(
            Box::new(substitute_initial_values(
                start, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
            Box::new(substitute_initial_values(
                step, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
            Box::new(substitute_initial_values(
                end, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
        ),
        ArrayLiteral(items) => ArrayLiteral(
            items
                .iter()
                .map(|e| substitute_initial_values(
                    e, state_var_index, discrete_var_index, param_var_index,
                    states, discrete_vals, params,
                ))
                .collect(),
        ),
        Dot(base, member) => Dot(
            Box::new(substitute_initial_values(
                base, state_var_index, discrete_var_index, param_var_index,
                states, discrete_vals, params,
            )),
            member.clone(),
        ),
    }
}

/// Replace occurrences of parameter variables in an expression with their
/// numeric values from `params`. Used for initial algorithm RHS when no state yet.
#[allow(dead_code)]
fn substitute_params(
    expr: &Expression,
    param_var_index: &HashMap<String, usize>,
    params: &[f64],
) -> Expression {
    use Expression::*;
    match expr {
        Variable(name) => {
            if let Some(&idx) = param_var_index.get(name) {
                if idx < params.len() {
                    return Number(params[idx]);
                }
            }
            Variable(name.clone())
        }
        Number(n) => Number(*n),
        BinaryOp(lhs, op, rhs) => BinaryOp(
            Box::new(substitute_params(lhs, param_var_index, params)),
            *op,
            Box::new(substitute_params(rhs, param_var_index, params)),
        ),
        Call(func, args) => Call(
            func.clone(),
            args.iter()
                .map(|a| substitute_params(a, param_var_index, params))
                .collect(),
        ),
        Der(inner) => Der(Box::new(substitute_params(
            inner,
            param_var_index,
            params,
        ))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(substitute_params(arr, param_var_index, params)),
            Box::new(substitute_params(idx, param_var_index, params)),
        ),
        If(cond, t, f) => If(
            Box::new(substitute_params(cond, param_var_index, params)),
            Box::new(substitute_params(t, param_var_index, params)),
            Box::new(substitute_params(f, param_var_index, params)),
        ),
        Range(start, step, end) => Range(
            Box::new(substitute_params(start, param_var_index, params)),
            Box::new(substitute_params(step, param_var_index, params)),
            Box::new(substitute_params(end, param_var_index, params)),
        ),
        ArrayLiteral(items) => ArrayLiteral(
            items
                .iter()
                .map(|e| substitute_params(e, param_var_index, params))
                .collect(),
        ),
        Dot(base, member) => Dot(
            Box::new(substitute_params(base, param_var_index, params)),
            member.clone(),
        ),
    }
}
