fn fold_const_expr(expr: &crate::ast::Expression) -> (crate::ast::Expression, u64) {
    use crate::ast::{Expression, Operator};
    match expr {
        Expression::BinaryOp(l, op, r) => {
            let (lf, lc) = fold_const_expr(l);
            let (rf, rc) = fold_const_expr(r);
            if let (Expression::Number(a), Expression::Number(b)) = (&lf, &rf) {
                let v = match op {
                    Operator::Add => Some(a + b),
                    Operator::Sub => Some(a - b),
                    Operator::Mul => Some(a * b),
                    Operator::Div => Some(a / b),
                    _ => None,
                };
                if let Some(n) = v {
                    return (Expression::Number(n), lc + rc + 1);
                }
            }
            (
                Expression::BinaryOp(Box::new(lf), *op, Box::new(rf)),
                lc + rc,
            )
        }
        Expression::If(c, t, f) => {
            let (cf, cc) = fold_const_expr(c);
            let (tf, tc) = fold_const_expr(t);
            let (ff, fc) = fold_const_expr(f);
            if let Expression::Number(n) = cf {
                return if n != 0.0 {
                    (tf, cc + tc + fc + 1)
                } else {
                    (ff, cc + tc + fc + 1)
                };
            }
            (
                Expression::If(Box::new(cf), Box::new(tf), Box::new(ff)),
                cc + tc + fc,
            )
        }
        _ => (expr.clone(), 0),
    }
}

fn collect_expr_vars(expr: &crate::ast::Expression, out: &mut std::collections::HashSet<String>) {
    use crate::ast::Expression;
    match expr {
        Expression::Variable(id) => {
            out.insert(crate::string_intern::resolve_id(*id));
        }
        Expression::BinaryOp(l, _, r) => {
            collect_expr_vars(l, out);
            collect_expr_vars(r, out);
        }
        Expression::Call(_, args) | Expression::ArrayLiteral(args) => {
            for a in args {
                collect_expr_vars(a, out);
            }
        }
        Expression::Der(inner)
        | Expression::Sample(inner)
        | Expression::Interval(inner)
        | Expression::Hold(inner)
        | Expression::Previous(inner) => collect_expr_vars(inner, out),
        Expression::SubSample(a, b)
        | Expression::SuperSample(a, b)
        | Expression::ShiftSample(a, b)
        | Expression::BackSample(a, b)
        | Expression::ArrayAccess(a, b) => {
            collect_expr_vars(a, out);
            collect_expr_vars(b, out);
        }
        Expression::Dot(base, _) => collect_expr_vars(base, out),
        Expression::If(c, t, f) => {
            collect_expr_vars(c, out);
            collect_expr_vars(t, out);
            collect_expr_vars(f, out);
        }
        Expression::Range(a, b, c) => {
            collect_expr_vars(a, out);
            collect_expr_vars(b, out);
            collect_expr_vars(c, out);
        }
        Expression::ArrayComprehension { expr, iter_range, .. } => {
            collect_expr_vars(expr, out);
            collect_expr_vars(iter_range, out);
        }
        Expression::Number(_) | Expression::StringLiteral(_) => {}
    }
}

pub(crate) fn optimize_equations_for_constfold_dce(
    equations: &mut Vec<crate::ast::Equation>,
    enable_const_fold: bool,
    enable_dce: bool,
) -> (u64, u64, std::collections::HashSet<String>) {
    use crate::ast::Equation;
    let mut folded = 0_u64;
    let mut folded_vars: std::collections::HashSet<String> = std::collections::HashSet::new();
    if enable_const_fold {
        for eq in equations.iter_mut() {
            if let Equation::Simple(lhs, rhs) = eq {
                let mut before_vars = std::collections::HashSet::new();
                collect_expr_vars(lhs, &mut before_vars);
                collect_expr_vars(rhs, &mut before_vars);
                let (nl, c1) = fold_const_expr(lhs);
                let (nr, c2) = fold_const_expr(rhs);
                *lhs = nl;
                *rhs = nr;
                folded += c1 + c2;
                if c1 + c2 > 0 {
                    folded_vars.extend(before_vars);
                }
            }
        }
    }
    let mut removed = 0_u64;
    if enable_dce {
        let before = equations.len();
        equations.retain(|eq| match eq {
            Equation::Simple(l, r) => !(matches!(l, crate::ast::Expression::Number(_))
                && matches!(r, crate::ast::Expression::Number(_))),
            _ => true,
        });
        removed = before.saturating_sub(equations.len()) as u64;
    }
    (folded, removed, folded_vars)
}
