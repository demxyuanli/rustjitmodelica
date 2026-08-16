//! Scalarize Real[3] inner products and keep `0 = e · t` as a Newton residual.
//!
//! MSL `v1 * v2` of two vectors is the inner product. After aliasing
//! `constantTorque.tau = 0` through the flange sum, BLT otherwise assigns
//! `revolute.tau = 0` and drops `tau = -frame_b.t · e`, so rod torque never
//! determines `der(w)`.

use crate::ast::{Equation, Expression, Operator};
use crate::string_intern::resolve_id;
use std::collections::HashSet;

pub(crate) fn rewrite_mb_inner_products(eqs: Vec<Equation>) -> Vec<Equation> {
    let eqs = expand_vector_inner_products(eqs);
    let eqs = promote_zero_assigned_defs_to_residuals(eqs);
    bind_internal_axis_tau(eqs)
}

fn expand_vector_inner_products(eqs: Vec<Equation>) -> Vec<Equation> {
    let vec3_bases = collect_vec3_bases(&eqs);
    eqs.into_iter()
        .map(|eq| expand_inner_product_eq(eq, &vec3_bases))
        .collect()
}

fn collect_vec3_bases(eqs: &[Equation]) -> HashSet<String> {
    let mut names = HashSet::new();
    for eq in eqs {
        crate::analysis::variable_collection::collect_vars_eq(eq, &mut names);
    }
    let mut bases = HashSet::new();
    for name in &names {
        if let Some((base, idx)) = name.rsplit_once('_') {
            if idx == "1"
                && names.contains(&format!("{base}_2"))
                && names.contains(&format!("{base}_3"))
            {
                bases.insert(base.to_string());
            }
        }
    }
    bases
}

fn expand_inner_product_eq(eq: Equation, vec3_bases: &HashSet<String>) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(
            fold_trivial_arith(expand_inner_product_expr(lhs, vec3_bases)),
            fold_trivial_arith(expand_inner_product_expr(rhs, vec3_bases)),
        ),
        Equation::If(c, then_eqs, elifs, else_eqs) => {
            let (then_eqs, elifs, else_eqs) = map_if_eq(
                then_eqs,
                elifs,
                else_eqs,
                |e| expand_inner_product_eq(e, vec3_bases),
                |ec| fold_trivial_arith(expand_inner_product_expr(ec, vec3_bases)),
            );
            Equation::If(
                fold_trivial_arith(expand_inner_product_expr(c, vec3_bases)),
                then_eqs,
                elifs,
                else_eqs,
            )
        }
        other => other,
    }
}

fn expand_inner_product_expr(expr: Expression, vec3_bases: &HashSet<String>) -> Expression {
    match expr {
        Expression::BinaryOp(l, op, r) => {
            let l = expand_inner_product_expr(*l, vec3_bases);
            let r = expand_inner_product_expr(*r, vec3_bases);
            if op == Operator::Mul
                && expr_is_vec3_static(&l, vec3_bases)
                && expr_is_vec3_static(&r, vec3_bases)
            {
                return inner_product_sum(l, r, vec3_bases);
            }
            Expression::BinaryOp(Box::new(l), op, Box::new(r))
        }
        Expression::Call(fname, args) => Expression::Call(
            fname,
            args.into_iter()
                .map(|a| expand_inner_product_expr(a, vec3_bases))
                .collect(),
        ),
        Expression::ArrayAccess(arr, idx) => {
            let arr = expand_inner_product_expr(*arr, vec3_bases);
            let idx = expand_inner_product_expr(*idx, vec3_bases);
            if let Expression::Number(n) = &idx {
                let i = *n as i64;
                if (1..=3).contains(&i) {
                    return index_at(arr, i, vec3_bases);
                }
            }
            Expression::ArrayAccess(Box::new(arr), Box::new(idx))
        }
        Expression::If(c, t, f) => Expression::If(
            Box::new(expand_inner_product_expr(*c, vec3_bases)),
            Box::new(expand_inner_product_expr(*t, vec3_bases)),
            Box::new(expand_inner_product_expr(*f, vec3_bases)),
        ),
        Expression::ArrayLiteral(es) => Expression::ArrayLiteral(
            es.into_iter()
                .map(|e| expand_inner_product_expr(e, vec3_bases))
                .collect(),
        ),
        Expression::Dot(inner, member) => {
            let inner = expand_inner_product_expr(*inner, vec3_bases);
            let dotted = Expression::Dot(Box::new(inner.clone()), member.clone());
            if let Some(flat) = crate::ast::expr_to_flat_scalar_prefix(&dotted) {
                Expression::var(&flat)
            } else {
                dotted
            }
        }
        other => other,
    }
}

