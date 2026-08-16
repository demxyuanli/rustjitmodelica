use crate::ast::{Equation, Expression};
use crate::string_intern::resolve_id;
use std::collections::{HashMap, HashSet};

pub(super) struct EqInfo {
    pub original_idx: usize,
    pub unknowns: Vec<String>,
}

pub(super) fn dfs_iter(
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
    let mut stack: Vec<(usize, usize, Option<(usize, usize)>)> = vec![(u_start, 0, None)];
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

/// Zero-residual claim, greedy LHS matching, then DFS rematch.
/// Zero residuals (`0 = f(u)`) must claim unknowns before greedy `u = Call(...)`.
pub(super) fn assign_equation_variables(
    equations: &[Equation],
    eq_infos: &[EqInfo],
    unknown_map: &HashMap<String, usize>,
    unknown_list: &[String],
) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
    let mut assigned_var = vec![None; eq_infos.len()];
    let mut assigned_eq = vec![None; unknown_list.len()];

    let mut zero_residual_vars: HashSet<String> = HashSet::new();
    for (i, info) in eq_infos.iter().enumerate() {
        let eq = &equations[info.original_idx];
        if !simple_eq_is_numeric_zero(eq) {
            continue;
        }
        for u in &info.unknowns {
            zero_residual_vars.insert(u.clone());
        }
        if assigned_var[i].is_some() {
            continue;
        }
        let mut candidates: Vec<&String> = info.unknowns.iter().collect();
        candidates.sort_by_key(|u| zero_residual_claim_key(u));
        for u in candidates {
            if let Some(&v_idx) = unknown_map.get(u) {
                if assigned_eq[v_idx].is_none() {
                    assigned_eq[v_idx] = Some(i);
                    assigned_var[i] = Some(v_idx);
                    break;
                }
            }
        }
    }

    for (i, info) in eq_infos.iter().enumerate() {
        let eq = &equations[info.original_idx];
        if assigned_var[i].is_some() {
            continue;
        }
        if let Equation::Simple(lhs, rhs) = eq {
            let mut target_var = None;
            if let Expression::Variable(id) = lhs {
                target_var = Some(resolve_id(*id));
            } else if let Expression::Der(inner) = lhs {
                if let Expression::Variable(id) = inner.as_ref() {
                    target_var = Some(format!("der_{}", resolve_id(*id)));
                }
            }
            // Prefer assigning der_* when the equation is `v = der_s` / `der_s = v`.
            // Otherwise greedy LHS matching turns `a = der(w)` into an assignment of
            // `a` and leaves `der_w` unmatched (stays 0 while w integrates from elsewhere).
            if let Expression::Variable(rid) = rhs {
                let r = resolve_id(*rid);
                if r.starts_with("der_") && unknown_map.contains_key(&r) {
                    target_var = Some(r);
                }
            } else if let Expression::Variable(lid) = lhs {
                let l = resolve_id(*lid);
                if l.starts_with("der_") && unknown_map.contains_key(&l) {
                    target_var = Some(l);
                }
            }
            if let Some(v) = target_var {
                // Leave vars claimed by planar/force zero residuals for DFS
                // rematching instead of locking them to definition equations.
                if zero_residual_vars.contains(&v) {
                    continue;
                }
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
        let _ = dfs_iter(i, &adj, &mut assigned_eq, &mut visited);
    }

    for (v_idx, eq_opt) in assigned_eq.iter().enumerate() {
        if let Some(eq_idx) = eq_opt {
            assigned_var[*eq_idx] = Some(v_idx);
        }
    }

    (assigned_var, assigned_eq)
}

fn simple_eq_is_numeric_zero(eq: &Equation) -> bool {
    matches!(
        eq,
        Equation::Simple(Expression::Number(n), _) if *n == 0.0
    ) || matches!(
        eq,
        Equation::Simple(_, Expression::Number(n)) if *n == 0.0
    )
}

/// Zero-residual claim order. Keep `_t_` at default prio 2 (not `_f_` prio 1).
fn zero_residual_claim_key(u: &str) -> (u8, usize) {
    let prio = if u.contains("r_rel") {
        0
    } else if u.contains("f_c") || u.contains("_f_") || u.ends_with("_f") {
        1
    } else {
        2
    };
    (prio, u.len())
}
