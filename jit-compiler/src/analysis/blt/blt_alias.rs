use crate::ast::{Equation, Expression, Operator};
use crate::string_intern::{resolve_id, var_starts_with};
use std::collections::{HashMap, HashSet};

use crate::analysis::expression_utils::{make_mul, make_num};
use crate::analysis::variable_collection::contains_var;

/// True if making `v = value` an alias would form a circular alias: after
/// resolving `value` through the aliases already collected, it still references
/// `v`. Such a self-reference is an algebraic loop (e.g. `a=2*b+1; b=3-a`), which
/// must stay as real equations for the block solver rather than collapse into an
/// unresolvable circular alias map (which the codegen would evaluate sequentially
/// against stale values). The plain `contains_var(value, v)` guards below only
/// catch a DIRECT self-reference, not one formed through another alias.
fn alias_would_cycle(
    v: &str,
    value: &Expression,
    alias_map: &HashMap<String, Expression>,
) -> bool {
    if alias_map.is_empty() {
        return false;
    }
    contains_var(&substitute_aliases_in_expr(value, alias_map), v)
}

fn expr_is_der_var(expr: &Expression) -> bool {
    matches!(expr, Expression::Variable(id) if var_starts_with(*id, "der_"))
        || matches!(expr, Expression::Der(_))
}

/// Alias RHS that can appear under `der(...)` / be rewritten from `der_*`.
/// Calls like `Frames.angularVelocity2(R)` are not differentiable here.
fn alias_rhs_is_der_safe(expr: &Expression) -> bool {
    matches!(expr, Expression::Variable(_))
        || crate::analysis::derivative::expand_der_linear(expr).is_some()
}

/// Non-linear / opaque RHS must stay as real equations (not collapsed aliases),
/// otherwise `der(w_a)` becomes `der(Frames.angularVelocity2(...))`.
fn alias_rhs_must_keep_equation(expr: &Expression) -> bool {
    fn walk(e: &Expression) -> bool {
        match e {
            Expression::Call(_, _) | Expression::If(_, _, _) => true,
            Expression::BinaryOp(l, _, r) => walk(l) || walk(r),
            Expression::ArrayAccess(a, i) => walk(a) || walk(i),
            Expression::Dot(b, _) => walk(b),
            Expression::ArrayLiteral(es) => es.iter().any(walk),
            Expression::Der(inner)
            | Expression::Sample(inner)
            | Expression::Interval(inner)
            | Expression::Hold(inner)
            | Expression::Previous(inner) => walk(inner),
            _ => false,
        }
    }
    walk(expr)
}

/// Follow variable-only aliases to a root name (for rewriting `der_x` after `x -> y -> z`).
fn resolve_alias_var_name(map: &HashMap<String, Expression>, name: &str) -> String {
    let mut current = name.to_string();
    for _ in 0..64 {
        match map.get(&current) {
            Some(Expression::Variable(id)) => {
                let next = resolve_id(*id);
                if next == current {
                    return current;
                }
                current = next;
            }
            _ => return current,
        }
    }
    current
}

/// If `name_1..name_n` all alias to `other_1..other_n` (n>=3), rewrite whole-array
/// `name` used in Calls (`resolve1(rod_frame_a_R, r)`) to `other`.
fn alias_array_base_name(map: &HashMap<String, Expression>, name: &str) -> Option<String> {
    let first = map.get(&format!("{name}_1"))?;
    let Expression::Variable(id) = first else {
        return None;
    };
    let t1 = resolve_id(*id);
    let base = t1.strip_suffix("_1")?;
    if base == name || base.is_empty() {
        return None;
    }
    let mut n = 1usize;
    for i in 2..=8 {
        match map.get(&format!("{name}_{i}")) {
            Some(Expression::Variable(id)) if resolve_id(*id) == format!("{base}_{i}") => {
                n = i;
            }
            Some(_) => return None,
            None => break,
        }
    }
    if n >= 3 {
        Some(base.to_string())
    } else {
        None
    }
}

/// Variables that must not be collapsed by alias elimination: integrator states,
/// discrete clocked vars, any `der_*` unknown, and Call/If-assigned kinematics.
/// Collapsing e.g. `inertia1_w = der_inertia1_phi` into an alias removes the only
/// equation that should assign `der_inertia1_phi := inertia1_w`, leaving phi stuck
/// at 0. Frame `R` / `r_0` connect copies (Variable = Variable) may alias the
/// undriven side onto the driven side so `resolve1(rod_frame_a_R, r)` follows
/// `revolute_frame_b_R` computed inside `if Connections.rooted then`.
fn may_alias_var(v: &str, protected: &HashSet<String>) -> bool {
    if v.starts_with("der_") || protected.contains(v) {
        return false;
    }
    true
}