fn expr_is_vec3_static(expr: &Expression, vec3_bases: &HashSet<String>) -> bool {
    match expr {
        Expression::ArrayLiteral(items) => items.len() == 3,
        Expression::Variable(id) => {
            let name = resolve_id(*id);
            vec3_bases.contains(&name) || is_likely_vec3_name(&name)
        }
        Expression::Call(fname, _) => matches!(
            frames_tail(fname),
            "normalize"
                | "normalizeWithAssert"
                | "cross"
                | "resolve1"
                | "resolve2"
                | "angularVelocity1"
                | "angularVelocity2"
                | "zeros"
                | "ones"
        ),
        Expression::ArrayAccess(arr, _) => expr_is_vec3_static(arr, vec3_bases),
        Expression::BinaryOp(l, Operator::Add | Operator::Sub, r) => {
            expr_is_vec3_static(l, vec3_bases) || expr_is_vec3_static(r, vec3_bases)
        }
        _ => false,
    }
}

fn is_likely_vec3_name(name: &str) -> bool {
    name.ends_with("_t")
        || name.ends_with("_f")
        || name.ends_with("_r_0")
        || name.ends_with("_n")
        || name.ends_with("_a_0")
        || name.ends_with("_v_0")
        || name.ends_with("_g_0")
        || name.ends_with("_w_a")
        || name.ends_with("_z_a")
}

fn inner_product_sum(a: Expression, b: Expression, vec3_bases: &HashSet<String>) -> Expression {
    let term = |i: i64| {
        Expression::BinaryOp(
            Box::new(vec_component(a.clone(), i, vec3_bases)),
            Operator::Mul,
            Box::new(vec_component(b.clone(), i, vec3_bases)),
        )
    };
    add(add(term(1), term(2)), term(3))
}

fn cross_component(
    a: Expression,
    b: Expression,
    i: i64,
    vec3_bases: &HashSet<String>,
) -> Expression {
    let a1 = vec_component(a.clone(), 1, vec3_bases);
    let a2 = vec_component(a.clone(), 2, vec3_bases);
    let a3 = vec_component(a, 3, vec3_bases);
    let b1 = vec_component(b.clone(), 1, vec3_bases);
    let b2 = vec_component(b.clone(), 2, vec3_bases);
    let b3 = vec_component(b, 3, vec3_bases);
    match i {
        1 => sub(mul(a2, b3), mul(a3, b2)),
        2 => sub(mul(a3, b1), mul(a1, b3)),
        _ => sub(mul(a1, b2), mul(a2, b1)),
    }
}

fn index_at(expr: Expression, i: i64, vec3_bases: &HashSet<String>) -> Expression {
    match expr {
        Expression::ArrayLiteral(items) => {
            array_item_at(&items, i).unwrap_or_else(|| at(Expression::ArrayLiteral(items), i))
        }
        Expression::Variable(_) => vec_component(expr, i, vec3_bases),
        Expression::BinaryOp(l, op @ (Operator::Add | Operator::Sub), r) => Expression::BinaryOp(
            Box::new(index_at(*l, i, vec3_bases)),
            op,
            Box::new(index_at(*r, i, vec3_bases)),
        ),
        Expression::BinaryOp(l, Operator::Mul, r) => {
            if expr_is_vec3_static(&r, vec3_bases) && !expr_is_vec3_static(&l, vec3_bases) {
                mul(*l, index_at(*r, i, vec3_bases))
            } else if expr_is_vec3_static(&l, vec3_bases) && !expr_is_vec3_static(&r, vec3_bases) {
                mul(index_at(*l, i, vec3_bases), *r)
            } else {
                at(mul(*l, *r), i)
            }
        }
        Expression::Call(fname, args) if frames_tail(&fname) == "cross" && args.len() >= 2 => {
            cross_component(args[0].clone(), args[1].clone(), i, vec3_bases)
        }
        Expression::Call(fname, args)
            if matches!(frames_tail(&fname), "resolve1" | "resolve2") && args.len() >= 2 =>
        {
            if args.get(1).is_some_and(expr_is_zero_vec) {
                Expression::Number(0.0)
            } else {
                quat_resolve_at(
                    frames_tail(&fname) == "resolve1",
                    args[0].clone(),
                    args[1].clone(),
                    i,
                    vec3_bases,
                )
            }
        }
        Expression::Call(fname, _) if frames_tail(&fname) == "zeros" => Expression::Number(0.0),
        Expression::Dot(_, _) => named_or_at(expr, i),
        Expression::ArrayAccess(inner, _) => index_at(*inner, i, vec3_bases),
        other => at(other, i),
    }
}

