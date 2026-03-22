use crate::ast::{Expression, Operator};
use crate::string_intern::resolve_id;

use super::variable_collection::contains_var;

pub fn make_num(n: f64) -> Expression {
    Expression::Number(n)
}

pub fn make_mul(lhs: Expression, rhs: Expression) -> Expression {
    Expression::BinaryOp(Box::new(lhs), Operator::Mul, Box::new(rhs))
}

#[allow(dead_code)]
pub fn make_div(lhs: Expression, rhs: Expression) -> Expression {
    Expression::BinaryOp(Box::new(lhs), Operator::Div, Box::new(rhs))
}

#[allow(dead_code)]
pub fn make_add(lhs: Expression, rhs: Expression) -> Expression {
    Expression::BinaryOp(Box::new(lhs), Operator::Add, Box::new(rhs))
}

pub fn make_binary(lhs: Expression, op: Operator, rhs: Expression) -> Expression {
    Expression::BinaryOp(Box::new(lhs), op, Box::new(rhs))
}

pub fn expression_is_zero(expr: &Expression) -> bool {
    match expr {
        Expression::Number(n) => n.abs() < 1e-15,
        _ => false,
    }
}

fn simplify_time_expr(expr: &Expression) -> Expression {
    match expr {
        Expression::BinaryOp(lhs, Operator::Mul, rhs) => {
            let sl = simplify_time_expr(lhs);
            let sr = simplify_time_expr(rhs);
            if expression_is_zero(&sl) || expression_is_zero(&sr) {
                return Expression::Number(0.0);
            }
            if let Expression::Number(n) = &sl {
                if (n - 1.0).abs() < 1e-15 {
                    return sr;
                }
            }
            if let Expression::Number(n) = &sr {
                if (n - 1.0).abs() < 1e-15 {
                    return sl;
                }
            }
            Expression::BinaryOp(Box::new(sl), Operator::Mul, Box::new(sr))
        }
        Expression::BinaryOp(lhs, Operator::Add, rhs) => {
            let sl = simplify_time_expr(lhs);
            let sr = simplify_time_expr(rhs);
            if expression_is_zero(&sl) {
                return sr;
            }
            if expression_is_zero(&sr) {
                return sl;
            }
            Expression::BinaryOp(Box::new(sl), Operator::Add, Box::new(sr))
        }
        Expression::BinaryOp(lhs, Operator::Sub, rhs) => {
            let sl = simplify_time_expr(lhs);
            let sr = simplify_time_expr(rhs);
            if expression_is_zero(&sr) {
                return sl;
            }
            Expression::BinaryOp(Box::new(sl), Operator::Sub, Box::new(sr))
        }
        Expression::BinaryOp(lhs, Operator::Div, rhs) => {
            let sl = simplify_time_expr(lhs);
            let sr = simplify_time_expr(rhs);
            if expression_is_zero(&sl) {
                return Expression::Number(0.0);
            }
            if let Expression::Number(n) = &sr {
                if (n - 1.0).abs() < 1e-15 {
                    return sl;
                }
            }
            Expression::BinaryOp(Box::new(sl), Operator::Div, Box::new(sr))
        }
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
            Box::new(simplify_time_expr(lhs)),
            *op,
            Box::new(simplify_time_expr(rhs)),
        ),
        _ => expr.clone(),
    }
}