fn simple_lhs_name(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Variable(id) => Some(resolve_id(*id)),
        Expression::ArrayAccess(arr, idx) => {
            let Expression::Variable(id) = arr.as_ref() else {
                return None;
            };
            let base = resolve_id(*id);
            if let Expression::Number(n) = idx.as_ref() {
                Some(format!("{}_{}", base, *n as i64))
            } else {
                Some(base)
            }
        }
        _ => None,
    }
}

fn expr_debug_path(expr: &Expression) -> String {
    match expr {
        Expression::Variable(id) => resolve_id(*id),
        Expression::Dot(base, member) => format!("{}.{}", expr_debug_path(base), member),
        Expression::ArrayAccess(arr, _) => expr_debug_path(arr),
        _ => String::new(),
    }
}

/// Matches JIT Connections stubs: `rooted` is true on tree models; `isRoot(x)`
/// is true when the argument path contains `world`. Also folds `not` and
/// numeric predicates (`if not(0)`, `elseif 1`) so those branches can be
/// lowered into ordinary Simple equations for BLT ordering.
fn eval_connections_pred(expr: &Expression) -> Option<bool> {
    match expr {
        Expression::Number(n) => Some(*n != 0.0),
        Expression::Call(name, args) => {
            let tail = name.rsplit('.').next().unwrap_or(name.as_str());
            match tail {
                "rooted" => Some(true),
                "isRoot" => {
                    let arg = args.first()?;
                    Some(expr_debug_path(arg).contains("world"))
                }
                "not" => args.first().and_then(eval_connections_pred).map(|b| !b),
                _ => None,
            }
        }
        Expression::BinaryOp(l, Operator::And, r) => {
            Some(eval_connections_pred(l)? && eval_connections_pred(r)?)
        }
        Expression::BinaryOp(l, Operator::Or, r) => {
            Some(eval_connections_pred(l)? || eval_connections_pred(r)?)
        }
        _ => None,
    }
}

/// True when every predicate on the taken path is a Connections/const fold.
pub(crate) fn connection_if_is_static(
    cond: &Expression,
    elifs: &[(Expression, Vec<Equation>)],
) -> bool {
    match eval_connections_pred(cond) {
        Some(true) => true,
        Some(false) => {
            for (c, _) in elifs {
                match eval_connections_pred(c) {
                    Some(true) => return true,
                    Some(false) => {}
                    None => return false,
                }
            }
            true
        }
        None => false,
    }
}

fn take_static_if_body(
    cond: &Expression,
    then_eqs: Vec<Equation>,
    elifs: Vec<(Expression, Vec<Equation>)>,
    else_eqs: Option<Vec<Equation>>,
) -> Vec<Equation> {
    match eval_connections_pred(cond) {
        Some(true) => then_eqs,
        Some(false) => {
            for (c, body) in elifs {
                if eval_connections_pred(&c) == Some(true) {
                    return body;
                }
            }
            else_eqs.unwrap_or_default()
        }
        None => Vec::new(),
    }
}

/// Lower `if Connections.rooted` / `isRoot` / const predicates into the taken
/// branch so BLT can schedule `frame_b.R = absoluteRotation(...)` before
/// `r_0 = resolve1(R, r)`. Unresolved Ifs are left intact.
pub(crate) fn expand_static_connection_ifs(eqs: Vec<Equation>) -> Vec<Equation> {
    let mut out = Vec::with_capacity(eqs.len());
    for eq in eqs {
        expand_static_connection_if(eq, &mut out);
    }
    out
}

fn expand_static_connection_if(eq: Equation, out: &mut Vec<Equation>) {
    match eq {
        Equation::If(cond, then_eqs, elifs, else_eqs) => {
            if connection_if_is_static(&cond, &elifs) {
                for sub in take_static_if_body(&cond, then_eqs, elifs, else_eqs) {
                    expand_static_connection_if(sub, out);
                }
            } else {
                out.push(Equation::If(cond, then_eqs, elifs, else_eqs));
            }
        }
        other => out.push(other),
    }
}

fn mark_computed_lhs(lhs: &Expression, rhs: &Expression, out: &mut HashSet<String>) {
    if matches!(rhs, Expression::Variable(_)) && !alias_rhs_must_keep_equation(rhs) {
        return;
    }
    if let Some(n) = simple_lhs_name(lhs) {
        out.insert(n);
    }
}