fn expr_is_zero_vec(expr: &Expression) -> bool {
    match expr {
        Expression::ArrayLiteral(items) => {
            items.len() == 3
                && items
                    .iter()
                    .all(|e| matches!(e, Expression::Number(n) if n.abs() < 1e-15))
        }
        Expression::Call(fname, _) => frames_tail(fname) == "zeros",
        _ => false,
    }
}

fn vec_component(expr: Expression, i: i64, vec3_bases: &HashSet<String>) -> Expression {
    match expr {
        Expression::Variable(id) => {
            let name = resolve_id(id);
            if vec3_bases.contains(&name) || is_likely_vec3_name(&name) {
                Expression::var(&format!("{name}_{i}"))
            } else {
                at(Expression::Variable(id), i)
            }
        }
        Expression::ArrayLiteral(items) => {
            array_item_at(&items, i).unwrap_or_else(|| at(Expression::ArrayLiteral(items), i))
        }
        Expression::Call(fname, args) if is_const_axis_normalize(&fname, &args) => {
            const_normalize_component(&args[0], i)
                .unwrap_or_else(|| at(Expression::Call(fname, args), i))
        }
        Expression::Dot(_, _) => named_or_at(expr, i),
        Expression::BinaryOp(_, _, _) | Expression::Call(_, _) => index_at(expr, i, vec3_bases),
        other => at(other, i),
    }
}

fn is_const_axis_normalize(fname: &str, args: &[Expression]) -> bool {
    matches!(frames_tail(fname), "normalize" | "normalizeWithAssert")
        && args.first().is_some_and(|a| {
            matches!(a, Expression::ArrayLiteral(items) if items.len() == 3
                && items.iter().all(|e| matches!(e, Expression::Number(_))))
        })
}

/// MSL Quaternions.resolve1 / resolve2, same formula as JIT `quat_resolve`.
/// `v1 = 2 * ((q4^2 - 0.5)*v + (u·v)*u ± q4*cross(u,v))` (`resolve2` uses `-q4`).
fn quat_resolve_at(
    resolve1: bool,
    q: Expression,
    v: Expression,
    i: i64,
    vec3_bases: &HashSet<String>,
) -> Expression {
    let q1 = quat_at(&q, 1);
    let q2 = quat_at(&q, 2);
    let q3 = quat_at(&q, 3);
    let q4 = quat_at(&q, 4);
    let v1 = index_at(v.clone(), 1, vec3_bases);
    let v2 = index_at(v.clone(), 2, vec3_bases);
    let v3 = index_at(v.clone(), 3, vec3_bases);
    let vi = pick3(i, &v1, &v2, &v3);
    let ui = pick3(i, &q1, &q2, &q3);
    let udotv = add(add(mul(q1.clone(), v1), mul(q2.clone(), v2)), mul(q3.clone(), v3));
    let cr = cross_component(
        Expression::ArrayLiteral(vec![q1, q2, q3]),
        v,
        i,
        vec3_bases,
    );
    let scale = sub(mul(q4.clone(), q4.clone()), Expression::Number(0.5));
    let signed_cr = if resolve1 {
        mul(q4, cr)
    } else {
        mul(sub(Expression::Number(0.0), q4), cr)
    };
    mul(
        Expression::Number(2.0),
        add(add(mul(scale, vi), mul(udotv, ui)), signed_cr),
    )
}

fn quat_at(q: &Expression, i: i64) -> Expression {
    match q {
        Expression::Variable(id) => {
            let name = resolve_id(*id);
            Expression::var(&format!("{name}_{i}"))
        }
        Expression::ArrayLiteral(items) => {
            array_item_at(items, i).unwrap_or_else(|| at(q.clone(), i))
        }
        Expression::Call(fname, _) if frames_tail(fname) == "nullRotation" => {
            if i == 4 {
                Expression::Number(1.0)
            } else {
                Expression::Number(0.0)
            }
        }
        Expression::Call(fname, args)
            if frames_tail(fname) == "planarRotation" && args.len() >= 2 =>
        {
            planar_quat_at(args, i)
        }
        Expression::Dot(_, _) => named_or_at(q.clone(), i),
        Expression::ArrayAccess(inner, _) => quat_at(inner, i),
        other => at(other.clone(), i),
    }
}

