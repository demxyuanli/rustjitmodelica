use std::collections::HashSet;
use std::collections::HashMap;
use crate::ast::{Equation, Expression, AlgorithmStatement};
use crate::analysis::order_initial_equations_for_application;
use crate::flatten::eval_const_expr;

pub fn apply_initial_conditions(
    flat_model: &crate::flatten::FlattenedModel,
    states: &mut [f64],
    discrete_vals: &mut [f64],
    params: &mut [f64],
    state_var_index: &HashMap<String, usize>,
    discrete_var_index: &HashMap<String, usize>,
    param_var_index: &HashMap<String, usize>,
) {
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

    let mut known_at_initial = HashSet::new();
    known_at_initial.insert("time".to_string());
    for name in param_var_index.keys() {
        known_at_initial.insert(name.clone());
    }
    let initial_order =
        order_initial_equations_for_application(&flat_model.initial_equations, &known_at_initial);
    let mut applied = true;
    let mut pass_limit = 20;
    while applied && pass_limit > 0 {
        pass_limit -= 1;
        applied = false;
        for &idx in &initial_order {
            let eq = &flat_model.initial_equations[idx];
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
                        let prev = state_var_index
                            .get(name)
                            .and_then(|&i| Some(states[i]))
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
                        name, rhs
                    );
                }
            }
        }
    }
}

pub fn substitute_initial_values(
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
                lhs,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
            *op,
            Box::new(substitute_initial_values(
                rhs,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
        ),
        Call(func, args) => Call(
            func.clone(),
            args.iter()
                .map(|a| {
                    substitute_initial_values(
                        a,
                        state_var_index,
                        discrete_var_index,
                        param_var_index,
                        states,
                        discrete_vals,
                        params,
                    )
                })
                .collect(),
        ),
        Der(inner) => Der(Box::new(substitute_initial_values(
            inner,
            state_var_index,
            discrete_var_index,
            param_var_index,
            states,
            discrete_vals,
            params,
        ))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(substitute_initial_values(
                arr,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
            Box::new(substitute_initial_values(
                idx,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
        ),
        If(cond, t, f) => If(
            Box::new(substitute_initial_values(
                cond,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
            Box::new(substitute_initial_values(
                t,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
            Box::new(substitute_initial_values(
                f,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
        ),
        Range(start, step, end) => Range(
            Box::new(substitute_initial_values(
                start,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
            Box::new(substitute_initial_values(
                step,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
            Box::new(substitute_initial_values(
                end,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
        ),
        ArrayLiteral(items) => ArrayLiteral(
            items
                .iter()
                .map(|e| {
                    substitute_initial_values(
                        e,
                        state_var_index,
                        discrete_var_index,
                        param_var_index,
                        states,
                        discrete_vals,
                        params,
                    )
                })
                .collect(),
        ),
        Dot(base, member) => Dot(
            Box::new(substitute_initial_values(
                base,
                state_var_index,
                discrete_var_index,
                param_var_index,
                states,
                discrete_vals,
                params,
            )),
            member.clone(),
        ),
    }
}

#[allow(dead_code)]
pub fn substitute_params(
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
