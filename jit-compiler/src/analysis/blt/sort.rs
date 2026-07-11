use crate::ast::{Equation, Expression, Operator};
use crate::string_intern::resolve_id;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::analysis::derivative::collect_states_from_eq;
use crate::analysis::expression_utils::time_derivative;
use crate::analysis::variable_collection::{collect_vars_expr, extract_unknowns};
use crate::analysis::AnalysisOptions;

use super::blt_expr::{make_residual, select_tearing_variable, solve_for_variable};
use super::helpers::{eval_const_expr, try_index_reduction};
use super::types::{BlockCausalityInfo, SortAlgebraicResult};
use super::eliminate_aliases;

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
                    Equation::Simple(Expression::Variable(id), rhs) => {
                        let lhs_name = resolve_id(*id);
                        let mut rhs_vars = HashSet::new();
                        collect_vars_expr(rhs, &mut rhs_vars);
                        rhs_vars.remove(&lhs_name);
                        if rhs_vars.iter().all(|v| ready_known.contains(v)) {
                            ready_known.insert(lhs_name);
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
    let sort_started_at = Instant::now();
    let dense_share_max_n = std::env::var("RUSTMODLICA_BLT_SHARE_EDGE_MAX_N")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(48);
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

    // Performance degradation guard: for larger algebraic systems, skip full BLT sorting
    // and emit a single SolvableBlock to keep compilation bounded. Default raised from 63
    // to 256; most models benefit from BLT. Override via RUSTMODLICA_BLT_MAX_EQ_FOR_SORT.
    let max_eq_for_full_sort = std::env::var("RUSTMODLICA_BLT_MAX_EQ_FOR_SORT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(256);
    if equations.len() > max_eq_for_full_sort {
        let mut unknown_set: HashSet<String> = HashSet::new();
        for eq in &equations {
            for u in extract_unknowns(eq, &current_known) {
                unknown_set.insert(u);
            }
        }
        let mut unknowns: Vec<String> = unknown_set.into_iter().collect();
        unknowns.sort();
        let tearing_var = unknowns.first().cloned();
        if blt_trace {
            eprintln!(
                "[blt] degrade_guard full_sort_skipped eqs={} limit={} unknowns={} elapsed_ms={}",
                equations.len(),
                max_eq_for_full_sort,
                unknowns.len(),
                sort_started_at.elapsed().as_millis()
            );
        }
        return SortAlgebraicResult {
            sorted_equations: vec![Equation::SolvableBlock {
                unknowns,
                tearing_var: tearing_var.clone(),
                equations: vec![],
                residuals: equations.iter().map(make_residual).collect(),
            }],
            differential_index: 1,
            constraint_equation_count: 0,
            constant_conflict_count: 0,
            alias_map,
            index_reduction_rounds: 0,
            dummy_derivative_equation_count: 0,
            tearing_block_count: 1,
            tearing_residual_equation_count: equations.len(),
            block_causality: vec![BlockCausalityInfo {
                diff_index: 1,
                tearing_vars: tearing_var.clone().into_iter().collect(),
                strongly_connected: true,
                is_nonlinear: true,
            }],
            blt_degrade_guard_triggered: true,
            blt_degrade_guard_limit: Some(max_eq_for_full_sort),
            blt_degrade_guard_equation_count: Some(equations.len()),
        };
    }

    let mut equations = equations;

    let mut state_set = HashSet::new();
    for eq in equations.iter() {
        collect_states_from_eq(eq, &mut state_set);
    }
    let mut state_vars: Vec<String> = state_set.into_iter().collect();
    state_vars.sort();
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
        // Iterative Kuhn augmenting-path search. Each stack frame carries the
        // reassignment to apply on success as (variable, parent equation): when
        // we descend into frame `child` through variable `v` (currently held by
        // `child`), on success `v` must be rematched to the PARENT equation that
        // reached it, not to `child`. Binding `v` to the current frame's own
        // equation (the previous bug) double-bound the child and left the start
        // equation unmatched, reporting a spurious high differential index.
        let mut stack: Vec<(usize, usize, Option<(usize, usize)>)> =
            vec![(u_start, 0, None)];
        while let Some((u, idx, reassign)) = stack.last_mut() {
            if *idx >= adj[*u].len() {
                if let Some((v, _)) = *reassign {
                    visited[v] = false;
                }
                stack.pop();
                continue;
            }
            let cur_eq = *u;
            let v = adj[cur_eq][*idx];
            *idx += 1;
            if visited[v] {
                continue;
            }
            visited[v] = true;
            if assigned_eq[v].is_none() {
                assigned_eq[v] = Some(cur_eq);
                while let Some((_eq, _, reassign)) = stack.pop() {
                    if let Some((var, parent_eq)) = reassign {
                        assigned_eq[var] = Some(parent_eq);
                    }
                }
                return true;
            }
            let next_eq = assigned_eq[v].unwrap();
            stack.push((next_eq, 0, Some((v, cur_eq))));
        }
        false
    }

    let mut differential_index: u32 = 2;
    let mut eq_infos = Vec::new();
    let mut unknown_list: Vec<String> = Vec::new();
    let mut unknown_map: HashMap<String, usize> = HashMap::new();
    let mut assigned_var: Vec<Option<usize>> = Vec::new();
    let mut assigned_eq: Vec<Option<usize>> = Vec::new();

    let max_index_reduction_rounds = std::env::var("RUSTMODLICA_INDEX_REDUCTION_MAX_ROUNDS")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(1, 64))
        .unwrap_or(20);
    let mut round = 0u32;
    let mut index_reduction_rounds = 0u32;
    let mut prev_unassigned_count: Option<usize> = None;
    loop {
        round += 1;
        if round > max_index_reduction_rounds {
            if blt_trace {
                eprintln!(
                    "[blt] index reduction iteration limit ({}) reached, stopping",
                    max_index_reduction_rounds
                );
            }
            break;
        }
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
        unknown_list.sort();
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
                if let Expression::Variable(id) = lhs {
                    target_var = Some(resolve_id(*id));
                } else if let Expression::Der(inner) = lhs {
                    if let Expression::Variable(id) = inner.as_ref() {
                        target_var = Some(format!("der_{}", resolve_id(*id)));
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

        let unassigned_count = assigned_var.iter().filter(|o| o.is_none()).count();
        differential_index = if unassigned_count > 0 { 2 } else { 1 };
        if differential_index == 1 {
            break;
        }
        let index_method = options.index_reduction_method.trim();
        if index_method.eq_ignore_ascii_case("none") {
            break;
        }
        if let Some(prev) = prev_unassigned_count {
            if unassigned_count >= prev {
                if blt_trace {
                    eprintln!(
                        "[blt] index reduction not making progress (unassigned: {} -> {}), stopping",
                        prev, unassigned_count
                    );
                }
                break;
            }
        }
        prev_unassigned_count = Some(unassigned_count);
        if let Some(new_eqs) = try_index_reduction(
            &equations,
            &assigned_var,
            &assigned_eq,
            &unknown_list,
            &state_vars,
            options,
        ) {
            equations = new_eqs;
            index_reduction_rounds = round;
        } else {
            break;
        }
    }

    let dummy_derivative_equation_count = equations
        .iter()
        .filter(|eq| {
            matches!(
                eq,
                Equation::Simple(Expression::Variable(id), _)
                    if resolve_id(*id).starts_with("$dummy_")
            )
        })
        .count();

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
                tearing_var: tearing_var.clone(),
                equations: vec![],
                residuals: equations.iter().map(|eq| make_residual(eq)).collect(),
            }],
            differential_index: 1,
            constraint_equation_count: 0,
            constant_conflict_count,
            alias_map,
            index_reduction_rounds,
            dummy_derivative_equation_count,
            tearing_block_count: 1,
            tearing_residual_equation_count: equations.len(),
            block_causality: vec![BlockCausalityInfo {
                diff_index: 1,
                tearing_vars: tearing_var.clone().into_iter().collect(),
                strongly_connected: true,
                is_nonlinear: true,
            }],
            blt_degrade_guard_triggered: false,
            blt_degrade_guard_limit: Some(max_eq_for_full_sort),
            blt_degrade_guard_equation_count: Some(equations.len()),
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
    // Guard against O(n^2) dense-share edge construction on larger systems.
    if eq_infos.len() <= dense_share_max_n {
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
    } else if blt_trace {
        eprintln!(
            "[blt] skip_dense_share_edges n_nodes={} limit={} elapsed_ms={}",
            eq_infos.len(),
            dense_share_max_n,
            sort_started_at.elapsed().as_millis()
        );
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
    let mut tearing_block_count = 0usize;
    let mut tearing_residual_equation_count = 0usize;
    let mut block_causality: Vec<BlockCausalityInfo> = Vec::new();

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
                        Expression::var(var_name),
                        expr,
                    ));
                    block_causality.push(BlockCausalityInfo {
                        diff_index: differential_index,
                        tearing_vars: Vec::new(),
                        strongly_connected: false,
                        is_nonlinear: false,
                    });
                } else {
                    let tearing_var = select_tearing_variable(
                        &[var_name.clone()],
                        &[eq.clone()],
                        &unknown_map,
                        &options.tearing_method,
                    );
                    sorted_equations.push(Equation::SolvableBlock {
                        unknowns: vec![var_name.clone()],
                        tearing_var: tearing_var.clone(),
                        equations: vec![],
                        residuals: vec![make_residual(eq)],
                    });
                    tearing_block_count += 1;
                    tearing_residual_equation_count += 1;
                    block_causality.push(BlockCausalityInfo {
                        diff_index: differential_index,
                        tearing_vars: tearing_var.clone().into_iter().collect(),
                        strongly_connected: false,
                        is_nonlinear: true,
                    });
                    current_known.insert(var_name.clone());
                }
            } else {
                let unknowns_from_eq = &eq_infos[idx].unknowns;
                if let Some(unk) = unknowns_from_eq.first().cloned() {
                    let tearing_var = select_tearing_variable(
                        &[unk.clone()],
                        &[eq.clone()],
                        &unknown_map,
                        &options.tearing_method,
                    );
                    sorted_equations.push(Equation::SolvableBlock {
                        unknowns: vec![unk],
                        tearing_var: tearing_var.clone(),
                        equations: vec![],
                        residuals: vec![make_residual(eq)],
                    });
                    tearing_block_count += 1;
                    tearing_residual_equation_count += 1;
                    block_causality.push(BlockCausalityInfo {
                        diff_index: differential_index,
                        tearing_vars: tearing_var.clone().into_iter().collect(),
                        strongly_connected: false,
                        is_nonlinear: true,
                    });
                } else {
                    // Keep residual equation without introducing synthetic "__dummy" unknowns.
                    // Pick a real variable from the residual as tearing variable so JIT can
                    // execute the single-residual Newton path.
                    let residual = make_residual(eq);
                    let mut residual_vars: HashSet<String> = HashSet::new();
                    collect_vars_expr(&residual, &mut residual_vars);
                    let mut vars_vec: Vec<String> = residual_vars.into_iter().collect();
                    vars_vec.sort();
                    let tearing_var = vars_vec
                        .iter()
                        .find(|v| !v.starts_with("__dummy"))
                        .cloned()
                        .or_else(|| vars_vec.first().cloned());
                    let unknowns_block = tearing_var.clone().into_iter().collect::<Vec<_>>();
                    sorted_equations.push(Equation::SolvableBlock {
                        unknowns: unknowns_block,
                        tearing_var: tearing_var.clone(),
                        equations: vec![],
                        residuals: vec![residual],
                    });
                    tearing_block_count += 1;
                    tearing_residual_equation_count += 1;
                    block_causality.push(BlockCausalityInfo {
                        diff_index: differential_index,
                        tearing_vars: tearing_var.clone().into_iter().collect(),
                        strongly_connected: false,
                        is_nonlinear: true,
                    });
                }
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
                tearing_var: tearing_var.clone(),
                equations: vec![],
                residuals: block_eqs.iter().map(|eq| make_residual(eq)).collect(),
            });
            tearing_block_count += 1;
            tearing_residual_equation_count += block_eqs.len();
            block_causality.push(BlockCausalityInfo {
                diff_index: differential_index,
                tearing_vars: tearing_var.clone().into_iter().collect(),
                strongly_connected: true,
                is_nonlinear: true,
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
        index_reduction_rounds,
        dummy_derivative_equation_count,
        tearing_block_count,
        tearing_residual_equation_count,
        block_causality,
        blt_degrade_guard_triggered: false,
        blt_degrade_guard_limit: Some(max_eq_for_full_sort),
        blt_degrade_guard_equation_count: Some(equations.len()),
    }
}