fn planar_quat_at(args: &[Expression], i: i64) -> Expression {
    let half = mul(args[1].clone(), Expression::Number(0.5));
    if i == 4 {
        Expression::Call("cos".to_string(), vec![half])
    } else {
        let ei = match &args[0] {
            Expression::ArrayLiteral(items) => {
                array_item_at(items, i).unwrap_or_else(|| at(args[0].clone(), i))
            }
            Expression::Variable(id) => {
                let name = resolve_id(*id);
                Expression::var(&format!("{name}_{i}"))
            }
            other => at(other.clone(), i),
        };
        mul(ei, Expression::Call("sin".to_string(), vec![half]))
    }
}

fn const_normalize_component(axis: &Expression, i: i64) -> Option<Expression> {
    let Expression::ArrayLiteral(items) = axis else {
        return None;
    };
    if i < 1 || (i as usize) > items.len() {
        return None;
    }
    let mut n2 = 0.0;
    let mut comps = [0.0; 3];
    for (k, e) in items.iter().take(3).enumerate() {
        let Expression::Number(v) = e else {
            return None;
        };
        comps[k] = *v;
        n2 += *v * *v;
    }
    if n2 < 1e-30 {
        return Some(Expression::Number(0.0));
    }
    let n = n2.sqrt();
    Some(Expression::Number(comps[(i as usize) - 1] / n))
}

/// MSL `InternalSupport internalAxis(tau=tau)` stores the modifier on an input
/// start value. Bind `*_internalAxis_tau = *_tau` so the flange sum sees the
/// joint scalar (needed for actuated joints; free pendulum uses the residual).
fn bind_internal_axis_tau(eqs: Vec<Equation>) -> Vec<Equation> {
    let mut names = HashSet::new();
    for eq in &eqs {
        crate::analysis::variable_collection::collect_vars_eq(eq, &mut names);
    }
    let mut extra = Vec::new();
    for n in &names {
        let Some(owner) = n.strip_suffix("_internalAxis_tau") else {
            continue;
        };
        if owner.is_empty() {
            continue;
        }
        let joint = format!("{owner}_tau");
        if already_binds_vars(&eqs, n, &joint) {
            continue;
        }
        extra.push(Equation::Simple(
            Expression::var(n),
            Expression::var(&joint),
        ));
    }
    if extra.is_empty() {
        return eqs;
    }
    let mut out = eqs;
    out.extend(extra);
    out
}

fn already_binds_vars(eqs: &[Equation], lhs: &str, rhs: &str) -> bool {
    eqs.iter().any(|eq| match eq {
        Equation::Simple(Expression::Variable(a), Expression::Variable(b)) => {
            resolve_id(*a) == lhs && resolve_id(*b) == rhs
        }
        _ => false,
    })
}

/// When `tau = 0` (motor) coexists with `tau = -t · e`, rewrite the projection
/// to `0 = -t · e` so BLT/Newton assign `t_axis` from rod torque, not `tau`.
/// Never rewrite `v = der_*` (that would freeze the pendulum).
fn promote_zero_assigned_defs_to_residuals(eqs: Vec<Equation>) -> Vec<Equation> {
    let zero_forced = collect_zero_forced_names(&eqs);
    if zero_forced.is_empty() {
        return eqs;
    }
    eqs.into_iter()
        .map(|eq| promote_zero_assigned_eq(eq, &zero_forced))
        .collect()
}

fn promote_zero_assigned_eq(eq: Equation, zero_forced: &HashSet<String>) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => {
            if let Expression::Variable(id) = &lhs {
                let name = resolve_id(*id);
                if is_tau_name(&name)
                    && tau_forced_by_motor(&name, zero_forced)
                    && expr_is_torque_projection(&rhs)
                {
                    return Equation::Simple(Expression::Number(0.0), rhs);
                }
            }
            Equation::Simple(lhs, rhs)
        }
        Equation::If(c, then_eqs, elifs, else_eqs) => {
            let (then_eqs, elifs, else_eqs) = map_if_eq(
                then_eqs,
                elifs,
                else_eqs,
                |e| promote_zero_assigned_eq(e, zero_forced),
                |ec| ec,
            );
            Equation::If(c, then_eqs, elifs, else_eqs)
        }
        other => other,
    }
}

