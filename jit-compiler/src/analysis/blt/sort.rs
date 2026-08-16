use crate::ast::{Equation, Expression, Operator};
use crate::string_intern::resolve_id;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::analysis::derivative::collect_states_from_eq;
use crate::analysis::expression_utils::time_derivative;
use crate::analysis::variable_collection::{
    collect_simple_lhs_names, collect_vars_eq, collect_vars_expr, expand_array_base_unknowns,
    extract_unknowns,
};
use crate::analysis::AnalysisOptions;

use super::blt_expr::{
    is_forbidden_tearing_var, make_residual, select_tearing_variable, simplify_expr,
};
use super::helpers::{eval_const_expr, try_index_reduction};
use super::sort_graph::{emit_scc_blocks, kosaraju_sccs};
use super::sort_match::{assign_equation_variables, EqInfo};
use super::sort_reorder::reorder_simple_variable_equations;
use super::types::{BlockCausalityInfo, SortAlgebraicResult};

pub fn sort_algebraic_equations(
    equations: Vec<Equation>,
    known_vars: &HashSet<String>,
    params: &[String],
    options: &AnalysisOptions,
) -> SortAlgebraicResult {
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
        super::eliminate_aliases_protected(equations, &current_known);
    let equations = crate::analysis::expand_frames_time_derivatives(
        equations,
        known_vars,
        &current_known,
    );

    // Freeze time/start/PWM clock params that must never become Newton unknowns.
    for eq in &equations {
        let mut vars = HashSet::new();
        collect_vars_eq(eq, &mut vars);
        for v in vars {
            if is_forbidden_tearing_var(&v) {
                current_known.insert(v);
            }
        }
    }

    let n_aliases = alias_map.len();
    let n_eqs = equations.len();
    if blt_trace {
        for (k, v) in &alias_map {
            if k.contains("_R_") || k.contains("_r_0_") {
                eprintln!(
                    "[blt] alias {} -> {}",
                    k,
                    crate::unparse::equation_to_string(&Equation::Simple(
                        Expression::var(k),
                        v.clone()
                    ))
                );
            }
        }
        for eq in &equations {
            let s = crate::unparse::equation_to_string(eq);
            if s.contains("resolve1")
                || s.contains("absoluteRotation")
                || s.contains("angularVelocity")
                || s.contains("cross(")
                || s.contains("rod_frame_a_R")
            {
                eprintln!("[blt] remain {}", s.replace('\n', " | "));
            }
        }
    }
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
    let mut equations = equations;
    let mut take_degrade_guard = equations.len() > max_eq_for_full_sort;
    if take_degrade_guard {
        let mut unknown_set: HashSet<String> = HashSet::new();
        let scalar_lhs = collect_simple_lhs_names(&equations);
        for eq in &equations {
            let mut u = extract_unknowns(eq, &current_known);
            expand_array_base_unknowns(&mut u, &scalar_lhs, &current_known);
            for name in u {
                if !is_forbidden_tearing_var(&name) {
                    unknown_set.insert(name);
                }
            }
        }
        let mut unknowns: Vec<String> = unknown_set.into_iter().collect();
        unknowns.sort();
        let tearing_var =
            select_tearing_variable(&unknowns, &equations, &HashMap::new(), &options.tearing_method);
        if unknowns.is_empty() || tearing_var.is_none() {
            // Do not emit a poison Newton block on time/start-only systems.
            take_degrade_guard = false;
            if blt_trace {
                eprintln!(
                    "[blt] degrade_guard skipped (no tearable unknowns) eqs={} limit={}",
                    equations.len(),
                    max_eq_for_full_sort
                );
            }
        } else {
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
                demoted_states: Vec::new(),
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
    }
    let _ = take_degrade_guard;

    let mut state_set = HashSet::new();
    for eq in equations.iter() {
        collect_states_from_eq(eq, &mut state_set);
    }
    let mut state_vars: Vec<String> = state_set.into_iter().collect();
    state_vars.sort();
    let initial_state_vars = state_vars.clone();
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

    let mut differential_index: u32 = 2;
    let mut eq_infos = Vec::new();
    let mut unknown_list: Vec<String> = Vec::new();
    let mut unknown_map: HashMap<String, usize> = HashMap::new();
    let mut assigned_var: Vec<Option<usize>> = Vec::new();
    let mut assigned_eq: Vec<Option<usize>> = Vec::new();
    let mut last_good_equations: Option<Vec<Equation>> = None;
    let mut last_good_state_vars: Option<Vec<String>> = None;
    let mut restore_and_stop = false;

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
        let scalar_lhs = collect_simple_lhs_names(&equations);
        for (i, eq) in equations.iter().enumerate() {
            let mut unknowns = extract_unknowns(eq, &current_known);
            expand_array_base_unknowns(&mut unknowns, &scalar_lhs, &current_known);
            unknowns.retain(|u| !is_forbidden_tearing_var(u));
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

        if blt_trace {
            eprintln!(
                "[blt] matching setup eqs={} unknowns={}",
                eq_infos.len(),
                unknown_list.len()
            );
        }

        let (av, ae) =
            assign_equation_variables(&equations, &eq_infos, &unknown_map, &unknown_list);
        assigned_var = av;
        assigned_eq = ae;

        let unassigned_count = assigned_var
            .iter()
            .enumerate()
            .filter(|(i, assigned)| {
                if assigned.is_some() {
                    return false;
                }
                let residual =
                    simplify_expr(&make_residual(&equations[eq_infos[*i].original_idx]));
                !matches!(&residual, Expression::Number(n) if n.abs() < 1e-15)
            })
            .count();
        if blt_trace && unassigned_count > 0 {
            for (i, assigned) in assigned_var.iter().enumerate() {
                if assigned.is_some() {
                    continue;
                }
                let residual = make_residual(&equations[eq_infos[i].original_idx]);
                let mut res_vars = HashSet::new();
                collect_vars_expr(&residual, &mut res_vars);
                let mut names: Vec<&str> = res_vars.iter().map(String::as_str).collect();
                names.sort_unstable();
                eprintln!(
                    "[blt] unmatched eq{} info_unknowns={:?} res_vars={}",
                    eq_infos[i].original_idx,
                    eq_infos[i].unknowns,
                    names.join(",")
                );
            }
        }
        // Dummy-derivative can temporarily worsen structural matching; keep the
        // best prior equation set instead of adopting the regressing round.
        if !restore_and_stop {
            if let Some(prev) = prev_unassigned_count {
                if unassigned_count > prev {
                    if blt_trace {
                        eprintln!(
                            "[blt] index reduction regressing (unassigned: {} -> {}), restore prior",
                            prev, unassigned_count
                        );
                    }
                    if let (Some(eqs), Some(sv)) =
                        (last_good_equations.take(), last_good_state_vars.take())
                    {
                        equations = eqs;
                        state_vars = sv;
                        let remaining: HashSet<&str> =
                            state_vars.iter().map(String::as_str).collect();
                        current_known.retain(|k| {
                            remaining.contains(k.as_str())
                                || k == "time"
                                || params.iter().any(|p| p == k)
                        });
                        for s in &state_vars {
                            current_known.insert(s.clone());
                        }
                        restore_and_stop = true;
                        continue;
                    }
                    break;
                }
            }
        }
        differential_index = if unassigned_count > 0 { 2 } else { 1 };
        last_good_equations = Some(equations.clone());
        last_good_state_vars = Some(state_vars.clone());
        if differential_index == 1 || restore_and_stop {
            break;
        }
        let index_method = options.index_reduction_method.trim();
        if index_method.eq_ignore_ascii_case("none") {
            break;
        }
        // Continue index reduction while it yields new equations. Dummy-derivative
        // may keep the same unassigned count for a round while demoting a state.
        prev_unassigned_count = Some(unassigned_count);
        let states_before = state_vars.len();
        if let Some(new_eqs) = try_index_reduction(
            &equations,
            &assigned_var,
            &assigned_eq,
            &unknown_list,
            &mut state_vars,
            options,
        ) {
            equations = new_eqs;
            index_reduction_rounds = round;
            if state_vars.len() >= states_before {
                if blt_trace {
                    eprintln!("[blt] index reduction did not demote a state, stopping");
                }
                break;
            }
            // Demoted states leave the independent-integrator set and become algebraics.
            let remaining: HashSet<&str> = state_vars.iter().map(String::as_str).collect();
            current_known.retain(|k| remaining.contains(k.as_str()) || k == "time" || params.iter().any(|p| p == k));
            for s in &state_vars {
                current_known.insert(s.clone());
            }
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

    let mut constraint_equation_count = assigned_var.iter().filter(|o| o.is_none()).count();
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

    // Unmatched residuals are differential index-2 only when they are holonomic
    // constraints on states: g(x)=0 with no algebraic unknown. Residuals that
    // also mention algebraic unknowns are torn kinematic loops (index-1).
    if differential_index == 2 && constraint_equation_count > 0 {
        let state_set: HashSet<&str> = state_vars.iter().map(String::as_str).collect();
        let mut any_pure_state_constraint = false;
        for (i, assigned) in assigned_var.iter().enumerate() {
            if assigned.is_some() {
                continue;
            }
            let residual = simplify_expr(&make_residual(&equations[eq_infos[i].original_idx]));
            if matches!(&residual, Expression::Number(n) if n.abs() < 1e-15) {
                continue;
            }
            let mut res_vars = HashSet::new();
            collect_vars_expr(&residual, &mut res_vars);
            let mentions_state = res_vars.iter().any(|v| {
                state_set.contains(v.as_str())
                    || v.strip_prefix("der_")
                        .is_some_and(|s| state_set.contains(s))
            });
            let has_algebraic_unknown = res_vars.iter().any(|v| {
                if state_set.contains(v.as_str()) || v.starts_with("der_") || v == "time" {
                    return false;
                }
                unknown_map.contains_key(v)
            });
            if mentions_state && !has_algebraic_unknown {
                any_pure_state_constraint = true;
                break;
            }
        }
        if !any_pure_state_constraint {
            if blt_trace {
                eprintln!(
                    "[blt] unmatched constraints are algebraic (loop/tearing); index -> 1"
                );
            }
            differential_index = 1;
            constraint_equation_count = 0;
        }
    }

    if differential_index == 2
        && state_vars.is_empty()
        && !unknown_list.is_empty()
        && unknown_list.len() == equations.len()
        && equations.iter().all(|eq| matches!(eq, Equation::Simple(_, _)))
        && eq_infos.iter().all(|info| !info.unknowns.is_empty())
    {
        let tearing_var =
            select_tearing_variable(&unknown_list, &equations, &unknown_map, &options.tearing_method);
        if tearing_var.is_some() {
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
                demoted_states: Vec::new(),
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
                        // Edge producer -> consumer so SCC topo emits producers first.
                        // (Was consumer -> producer, which scheduled der_w = a before a = tau/J.)
                        adj[solver_eq_idx].push(i);
                        radj[i].push(solver_eq_idx);
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
    let sccs = kosaraju_sccs(&adj, &radj);

    if blt_trace {
        eprintln!("[blt] solve_blocks");
    }
    let emitted = emit_scc_blocks(
        sccs,
        &equations,
        &eq_infos,
        &assigned_var,
        &unknown_list,
        &unknown_map,
        &mut current_known,
        differential_index,
        &options.tearing_method,
        blt_trace,
    );
    let tearing_block_count = emitted.tearing_block_count;
    let tearing_residual_equation_count = emitted.tearing_residual_equation_count;
    let block_causality = emitted.block_causality;

    let sorted_equations =
        reorder_simple_variable_equations(emitted.sorted_equations, known_vars, params);

    SortAlgebraicResult {
        sorted_equations,
        differential_index,
        constraint_equation_count,
        constant_conflict_count,
        alias_map,
        index_reduction_rounds,
        dummy_derivative_equation_count,
        demoted_states: {
            let remaining: HashSet<&str> = state_vars.iter().map(String::as_str).collect();
            initial_state_vars
                .into_iter()
                .filter(|s| !remaining.contains(s.as_str()))
                .collect()
        },
        tearing_block_count,
        tearing_residual_equation_count,
        block_causality,
        blt_degrade_guard_triggered: false,
        blt_degrade_guard_limit: Some(max_eq_for_full_sort),
        blt_degrade_guard_equation_count: Some(equations.len()),
    }
}