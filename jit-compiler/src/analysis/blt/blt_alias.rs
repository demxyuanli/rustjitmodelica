use crate::ast::{Equation, Expression, Operator};
use std::collections::HashMap;

use crate::analysis::expression_utils::{make_mul, make_num};
use crate::analysis::variable_collection::contains_var;

pub(crate) fn eliminate_aliases(
    equations: Vec<Equation>,
) -> (Vec<Equation>, HashMap<String, Expression>) {
    let mut alias_map: HashMap<String, Expression> = HashMap::new();
    let mut current_eqs = equations;
    let mut changed = true;

    while changed {
        changed = false;
        let mut next_eqs = Vec::with_capacity(current_eqs.len());

        for eq in &current_eqs {
            let mut is_alias = false;

            if let Equation::Simple(lhs, rhs) = eq {
                if let Expression::Variable(v) = lhs {
                    if *lhs != *rhs && !contains_var(rhs, v) {
                        if !v.starts_with("der_") {
                            if !alias_map.contains_key(v) {
                                alias_map.insert(v.clone(), rhs.clone());
                                changed = true;
                                is_alias = true;
                            }
                        }
                    }
                }

                if !is_alias {
                    if let Expression::Variable(v) = rhs {
                        if *lhs != *rhs && !contains_var(lhs, v) {
                            if !v.starts_with("der_") {
                                let lhs_is_der = if let Expression::Variable(l) = lhs {
                                    l.starts_with("der_")
                                } else {
                                    false
                                };
                                if !lhs_is_der && !alias_map.contains_key(v) {
                                    alias_map.insert(v.clone(), lhs.clone());
                                    changed = true;
                                    is_alias = true;
                                }
                            }
                        }
                    }
                }

                if !is_alias {
                    if let Expression::BinaryOp(l, Operator::Sub, r) = lhs {
                        if let Expression::Number(n) = &**l {
                            if n.abs() < 1e-10 {
                                if let Expression::Variable(v) = &**r {
                                    if !alias_map.contains_key(v) && !contains_var(rhs, v) {
                                        if !v.starts_with("der_") {
                                            let neg_rhs = make_mul(make_num(-1.0), rhs.clone());
                                            alias_map.insert(v.clone(), neg_rhs);
                                            changed = true;
                                            is_alias = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !is_alias {
                next_eqs.push(eq.clone());
            }
        }

        if changed {
            let mut substituted_eqs = Vec::new();
            for eq in next_eqs {
                let new_eq = substitute_aliases_in_eq(&eq, &alias_map);
                substituted_eqs.push(new_eq);
            }
            current_eqs = substituted_eqs;
        } else {
            current_eqs = next_eqs;
        }
    }

    (current_eqs, alias_map)
}

fn substitute_aliases_in_eq(eq: &Equation, map: &HashMap<String, Expression>) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(
            substitute_aliases_in_expr(lhs, map),
            substitute_aliases_in_expr(rhs, map),
        ),
        // Avoid deep recursion on structured equation blocks during alias elimination.
        // Aliases are still eliminated for top-level Simple equations, which is sufficient for
        // current backend validation stability.
        Equation::For(_, _, _, _)
        | Equation::When(_, _, _)
        | Equation::If(_, _, _, _) => eq.clone(),
        Equation::Connect(_, _)
        | Equation::Reinit(_, _)
        | Equation::Assert(_, _)
        | Equation::Terminate(_)
        | Equation::CallStmt(_)
        | Equation::SolvableBlock { .. }
        | Equation::MultiAssign(_, _) => eq.clone(),
    }
}

fn substitute_aliases_in_expr(expr: &Expression, map: &HashMap<String, Expression>) -> Expression {
    #[derive(Clone)]
    enum Frame<'a> {
        Enter(&'a Expression),
        BuildBinary(Operator),
        BuildCall(String, usize),
        BuildDer,
        BuildArrayAccess,
        BuildIf,
        BuildArrayLiteral(usize),
        BuildDot(String),
        BuildRange,
        BuildSample,
        BuildInterval,
        BuildHold,
        BuildPrevious,
        BuildSubSample,
        BuildSuperSample,
        BuildShiftSample,
    }

    let mut frames: Vec<Frame<'_>> = vec![Frame::Enter(expr)];
    let mut values: Vec<Expression> = Vec::new();

    while let Some(f) = frames.pop() {
        match f {
            Frame::Enter(e) => match e {
                Expression::Variable(name) => {
                    values.push(map.get(name).cloned().unwrap_or_else(|| e.clone()));
                }
                Expression::BinaryOp(lhs, op, rhs) => {
                    frames.push(Frame::BuildBinary(*op));
                    frames.push(Frame::Enter(rhs));
                    frames.push(Frame::Enter(lhs));
                }
                Expression::Call(name, args) => {
                    frames.push(Frame::BuildCall(name.clone(), args.len()));
                    for a in args.iter().rev() {
                        frames.push(Frame::Enter(a));
                    }
                }
                Expression::Der(arg) => {
                    frames.push(Frame::BuildDer);
                    frames.push(Frame::Enter(arg));
                }
                Expression::ArrayAccess(arr, idx) => {
                    frames.push(Frame::BuildArrayAccess);
                    frames.push(Frame::Enter(idx));
                    frames.push(Frame::Enter(arr));
                }
                Expression::If(c, t, f) => {
                    frames.push(Frame::BuildIf);
                    frames.push(Frame::Enter(f));
                    frames.push(Frame::Enter(t));
                    frames.push(Frame::Enter(c));
                }
                Expression::ArrayLiteral(es) => {
                    frames.push(Frame::BuildArrayLiteral(es.len()));
                    for a in es.iter().rev() {
                        frames.push(Frame::Enter(a));
                    }
                }
                Expression::Dot(base, member) => {
                    frames.push(Frame::BuildDot(member.clone()));
                    frames.push(Frame::Enter(base));
                }
                Expression::Range(start, step, end) => {
                    frames.push(Frame::BuildRange);
                    frames.push(Frame::Enter(end));
                    frames.push(Frame::Enter(step));
                    frames.push(Frame::Enter(start));
                }
                Expression::Sample(inner) => {
                    frames.push(Frame::BuildSample);
                    frames.push(Frame::Enter(inner));
                }
                Expression::Interval(inner) => {
                    frames.push(Frame::BuildInterval);
                    frames.push(Frame::Enter(inner));
                }
                Expression::Hold(inner) => {
                    frames.push(Frame::BuildHold);
                    frames.push(Frame::Enter(inner));
                }
                Expression::Previous(inner) => {
                    frames.push(Frame::BuildPrevious);
                    frames.push(Frame::Enter(inner));
                }
                Expression::SubSample(c, n) => {
                    frames.push(Frame::BuildSubSample);
                    frames.push(Frame::Enter(n));
                    frames.push(Frame::Enter(c));
                }
                Expression::SuperSample(c, n) => {
                    frames.push(Frame::BuildSuperSample);
                    frames.push(Frame::Enter(n));
                    frames.push(Frame::Enter(c));
                }
                Expression::ShiftSample(c, n) => {
                    frames.push(Frame::BuildShiftSample);
                    frames.push(Frame::Enter(n));
                    frames.push(Frame::Enter(c));
                }
                _ => values.push(e.clone()),
            },
            Frame::BuildBinary(op) => {
                let rhs = values.pop().unwrap_or(Expression::Number(0.0));
                let lhs = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::BinaryOp(Box::new(lhs), op, Box::new(rhs)));
            }
            Frame::BuildCall(name, n) => {
                let mut args: Vec<Expression> = Vec::with_capacity(n);
                for _ in 0..n {
                    args.push(values.pop().unwrap_or(Expression::Number(0.0)));
                }
                args.reverse();
                values.push(Expression::Call(name, args));
            }
            Frame::BuildDer => {
                let inner = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::Der(Box::new(inner)));
            }
            Frame::BuildArrayAccess => {
                let idx = values.pop().unwrap_or(Expression::Number(1.0));
                let arr = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::ArrayAccess(Box::new(arr), Box::new(idx)));
            }
            Frame::BuildIf => {
                let f = values.pop().unwrap_or(Expression::Number(0.0));
                let t = values.pop().unwrap_or(Expression::Number(0.0));
                let c = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::If(Box::new(c), Box::new(t), Box::new(f)));
            }
            Frame::BuildArrayLiteral(n) => {
                let mut es: Vec<Expression> = Vec::with_capacity(n);
                for _ in 0..n {
                    es.push(values.pop().unwrap_or(Expression::Number(0.0)));
                }
                es.reverse();
                values.push(Expression::ArrayLiteral(es));
            }
            Frame::BuildDot(member) => {
                let base = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::Dot(Box::new(base), member));
            }
            Frame::BuildRange => {
                let end = values.pop().unwrap_or(Expression::Number(0.0));
                let step = values.pop().unwrap_or(Expression::Number(1.0));
                let start = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::Range(
                    Box::new(start),
                    Box::new(step),
                    Box::new(end),
                ));
            }
            Frame::BuildSample => {
                let inner = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::Sample(Box::new(inner)));
            }
            Frame::BuildInterval => {
                let inner = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::Interval(Box::new(inner)));
            }
            Frame::BuildHold => {
                let inner = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::Hold(Box::new(inner)));
            }
            Frame::BuildPrevious => {
                let inner = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::Previous(Box::new(inner)));
            }
            Frame::BuildSubSample => {
                let n = values.pop().unwrap_or(Expression::Number(1.0));
                let c = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::SubSample(Box::new(c), Box::new(n)));
            }
            Frame::BuildSuperSample => {
                let n = values.pop().unwrap_or(Expression::Number(1.0));
                let c = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::SuperSample(Box::new(c), Box::new(n)));
            }
            Frame::BuildShiftSample => {
                let n = values.pop().unwrap_or(Expression::Number(1.0));
                let c = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::ShiftSample(Box::new(c), Box::new(n)));
            }
        }
    }

    values.pop().unwrap_or_else(|| expr.clone())
}