fn collect_zero_forced_names(eqs: &[Equation]) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_zero_forced_in(eqs, &mut out);
    out
}

fn collect_zero_forced_in(eqs: &[Equation], out: &mut HashSet<String>) {
    for eq in eqs {
        match eq {
            Equation::Simple(lhs, rhs) => {
                if let Some(name) = zero_forced_name(lhs, rhs) {
                    out.insert(name);
                }
            }
            Equation::If(_, then_eqs, elifs, else_eqs) => {
                collect_zero_forced_in(then_eqs, out);
                for (_, body) in elifs {
                    collect_zero_forced_in(body, out);
                }
                if let Some(body) = else_eqs {
                    collect_zero_forced_in(body, out);
                }
            }
            _ => {}
        }
    }
}

fn zero_forced_name(lhs: &Expression, rhs: &Expression) -> Option<String> {
    if is_num_zero(rhs) {
        return as_single_var(lhs);
    }
    if is_num_zero(lhs) {
        return as_single_var(rhs);
    }
    None
}

fn as_single_var(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Variable(id) => Some(resolve_id(*id)),
        Expression::BinaryOp(l, Operator::Add, r) => {
            if is_num_zero(l) {
                as_single_var(r)
            } else if is_num_zero(r) {
                as_single_var(l)
            } else {
                None
            }
        }
        Expression::BinaryOp(l, Operator::Sub, r) if is_num_zero(r) => as_single_var(l),
        _ => None,
    }
}

fn is_tau_name(name: &str) -> bool {
    name == "tau" || name.ends_with("_tau")
}

/// `constantTorque.tau = 0` does not alias `revolute.tau` (the projection
/// `tau = -t · e` protects it). Still treat the joint scalar as motor-zeroed.
fn tau_forced_by_motor(name: &str, zero_forced: &HashSet<String>) -> bool {
    if zero_forced.contains(name) {
        return true;
    }
    let Some(owner) = name.strip_suffix("_tau") else {
        return false;
    };
    if owner.contains("constantTorque") || owner.contains("ConstantTorque") {
        return false;
    }
    let prefix = format!("{owner}_");
    zero_forced.iter().any(|z| {
        z.starts_with(&prefix)
            && z.ends_with("_tau")
            && (z.contains("constantTorque") || z.contains("ConstantTorque"))
    })
}

fn expr_is_torque_projection(expr: &Expression) -> bool {
    !expr_contains_der(expr) && expr_contains_frame_torque(expr)
}

fn expr_contains_der(expr: &Expression) -> bool {
    walk_expr_any(expr, &|e| match e {
        Expression::Der(_) => true,
        Expression::Variable(id) => resolve_id(*id).starts_with("der_"),
        _ => false,
    })
}

fn expr_contains_frame_torque(expr: &Expression) -> bool {
    walk_expr_any(expr, &|e| match e {
        Expression::Variable(id) => is_frame_torque_name(&resolve_id(*id)),
        _ => false,
    })
}

fn is_frame_torque_name(name: &str) -> bool {
    if let Some((base, idx)) = name.rsplit_once('_') {
        if matches!(idx, "1" | "2" | "3") {
            return base.ends_with("_t");
        }
    }
    name.ends_with("_t")
}

