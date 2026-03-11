use std::collections::{HashMap, HashSet};

use crate::ast::{AlgorithmStatement, Equation, Expression};
use crate::analysis::{partial_derivative, expression_is_zero};
use crate::backend_dae::{BlockType, SimulationDae};
use crate::i18n;

use super::CompilerOptions;

/// Sparse ODE Jacobian: only non-zero entries (row, col, expr). IR4-4.
#[derive(Debug, Clone)]
pub struct SparseOdeJacobian {
    pub n: usize,
    pub entries: Vec<(usize, usize, Expression)>,
}

impl SparseOdeJacobian {
    pub fn nnz(&self) -> usize {
        self.entries.len()
    }
    pub fn density(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            self.entries.len() as f64 / (self.n * self.n) as f64
        }
    }
    /// Convert to dense row-major for existing numeric/symbolic eval.
    pub fn to_dense(&self) -> Vec<Vec<Expression>> {
        let mut jac = vec![vec![Expression::Number(0.0); self.n]; self.n];
        for (i, j, e) in &self.entries {
            if *i < self.n && *j < self.n {
                jac[*i][*j] = e.clone();
            }
        }
        jac
    }
}

/// Build ODE Jacobian in sparse form (only non-zero partial derivatives). IR4-4.
pub fn build_ode_jacobian_sparse(
    state_vars: &[String],
    sorted_eqs: &[Equation],
    state_var_index: &HashMap<String, usize>,
) -> Option<SparseOdeJacobian> {
    let mut rhs_by_state: Vec<Option<Expression>> = vec![None; state_vars.len()];
    for eq in sorted_eqs {
        if let Equation::Simple(lhs, rhs) = eq {
            let der_var = match lhs {
                Expression::Variable(v) if v.starts_with("der_") => v.clone(),
                _ => continue,
            };
            let state_name = der_var.strip_prefix("der_")?;
            if let Some(&i) = state_var_index.get(state_name) {
                if i < rhs_by_state.len() {
                    rhs_by_state[i] = Some(rhs.clone());
                }
            }
        }
    }
    if rhs_by_state.iter().any(|o| o.is_none()) {
        return None;
    }
    let n = state_vars.len();
    let mut entries = Vec::new();
    for (i, rhs) in rhs_by_state.iter().enumerate() {
        let rhs = rhs.as_ref().unwrap();
        for (j, state_j) in state_vars.iter().enumerate() {
            let pd = partial_derivative(rhs, state_j);
            if !expression_is_zero(&pd) {
                entries.push((i, j, pd));
            }
        }
    }
    Some(SparseOdeJacobian { n, entries })
}

pub fn build_ode_jacobian_expressions(
    state_vars: &[String],
    sorted_eqs: &[Equation],
    state_var_index: &HashMap<String, usize>,
) -> Option<Vec<Vec<Expression>>> {
    let mut rhs_by_state: Vec<Option<Expression>> = vec![None; state_vars.len()];
    for eq in sorted_eqs {
        if let Equation::Simple(lhs, rhs) = eq {
            let der_var = match lhs {
                Expression::Variable(v) if v.starts_with("der_") => v.clone(),
                _ => continue,
            };
            let state_name = der_var.strip_prefix("der_")?;
            if let Some(&i) = state_var_index.get(state_name) {
                if i < rhs_by_state.len() {
                    rhs_by_state[i] = Some(rhs.clone());
                }
            }
        }
    }
    if rhs_by_state.iter().any(|o| o.is_none()) {
        return None;
    }
    let mut jac = Vec::with_capacity(state_vars.len());
    for i in 0..state_vars.len() {
        let rhs = rhs_by_state[i].as_ref().unwrap();
        let mut row = Vec::with_capacity(state_vars.len());
        for state_j in state_vars {
            row.push(partial_derivative(rhs, state_j));
        }
        jac.push(row);
    }
    Some(jac)
}