fn collect_computed_lhs(eqs: &[Equation], out: &mut HashSet<String>) {
    for eq in eqs {
        match eq {
            Equation::Simple(lhs, rhs) => mark_computed_lhs(lhs, rhs, out),
            Equation::If(cond, then_eqs, elifs, else_eqs) => {
                if let Some(taken) = eval_connections_pred(cond) {
                    if taken {
                        collect_computed_lhs(then_eqs, out);
                    } else {
                        let mut elif_taken = false;
                        for (c, body) in elifs {
                            match eval_connections_pred(c) {
                                Some(true) => {
                                    collect_computed_lhs(body, out);
                                    elif_taken = true;
                                    break;
                                }
                                Some(false) => {}
                                None => collect_computed_lhs(body, out),
                            }
                        }
                        if !elif_taken {
                            if let Some(body) = else_eqs {
                                collect_computed_lhs(body, out);
                            }
                        }
                    }
                } else {
                    collect_computed_lhs(then_eqs, out);
                    for (_, body) in elifs {
                        collect_computed_lhs(body, out);
                    }
                    if let Some(body) = else_eqs {
                        collect_computed_lhs(body, out);
                    }
                }
            }
            Equation::For(_, _, _, body) => collect_computed_lhs(body, out),
            Equation::When(_, body, elsewhens) => {
                collect_computed_lhs(body, out);
                for (_, b) in elsewhens {
                    collect_computed_lhs(b, out);
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn eliminate_aliases(
    equations: Vec<Equation>,
) -> (Vec<Equation>, HashMap<String, Expression>) {
    eliminate_aliases_protected(equations, &HashSet::new())
}

pub(crate) fn eliminate_aliases_protected(
    equations: Vec<Equation>,
    protected: &HashSet<String>,
) -> (Vec<Equation>, HashMap<String, Expression>) {
    let mut protected_all: HashSet<String> = protected.iter().cloned().collect();
    collect_computed_lhs(&equations, &mut protected_all);
    let protected = &protected_all;

    let mut alias_map: HashMap<String, Expression> = HashMap::new();
    let mut current_eqs = equations;
    let mut changed = true;

    while changed {
        changed = false;
        let mut next_eqs = Vec::with_capacity(current_eqs.len());

        for eq in &current_eqs {
            let mut is_alias = false;

            if let Equation::Simple(lhs, rhs) = eq {
                if let Expression::Variable(id) = lhs {
                    let v = resolve_id(*id);
                    // Keep kinematic / derivative links (w = der(phi), a = der(w)).
                    if may_alias_var(&v, protected)
                        && !expr_is_der_var(rhs)
                        && !alias_rhs_must_keep_equation(rhs)
                        && *lhs != *rhs
                        && !contains_var(rhs, &v)
                        && !alias_would_cycle(&v, rhs, &alias_map)
                        && !alias_map.contains_key(&v)
                    {
                        alias_map.insert(v, rhs.clone());
                        changed = true;
                        is_alias = true;
                    }
                }

                if !is_alias {
                    if let Expression::Variable(id) = rhs {
                        let v = resolve_id(*id);
                        if may_alias_var(&v, protected)
                            && !expr_is_der_var(lhs)
                            && !alias_rhs_must_keep_equation(lhs)
                            && *lhs != *rhs
                            && !contains_var(lhs, &v)
                            && !alias_would_cycle(&v, lhs, &alias_map)
                            && !alias_map.contains_key(&v)
                        {
                            alias_map.insert(v, lhs.clone());
                            changed = true;
                            is_alias = true;
                        }
                    }
                }

                if !is_alias {
                    if let Expression::BinaryOp(l, Operator::Sub, r) = lhs {
                        if let Expression::Number(n) = &**l {
                            if n.abs() < 1e-10 {
                                if let Expression::Variable(id) = &**r {
                                    let v = resolve_id(*id);
                                    if may_alias_var(&v, protected)
                                        && !expr_is_der_var(rhs)
                                        && !alias_rhs_must_keep_equation(rhs)
                                        && !alias_map.contains_key(&v)
                                        && !contains_var(rhs, &v)
                                        && !alias_would_cycle(&v, rhs, &alias_map)
                                    {
                                        let neg_rhs = make_mul(make_num(-1.0), rhs.clone());
                                        alias_map.insert(v, neg_rhs);
                                        changed = true;
                                        is_alias = true;
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

    // Resolve alias values transitively so each maps directly to free/solved
    // variables (a -> b+1 -> c+2). Otherwise a back-substituted alias output can
    // be computed before the alias it depends on, reading a stale value.
    // Terminates because alias_would_cycle keeps the map acyclic.
    let resolved: HashMap<String, Expression> = alias_map
        .iter()
        .map(|(k, v)| (k.clone(), substitute_aliases_in_expr(v, &alias_map)))
        .collect();
    alias_map = resolved;

    // Final pass so `der_*` names follow fully resolved potential aliases
    // (e.g. der_flange_a -> der_inertia_phi after flange_a -> inertia_phi).
    current_eqs = current_eqs
        .into_iter()
        .map(|eq| substitute_aliases_in_eq(&eq, &alias_map))
        .collect();

    (current_eqs, alias_map)
}

fn substitute_aliases_in_eq(eq: &Equation, map: &HashMap<String, Expression>) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(
            substitute_aliases_in_expr(lhs, map),
            substitute_aliases_in_expr(rhs, map),
        ),
        Equation::If(cond, then_eqs, elifs, else_eqs) => Equation::If(
            substitute_aliases_in_expr(cond, map),
            then_eqs
                .iter()
                .map(|e| substitute_aliases_in_eq(e, map))
                .collect(),
            elifs
                .iter()
                .map(|(c, body)| {
                    (
                        substitute_aliases_in_expr(c, map),
                        body.iter()
                            .map(|e| substitute_aliases_in_eq(e, map))
                            .collect(),
                    )
                })
                .collect(),
            else_eqs.as_ref().map(|body| {
                body.iter()
                    .map(|e| substitute_aliases_in_eq(e, map))
                    .collect()
            }),
        ),
        Equation::For(v, start, end, body) => Equation::For(
            v.clone(),
            Box::new(substitute_aliases_in_expr(start, map)),
            Box::new(substitute_aliases_in_expr(end, map)),
            body.iter()
                .map(|e| substitute_aliases_in_eq(e, map))
                .collect(),
        ),
        Equation::When(cond, body, elsewhens) => Equation::When(
            substitute_aliases_in_expr(cond, map),
            body.iter()
                .map(|e| substitute_aliases_in_eq(e, map))
                .collect(),
            elsewhens
                .iter()
                .map(|(c, b)| {
                    (
                        substitute_aliases_in_expr(c, map),
                        b.iter()
                            .map(|e| substitute_aliases_in_eq(e, map))
                            .collect(),
                    )
                })
                .collect(),
        ),
        Equation::Connect(_, _)
        | Equation::Reinit(_, _)
        | Equation::Assert(_, _)
        | Equation::Terminate(_)
        | Equation::CallStmt(_)
        | Equation::SolvableBlock { .. }
        | Equation::MultiAssign(_, _) => eq.clone(),
    }
}

fn substitute_aliases_in_expr<'a>(
    expr: &'a Expression,
    map: &'a HashMap<String, Expression>,
) -> Expression {
    #[derive(Clone)]
    enum Frame<'a> {
        Enter(&'a Expression, bool),
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
        BuildBackSample,
    }

    let mut frames: Vec<Frame<'a>> = vec![Frame::Enter(expr, false)];
    let mut values: Vec<Expression> = Vec::new();

    while let Some(f) = frames.pop() {
        match f {
            Frame::Enter(e, inside_der) => match e {
                Expression::Variable(id) => {
                    let name = resolve_id(*id);
                    // Recurse into the alias value so multi-level chains
                    // (a -> b+1, b -> c+1) resolve fully to the free variable.
                    // Safe from infinite recursion: alias_would_cycle keeps the
                    // alias map acyclic.
                    match map.get(&name) {
                        Some(val) if inside_der && !alias_rhs_is_der_safe(val) => {
                            // Keep `der(w_a)` when `w_a = Frames.angularVelocity2(...)`.
                            values.push(e.clone());
                        }
                        Some(val) => frames.push(Frame::Enter(val, inside_der)),
                        None => {
                            // If `flange_phi -> inertia_flange_a_phi -> inertia_phi`,
                            // rewrite `der_flange_phi` all the way to `der_inertia_phi`.
                            if let Some(base) = name.strip_prefix("der_") {
                                if map.contains_key(base) {
                                    let resolved = resolve_alias_var_name(map, base);
                                    if resolved != base {
                                        values.push(Expression::var(&format!("der_{}", resolved)));
                                        continue;
                                    }
                                    if let Some(aliased) = map.get(base) {
                                        if alias_rhs_is_der_safe(aliased) {
                                            frames.push(Frame::BuildDer);
                                            frames.push(Frame::Enter(aliased, true));
                                            continue;
                                        }
                                        // Non-linear alias: keep symbolic der_* unknown.
                                        values.push(e.clone());
                                        continue;
                                    }
                                }
                            }
                            if let Some(arr_base) = alias_array_base_name(map, &name) {
                                values.push(Expression::var(&arr_base));
                                continue;
                            }
                            values.push(e.clone());
                        }
                    }
                }
                Expression::BinaryOp(lhs, op, rhs) => {
                    frames.push(Frame::BuildBinary(*op));
                    frames.push(Frame::Enter(rhs, inside_der));
                    frames.push(Frame::Enter(lhs, inside_der));
                }
                Expression::Call(name, args) => {
                    frames.push(Frame::BuildCall(name.clone(), args.len()));
                    for a in args.iter().rev() {
                        frames.push(Frame::Enter(a, inside_der));
                    }
                }
                Expression::Der(arg) => {
                    frames.push(Frame::BuildDer);
                    frames.push(Frame::Enter(arg, true));
                }
                Expression::ArrayAccess(arr, idx) => {
                    frames.push(Frame::BuildArrayAccess);
                    frames.push(Frame::Enter(idx, inside_der));
                    frames.push(Frame::Enter(arr, inside_der));
                }
                Expression::If(c, t, f) => {
                    frames.push(Frame::BuildIf);
                    frames.push(Frame::Enter(f, inside_der));
                    frames.push(Frame::Enter(t, inside_der));
                    frames.push(Frame::Enter(c, inside_der));
                }
                Expression::ArrayLiteral(es) => {
                    frames.push(Frame::BuildArrayLiteral(es.len()));
                    for a in es.iter().rev() {
                        frames.push(Frame::Enter(a, inside_der));
                    }
                }
                Expression::Dot(base, member) => {
                    frames.push(Frame::BuildDot(member.clone()));
                    frames.push(Frame::Enter(base, inside_der));
                }
                Expression::Range(start, step, end) => {
                    frames.push(Frame::BuildRange);
                    frames.push(Frame::Enter(end, inside_der));
                    frames.push(Frame::Enter(step, inside_der));
                    frames.push(Frame::Enter(start, inside_der));
                }
                Expression::Sample(inner) => {
                    frames.push(Frame::BuildSample);
                    frames.push(Frame::Enter(inner, inside_der));
                }
                Expression::Interval(inner) => {
                    frames.push(Frame::BuildInterval);
                    frames.push(Frame::Enter(inner, inside_der));
                }
                Expression::Hold(inner) => {
                    frames.push(Frame::BuildHold);
                    frames.push(Frame::Enter(inner, inside_der));
                }
                Expression::Previous(inner) => {
                    frames.push(Frame::BuildPrevious);
                    frames.push(Frame::Enter(inner, inside_der));
                }
                Expression::SubSample(c, n) => {
                    frames.push(Frame::BuildSubSample);
                    frames.push(Frame::Enter(n, inside_der));
                    frames.push(Frame::Enter(c, inside_der));
                }
                Expression::SuperSample(c, n) => {
                    frames.push(Frame::BuildSuperSample);
                    frames.push(Frame::Enter(n, inside_der));
                    frames.push(Frame::Enter(c, inside_der));
                }
                Expression::ShiftSample(c, n) => {
                    frames.push(Frame::BuildShiftSample);
                    frames.push(Frame::Enter(n, inside_der));
                    frames.push(Frame::Enter(c, inside_der));
                }
                Expression::BackSample(c, n) => {
                    frames.push(Frame::BuildBackSample);
                    frames.push(Frame::Enter(n, inside_der));
                    frames.push(Frame::Enter(c, inside_der));
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
                if let Some(expanded) = crate::analysis::derivative::expand_der_linear(&inner) {
                    values.push(expanded);
                } else {
                    values.push(Expression::Der(Box::new(inner)));
                }
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
            Frame::BuildBackSample => {
                let n = values.pop().unwrap_or(Expression::Number(1.0));
                let c = values.pop().unwrap_or(Expression::Number(0.0));
                values.push(Expression::BackSample(Box::new(c), Box::new(n)));
            }
        }
    }

    values.pop().unwrap_or_else(|| expr.clone())
}