fn fold_trivial_arith(expr: Expression) -> Expression {
    match expr {
        Expression::BinaryOp(l, op, r) => {
            let l = fold_trivial_arith(*l);
            let r = fold_trivial_arith(*r);
            match op {
                Operator::Mul => {
                    if is_num_zero(&l) || is_num_zero(&r) {
                        Expression::Number(0.0)
                    } else if is_num_one(&l) {
                        r
                    } else if is_num_one(&r) {
                        l
                    } else {
                        Expression::BinaryOp(Box::new(l), op, Box::new(r))
                    }
                }
                Operator::Add => {
                    if is_num_zero(&l) {
                        r
                    } else if is_num_zero(&r) {
                        l
                    } else {
                        Expression::BinaryOp(Box::new(l), op, Box::new(r))
                    }
                }
                Operator::Sub => {
                    if is_num_zero(&r) {
                        l
                    } else if is_num_zero(&l) {
                        if let Expression::Number(v) = r {
                            Expression::Number(-v)
                        } else {
                            Expression::BinaryOp(
                                Box::new(Expression::Number(0.0)),
                                op,
                                Box::new(r),
                            )
                        }
                    } else {
                        Expression::BinaryOp(Box::new(l), op, Box::new(r))
                    }
                }
                _ => Expression::BinaryOp(Box::new(l), op, Box::new(r)),
            }
        }
        Expression::Call(f, args) => {
            Expression::Call(f, args.into_iter().map(fold_trivial_arith).collect())
        }
        Expression::ArrayAccess(a, i) => Expression::ArrayAccess(
            Box::new(fold_trivial_arith(*a)),
            Box::new(fold_trivial_arith(*i)),
        ),
        Expression::If(c, t, f) => Expression::If(
            Box::new(fold_trivial_arith(*c)),
            Box::new(fold_trivial_arith(*t)),
            Box::new(fold_trivial_arith(*f)),
        ),
        Expression::ArrayLiteral(es) => {
            Expression::ArrayLiteral(es.into_iter().map(fold_trivial_arith).collect())
        }
        other => other,
    }
}

fn is_num_zero(expr: &Expression) -> bool {
    matches!(expr, Expression::Number(n) if n.abs() < 1e-15)
}

fn is_num_one(expr: &Expression) -> bool {
    matches!(expr, Expression::Number(n) if (n - 1.0).abs() < 1e-15)
}

fn named_component(expr: &Expression, i: i64) -> Option<Expression> {
    let path = crate::ast::expr_to_connector_path(expr)?;
    Some(Expression::var(&format!("{}_{}", path.replace('.', "_"), i)))
}

fn named_or_at(expr: Expression, i: i64) -> Expression {
    named_component(&expr, i).unwrap_or_else(|| at(expr, i))
}

fn array_item_at(items: &[Expression], i: i64) -> Option<Expression> {
    if i >= 1 && (i as usize) <= items.len() {
        Some(items[(i as usize) - 1].clone())
    } else {
        None
    }
}

fn pick3<T: Clone>(i: i64, a: &T, b: &T, c: &T) -> T {
    match i {
        1 => a.clone(),
        2 => b.clone(),
        _ => c.clone(),
    }
}

fn map_if_eq<F, C>(
    then_eqs: Vec<Equation>,
    elifs: Vec<(Expression, Vec<Equation>)>,
    else_eqs: Option<Vec<Equation>>,
    mut map_eq: F,
    mut map_cond: C,
) -> (
    Vec<Equation>,
    Vec<(Expression, Vec<Equation>)>,
    Option<Vec<Equation>>,
)
where
    F: FnMut(Equation) -> Equation,
    C: FnMut(Expression) -> Expression,
{
    (
        then_eqs.into_iter().map(&mut map_eq).collect(),
        elifs
            .into_iter()
            .map(|(ec, body)| (map_cond(ec), body.into_iter().map(&mut map_eq).collect()))
            .collect(),
        else_eqs.map(|body| body.into_iter().map(map_eq).collect()),
    )
}

fn walk_expr_any(expr: &Expression, pred: &impl Fn(&Expression) -> bool) -> bool {
    if pred(expr) {
        return true;
    }
    match expr {
        Expression::BinaryOp(l, _, r) => walk_expr_any(l, pred) || walk_expr_any(r, pred),
        Expression::Call(_, args) => args.iter().any(|a| walk_expr_any(a, pred)),
        Expression::ArrayAccess(a, i) => walk_expr_any(a, pred) || walk_expr_any(i, pred),
        Expression::If(c, t, f) => {
            walk_expr_any(c, pred) || walk_expr_any(t, pred) || walk_expr_any(f, pred)
        }
        Expression::ArrayLiteral(es) => es.iter().any(|e| walk_expr_any(e, pred)),
        _ => false,
    }
}

fn frames_tail(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn add(a: Expression, b: Expression) -> Expression {
    Expression::BinaryOp(Box::new(a), Operator::Add, Box::new(b))
}

fn sub(a: Expression, b: Expression) -> Expression {
    Expression::BinaryOp(Box::new(a), Operator::Sub, Box::new(b))
}

fn mul(a: Expression, b: Expression) -> Expression {
    Expression::BinaryOp(Box::new(a), Operator::Mul, Box::new(b))
}

fn at(expr: Expression, i: i64) -> Expression {
    Expression::ArrayAccess(Box::new(expr), Box::new(Expression::Number(i as f64)))
}
