//! Time derivatives of scalarized MultiBody Frames calls.
//!
//! Flatten stores Orientation as a quaternion Q[4]; MSL `R.w` is recovered from
//! `planarRotation(e, angle, der_angle)` (`w = e * der_angle`) and
//! `absoluteRotation` (`w = resolve2(R_rel, R1.w) + R_rel.w`).
//! `der(resolve1(R, v)) = resolve1(R, der(v)) + cross(angularVelocity1(R), resolve1(R, v))`.

use crate::ast::{Equation, Expression, Operator};
use crate::string_intern::resolve_id;
use std::collections::{HashMap, HashSet};

pub fn expand_der_frames_expr(inner: &Expression) -> Option<Expression> {
    if !contains_frames_kinematics(inner) {
        return None;
    }
    let empty_defs = HashMap::new();
    let empty = HashSet::new();
    Some(time_der_expr(
        inner, &empty_defs, &empty, &empty, 0,
    ))
}

/// Rewrite `der(algebraic)` / `der_*` using defining equations, and fold
/// `angularVelocity2(R)` when `R` is a Frames orientation Call.
pub fn expand_frames_time_derivatives(
    eqs: Vec<Equation>,
    states: &HashSet<String>,
    known: &HashSet<String>,
) -> Vec<Equation> {
    let mut eqs = eqs;
    for _ in 0..8 {
        let defs = collect_simple_defs(&eqs);
        let mut changed = false;
        eqs = eqs
            .into_iter()
            .map(|eq| rewrite_eq(eq, &defs, states, known, &mut changed))
            .collect();
        if !changed {
            break;
        }
    }
    crate::analysis::frames_inner::rewrite_mb_inner_products(eqs)
}

fn collect_simple_defs(eqs: &[Equation]) -> HashMap<String, Expression> {
    let mut defs = HashMap::new();
    for eq in eqs {
        collect_simple_defs_eq(eq, &mut defs);
    }
    defs
}

fn collect_simple_defs_eq(eq: &Equation, defs: &mut HashMap<String, Expression>) {
    match eq {
        Equation::Simple(Expression::Variable(id), rhs) => {
            defs.entry(resolve_id(*id)).or_insert_with(|| rhs.clone());
        }
        Equation::If(_, then_eqs, elifs, else_eqs) => {
            for e in then_eqs {
                collect_simple_defs_eq(e, defs);
            }
            for (_, body) in elifs {
                for e in body {
                    collect_simple_defs_eq(e, defs);
                }
            }
            if let Some(body) = else_eqs {
                for e in body {
                    collect_simple_defs_eq(e, defs);
                }
            }
        }
        Equation::For(_, _, _, body) => {
            for e in body {
                collect_simple_defs_eq(e, defs);
            }
        }
        _ => {}
    }
}

fn rewrite_eq(
    eq: Equation,
    defs: &HashMap<String, Expression>,
    states: &HashSet<String>,
    known: &HashSet<String>,
    changed: &mut bool,
) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => {
            Equation::Simple(lhs, rewrite_expr(&rhs, defs, states, known, changed))
        }
        Equation::If(c, then_eqs, elifs, else_eqs) => Equation::If(
            rewrite_expr(&c, defs, states, known, changed),
            then_eqs
                .into_iter()
                .map(|e| rewrite_eq(e, defs, states, known, changed))
                .collect(),
            elifs
                .into_iter()
                .map(|(ec, body)| {
                    (
                        rewrite_expr(&ec, defs, states, known, changed),
                        body.into_iter()
                            .map(|e| rewrite_eq(e, defs, states, known, changed))
                            .collect(),
                    )
                })
                .collect(),
            else_eqs.map(|body| {
                body.into_iter()
                    .map(|e| rewrite_eq(e, defs, states, known, changed))
                    .collect()
            }),
        ),
        other => other,
    }
}

