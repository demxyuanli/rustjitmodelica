use crate::ast::Expression;
use std::collections::HashMap;

pub(super) fn substitute_expr(expr: &Expression, subst: &HashMap<String, Expression>) -> Expression {
    use Expression::*;
    match expr {
        Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            subst.get(&name).cloned().unwrap_or_else(|| Variable(*id))
        }
        Number(n) => Number(*n),
        BinaryOp(lhs, op, rhs) => BinaryOp(
            Box::new(substitute_expr(lhs, subst)),
            *op,
            Box::new(substitute_expr(rhs, subst)),
        ),
        Call(func, args) => Call(func.clone(), args.iter().map(|a| substitute_expr(a, subst)).collect()),
        Der(inner) => Der(Box::new(substitute_expr(inner, subst))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(substitute_expr(arr, subst)),
            Box::new(substitute_expr(idx, subst)),
        ),
        If(cond, t, f) => If(
            Box::new(substitute_expr(cond, subst)),
            Box::new(substitute_expr(t, subst)),
            Box::new(substitute_expr(f, subst)),
        ),
        Range(start, step, end) => Range(
            Box::new(substitute_expr(start, subst)),
            Box::new(substitute_expr(step, subst)),
            Box::new(substitute_expr(end, subst)),
        ),
        ArrayLiteral(items) => ArrayLiteral(items.iter().map(|e| substitute_expr(e, subst)).collect()),
        ArrayComprehension { expr, iter_var, iter_range } => ArrayComprehension {
            expr: Box::new(substitute_expr(expr, subst)),
            iter_var: iter_var.clone(),
            iter_range: Box::new(substitute_expr(iter_range, subst)),
        },
        Dot(base, member) => Dot(Box::new(substitute_expr(base, subst)), member.clone()),
        Sample(inner) => Sample(Box::new(substitute_expr(inner, subst))),
        Interval(inner) => Interval(Box::new(substitute_expr(inner, subst))),
        Hold(inner) => Hold(Box::new(substitute_expr(inner, subst))),
        Previous(inner) => Previous(Box::new(substitute_expr(inner, subst))),
        SubSample(c, n) => SubSample(Box::new(substitute_expr(c, subst)), Box::new(substitute_expr(n, subst))),
        SuperSample(c, n) => SuperSample(Box::new(substitute_expr(c, subst)), Box::new(substitute_expr(n, subst))),
        ShiftSample(c, n) => ShiftSample(Box::new(substitute_expr(c, subst)), Box::new(substitute_expr(n, subst))),
        StringLiteral(s) => StringLiteral(s.clone()),
    }
}
