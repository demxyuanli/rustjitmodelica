use crate::ast::{Expression, Operator};
use pest::iterators::Pair;

use super::expression;
use super::Rule;

pub(super) fn expr_to_string(expr: Expression) -> String {
    match expr {
        Expression::Variable(id) => crate::string_intern::resolve_id(id),
        Expression::Dot(base, member) => format!("{}.{}", expr_to_string(*base), member),
        Expression::ArrayAccess(base, _idx) => format!("{}[?]", expr_to_string(*base)),
        _ => "unknown".to_string(),
    }
}

#[allow(dead_code)]
pub(super) fn parse_const_expression(pair: Pair<Rule>) -> Option<f64> {
    let expr = expression::parse_expression(pair);
    eval_const_expr(&expr)
}

pub(super) fn eval_const_expr(expr: &Expression) -> Option<f64> {
    match expr {
        Expression::Number(n) => Some(*n),
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = eval_const_expr(lhs)?;
            let r = eval_const_expr(rhs)?;
            match op {
                Operator::Add => Some(l + r),
                Operator::Sub => Some(l - r),
                Operator::Mul => Some(l * r),
                Operator::Div => Some(l / r),
                _ => None,
            }
        }
        Expression::If(cond, t_expr, f_expr) => {
            let c = eval_const_expr(cond)?;
            if c != 0.0 {
                eval_const_expr(t_expr)
            } else {
                eval_const_expr(f_expr)
            }
        }
        _ => None,
    }
}