fn rewrite_expr(
    expr: &Expression,
    defs: &HashMap<String, Expression>,
    states: &HashSet<String>,
    known: &HashSet<String>,
    changed: &mut bool,
) -> Expression {
    match expr {
        Expression::Der(inner) => {
            *changed = true;
            time_der_expr(inner, defs, states, known, 0)
        }
        Expression::Variable(id) => {
            let name = resolve_id(*id);
            if let Some(base) = name.strip_prefix("der_") {
                if !states.contains(base) {
                    if let Some(rhs) = defs.get(base) {
                        *changed = true;
                        return time_der_expr(rhs, defs, states, known, 0);
                    }
                }
            }
            expr.clone()
        }
        Expression::Call(fname, args) => {
            let args: Vec<Expression> = args
                .iter()
                .map(|a| rewrite_expr(a, defs, states, known, changed))
                .collect();
            if frames_tail(fname) == "angularVelocity2" || frames_tail(fname) == "angularVelocity1" {
                if let Some(expanded) =
                    expand_angular_velocity(fname, &args, None, defs, states, known, 0)
                {
                    *changed = true;
                    return expanded;
                }
            }
            Expression::Call(fname.clone(), args)
        }
        Expression::ArrayAccess(arr, idx) => {
            let arr_r = rewrite_expr(arr, defs, states, known, changed);
            let idx_r = rewrite_expr(idx, defs, states, known, changed);
            if let (Expression::Call(fname, args), Expression::Number(n)) = (&arr_r, &idx_r) {
                if frames_tail(fname) == "angularVelocity2"
                    || frames_tail(fname) == "angularVelocity1"
                {
                    if let Some(expanded) = expand_angular_velocity(
                        fname,
                        args,
                        Some(*n as i64),
                        defs,
                        states,
                        known,
                        0,
                    ) {
                        *changed = true;
                        return expanded;
                    }
                }
            }
            Expression::ArrayAccess(Box::new(arr_r), Box::new(idx_r))
        }
        Expression::BinaryOp(l, op, r) => Expression::BinaryOp(
            Box::new(rewrite_expr(l, defs, states, known, changed)),
            *op,
            Box::new(rewrite_expr(r, defs, states, known, changed)),
        ),
        Expression::If(c, t, f) => Expression::If(
            Box::new(rewrite_expr(c, defs, states, known, changed)),
            Box::new(rewrite_expr(t, defs, states, known, changed)),
            Box::new(rewrite_expr(f, defs, states, known, changed)),
        ),
        Expression::ArrayLiteral(es) => Expression::ArrayLiteral(
            es.iter()
                .map(|e| rewrite_expr(e, defs, states, known, changed))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn time_der_expr(
    expr: &Expression,
    defs: &HashMap<String, Expression>,
    states: &HashSet<String>,
    known: &HashSet<String>,
    depth: usize,
) -> Expression {
    if depth > 16 {
        return Expression::Number(0.0);
    }
    match expr {
        Expression::Number(_) => Expression::Number(0.0),
        Expression::Der(inner) => {
            let first = time_der_expr(inner, defs, states, known, depth + 1);
            time_der_expr(&first, defs, states, known, depth + 1)
        }
        Expression::Variable(id) => time_der_variable(resolve_id(*id), defs, states, known, depth),
        Expression::BinaryOp(l, Operator::Add, r) => Expression::BinaryOp(
            Box::new(time_der_expr(l, defs, states, known, depth + 1)),
            Operator::Add,
            Box::new(time_der_expr(r, defs, states, known, depth + 1)),
        ),
        Expression::BinaryOp(l, Operator::Sub, r) => Expression::BinaryOp(
            Box::new(time_der_expr(l, defs, states, known, depth + 1)),
            Operator::Sub,
            Box::new(time_der_expr(r, defs, states, known, depth + 1)),
        ),
        Expression::BinaryOp(l, Operator::Mul, r) => {
            let dl = time_der_expr(l, defs, states, known, depth + 1);
            let dr = time_der_expr(r, defs, states, known, depth + 1);
            add(
                Expression::BinaryOp(l.clone(), Operator::Mul, Box::new(dr)),
                Expression::BinaryOp(Box::new(dl), Operator::Mul, r.clone()),
            )
        }
        Expression::BinaryOp(l, Operator::Div, r) => {
            if let Expression::Number(c) = r.as_ref() {
                if c.abs() > 1e-15 {
                    return Expression::BinaryOp(
                        Box::new(time_der_expr(l, defs, states, known, depth + 1)),
                        Operator::Div,
                        r.clone(),
                    );
                }
            }
            Expression::Number(0.0)
        }
        Expression::ArrayLiteral(items) => Expression::ArrayLiteral(
            items
                .iter()
                .map(|e| time_der_expr(e, defs, states, known, depth + 1))
                .collect(),
        ),
        Expression::ArrayAccess(arr, idx) => {
            if let Expression::Number(n) = idx.as_ref() {
                let i = *n as i64;
                match arr.as_ref() {
                    Expression::Call(fname, args) => {
                        frames_call_der(fname, args, Some(i), defs, states, known, depth)
                    }
                    Expression::ArrayLiteral(items) if i >= 1 && (i as usize) <= items.len() => {
                        time_der_expr(&items[(i as usize) - 1], defs, states, known, depth + 1)
                    }
                    Expression::Variable(id) => time_der_variable(
                        format!("{}_{}", resolve_id(*id), i),
                        defs,
                        states,
                        known,
                        depth,
                    ),
                    _ => at(
                        time_der_expr(arr, defs, states, known, depth + 1),
                        i,
                    ),
                }
            } else {
                Expression::Number(0.0)
            }
        }
        Expression::Call(fname, args) => {
            frames_call_der(fname, args, None, defs, states, known, depth)
        }
        other => crate::analysis::derivative::expand_der_linear(other)
            .unwrap_or_else(|| Expression::Number(0.0)),
    }
}

fn time_der_variable(
    name: String,
    defs: &HashMap<String, Expression>,
    states: &HashSet<String>,
    known: &HashSet<String>,
    depth: usize,
) -> Expression {
    if let Some(base) = name.strip_prefix("der_") {
        // Differentiating the symbol der_X is d2X/dt2.
        if states.contains(&name) {
            return Expression::var(&format!("der_{name}"));
        }
        if states.contains(base) {
            if let Some(rhs) = defs.get(&name) {
                return time_der_expr(rhs, defs, states, known, depth + 1);
            }
            return Expression::var(&format!("der_{name}"));
        }
        if known.contains(base) && !states.contains(base) {
            return Expression::Number(0.0);
        }
        if let Some(rhs) = defs.get(base) {
            let first = time_der_expr(rhs, defs, states, known, depth + 1);
            return time_der_expr(&first, defs, states, known, depth + 1);
        }
        return Expression::var(&format!("der_{name}"));
    }
    if states.contains(&name) {
        return Expression::var(&format!("der_{name}"));
    }
    if known.contains(&name) && !states.contains(&name) {
        return Expression::Number(0.0);
    }
    if let Some(rhs) = defs.get(&name) {
        return time_der_expr(rhs, defs, states, known, depth + 1);
    }
    Expression::var(&format!("der_{name}"))
}

fn frames_call_der(
    fname: &str,
    args: &[Expression],
    idx: Option<i64>,
    defs: &HashMap<String, Expression>,
    states: &HashSet<String>,
    known: &HashSet<String>,
    depth: usize,
) -> Expression {
    let tail = frames_tail(fname);
    match tail {
        "resolve1" if args.len() >= 2 => {
            let r = args[0].clone();
            let v = args[1].clone();
            let w2 = angular_velocity2_expr(&r, defs, states, known, depth);
            let w1 = Expression::Call(fname.to_string(), vec![r.clone(), w2]);
            let v1 = Expression::Call(fname.to_string(), vec![r.clone(), v.clone()]);
            let cr = Expression::Call("cross".to_string(), vec![w1, v1]);
            let dv = time_der_expr(&v, defs, states, known, depth + 1);
            let rdv = Expression::Call(fname.to_string(), vec![r, dv]);
            maybe_at(add(cr, rdv), idx)
        }
        "resolve2" if args.len() >= 2 => {
            let r = args[0].clone();
            let v = args[1].clone();
            let w2 = angular_velocity2_expr(&r, defs, states, known, depth);
            let v2 = Expression::Call(fname.to_string(), vec![r.clone(), v.clone()]);
            let cr = Expression::Call("cross".to_string(), vec![w2, v2]);
            let dv = time_der_expr(&v, defs, states, known, depth + 1);
            let rdv = Expression::Call(fname.to_string(), vec![r, dv]);
            maybe_at(sub(rdv, cr), idx)
        }
        "planarRotation" => match idx {
            Some(i) => planar_rotation_der(args, i, defs, states, known, depth),
            None => Expression::ArrayLiteral(
                (1..=4)
                    .map(|i| planar_rotation_der(args, i, defs, states, known, depth))
                    .collect(),
            ),
        },
        "absoluteRotation" if args.len() >= 2 => {
            // Product rule on the quaternion composition: d(Q1 o Qr)/dt.
            let d1: Vec<Expression> = (1..=4)
                .map(|i| {
                    time_der_expr(&at(args[0].clone(), i), defs, states, known, depth + 1)
                })
                .collect();
            let dr: Vec<Expression> = (1..=4)
                .map(|i| {
                    time_der_expr(&at(args[1].clone(), i), defs, states, known, depth + 1)
                })
                .collect();
            let term1 = Expression::Call(
                fname.to_string(),
                vec![quat_literal(d1), args[1].clone()],
            );
            let term2 = Expression::Call(
                fname.to_string(),
                vec![args[0].clone(), quat_literal(dr)],
            );
            maybe_at(add(term1, term2), idx)
        }
        "nullRotation" | "zeros" | "ones" | "identity" => Expression::Number(0.0),
        "angularVelocity2" | "angularVelocity1" => {
            match expand_angular_velocity(fname, args, idx, defs, states, known, depth) {
                Some(w) => time_der_expr(&w, defs, states, known, depth + 1),
                None => Expression::Number(0.0),
            }
        }
        "cross" if args.len() >= 2 => {
            let da = time_der_expr(&args[0], defs, states, known, depth + 1);
            let db = time_der_expr(&args[1], defs, states, known, depth + 1);
            let t1 = Expression::Call("cross".to_string(), vec![da, args[1].clone()]);
            let t2 = Expression::Call("cross".to_string(), vec![args[0].clone(), db]);
            maybe_at(add(t1, t2), idx)
        }
        "sin" if args.len() == 1 => {
            let du = time_der_expr(&args[0], defs, states, known, depth + 1);
            Expression::BinaryOp(
                Box::new(Expression::Call("cos".to_string(), vec![args[0].clone()])),
                Operator::Mul,
                Box::new(du),
            )
        }
        "cos" if args.len() == 1 => {
            let du = time_der_expr(&args[0], defs, states, known, depth + 1);
            Expression::BinaryOp(
                Box::new(Expression::Number(0.0)),
                Operator::Sub,
                Box::new(Expression::BinaryOp(
                    Box::new(Expression::Call("sin".to_string(), vec![args[0].clone()])),
                    Operator::Mul,
                    Box::new(du),
                )),
            )
        }
        _ => Expression::Number(0.0),
    }
}

fn planar_rotation_der(
    args: &[Expression],
    idx: i64,
    defs: &HashMap<String, Expression>,
    states: &HashSet<String>,
    known: &HashSet<String>,
    depth: usize,
) -> Expression {
    if args.len() < 2 {
        return Expression::Number(0.0);
    }
    let angle = args[1].clone();
    let dangle = if args.len() >= 3 {
        args[2].clone()
    } else {
        time_der_expr(&angle, defs, states, known, depth + 1)
    };
    let half = Expression::BinaryOp(
        Box::new(angle),
        Operator::Mul,
        Box::new(Expression::Number(0.5)),
    );
    let dhalf = Expression::BinaryOp(
        Box::new(dangle),
        Operator::Mul,
        Box::new(Expression::Number(0.5)),
    );
    if idx == 4 || idx == 0 {
        // d(cos(angle/2))/dt = -sin(angle/2) * der(angle)/2
        return Expression::BinaryOp(
            Box::new(Expression::Number(0.0)),
            Operator::Sub,
            Box::new(Expression::BinaryOp(
                Box::new(Expression::Call("sin".to_string(), vec![half])),
                Operator::Mul,
                Box::new(dhalf),
            )),
        );
    }
    // d(e_i * sin(angle/2))/dt = e_i * cos(angle/2) * der(angle)/2
    Expression::BinaryOp(
        Box::new(at(args[0].clone(), idx)),
        Operator::Mul,
        Box::new(Expression::BinaryOp(
            Box::new(Expression::Call("cos".to_string(), vec![half])),
            Operator::Mul,
            Box::new(dhalf),
        )),
    )
}

fn angular_velocity2_expr(
    r: &Expression,
    defs: &HashMap<String, Expression>,
    states: &HashSet<String>,
    known: &HashSet<String>,
    depth: usize,
) -> Expression {
    expand_angular_velocity("angularVelocity2", &[r.clone()], None, defs, states, known, depth)
        .unwrap_or_else(|| frames_call("angularVelocity2", vec![r.clone()]))
}

fn expand_angular_velocity(
    fname: &str,
    args: &[Expression],
    idx: Option<i64>,
    defs: &HashMap<String, Expression>,
    states: &HashSet<String>,
    known: &HashSet<String>,
    depth: usize,
) -> Option<Expression> {
    if args.is_empty() || depth > 12 {
        return Some(Expression::Number(0.0));
    }
    let w2 = expand_angular_velocity2(&args[0], defs, states, known, depth)?;
    if frames_tail(fname) == "angularVelocity1" {
        let w1 = frames_call("resolve1", vec![args[0].clone(), w2]);
        return Some(maybe_at(w1, idx));
    }
    Some(maybe_at(w2, idx))
}

fn expand_angular_velocity2(
    r: &Expression,
    defs: &HashMap<String, Expression>,
    states: &HashSet<String>,
    known: &HashSet<String>,
    depth: usize,
) -> Option<Expression> {
    let (fname, args) = orientation_call(r, defs, depth)?;
    match frames_tail(&fname) {
        "nullRotation" => Some(Expression::ArrayLiteral(vec![
            Expression::Number(0.0),
            Expression::Number(0.0),
            Expression::Number(0.0),
        ])),
        "planarRotation" if args.len() >= 3 => {
            // R.w = e * der_angle (MSL Frames.planarRotation)
            Some(scale_vec3(args[0].clone(), args[2].clone()))
        }
        "planarRotation" if args.len() == 2 => {
            let da = time_der_expr(&args[1], defs, states, known, depth + 1);
            Some(scale_vec3(args[0].clone(), da))
        }
        "absoluteRotation" if args.len() >= 2 => {
            let w1 = angular_velocity2_expr(&args[0], defs, states, known, depth + 1);
            let wrel = angular_velocity2_expr(&args[1], defs, states, known, depth + 1);
            let mapped = frames_call("resolve2", vec![args[1].clone(), w1]);
            Some(add(mapped, wrel))
        }
        _ => None,
    }
}

fn orientation_call(
    expr: &Expression,
    defs: &HashMap<String, Expression>,
    depth: usize,
) -> Option<(String, Vec<Expression>)> {
    if depth > 10 {
        return None;
    }
    match expr {
        Expression::Call(n, a) if is_orientation_call(n) => Some((n.clone(), a.clone())),
        Expression::ArrayAccess(inner, _) => orientation_call(inner, defs, depth + 1),
        Expression::Variable(id) => {
            let name = resolve_id(*id);
            let rhs = defs
                .get(&name)
                .or_else(|| defs.get(&format!("{name}_1")))?;
            if let Expression::Variable(id2) = rhs {
                let n2 = resolve_id(*id2);
                let base = strip_index_suffix(&n2);
                return orientation_call(&Expression::var(&base), defs, depth + 1);
            }
            orientation_call(rhs, defs, depth + 1)
        }
        _ => None,
    }
}

fn is_orientation_call(name: &str) -> bool {
    matches!(
        frames_tail(name),
        "planarRotation"
            | "absoluteRotation"
            | "nullRotation"
            | "from_Q"
            | "from_T"
            | "axesRotations"
            | "axisRotation"
    )
}

fn frames_tail(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn frames_qualified(kind: &str) -> String {
    format!("Modelica.Mechanics.MultiBody.Frames.{kind}")
}

fn frames_call(kind: &str, args: Vec<Expression>) -> Expression {
    Expression::Call(frames_qualified(kind), args)
}

fn contains_frames_kinematics(expr: &Expression) -> bool {
    match expr {
        Expression::Call(fname, args) => {
            matches!(
                frames_tail(fname),
                "resolve1"
                    | "resolve2"
                    | "planarRotation"
                    | "absoluteRotation"
                    | "angularVelocity1"
                    | "angularVelocity2"
                    | "nullRotation"
                    | "cross"
            ) || args.iter().any(contains_frames_kinematics)
        }
        Expression::ArrayAccess(arr, idx) => {
            contains_frames_kinematics(arr) || contains_frames_kinematics(idx)
        }
        Expression::BinaryOp(l, _, r) => {
            contains_frames_kinematics(l) || contains_frames_kinematics(r)
        }
        Expression::ArrayLiteral(es) => es.iter().any(contains_frames_kinematics),
        Expression::Der(inner) => contains_frames_kinematics(inner),
        Expression::If(c, t, f) => {
            contains_frames_kinematics(c)
                || contains_frames_kinematics(t)
                || contains_frames_kinematics(f)
        }
        _ => false,
    }
}

fn strip_index_suffix(name: &str) -> String {
    if let Some((base, idx)) = name.rsplit_once('_') {
        if idx.parse::<u32>().is_ok() {
            return base.to_string();
        }
    }
    name.to_string()
}

fn at(expr: Expression, i: i64) -> Expression {
    Expression::ArrayAccess(Box::new(expr), Box::new(Expression::Number(i as f64)))
}

fn maybe_at(expr: Expression, idx: Option<i64>) -> Expression {
    match idx {
        Some(i) if i > 0 => at(expr, i),
        _ => expr,
    }
}

fn add(a: Expression, b: Expression) -> Expression {
    Expression::BinaryOp(Box::new(a), Operator::Add, Box::new(b))
}

fn sub(a: Expression, b: Expression) -> Expression {
    Expression::BinaryOp(Box::new(a), Operator::Sub, Box::new(b))
}

fn scale_vec3(v: Expression, s: Expression) -> Expression {
    Expression::ArrayLiteral(vec![
        Expression::BinaryOp(
            Box::new(s.clone()),
            Operator::Mul,
            Box::new(at(v.clone(), 1)),
        ),
        Expression::BinaryOp(
            Box::new(s.clone()),
            Operator::Mul,
            Box::new(at(v.clone(), 2)),
        ),
        Expression::BinaryOp(Box::new(s), Operator::Mul, Box::new(at(v, 3))),
    ])
}

fn quat_literal(comps: Vec<Expression>) -> Expression {
    Expression::ArrayLiteral(comps)
}