fn collect_vars_expr_local(expr: &Expression, vars: &mut HashSet<String>) {
    match expr {
        Expression::Variable(n) => {
            vars.insert(n.clone());
        }
        Expression::Der(e) => {
            collect_vars_expr_local(e, vars);
        }
        Expression::BinaryOp(l, _, r) => {
            collect_vars_expr_local(l, vars);
            collect_vars_expr_local(r, vars);
        }
        Expression::Call(_, args) => {
            for a in args {
                collect_vars_expr_local(a, vars);
            }
        }
        Expression::ArrayAccess(a, i) => {
            collect_vars_expr_local(a, vars);
            collect_vars_expr_local(i, vars);
        }
        Expression::If(c, t, f) => {
            collect_vars_expr_local(c, vars);
            collect_vars_expr_local(t, vars);
            collect_vars_expr_local(f, vars);
        }
        Expression::ArrayLiteral(es) => {
            for e in es {
                collect_vars_expr_local(e, vars);
            }
        }
        Expression::Dot(b, _) => {
            collect_vars_expr_local(b, vars);
        }
        Expression::Range(s, st, e) => {
            collect_vars_expr_local(s, vars);
            collect_vars_expr_local(st, vars);
            collect_vars_expr_local(e, vars);
        }
        _ => {}
    }
}

fn collect_vars_eq_local(eq: &Equation, vars: &mut HashSet<String>) {
    match eq {
        Equation::Simple(l, r) => {
            collect_vars_expr_local(l, vars);
            collect_vars_expr_local(r, vars);
        }
        Equation::For(_, s, e, body) => {
            collect_vars_expr_local(s, vars);
            collect_vars_expr_local(e, vars);
            for sub in body {
                collect_vars_eq_local(sub, vars);
            }
        }
        Equation::When(c, b, elses) => {
            collect_vars_expr_local(c, vars);
            for sub in b {
                collect_vars_eq_local(sub, vars);
            }
            for (ec, eb) in elses {
                collect_vars_expr_local(ec, vars);
                for sub in eb {
                    collect_vars_eq_local(sub, vars);
                }
            }
        }
        Equation::Reinit(_, e) => {
            collect_vars_expr_local(e, vars);
        }
        Equation::Connect(a, b) => {
            collect_vars_expr_local(a, vars);
            collect_vars_expr_local(b, vars);
        }
        Equation::SolvableBlock { unknowns, residuals, .. } => {
            for u in unknowns {
                vars.insert(u.clone());
            }
            for r in residuals {
                collect_vars_expr_local(r, vars);
            }
        }
        Equation::Assert(cond, msg) => {
            collect_vars_expr_local(cond, vars);
            collect_vars_expr_local(msg, vars);
        }
        Equation::Terminate(msg) => {
            collect_vars_expr_local(msg, vars);
        }
        Equation::If(c, then_eqs, elseif_list, else_eqs) => {
            collect_vars_expr_local(c, vars);
            for sub in then_eqs { collect_vars_eq_local(sub, vars); }
            for (ec, eb) in elseif_list {
                collect_vars_expr_local(ec, vars);
                for sub in eb { collect_vars_eq_local(sub, vars); }
            }
            if let Some(eqs) = else_eqs {
                for sub in eqs { collect_vars_eq_local(sub, vars); }
            }
        }
        Equation::MultiAssign(lhss, rhs) => {
            for e in lhss {
                collect_vars_expr_local(e, vars);
            }
            collect_vars_expr_local(rhs, vars);
        }
    }
}

