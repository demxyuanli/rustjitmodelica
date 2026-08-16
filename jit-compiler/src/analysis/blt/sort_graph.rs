use crate::ast::{Equation, Expression};
use std::collections::{HashMap, HashSet};

use super::blt_expr::{
    is_forbidden_tearing_var, make_residual, select_tearing_variable, solve_for_variable,
};
use super::sort_match::EqInfo;
use super::types::BlockCausalityInfo;

pub(super) struct EmittedBlocks {
    pub sorted_equations: Vec<Equation>,
    pub tearing_block_count: usize,
    pub tearing_residual_equation_count: usize,
    pub block_causality: Vec<BlockCausalityInfo>,
}

/// Iterative Kosaraju SCC (finish-order on `adj`, then DFS on `radj`).
pub(super) fn kosaraju_sccs(adj: &[Vec<usize>], radj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n_nodes = adj.len();
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
    sccs
}

pub(super) fn emit_scc_blocks(
    sccs: Vec<Vec<usize>>,
    equations: &[Equation],
    eq_infos: &[EqInfo],
    assigned_var: &[Option<usize>],
    unknown_list: &[String],
    unknown_map: &HashMap<String, usize>,
    current_known: &mut HashSet<String>,
    differential_index: u32,
    tearing_method: &str,
    blt_trace: bool,
) -> EmittedBlocks {
    let mut sorted_equations = Vec::new();
    let mut tearing_block_count = 0usize;
    let mut tearing_residual_equation_count = 0usize;
    let mut block_causality: Vec<BlockCausalityInfo> = Vec::new();

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
                if is_forbidden_tearing_var(var_name) {
                    current_known.insert(var_name.clone());
                    if blt_trace {
                        eprintln!("[blt] skip forbidden assigned unknown {}", var_name);
                    }
                    continue;
                }
                if let Some(expr) = solve_for_variable(eq, var_name) {
                    current_known.insert(var_name.clone());
                    sorted_equations.push(Equation::Simple(Expression::var(var_name), expr));
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
                        unknown_map,
                        tearing_method,
                    );
                    if tearing_var.is_none() {
                        current_known.insert(var_name.clone());
                        if blt_trace {
                            eprintln!("[blt] skip non-tearable single unknown {}", var_name);
                        }
                        continue;
                    }
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
                let unknowns_from_eq: Vec<String> = eq_infos[idx]
                    .unknowns
                    .iter()
                    .filter(|u| !is_forbidden_tearing_var(u))
                    .cloned()
                    .collect();
                if let Some(_unk) = unknowns_from_eq.first().cloned() {
                    let tearing_var = select_tearing_variable(
                        &unknowns_from_eq,
                        &[eq.clone()],
                        unknown_map,
                        tearing_method,
                    );
                    if tearing_var.is_none() {
                        if blt_trace {
                            eprintln!(
                                "[blt] skip unassigned non-tearable equation unknowns={:?}",
                                unknowns_from_eq
                            );
                        }
                        continue;
                    }
                    sorted_equations.push(Equation::SolvableBlock {
                        unknowns: unknowns_from_eq,
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
                    // Residual with no algebraic unknowns (only time/start/knowns):
                    // consistency check only — never invent Newton on time/start.
                    if blt_trace {
                        eprintln!(
                            "[blt] skip residual-only frozen equation idx={}",
                            eq_infos[idx].original_idx
                        );
                    }
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
                .filter(|u| !is_forbidden_tearing_var(u))
                .collect();

            if block_unknowns.is_empty() && !block_eqs.is_empty() {
                for &idx in &block_indices {
                    for u in &eq_infos[idx].unknowns {
                        if unknown_map.contains_key(u)
                            && !is_forbidden_tearing_var(u)
                            && !block_unknowns.contains(u)
                        {
                            block_unknowns.push(u.clone());
                        }
                    }
                }
            }
            let tearing_var =
                select_tearing_variable(&block_unknowns, &block_eqs, unknown_map, tearing_method);
            if block_unknowns.is_empty() || tearing_var.is_none() {
                if blt_trace {
                    eprintln!(
                        "[blt] skip SCC with no tearable unknowns size={}",
                        block_indices.len()
                    );
                }
                continue;
            }

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

    EmittedBlocks {
        sorted_equations,
        tearing_block_count,
        tearing_residual_equation_count,
        block_causality,
    }
}
