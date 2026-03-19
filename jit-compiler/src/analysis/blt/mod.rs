use crate::ast::{Equation, Expression, Operator};
use std::collections::{HashMap, HashSet};

use crate::analysis::derivative::collect_states_from_eq;
use crate::analysis::expression_utils::{make_binary, time_derivative};
use crate::analysis::variable_collection::{
    collect_vars_expr, contains_var, equation_contains_var, extract_unknowns,
};
use crate::analysis::AnalysisOptions;

mod blt_alias;
mod blt_expr;

pub(crate) use blt_alias::eliminate_aliases;
use blt_expr::{
    make_residual, select_tearing_variable, simplify_expr, solve_for_variable,
    solve_residual_linear, substitute_der_in_expr,
};

fn try_index_reduction(
    equations: &[Equation],
    assigned_var: &[Option<usize>],
    _assigned_eq: &[Option<usize>],
    unknown_list: &[String],
    state_vars: &[String],
) -> Option<Vec<Equation>> {
    let mut der_map: HashMap<String, Expression> = HashMap::new();
    for eq in equations.iter() {
        if let Equation::Simple(lhs, rhs) = eq {
            let entry = match lhs {
                Expression::Variable(l) if l.starts_with("der_") => Some((l.clone(), rhs.clone())),
                Expression::Der(inner) => {
                    if let Expression::Variable(v) = inner.as_ref() {
                        Some((format!("der_{}", v), rhs.clone()))
                    } else {
                        None
                    }
                }
                Expression::BinaryOp(coeff, Operator::Mul, r) => {
                    let (der_name, div_by) = if let Expression::Variable(n) = &**r {
                        if n.starts_with("der_") {
                            (Some(n.clone()), Some(coeff.clone()))
                        } else {
                            (None, None)
                        }
                    } else if let Expression::Variable(n) = &**coeff {
                        if n.starts_with("der_") {
                            (Some(n.clone()), Some(r.clone()))
                        } else {
                            (None, None)
                        }
                    } else if let Expression::BinaryOp(c2, Operator::Mul, r2) = r.as_ref() {
                        if let Expression::Variable(n) = &**r2 {
                            if n.starts_with("der_") {
                                (
                                    Some(n.clone()),
                                    Some(Box::new(Expression::BinaryOp(
                                        coeff.clone(),
                                        Operator::Mul,
                                        c2.clone(),
                                    ))),
                                )
                            } else {
                                (None, None)
                            }
                        } else if let Expression::Variable(n) = &**c2 {
                            if n.starts_with("der_") {
                                (
                                    Some(n.clone()),
                                    Some(Box::new(Expression::BinaryOp(
                                        coeff.clone(),
                                        Operator::Mul,
                                        r2.clone(),
                                    ))),
                                )
                            } else {
                                (None, None)
                            }
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    };
                    match (der_name, div_by) {
                        (Some(name), Some(div_by)) => Some((
                            name,
                            Expression::BinaryOp(Box::new(rhs.clone()), Operator::Div, div_by),
                        )),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some((name, expr)) = entry {
                der_map.insert(name, expr);
            }
        }
    }
    let unassigned: Vec<usize> = assigned_var
        .iter()
        .enumerate()
        .filter_map(|(i, o)| if o.is_none() { Some(i) } else { None })
        .collect();
    for eq_idx in unassigned {
        let eq = &equations[eq_idx];
        let (is_constraint, residual) = match eq {
            Equation::Simple(lhs, rhs) => {
                let mut lhs_vars = HashSet::new();
                collect_vars_expr(lhs, &mut lhs_vars);
                let lhs_has_der = lhs_vars.iter().any(|v| v.starts_with("der_"))
                    || matches!(lhs, Expression::Der(_))
                    || matches!(lhs, Expression::Variable(n) if n.starts_with("der_"));
                (
                    !lhs_has_der,
                    make_binary(lhs.clone(), Operator::Sub, rhs.clone()),
                )
            }
            _ => continue,
        };
        if !is_constraint {
            continue;
        }
        let mut diff_expr = time_derivative(&residual, state_vars);
        diff_expr = substitute_der_in_expr(&diff_expr, &der_map);
        let mut alg_vars: Vec<&String> = unknown_list
            .iter()
            .filter(|u| !u.starts_with("der_") && !state_vars.iter().any(|s| s == *u))
            .collect();
        alg_vars.sort_by_key(|v| {
            equations
                .iter()
                .filter(|eq| equation_contains_var(eq, v))
                .count()
        });
        for alg_var in &alg_vars {
            if contains_var(&diff_expr, alg_var) {
                if let Some(sol) = solve_residual_linear(&diff_expr, alg_var) {
                    let mut new_eqs = equations.to_vec();
                    new_eqs[eq_idx] =
                        Equation::Simple(Expression::Variable((*alg_var).clone()), sol);
                    return Some(new_eqs);
                }
            }
        }
        let diff2 = time_derivative(&diff_expr, state_vars);
        let diff2_sub_raw = substitute_der_in_expr(&diff2, &der_map);
        let diff2_sub = simplify_expr(&diff2_sub_raw);
        for alg_var in &alg_vars {
            if let Some(sol) = solve_residual_linear(&diff2_sub, alg_var) {
                let mut new_eqs = equations.to_vec();
                new_eqs[eq_idx] = Equation::Simple(Expression::Variable((*alg_var).clone()), sol);
                return Some(new_eqs);
            }
        }
    }
    None
}

fn eval_const_expr(expr: &Expression) -> Option<f64> {
    match expr {
        Expression::Number(n) => Some(*n),
        Expression::BinaryOp(lhs, Operator::Add, rhs) => {
            Some(eval_const_expr(lhs)? + eval_const_expr(rhs)?)
        }
        Expression::BinaryOp(lhs, Operator::Sub, rhs) => {
            Some(eval_const_expr(lhs)? - eval_const_expr(rhs)?)
        }
        Expression::BinaryOp(lhs, Operator::Mul, rhs) => {
            Some(eval_const_expr(lhs)? * eval_const_expr(rhs)?)
        }
        Expression::BinaryOp(lhs, Operator::Div, rhs) => {
            let denom = eval_const_expr(rhs)?;
            if denom.abs() < 1e-15 {
                None
            } else {
                Some(eval_const_expr(lhs)? / denom)
            }
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct SortAlgebraicResult {
    pub sorted_equations: Vec<Equation>,
    pub differential_index: u32,
    pub constraint_equation_count: usize,
    pub constant_conflict_count: usize,
    pub alias_map: HashMap<String, Expression>,
}

pub fn sort_algebraic_equations(
    equations: Vec<Equation>,
    known_vars: &HashSet<String>,
    params: &[String],
    options: &AnalysisOptions,
) -> SortAlgebraicResult {
    fn reorder_simple_variable_equations(
        equations: Vec<Equation>,
        known_vars: &HashSet<String>,
        params: &[String],
    ) -> Vec<Equation> {
        if !equations.iter().all(|eq| matches!(eq, Equation::Simple(Expression::Variable(_), _))) {
            return equations;
        }
        let mut ready_known = known_vars.clone();
        for p in params {
            ready_known.insert(p.clone());
        }
        ready_known.insert("time".to_string());

        let mut pending = equations;
        let mut reordered = Vec::new();
        loop {
            let mut progressed = false;
            let mut remaining = Vec::new();
            for eq in pending {
                match &eq {
                    Equation::Simple(Expression::Variable(lhs), rhs) => {
                        let mut rhs_vars = HashSet::new();
                        collect_vars_expr(rhs, &mut rhs_vars);
                        rhs_vars.remove(lhs);
                        if rhs_vars.iter().all(|v| ready_known.contains(v)) {
                            ready_known.insert(lhs.clone());
                            reordered.push(eq);
                            progressed = true;
                        } else {
                            remaining.push(eq);
                        }
                    }
                    _ => remaining.push(eq),
                }
            }
            if !progressed {
                reordered.extend(remaining);
                break;
            }
            pending = remaining;
            if pending.is_empty() {
                break;
            }
        }
        reordered
    }

    let blt_trace = std::env::var("RUSTMODLICA_BLT_TRACE")
        .ok()
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false);
    if blt_trace {
        eprintln!("[blt] start");
    }
    let mut current_known = known_vars.clone();
    for p in params {
        current_known.insert(p.clone());
    }
    current_known.insert("time".to_string());

    if blt_trace {
        eprintln!("[blt] eliminate_aliases");
    }
    let (equations, alias_map): (Vec<Equation>, HashMap<String, Expression>) =
        eliminate_aliases(equations);

    let n_aliases = alias_map.len();
    let n_eqs = equations.len();
    if !options.quiet {
        println!(
            "{}",
            crate::i18n::msg(
                "aliases_eliminated",
                &[&n_aliases as &dyn std::fmt::Display]
            )
        );
        println!(
            "{}",
            crate::i18n::msg("remaining_equations", &[&n_eqs as &dyn std::fmt::Display])
        );
    }

    let mut equations = equations;

    let mut state_set = HashSet::new();
    for eq in equations.iter() {
        collect_states_from_eq(eq, &mut state_set);
    }
    let state_vars: Vec<String> = state_set.into_iter().collect();
    if blt_trace {
        eprintln!("[blt] build_eq_info");
    }
    if options.index_reduction_method == "debugPrint" && !state_vars.is_empty() {
        for eq in equations.iter() {
            if let Equation::Simple(lhs, rhs) = eq {
                let is_ode = matches!(lhs, Expression::Der(_));
                if !is_ode {
                    let residual = Expression::BinaryOp(
                        Box::new(lhs.clone()),
                        Operator::Sub,
                        Box::new(rhs.clone()),
                    );
                    let dt = time_derivative(&residual, &state_vars);
                    eprintln!(
                        "[debugPrint] time_derivative of constraint residual: {:?}",
                        dt
                    );
                    break;
                }
            }
        }
    }

    struct EqInfo {
        original_idx: usize,
        unknowns: Vec<String>,
    }

    fn dfs_iter(
        u_start: usize,
        adj: &[Vec<usize>],
        assigned_eq: &mut [Option<usize>],
        visited: &mut [bool],
    ) -> bool {
        let mut stack: Vec<(usize, usize, Option<usize>)> = vec![(u_start, 0, None)];
        while let Some((u, idx, var_opt)) = stack.last_mut() {
            if *idx >= adj[*u].len() {
                if let Some(v) = *var_opt {
                    visited[v] = false;
                }
                stack.pop();
                continue;
            }
            let v = adj[*u][*idx];
            *idx += 1;
            if visited[v] {
                continue;
            }
            visited[v] = true;
            if assigned_eq[v].is_none() {
                assigned_eq[v] = Some(*u);
                while let Some((eq, _, pop_var)) = stack.pop() {
                    if let Some(var) = pop_var {
                        assigned_eq[var] = Some(eq);
                    }
                }
                return true;
            }
            let next_eq = assigned_eq[v].unwrap();
            stack.push((next_eq, 0, Some(v)));
        }
        false
    }

    let mut differential_index: u32;
    let mut eq_infos = Vec::new();
    let mut unknown_list: Vec<String>;
    let mut unknown_map: HashMap<String, usize>;
    let mut assigned_var: Vec<Option<usize>>;
    let mut assigned_eq: Vec<Option<usize>>;

    loop {
        eq_infos.clear();
        let mut all_unknowns: HashSet<String> = HashSet::new();
        for (i, eq) in equations.iter().enumerate() {
            let unknowns = extract_unknowns(eq, &current_known);
            for u in &unknowns {
                all_unknowns.insert(u.clone());
            }
            eq_infos.push(EqInfo {
                original_idx: i,
                unknowns,
            });
        }

        unknown_list = all_unknowns.into_iter().collect();
        unknown_map = unknown_list
            .iter()
            .enumerate()
            .map(|(i, u)| (u.clone(), i))
            .collect();

        assigned_var = vec![None; eq_infos.len()];
        assigned_eq = vec![None; unknown_list.len()];

        if blt_trace {
            eprintln!(
                "[blt] matching setup eqs={} unknowns={}",
                eq_infos.len(),
                unknown_list.len()
            );
        }
        for (i, info) in eq_infos.iter().enumerate() {
            let eq = &equations[info.original_idx];
            if let Equation::Simple(lhs, _) = eq {
                let mut target_var = None;
                if let Expression::Variable(v) = lhs {
                    target_var = Some(v.clone());
                } else if let Expression::Der(inner) = lhs {
                    if let Expression::Variable(v) = inner.as_ref() {
                        target_var = Some(format!("der_{}", v));
                    }
                }
                if let Some(v) = target_var {
                    if let Some(&v_idx) = unknown_map.get(&v) {
                        if assigned_eq[v_idx].is_none() {
                            assigned_eq[v_idx] = Some(i);
                            assigned_var[i] = Some(v_idx);
                        }
                    }
                }
            }
        }

        let mut adj = vec![Vec::new(); eq_infos.len()];
        for (i, info) in eq_infos.iter().enumerate() {
            for u in &info.unknowns {
                if let Some(&v_idx) = unknown_map.get(u) {
                    adj[i].push(v_idx);
                }
            }
        }

        for i in 0..eq_infos.len() {
            if assigned_var[i].is_some() {
                continue;
            }
            let mut visited = vec![false; unknown_list.len()];
            if dfs_iter(i, &adj, &mut assigned_eq, &mut visited) {}
        }

        for (v_idx, eq_opt) in assigned_eq.iter().enumerate() {
            if let Some(eq_idx) = eq_opt {
                assigned_var[*eq_idx] = Some(v_idx);
            }
        }

        differential_index = if assigned_var.iter().any(|o| o.is_none()) {
            2
        } else {
            1
        };
        if differential_index == 1 {
            break;
        }
        if options.index_reduction_method == "none" {
            break;
        }
        if let Some(new_eqs) = try_index_reduction(
            &equations,
            &assigned_var,
            &assigned_eq,
            &unknown_list,
            &state_vars,
        ) {
            equations = new_eqs;
        } else {
            break;
        }
    }

    let constraint_equation_count = assigned_var.iter().filter(|o| o.is_none()).count();
    let constant_conflict_count = eq_infos
        .iter()
        .filter(|info| info.unknowns.is_empty())
        .filter(|info| {
            let residual = make_residual(&equations[info.original_idx]);
            eval_const_expr(&residual)
                .map(|value| value.abs() >= 1e-12)
                .unwrap_or(false)
        })
        .count();

    if differential_index == 2
        && state_vars.is_empty()
        && !unknown_list.is_empty()
        && unknown_list.len() == equations.len()
        && equations.iter().all(|eq| matches!(eq, Equation::Simple(_, _)))
        && eq_infos.iter().all(|info| !info.unknowns.is_empty())
    {
        let tearing_var =
            select_tearing_variable(&unknown_list, &equations, &unknown_map, &options.tearing_method);
        return SortAlgebraicResult {
            sorted_equations: vec![Equation::SolvableBlock {
                unknowns: unknown_list.clone(),
                tearing_var,
                equations: vec![],
                residuals: equations.iter().map(|eq| make_residual(eq)).collect(),
            }],
            differential_index: 1,
            constraint_equation_count: 0,
            constant_conflict_count,
            alias_map,
        };
    }

    if blt_trace {
        eprintln!("[blt] build_dependency_graph");
    }
    let n_nodes = eq_infos.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n_nodes];
    let mut radj: Vec<Vec<usize>> = vec![Vec::new(); n_nodes];

    for (i, info) in eq_infos.iter().enumerate() {
        for u in &info.unknowns {
            if let Some(&v_idx) = unknown_map.get(u) {
                if Some(v_idx) == assigned_var[i] {
                    continue;
                }
                if let Some(solver_eq_idx) = assigned_eq[v_idx] {
                    if i < n_nodes && solver_eq_idx < n_nodes {
                        adj[i].push(solver_eq_idx);
                        radj[solver_eq_idx].push(i);
                    }
                }
            }
        }
    }
    for i in 0..eq_infos.len() {
        if assigned_var[i].is_some() {
            continue;
        }
        for j in (i + 1)..eq_infos.len() {
            if assigned_var[j].is_some() {
                continue;
            }
            let shared = eq_infos[i]
                .unknowns
                .iter()
                .any(|u| eq_infos[j].unknowns.contains(u));
            if shared {
                adj[i].push(j);
                radj[j].push(i);
                adj[j].push(i);
                radj[i].push(j);
            }
        }
    }

    if blt_trace {
        eprintln!("[blt] scc");
    }
    // Iterative SCC to avoid stack overflow on large graphs.
    // Kosaraju: order by finish time on adj, then DFS on reversed graph.
    let mut visited = vec![false; n_nodes];
    let mut order: Vec<usize> = Vec::with_capacity(n_nodes);
    for start in 0..n_nodes {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some((v, next_i)) = stack.pop() {
            if next_i < adj[v].len() {
                // Put back current with advanced iterator
                stack.push((v, next_i + 1));
                let to = adj[v][next_i];
                if !visited[to] {
                    visited[to] = true;
                    stack.push((to, 0));
                }
            } else {
                order.push(v);
            }
        }
    }

    let mut visited2 = vec![false; n_nodes];
    let mut sccs: Vec<Vec<usize>> = Vec::new();
    for &v in order.iter().rev() {
        if visited2[v] {
            continue;
        }
        visited2[v] = true;
        let mut comp: Vec<usize> = Vec::new();
        let mut stack: Vec<usize> = vec![v];
        while let Some(x) = stack.pop() {
            comp.push(x);
            for &to in &radj[x] {
                if !visited2[to] {
                    visited2[to] = true;
                    stack.push(to);
                }
            }
        }
        sccs.push(comp);
    }

    let mut sorted_equations = Vec::new();

    if blt_trace {
        eprintln!("[blt] solve_blocks");
    }
    for scc in sccs {
        let block_indices: Vec<usize> = scc;

        if block_indices.is_empty() {
            continue;
        }

        if block_indices.len() == 1 {
            let idx = block_indices[0];
            let eq = &equations[eq_infos[idx].original_idx];

            if let Some(var_idx) = assigned_var[idx] {
                let var_name = &unknown_list[var_idx];
                if let Some(expr) = solve_for_variable(eq, var_name) {
                    current_known.insert(var_name.clone());
                    sorted_equations.push(Equation::Simple(
                        Expression::Variable(var_name.clone()),
                        expr,
                    ));
                } else {
                    let tearing_var = select_tearing_variable(
                        &[var_name.clone()],
                        &[eq.clone()],
                        &unknown_map,
                        &options.tearing_method,
                    );
                    sorted_equations.push(Equation::SolvableBlock {
                        unknowns: vec![var_name.clone()],
                        tearing_var,
                        equations: vec![],
                        residuals: vec![make_residual(eq)],
                    });
                    current_known.insert(var_name.clone());
                }
            } else {
                let unknowns_from_eq = &eq_infos[idx].unknowns;
                let unk = unknowns_from_eq
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "__dummy".to_string());
                let tearing_var = select_tearing_variable(
                    &[unk.clone()],
                    &[eq.clone()],
                    &unknown_map,
                    &options.tearing_method,
                );
                sorted_equations.push(Equation::SolvableBlock {
                    unknowns: vec![unk],
                    tearing_var,
                    equations: vec![],
                    residuals: vec![make_residual(eq)],
                });
            }
        } else {
            let block_eqs: Vec<Equation> = block_indices
                .iter()
                .map(|&idx| equations[eq_infos[idx].original_idx].clone())
                .collect();

            let mut block_unknowns: Vec<String> = block_indices
                .iter()
                .filter_map(|&idx| assigned_var[idx].map(|v_idx| unknown_list[v_idx].clone()))
                .collect();

            if block_unknowns.is_empty() && !block_eqs.is_empty() {
                for &idx in &block_indices {
                    for u in &eq_infos[idx].unknowns {
                        if unknown_map.contains_key(u) && !block_unknowns.contains(u) {
                            block_unknowns.push(u.clone());
                        }
                    }
                }
            }
            let tearing_var = select_tearing_variable(
                &block_unknowns,
                &block_eqs,
                &unknown_map,
                &options.tearing_method,
            );

            sorted_equations.push(Equation::SolvableBlock {
                unknowns: block_unknowns.clone(),
                tearing_var,
                equations: vec![],
                residuals: block_eqs.iter().map(|eq| make_residual(eq)).collect(),
            });

            for u in block_unknowns {
                current_known.insert(u);
            }
        }
    }

    let sorted_equations = reorder_simple_variable_equations(sorted_equations, known_vars, params);

    SortAlgebraicResult {
        sorted_equations,
        differential_index,
        constraint_equation_count,
        constant_conflict_count,
        alias_map,
    }
}