pub fn partial_derivative(expr: &Expression, var: &str) -> Expression {
    use crate::ast::Operator;
    match expr {
        Expression::Variable(id) => {
            if resolve_id(*id) == var {
                Expression::Number(1.0)
            } else {
                Expression::Number(0.0)
            }
        }
        Expression::Number(_) => Expression::Number(0.0),
        Expression::BinaryOp(lhs, op, rhs) => {
            let dl = partial_derivative(lhs, var);
            let dr = partial_derivative(rhs, var);
            match op {
                Operator::Add | Operator::Sub => {
                    let r = if *op == Operator::Add {
                        Operator::Add
                    } else {
                        Operator::Sub
                    };
                    Expression::BinaryOp(Box::new(dl), r, Box::new(dr))
                }
                Operator::Mul => {
                    let term1 =
                        Expression::BinaryOp(Box::new(dl.clone()), Operator::Mul, rhs.clone());
                    let term2 = Expression::BinaryOp(
                        Box::new((**lhs).clone()),
                        Operator::Mul,
                        Box::new(dr),
                    );
                    Expression::BinaryOp(Box::new(term1), Operator::Add, Box::new(term2))
                }
                Operator::Div => {
                    let num = Expression::BinaryOp(
                        Box::new(Expression::BinaryOp(
                            Box::new(dl.clone()),
                            Operator::Mul,
                            rhs.clone(),
                        )),
                        Operator::Sub,
                        Box::new(Expression::BinaryOp(
                            Box::new((**lhs).clone()),
                            Operator::Mul,
                            Box::new(dr.clone()),
                        )),
                    );
                    let r = (**rhs).clone();
                    let den = Expression::BinaryOp(Box::new(r.clone()), Operator::Mul, Box::new(r));
                    Expression::BinaryOp(Box::new(num), Operator::Div, Box::new(den))
                }
                _ => Expression::Number(0.0),
            }
        }
        Expression::Der(inner) => {
            if contains_var(inner, var) {
                Expression::Der(Box::new(partial_derivative(inner, var)))
            } else {
                Expression::Number(0.0)
            }
        }
        Expression::Call(func_name, args) => {
            // Chain rule for known single-argument math functions:
            // d/dx f(g(x)) = f'(g(x)) * g'(x)
            if args.len() == 1 {
                let inner = &args[0];
                let d_inner = partial_derivative(inner, var);
                if expression_is_zero(&d_inner) {
                    return Expression::Number(0.0);
                }
                let f_prime = match func_name.as_str() {
                    "sin" => Some(Expression::Call("cos".to_string(), vec![inner.clone()])),
                    "cos" => Some(Expression::BinaryOp(
                        Box::new(Expression::Number(0.0)),
                        Operator::Sub,
                        Box::new(Expression::Call("sin".to_string(), vec![inner.clone()])),
                    )),
                    "exp" => Some(Expression::Call("exp".to_string(), vec![inner.clone()])),
                    "log" | "ln" => Some(Expression::BinaryOp(
                        Box::new(Expression::Number(1.0)),
                        Operator::Div,
                        Box::new(inner.clone()),
                    )),
                    "sqrt" => Some(Expression::BinaryOp(
                        Box::new(Expression::Number(0.5)),
                        Operator::Div,
                        Box::new(Expression::Call("sqrt".to_string(), vec![inner.clone()])),
                    )),
                    "tan" => {
                        // d/dx tan(u) = (1 + tan(u)^2) * du/dx
                        let tan_u = Expression::Call("tan".to_string(), vec![inner.clone()]);
                        Some(Expression::BinaryOp(
                            Box::new(Expression::Number(1.0)),
                            Operator::Add,
                            Box::new(Expression::BinaryOp(
                                Box::new(tan_u.clone()),
                                Operator::Mul,
                                Box::new(tan_u),
                            )),
                        ))
                    }
                    "asin" => Some(Expression::BinaryOp(
                        Box::new(Expression::Number(1.0)),
                        Operator::Div,
                        Box::new(Expression::Call("sqrt".to_string(), vec![
                            Expression::BinaryOp(
                                Box::new(Expression::Number(1.0)),
                                Operator::Sub,
                                Box::new(Expression::BinaryOp(
                                    Box::new(inner.clone()),
                                    Operator::Mul,
                                    Box::new(inner.clone()),
                                )),
                            ),
                        ])),
                    )),
                    "acos" => Some(Expression::BinaryOp(
                        Box::new(Expression::Number(0.0)),
                        Operator::Sub,
                        Box::new(Expression::BinaryOp(
                            Box::new(Expression::Number(1.0)),
                            Operator::Div,
                            Box::new(Expression::Call("sqrt".to_string(), vec![
                                Expression::BinaryOp(
                                    Box::new(Expression::Number(1.0)),
                                    Operator::Sub,
                                    Box::new(Expression::BinaryOp(
                                        Box::new(inner.clone()),
                                        Operator::Mul,
                                        Box::new(inner.clone()),
                                    )),
                                ),
                            ])),
                        )),
                    )),
                    "atan" => Some(Expression::BinaryOp(
                        Box::new(Expression::Number(1.0)),
                        Operator::Div,
                        Box::new(Expression::BinaryOp(
                            Box::new(Expression::Number(1.0)),
                            Operator::Add,
                            Box::new(Expression::BinaryOp(
                                Box::new(inner.clone()),
                                Operator::Mul,
                                Box::new(inner.clone()),
                            )),
                        )),
                    )),
                    "abs" => Some(Expression::Call("sign".to_string(), vec![inner.clone()])),
                    _ => None,
                };
                if let Some(fp) = f_prime {
                    return Expression::BinaryOp(
                        Box::new(fp),
                        Operator::Mul,
                        Box::new(d_inner),
                    );
                }
            }
            if args.len() == 2 && func_name == "atan2" {
                // d/dx atan2(y, x) = (x*dy/dx - y*dx/dx) / (x^2 + y^2)
                let (y, x) = (&args[0], &args[1]);
                let dy = partial_derivative(y, var);
                let dx = partial_derivative(x, var);
                if expression_is_zero(&dy) && expression_is_zero(&dx) {
                    return Expression::Number(0.0);
                }
                let num = Expression::BinaryOp(
                    Box::new(Expression::BinaryOp(
                        Box::new(x.clone()),
                        Operator::Mul,
                        Box::new(dy),
                    )),
                    Operator::Sub,
                    Box::new(Expression::BinaryOp(
                        Box::new(y.clone()),
                        Operator::Mul,
                        Box::new(dx),
                    )),
                );
                let den = Expression::BinaryOp(
                    Box::new(Expression::BinaryOp(
                        Box::new(x.clone()),
                        Operator::Mul,
                        Box::new(x.clone()),
                    )),
                    Operator::Add,
                    Box::new(Expression::BinaryOp(
                        Box::new(y.clone()),
                        Operator::Mul,
                        Box::new(y.clone()),
                    )),
                );
                return Expression::BinaryOp(Box::new(num), Operator::Div, Box::new(den));
            }
            Expression::Number(0.0)
        }
        Expression::If(cond, then_expr, else_expr) => {
            // d/dx if(c, t, e) = if(c, dt/dx, de/dx)
            // Condition is assumed constant w.r.t. continuous variables
            let dt = partial_derivative(then_expr, var);
            let de = partial_derivative(else_expr, var);
            if expression_is_zero(&dt) && expression_is_zero(&de) {
                Expression::Number(0.0)
            } else {
                Expression::If(cond.clone(), Box::new(dt), Box::new(de))
            }
        }
        Expression::ArrayAccess(_, _)
        | Expression::Dot(_, _)
        | Expression::Range(_, _, _)
        | Expression::ArrayLiteral(_)
        | Expression::ArrayComprehension { .. }
        | Expression::Sample(_)
        | Expression::Interval(_)
        | Expression::Hold(_)
        | Expression::Previous(_)
        | Expression::SubSample(_, _)
        | Expression::SuperSample(_, _)
        | Expression::ShiftSample(_, _)
        | Expression::StringLiteral(_) => Expression::Number(0.0),
    }
}

pub fn time_derivative(expr: &Expression, state_vars: &[String]) -> Expression {
    let mut sum: Option<Expression> = None;
    for x in state_vars {
        let pd = simplify_time_expr(&partial_derivative(expr, x));
        if expression_is_zero(&pd) {
            continue;
        }
        let der_x = Expression::var(&format!("der_{}", x));
        let term = Expression::BinaryOp(Box::new(pd), Operator::Mul, Box::new(der_x));
        sum = Some(match sum {
            None => term,
            Some(s) => Expression::BinaryOp(Box::new(s), Operator::Add, Box::new(term)),
        });
    }
    simplify_time_expr(&sum.unwrap_or_else(|| Expression::Number(0.0)))
}