pub fn print_backend_dae_info(
    opts: &CompilerOptions,
    differential_index: u32,
    total_equations: usize,
    total_declarations: usize,
    state_vars: &[String],
    discrete_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    clocked_vars: &HashSet<String>,
    _continuous_eqs: &[Equation],
    sorted_eqs: &[Equation],
    all_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
    strong_component_jacobians: bool,
    symbolic_ode_jacobian: bool,
    numeric_ode_jacobian: bool,
    ode_jacobian_symbolic: Option<&Vec<Vec<Expression>>>,
    ode_jacobian_sparse: Option<&SparseOdeJacobian>,
    simulation_dae: Option<&SimulationDae>,
) {
    let mut _simple = 0usize;
    let mut _for_eq = 0usize;
    let mut when_eq = 0usize;
    let mut _reinit_eq = 0usize;
    let mut _connect_eq = 0usize;
    let mut _solvable_block = 0usize;
    for eq in all_equations {
        match eq {
            Equation::Simple(_, _) => _simple += 1,
            Equation::For(_, _, _, _) => _for_eq += 1,
            Equation::When(_, _, _) => when_eq += 1,
            Equation::Reinit(_, _) => _reinit_eq += 1,
            Equation::Connect(_, _) => _connect_eq += 1,
            Equation::SolvableBlock { .. } => _solvable_block += 1,
            Equation::If(_, _, _, _) => {}
            Equation::Assert(_, _) | Equation::Terminate(_) => {}
            Equation::MultiAssign(_, _) => {}
        }
    }
    let mut sorted_simple = 0usize;
    let mut sorted_for = 0usize;
    let mut sorted_block = 0usize;
    let mut tearing_var_count = 0usize;
    let mut torn_unknowns_total = 0usize;
    let mut block_sizes: Vec<usize> = Vec::new();
    let mut block_residual_sizes: Vec<usize> = Vec::new();
    for eq in sorted_eqs {
        match eq {
            Equation::Simple(_, _) => sorted_simple += 1,
            Equation::For(_, _, _, _) => sorted_for += 1,
            Equation::SolvableBlock { unknowns, tearing_var, residuals, .. } => {
                sorted_block += 1;
                torn_unknowns_total += unknowns.len();
                block_sizes.push(unknowns.len());
                block_residual_sizes.push(residuals.len());
                if tearing_var.is_some() {
                    tearing_var_count += 1;
                }
            }
            Equation::If(_, _, _, _) => {}
            _ => {}
        }
    }

    let mut var_eq_counts: HashMap<String, usize> = HashMap::new();
    for eq in all_equations {
        let mut vars = HashSet::new();
        collect_vars_eq_local(eq, &mut vars);
        for v in vars {
            *var_eq_counts.entry(v).or_insert(0) += 1;
        }
    }
    let mut min_eq_count = usize::MAX;
    let mut max_eq_count = 0usize;
    let mut sum_eq_count = 0usize;
    let mut counted_vars = 0usize;
    for (_v, c) in &var_eq_counts {
        min_eq_count = min_eq_count.min(*c);
        max_eq_count = max_eq_count.max(*c);
        sum_eq_count += *c;
        counted_vars += 1;
    }

    let states_list = if state_vars.is_empty() {
        " ()".to_string()
    } else {
        format!(" ({})", state_vars.join(","))
    };
    let discrete_list = if discrete_vars.is_empty() {
        " ()".to_string()
    } else {
        format!(" ({})", discrete_vars.join(","))
    };
    let clocked_state_names: Vec<String> = state_vars
        .iter()
        .filter(|v| clocked_vars.contains(*v))
        .cloned()
        .collect();
    let clocked_list = if clocked_state_names.is_empty() {
        " ()".to_string()
    } else {
        format!(" ({})", clocked_state_names.join(","))
    };

    println!("\n{}", i18n::msg0("notification_frontend"));
    println!("{}", i18n::msg("number_of_equations", &[&total_equations]));
    println!("{}", i18n::msg("number_of_variables", &[&total_declarations]));

    if let Some(dae) = simulation_dae {
        println!("{}", i18n::msg0("notification_dae_form"));
        println!("{}", i18n::msg("states_x", &[&dae.dae.variables.state_count()]));
        println!("{}", i18n::msg("derivatives", &[&dae.dae.variables.derivative_count()]));
        println!("{}", i18n::msg("algebraic_z", &[&dae.dae.variables.algebraic_count()]));
        println!("{}", i18n::msg("inputs_u", &[&dae.dae.variables.input_count()]));
        println!("{}", i18n::msg("discrete", &[&dae.dae.variables.discrete_count()]));
        println!("{}", i18n::msg("parameters_count", &[&dae.dae.variables.parameter_count()]));
        println!("{}", i18n::msg("simulation_equations", &[&dae.dae.total_equation_count]));
        println!("{}", i18n::msg("initial_equations", &[&dae.initial.equation_count]));
        if dae.dae.differential_index > 1 {
            println!("{}", i18n::msg("constraint_equations", &[&dae.dae.constraint_equation_count]));
        }
        let (single_b, torn_b, mixed_b) = dae.dae.blocks.iter().fold((0usize, 0usize, 0usize), |(s, t, m), b| {
            match b.block_type {
                BlockType::Single => (s + 1, t, m),
                BlockType::Torn => (s, t + 1, m),
                BlockType::Mixed => (s, t, m + 1),
            }
        });
        println!("{}", i18n::msg("blocks_partitioning", &[&single_b as &dyn std::fmt::Display, &torn_b as &dyn std::fmt::Display, &mixed_b as &dyn std::fmt::Display]));
    }

    println!("{}", i18n::msg0("notification_backend"));
    println!("{}", i18n::msg0("independent_subsystems"));
    println!("{}", i18n::msg("number_of_states", &[&state_vars.len(), &states_list]));
    println!("{}", i18n::msg("number_of_discrete", &[&discrete_vars.len(), &discrete_list]));
    println!("{}", i18n::msg("clocked_states", &[&clocked_state_names.len(), &clocked_list]));
    if let Some(dae) = simulation_dae {
        if !dae.dae.clock_partitions.is_empty() {
            println!("{}", i18n::msg("clock_partitions_count", &[&dae.dae.clock_partitions.len()]));
            for p in &dae.dae.clock_partitions {
                let mut names: Vec<&String> = p.var_names.iter().collect();
                names.sort();
                let list = if names.is_empty() {
                    " ()".to_string()
                } else {
                    format!(" ({})", names.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(","))
                };
                println!("{}", i18n::msg("clock_partition_vars", &[&p.id, &list]));
            }
        }
    }
    println!("{}", i18n::msg0("top_level_inputs"));

    let strong_total = sorted_simple + sorted_for + sorted_block;
    println!("{}", i18n::msg("notification_strong", &[&strong_total]));
    println!("{}", i18n::msg("single_equations", &[&sorted_simple]));
    println!("{}", i18n::msg0("array_equations"));
    println!("{}", i18n::msg("algorithm_blocks", &[&algorithms.len()]));
    println!("{}", i18n::msg0("record_equations"));
    println!("{}", i18n::msg("when_equations", &[&when_eq]));
    println!("{}", i18n::msg0("if_equations"));
    println!("{}", i18n::msg0("equation_systems"));
    println!("{}", i18n::msg("torn_equation_systems", &[&sorted_block]));
    println!("{}", i18n::msg0("mixed_systems"));

    println!("{}", i18n::msg0("notification_backend_details"));
    println!("{}", i18n::msg("parameters_count", &[&param_vars.len()]));
    println!("{}", i18n::msg("outputs", &[&output_vars.len()]));
    println!("{}", i18n::msg("index_reduction_method", &[&opts.index_reduction_method]));
    println!("{}", i18n::msg("differential_index", &[&differential_index]));
    println!("{}", i18n::msg("tearing_method", &[&opts.tearing_method]));
    println!("{}", i18n::msg("tearing_variables_selected", &[&tearing_var_count]));
    println!("{}", i18n::msg("torn_unknowns_total", &[&torn_unknowns_total]));
    if !block_sizes.is_empty() {
        let min_block = *block_sizes.iter().min().unwrap_or(&0);
        let max_block = *block_sizes.iter().max().unwrap_or(&0);
        println!("{}", i18n::msg("block_unknowns_min_max", &[&min_block, &max_block]));
    }
    if !block_residual_sizes.is_empty() {
        let min_res = *block_residual_sizes.iter().min().unwrap_or(&0);
        let max_res = *block_residual_sizes.iter().max().unwrap_or(&0);
        println!("{}", i18n::msg("block_residuals_min_max", &[&min_res, &max_res]));
    }
    if counted_vars > 0 {
        let avg = sum_eq_count as f64 / counted_vars as f64;
        let avg_fmt = format!("{:.2}", avg);
        println!("{}", i18n::msg("vars_with_equations", &[&counted_vars]));
        println!("{}", i18n::msg("equations_per_var", &[&min_eq_count, &max_eq_count, &avg_fmt as &dyn std::fmt::Display]));
    }
    println!("{}", i18n::msg("generate_dynamic_jacobian", &[&opts.generate_dynamic_jacobian]));
    let scj = if strong_component_jacobians { i18n::msg0("yes") } else { i18n::msg0("no") };
    println!("{}", i18n::msg("strong_component_jacobians", &[&scj]));
    let soj = if symbolic_ode_jacobian { i18n::msg0("yes") } else { i18n::msg0("no") };
    println!("{}", i18n::msg("symbolic_ode_jacobian", &[&soj]));
    let noj = if numeric_ode_jacobian { i18n::msg0("yes") } else { i18n::msg0("no") };
    println!("{}", i18n::msg("numeric_ode_jacobian", &[&noj]));
    if let Some(jac) = ode_jacobian_symbolic {
        println!("{}", i18n::msg("symbolic_jacobian_size", &[&jac.len(), &state_vars.len()]));
        for (i, row) in jac.iter().enumerate() {
            let sv = state_vars.get(i).map(|s| s.as_str()).unwrap_or("?");
            print!("   d(der({}))/d(x):", sv);
            for expr in row.iter() {
                print!(" {:?}", expr);
            }
            println!();
        }
    }
    if let Some(sparse) = ode_jacobian_sparse {
        println!("{}", i18n::msg("ode_jacobian_sparsity", &[&sparse.nnz(), &(sparse.n * sparse.n), &format!("{:.2}%", sparse.density() * 100.0)]));
    }
    println!();
}
