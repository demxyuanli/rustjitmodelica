use crate::ast::{Expression, Operator};
use std::collections::HashMap;

pub fn eval_jac_expr_at_state(
    expr: &Expression,
    state_var_index: &HashMap<String, usize>,
    states: &[f64],
) -> f64 {
    match expr {
        Expression::Number(n) => *n,
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            if let Some(&idx) = state_var_index.get(&name) {
                if idx < states.len() {
                    return states[idx];
                }
            }
            0.0
        }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = eval_jac_expr_at_state(lhs, state_var_index, states);
            let r = eval_jac_expr_at_state(rhs, state_var_index, states);
            match op {
                Operator::Add => l + r,
                Operator::Sub => l - r,
                Operator::Mul => l * r,
                Operator::Div => l / r,
                _ => 0.0,
            }
        }
        Expression::If(c, t, f) => {
            let cv = eval_jac_expr_at_state(c, state_var_index, states);
            if cv != 0.0 {
                eval_jac_expr_at_state(t, state_var_index, states)
            } else {
                eval_jac_expr_at_state(f, state_var_index, states)
            }
        }
        Expression::ArrayLiteral(items) => {
            if let Some(first) = items.first() {
                eval_jac_expr_at_state(first, state_var_index, states)
            } else {
                0.0
            }
        }
        Expression::ArrayComprehension { .. } => 0.0,
        Expression::Der(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::ArrayAccess(base, _idx) => eval_jac_expr_at_state(base, state_var_index, states),
        Expression::Dot(base, _member) => eval_jac_expr_at_state(base, state_var_index, states),
        Expression::Range(_, _, _) => 0.0,
        Expression::Call(_, _) => 0.0,
        Expression::Sample(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::Interval(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::Hold(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::Previous(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::SubSample(c, _) | Expression::SuperSample(c, _) | Expression::ShiftSample(c, _) => {
            eval_jac_expr_at_state(c, state_var_index, states)
        }
        Expression::StringLiteral(_) => 0.0,
    }
}
