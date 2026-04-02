use crate::ast::Expression;
use std::collections::HashMap;

/// Returns `(new_expr, structural_change)`.
fn substitute_expr_impl(expr: &Expression, subst: &HashMap<String, Expression>) -> (Expression, bool) {
    use Expression::*;
    match expr {
        Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            if let Some(rep) = subst.get(&name) {
                (rep.clone(), true)
            } else {
                (Variable(*id), false)
            }
        }
        Number(n) => (Number(*n), false),
        StringLiteral(s) => (StringLiteral(s.clone()), false),
        BinaryOp(lhs, op, rhs) => {
            let (nl, cl) = substitute_expr_impl(lhs, subst);
            let (nr, cr) = substitute_expr_impl(rhs, subst);
            if !cl && !cr {
                (expr.clone(), false)
            } else {
                (BinaryOp(Box::new(nl), *op, Box::new(nr)), true)
            }
        }
        Call(func, args) => {
            let mut out = Vec::with_capacity(args.len());
            let mut any = false;
            for a in args {
                let (na, ca) = substitute_expr_impl(a, subst);
                out.push(na);
                any |= ca;
            }
            if !any {
                (expr.clone(), false)
            } else {
                (Call(func.clone(), out), true)
            }
        }
        Der(inner) => {
            let (ni, ci) = substitute_expr_impl(inner, subst);
            if !ci {
                (expr.clone(), false)
            } else {
                (Der(Box::new(ni)), true)
            }
        }
        Sample(inner) => {
            let (ni, ci) = substitute_expr_impl(inner, subst);
            if !ci {
                (expr.clone(), false)
            } else {
                (Sample(Box::new(ni)), true)
            }
        }
        Interval(inner) => {
            let (ni, ci) = substitute_expr_impl(inner, subst);
            if !ci {
                (expr.clone(), false)
            } else {
                (Interval(Box::new(ni)), true)
            }
        }
        Hold(inner) => {
            let (ni, ci) = substitute_expr_impl(inner, subst);
            if !ci {
                (expr.clone(), false)
            } else {
                (Hold(Box::new(ni)), true)
            }
        }
        Previous(inner) => {
            let (ni, ci) = substitute_expr_impl(inner, subst);
            if !ci {
                (expr.clone(), false)
            } else {
                (Previous(Box::new(ni)), true)
            }
        }
        SubSample(c, n) => {
            let (nc, cc) = substitute_expr_impl(c, subst);
            let (nn, cn) = substitute_expr_impl(n, subst);
            if !cc && !cn {
                (expr.clone(), false)
            } else {
                (SubSample(Box::new(nc), Box::new(nn)), true)
            }
        }
        SuperSample(c, n) => {
            let (nc, cc) = substitute_expr_impl(c, subst);
            let (nn, cn) = substitute_expr_impl(n, subst);
            if !cc && !cn {
                (expr.clone(), false)
            } else {
                (SuperSample(Box::new(nc), Box::new(nn)), true)
            }
        }
        ShiftSample(c, n) => {
            let (nc, cc) = substitute_expr_impl(c, subst);
            let (nn, cn) = substitute_expr_impl(n, subst);
            if !cc && !cn {
                (expr.clone(), false)
            } else {
                (ShiftSample(Box::new(nc), Box::new(nn)), true)
            }
        }
        BackSample(c, n) => {
            let (nc, cc) = substitute_expr_impl(c, subst);
            let (nn, cn) = substitute_expr_impl(n, subst);
            if !cc && !cn {
                (expr.clone(), false)
            } else {
                (BackSample(Box::new(nc), Box::new(nn)), true)
            }
        }
        ArrayAccess(arr, idx) => {
            let (na, ca) = substitute_expr_impl(arr, subst);
            let (ni, ci) = substitute_expr_impl(idx, subst);
            if !ca && !ci {
                (expr.clone(), false)
            } else {
                (ArrayAccess(Box::new(na), Box::new(ni)), true)
            }
        }
        If(cond, t, f) => {
            let (nc, cc) = substitute_expr_impl(cond, subst);
            let (nt, ct) = substitute_expr_impl(t, subst);
            let (nf, cf) = substitute_expr_impl(f, subst);
            if !cc && !ct && !cf {
                (expr.clone(), false)
            } else {
                (If(Box::new(nc), Box::new(nt), Box::new(nf)), true)
            }
        }
        Range(start, step, end) => {
            let (ns, cs) = substitute_expr_impl(start, subst);
            let (nstep, cstep) = substitute_expr_impl(step, subst);
            let (ne, ce) = substitute_expr_impl(end, subst);
            if !cs && !cstep && !ce {
                (expr.clone(), false)
            } else {
                (Range(Box::new(ns), Box::new(nstep), Box::new(ne)), true)
            }
        }
        ArrayLiteral(items) => {
            let mut out = Vec::with_capacity(items.len());
            let mut any = false;
            for it in items {
                let (ni, ci) = substitute_expr_impl(it, subst);
                out.push(ni);
                any |= ci;
            }
            if !any {
                (expr.clone(), false)
            } else {
                (ArrayLiteral(out), true)
            }
        }
        ArrayComprehension {
            expr: inner,
            iter_var,
            iter_range,
        } => {
            let (ne, ce) = substitute_expr_impl(inner, subst);
            let (nr, cr) = substitute_expr_impl(iter_range, subst);
            if !ce && !cr {
                (expr.clone(), false)
            } else {
                (
                    ArrayComprehension {
                        expr: Box::new(ne),
                        iter_var: iter_var.clone(),
                        iter_range: Box::new(nr),
                    },
                    true,
                )
            }
        }
        Dot(base, member) => {
            let (nb, cb) = substitute_expr_impl(base, subst);
            if !cb {
                (expr.clone(), false)
            } else {
                (Dot(Box::new(nb), member.clone()), true)
            }
        }
    }
}

pub(super) fn substitute_expr(expr: &Expression, subst: &HashMap<String, Expression>) -> Option<Expression> {
    if subst.is_empty() {
        return None;
    }
    let (out, changed) = substitute_expr_impl(expr, subst);
    if changed {
        Some(out)
    } else {
        None
    }
}
