use std::collections::{HashMap, HashSet};

use crate::ast::{Equation, Expression, AlgorithmStatement};
use crate::loader::ModelLoader;
use crate::flatten::{Flattener, eval_const_expr};
use crate::analysis::{sort_algebraic_equations, collect_states_from_eq};
use crate::jit::{Jit, CalcDerivsFunc, ArrayInfo, ArrayType};

#[derive(Clone)]
pub struct CompilerOptions {
    pub backend_dae_info: bool,
    pub index_reduction_method: String,
    pub generate_dynamic_jacobian: String,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        CompilerOptions {
            backend_dae_info: false,
            index_reduction_method: "none".to_string(),
            generate_dynamic_jacobian: "none".to_string(),
        }
    }
}

pub struct Compiler {
    pub loader: ModelLoader,
    pub jit: Jit,
    pub options: CompilerOptions,
}

pub struct Artifacts {
    pub calc_derivs: CalcDerivsFunc,
    pub states: Vec<f64>,
    pub discrete_vals: Vec<f64>,
    pub params: Vec<f64>,
    pub state_vars: Vec<String>,
    pub discrete_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub when_count: usize,
    pub crossings_count: usize,
    pub t_end: f64,
    pub dt: f64,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            loader: ModelLoader::new(),
            jit: Jit::new(),
            options: CompilerOptions::default(),
        }
    }

    pub fn compile(&mut self, model_name: &str) -> Result<Artifacts, Box<dyn std::error::Error + Send + Sync>> {
        let opts = &self.options;
        println!("Loading model '{}'...", model_name);
        let mut root_model = self.loader.load_model(model_name)
            .map_err(|e| format!("Failed to load model: {}", e))?;

        println!("Flattening model...");
        let mut flattener = Flattener::new();
        for path in &self.loader.library_paths {
            flattener.loader.add_path(path.clone());
        }
        let flat_model = flattener.flatten(&mut root_model)?;
        
        // Output detailed statistics
        let total_equations = flat_model.equations.len();
        let total_declarations = flat_model.declarations.len();
        println!("  Flattened equations: {}", total_equations);
        println!("  Flattened declarations: {}", total_declarations);

        // 2. Identify Variables
        println!("Analyzing variables...");
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

        for var in &state_vars_sorted { output_vars.push(var.clone()); }
        for var in &discrete_vars_sorted { output_vars.push(var.clone()); }
        output_vars.sort(); 
        output_vars.dedup();
        
        let state_var_index: HashMap<String, usize> = state_vars_sorted.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        let discrete_var_index: HashMap<String, usize> = discrete_vars_sorted.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        let param_var_index: HashMap<String, usize> = param_vars.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
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
        println!("Normalizing derivatives...");
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
        println!("Performing Structure Analysis...");
        
        let mut continuous_eqs = Vec::new();
        
        for eq in normalized_eqs {
             match eq {
                 Equation::When(_, _, _) | Equation::Reinit(_, _) => {
                     // handled later
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
        
        let sorted_eqs = sort_algebraic_equations(&continuous_eqs, &known_vars, &param_vars);
        alg_equations = sorted_eqs;

        if opts.backend_dae_info {
            print_backend_dae_info(
                opts,
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
            );
        }

        // 5. JIT Compile
        println!("JIT Compiling...");
        println!("  Equations after aliasing/sorting: {}", alg_equations.len());
        println!("  State variables: {}", state_vars_sorted.len());
        println!("  Discrete variables: {}", discrete_vars_sorted.len());
        println!("  Parameters: {}", param_vars.len());
        
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
                _ => {}
            }
        }

        let res = self.jit.compile(
            &state_vars_sorted,
            &discrete_vars_sorted,
            &param_vars,
            &output_vars,
            &array_info,
            &alg_equations,
            &diff_equations,
            &algorithms
        );
        
        match res {
            Ok((calc_derivs, when_count, crossings_count)) => Ok(Artifacts {
                calc_derivs,
                states,
                discrete_vals,
                params,
                state_vars: state_vars_sorted,
                discrete_vars: discrete_vars_sorted,
                output_vars,
                when_count,
                crossings_count,
                t_end: 10.0,
                dt: 0.01,
            }),
            Err(e) => Err(format!("JIT compilation failed: {}", e).into()),
        }
    }
}

fn print_backend_dae_info(
    opts: &CompilerOptions,
    total_equations: usize,
    total_declarations: usize,
    state_vars: &[String],
    discrete_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    continuous_eqs: &[Equation],
    sorted_eqs: &[Equation],
    all_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
) {
    let mut simple = 0usize;
    let mut for_eq = 0usize;
    let mut when_eq = 0usize;
    let mut reinit_eq = 0usize;
    let mut connect_eq = 0usize;
    let mut solvable_block = 0usize;
    for eq in all_equations {
        match eq {
            Equation::Simple(_, _) => simple += 1,
            Equation::For(_, _, _, _) => for_eq += 1,
            Equation::When(_, _, _) => when_eq += 1,
            Equation::Reinit(_, _) => reinit_eq += 1,
            Equation::Connect(_, _) => connect_eq += 1,
            Equation::SolvableBlock { .. } => solvable_block += 1,
        }
    }
    let mut sorted_simple = 0usize;
    let mut sorted_for = 0usize;
    let mut sorted_block = 0usize;
    let mut tearing_var_count = 0usize;
    let mut torn_unknowns_total = 0usize;
    for eq in sorted_eqs {
        match eq {
            Equation::Simple(_, _) => sorted_simple += 1,
            Equation::For(_, _, _, _) => sorted_for += 1,
            Equation::SolvableBlock { unknowns, tearing_var, .. } => {
                sorted_block += 1;
                torn_unknowns_total += unknowns.len();
                if tearing_var.is_some() {
                    tearing_var_count += 1;
                }
            }
            _ => {}
        }
    }

    println!("\nBackend DAE info (OpenModelica-style):");
    println!("  [Variables]");
    println!("    Total: {}", total_declarations);
    println!("    State (der): {}", state_vars.len());
    println!("    Discrete: {}", discrete_vars.len());
    println!("    Parameters: {}", param_vars.len());
    println!("    Outputs: {}", output_vars.len());
    println!("  [Equations (flattened)]");
    println!("    Total: {}", total_equations);
    println!("    Simple (single assignment): {}", simple);
    println!("    For: {}", for_eq);
    println!("    When: {}", when_eq);
    println!("    Reinit: {}", reinit_eq);
    println!("    Connect: {}", connect_eq);
    println!("    SolvableBlock (algebraic blocks): {}", solvable_block);
    println!("  [Equations (after BLT)]");
    println!("    Continuous (sorted): {}", continuous_eqs.len());
    println!("    Sorted: Simple {}, For {}, SolvableBlock {}", sorted_simple, sorted_for, sorted_block);
    println!("  [Algorithms]");
    println!("    Algorithm sections: {}", algorithms.len());
    println!("  [Index reduction]");
    println!("    Method: {}", opts.index_reduction_method);
    println!("    Differential index: 1 (no reduction applied)");
    println!("  [Tearing]");
    println!("    Algebraic loops (strong components): {}", sorted_block);
    println!("    Tearing variables (selected): {}", tearing_var_count);
    println!("    Torn unknowns (total in blocks): {}", torn_unknowns_total);
    println!("  [Jacobian]");
    println!("    generateDynamicJacobian: {}", opts.generate_dynamic_jacobian);
    println!("    Strong component Jacobians: no");
    println!("    Symbolic ODE Jacobian: no");
    println!();
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
