use crate::ast::{flat_index_suffix_for_scalar_name, Equation, Expression, Operator};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// Thread-local enumerations map for enum literal validation during flatten.
static FLATTEN_ENUMS: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

/// Set the enumerations map for the current flatten session.
pub fn set_flatten_enumerations(enums: HashMap<String, Vec<String>>) {
    let _ = FLATTEN_ENUMS.set(enums);
}

/// Clear after flatten completes.
pub fn clear_flatten_enumerations() {
    // OnceLock can't be cleared, but it's set once per flatten session
}

fn resolve_enum_dot(base_variable: &str, member: &str) -> Option<f64> {
    let enums = FLATTEN_ENUMS.get()?;
    let literals = enums.get(base_variable)?;
    literals.iter().position(|l| l == member).map(|idx| idx as f64)
}

/// Fold `prefix_expression` results for array access into a scalar `Variable` name when the
/// flatten/JIT convention is `array_index` / `array_{idxVar}` (matches `expr_to_flat_scalar_prefix`).
fn try_fold_array_access_after_prefix(arr_flat: &Expression, idx_flat: &Expression) -> Option<Expression> {
    if let (Expression::ArrayLiteral(elements), Expression::Number(n)) = (arr_flat, idx_flat) {
        let idx = *n as usize;
        if idx > 0 && idx <= elements.len() {
            return Some(elements[idx - 1].clone());
        }
        if elements.len() == 1 {
            return Some(elements[0].clone());
        }
        eprintln!(
            "Index out of bounds in flattening: {} (len {})",
            idx,
            elements.len()
        );
        return Some(Expression::Number(0.0));
    }
    if let Expression::Variable(id) = arr_flat {
        let name = crate::string_intern::resolve_id(*id);
        if let Some(v) = eval_const_expr(idx_flat) {
            let n_int = v.round() as i64;
            return Some(Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, n_int))));
        }
        if let Some(suf) = flat_index_suffix_for_scalar_name(idx_flat) {
            return Some(Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, suf))));
        }
        return None;
    }
    if let Expression::ArrayAccess(inner_arr, inner_idx) = arr_flat {
        let inner_folded = try_fold_array_access_after_prefix(inner_arr.as_ref(), inner_idx.as_ref())?;
        return try_fold_array_access_after_prefix(&inner_folded, idx_flat);
    }
    None
}

/// Collapse `Dot` chains and fold indexable tails so JIT sees scalar `Variable` names (MSL MultiBody).
fn append_dot_member_to_flat(base: Expression, member: &str) -> Expression {
    match base {
        Expression::Variable(id) => {
            // Enum literal access: E.field → integer index constant.
            let name = crate::string_intern::resolve_id(id);
            if let Some(idx) = resolve_enum_dot(&name, member) {
                return Expression::Number(idx);
            }
            if member == "signal" {
                Expression::Variable(id)
            } else {
                Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, member)))
            }
        }
        Expression::Dot(inner, m) => {
            let step = append_dot_member_to_flat(*inner, &m);
            append_dot_member_to_flat(step, member)
        }
        Expression::ArrayAccess(arr, idx) => {
            if let Some(v) = try_fold_array_access_after_prefix(arr.as_ref(), idx.as_ref()) {
                append_dot_member_to_flat(v, member)
            } else {
                Expression::Dot(Box::new(Expression::ArrayAccess(arr, idx)), member.to_string())
            }
        }
        Expression::If(cond, t, f) => Expression::If(
            cond,
            Box::new(append_dot_member_to_flat(*t, member)),
            Box::new(append_dot_member_to_flat(*f, member)),
        ),
        other => Expression::Dot(Box::new(other), member.to_string()),
    }
}

pub fn prefix_expression(expr: &Expression, prefix: &str) -> Expression {
    let prefix_str = if prefix.is_empty() {
        String::new()
    } else {
        format!("{}_", prefix)
    };

    match expr {
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            if name == "time" {
                return expr.clone();
            }
            // `import Modelica.Constants.pi` (and similar) must not become `inst_pi`.
            // Unbound instance-prefixed names silently evaluate to 0 and wipe Sine/Cosine.
            if name == "pi"
                || name == "Modelica.Constants.pi"
                || name == "Modelica_Constants_pi"
            {
                return Expression::Number(std::f64::consts::PI);
            }
            if name == "eps"
                || name == "Modelica.Constants.eps"
                || name == "Modelica_Constants_eps"
            {
                return Expression::Number(f64::EPSILON);
            }
            let flat_name = name.replace('.', "_");
            Expression::Variable(crate::string_intern::intern(&format!(
                "{}{}",
                prefix_str, flat_name
            )))
        }
        Expression::Number(n) => Expression::Number(*n),
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
            Box::new(prefix_expression(lhs, prefix)),
            op.clone(),
            Box::new(prefix_expression(rhs, prefix)),
        ),
        Expression::Call(func, args) => Expression::Call(
            func.clone(),
            args.iter()
                .map(|arg| prefix_expression(arg, prefix))
                .collect(),
        ),
        Expression::Der(arg) => Expression::Der(Box::new(prefix_expression(arg, prefix))),
        Expression::ArrayAccess(arr, idx) => {
            let arr_flat = prefix_expression(arr, prefix);
            let idx_flat = prefix_expression(idx, prefix);
            try_fold_array_access_after_prefix(&arr_flat, &idx_flat).unwrap_or_else(|| {
                Expression::ArrayAccess(Box::new(arr_flat), Box::new(idx_flat))
            })
        }
        Expression::Dot(base, member) => {
            let base_flat = prefix_expression(base, prefix);
            let folded_base = match &base_flat {
                Expression::ArrayAccess(arr, idx) => {
                    try_fold_array_access_after_prefix(arr.as_ref(), idx.as_ref())
                        .unwrap_or(base_flat)
                }
                _ => base_flat,
            };
            append_dot_member_to_flat(folded_base, member)
        }
        Expression::If(cond, t_expr, f_expr) => Expression::If(
            Box::new(prefix_expression(cond, prefix)),
            Box::new(prefix_expression(t_expr, prefix)),
            Box::new(prefix_expression(f_expr, prefix)),
        ),
        Expression::Range(start, step, end) => Expression::Range(
            Box::new(prefix_expression(start, prefix)),
            Box::new(prefix_expression(step, prefix)),
            Box::new(prefix_expression(end, prefix)),
        ),
        Expression::ArrayLiteral(exprs) => {
            Expression::ArrayLiteral(exprs.iter().map(|e| prefix_expression(e, prefix)).collect())
        }
        Expression::ArrayComprehension { expr, iter_var, iter_range } => Expression::ArrayComprehension {
            expr: Box::new(prefix_expression(expr, prefix)),
            iter_var: iter_var.clone(),
            iter_range: Box::new(prefix_expression(iter_range, prefix)),
        },
        Expression::Sample(inner) => Expression::Sample(Box::new(prefix_expression(inner, prefix))),
        Expression::Interval(inner) => {
            Expression::Interval(Box::new(prefix_expression(inner, prefix)))
        }
        Expression::Hold(inner) => Expression::Hold(Box::new(prefix_expression(inner, prefix))),
        Expression::Previous(inner) => {
            Expression::Previous(Box::new(prefix_expression(inner, prefix)))
        }
        Expression::SubSample(c, n) => Expression::SubSample(
            Box::new(prefix_expression(c, prefix)),
            Box::new(prefix_expression(n, prefix)),
        ),
        Expression::SuperSample(c, n) => Expression::SuperSample(
            Box::new(prefix_expression(c, prefix)),
            Box::new(prefix_expression(n, prefix)),
        ),
        Expression::ShiftSample(c, n) => Expression::ShiftSample(
            Box::new(prefix_expression(c, prefix)),
            Box::new(prefix_expression(n, prefix)),
        ),
        Expression::BackSample(c, n) => Expression::BackSample(
            Box::new(prefix_expression(c, prefix)),
            Box::new(prefix_expression(n, prefix)),
        ),
        Expression::StringLiteral(s) => Expression::StringLiteral(s.clone()),
    }
}

/// True when `f(...)[i]` must keep whole-vector/tensor arguments.
pub(crate) fn vector_call_indexes_result(func: &str) -> bool {
    let tail = func.rsplit('.').next().unwrap_or(func);
    func.contains("Frames.")
        || func.contains("gravityAcceleration")
        || func.contains("GravityAcceleration")
        || matches!(
            tail,
            "resolve1"
                | "resolve2"
                | "resolveRelative"
                | "resolveDyade1"
                | "resolveDyade2"
                | "planarRotation"
                | "absoluteRotation"
                | "relativeRotation"
                | "inverseRotation"
                | "nullRotation"
                | "from_Q"
                | "from_nxy"
                | "from_nxz"
                | "from_T"
                | "axesRotations"
                | "angularVelocity1"
                | "angularVelocity2"
                | "orientationConstraint"
                | "cross"
                | "skew"
                | "normalize"
                | "normalizeWithAssert"
                | "zeros"
                | "ones"
                | "identity"
                | "fill"
                | "diagonal"
                | "outerProduct"
        )
}

pub fn index_expression(expr: &Expression, idx: usize) -> Expression {
    match expr {
        Expression::Variable(_name) => Expression::ArrayAccess(
            Box::new(expr.clone()),
            Box::new(Expression::Number(idx as f64)),
        ),
        Expression::Number(_) => expr.clone(),
        Expression::BinaryOp(lhs, op, rhs) => {
            if *op == Operator::Mul {
                if let Some(comp) = index_tensor3_times_vec3(lhs, rhs, idx) {
                    return comp;
                }
            }
            Expression::BinaryOp(
                Box::new(index_expression(lhs, idx)),
                *op,
                Box::new(index_expression(rhs, idx)),
            )
        }
        Expression::ArrayLiteral(elements) => {
            if idx > 0 && idx <= elements.len() {
                elements[idx - 1].clone()
            } else if elements.len() == 1 {
                // Scalar / length-1 binding broadcast to array dimension (common in MSL records).
                elements[0].clone()
            } else {
                eprintln!(
                    "Index out of bounds for ArrayLiteral: {} (len {})",
                    idx,
                    elements.len()
                );
                Expression::Number(0.0)
            }
        }
        Expression::Call(func, args) => {
            if vector_call_indexes_result(func) {
                // resolve1(R, r)[i] needs the full vectors, not resolve1(R[i], r[i]).
                Expression::ArrayAccess(
                    Box::new(Expression::Call(func.clone(), args.clone())),
                    Box::new(Expression::Number(idx as f64)),
                )
            } else {
                Expression::Call(
                    func.clone(),
                    args.iter().map(|arg| index_expression(arg, idx)).collect(),
                )
            }
        }
        Expression::Der(arg) => Expression::Der(Box::new(index_expression(arg, idx))),
        Expression::ArrayAccess(_arr, _) => expr.clone(),
        Expression::ArrayComprehension { expr: body, iter_var, .. } => {
            let substituted = substitute_var_in_expr(body, iter_var, &Expression::Number(idx as f64));
            substituted
        }
        Expression::If(c, t, f) => Expression::If(
            Box::new(index_expression(c, idx)),
            Box::new(index_expression(t, idx)),
            Box::new(index_expression(f, idx)),
        ),
        _ => expr.clone(),
    }
}

fn expr_is_tensor3_literal(expr: &Expression) -> bool {
    matches!(expr, Expression::ArrayLiteral(items) if items.len() == 9)
}

fn expr_looks_like_vec3(expr: &Expression) -> bool {
    match expr {
        Expression::ArrayLiteral(items) => items.len() == 3,
        Expression::Call(func, _) => {
            let tail = func.rsplit('.').next().unwrap_or(func);
            matches!(
                tail,
                "cross"
                    | "resolve1"
                    | "resolve2"
                    | "normalize"
                    | "normalizeWithAssert"
                    | "angularVelocity1"
                    | "angularVelocity2"
                    | "zeros"
                    | "ones"
            )
        }
        Expression::Variable(_) | Expression::ArrayAccess(_, _) => true,
        Expression::BinaryOp(l, Operator::Add | Operator::Sub, r) => {
            expr_looks_like_vec3(l) || expr_looks_like_vec3(r)
        }
        _ => false,
    }
}

/// MSL `I*z` is a 3x3 tensor times a vector, not element-wise `I[i]*z[i]`.
fn index_tensor3_times_vec3(lhs: &Expression, rhs: &Expression, idx: usize) -> Option<Expression> {
    if idx == 0 || idx > 3 {
        return None;
    }
    let (tensor, vec) = if expr_is_tensor3_literal(lhs) && expr_looks_like_vec3(rhs) {
        (lhs, rhs)
    } else if expr_is_tensor3_literal(rhs) && expr_looks_like_vec3(lhs) {
        (rhs, lhs)
    } else {
        return None;
    };
    let row0 = 3 * (idx - 1);
    let mul_at = |k: usize, vi: usize| {
        Expression::BinaryOp(
            Box::new(index_expression(tensor, row0 + k)),
            Operator::Mul,
            Box::new(index_expression(vec, vi)),
        )
    };
    Some(Expression::BinaryOp(
        Box::new(Expression::BinaryOp(
            Box::new(mul_at(1, 1)),
            Operator::Add,
            Box::new(mul_at(2, 2)),
        )),
        Operator::Add,
        Box::new(mul_at(3, 3)),
    ))
}

fn substitute_var_in_expr(expr: &Expression, var: &str, replacement: &Expression) -> Expression {
    match expr {
        Expression::Variable(id) if crate::string_intern::resolve_id(*id) == var => replacement.clone(),
        Expression::Variable(_) | Expression::Number(_) | Expression::StringLiteral(_) => {
            expr.clone()
        }
        Expression::BinaryOp(l, op, r) => Expression::BinaryOp(
            Box::new(substitute_var_in_expr(l, var, replacement)),
            *op,
            Box::new(substitute_var_in_expr(r, var, replacement)),
        ),
        Expression::If(c, t, f) => Expression::If(
            Box::new(substitute_var_in_expr(c, var, replacement)),
            Box::new(substitute_var_in_expr(t, var, replacement)),
            Box::new(substitute_var_in_expr(f, var, replacement)),
        ),
        Expression::Call(name, args) => Expression::Call(
            name.clone(),
            args.iter()
                .map(|a| substitute_var_in_expr(a, var, replacement))
                .collect(),
        ),
        Expression::Der(inner) => {
            Expression::Der(Box::new(substitute_var_in_expr(inner, var, replacement)))
        }
        Expression::ArrayLiteral(items) => Expression::ArrayLiteral(
            items
                .iter()
                .map(|e| substitute_var_in_expr(e, var, replacement))
                .collect(),
        ),
        Expression::ArrayAccess(arr, idx) => Expression::ArrayAccess(
            Box::new(substitute_var_in_expr(arr, var, replacement)),
            Box::new(substitute_var_in_expr(idx, var, replacement)),
        ),
        Expression::Dot(base, member) => Expression::Dot(
            Box::new(substitute_var_in_expr(base, var, replacement)),
            member.clone(),
        ),
        Expression::Previous(inner) => {
            Expression::Previous(Box::new(substitute_var_in_expr(inner, var, replacement)))
        }
        _ => expr.clone(),
    }
}

pub fn eval_const_expr(expr: &Expression) -> Option<f64> {
    eval_const_expr_with_params(expr, &HashMap::new())
}

pub fn eval_const_expr_with_params(
    expr: &Expression,
    params: &HashMap<String, f64>,
) -> Option<f64> {
    match expr {
        Expression::Number(n) => Some(*n),
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            if let Some(v) = params.get(&name).copied() {
                return Some(v);
            }
            let underscore_name = name.replace('.', "_");
            if underscore_name != name {
                if let Some(v) = params.get(&underscore_name).copied() {
                    return Some(v);
                }
            }
            match name.as_str() {
                "pi" | "Modelica.Constants.pi" | "Modelica_Constants_pi" => {
                    Some(std::f64::consts::PI)
                }
                "eps" | "Modelica.Constants.eps" | "Modelica_Constants_eps" => Some(f64::EPSILON),
                "Modelica.Constants.small" | "Modelica_Constants_small" => Some(1.0e-60),
                "Modelica.Constants.inf" | "Modelica_Constants_inf" => Some(f64::INFINITY),
                "Modelica.Constants.g_n" | "Modelica_Constants_g_n" => Some(9.80665),
                "Modelica.Constants.T_zero" => Some(273.15),
                "Modelica.Constants.sigma" => Some(5.670374419e-8),
                "Modelica.Constants.R" => Some(8.314462618),
                "Modelica.Constants.N_A" => Some(6.02214076e23),
                _ => None,
            }
        }
        Expression::Dot(base, member) => {
            if let Some(base_path) = expr_to_path(base) {
                let dot_name = format!("{}.{}", base_path, member);
                if let Some(v) = params.get(&dot_name).copied() {
                    return Some(v);
                }
                let underscore_name = dot_name.replace('.', "_");
                if let Some(v) = params.get(&underscore_name).copied() {
                    return Some(v);
                }
            }
            None
        }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = eval_const_expr_with_params(lhs, params)?;
            let r = eval_const_expr_with_params(rhs, params)?;
            match op {
                Operator::Add => Some(l + r),
                Operator::Sub => Some(l - r),
                Operator::Mul => Some(l * r),
                Operator::Div if r != 0.0 => Some(l / r),
                Operator::Less => Some(if l < r { 1.0 } else { 0.0 }),
                Operator::Greater => Some(if l > r { 1.0 } else { 0.0 }),
                Operator::LessEq => Some(if l <= r { 1.0 } else { 0.0 }),
                Operator::GreaterEq => Some(if l >= r { 1.0 } else { 0.0 }),
                Operator::Equal => Some(if (l - r).abs() < 1e-15 { 1.0 } else { 0.0 }),
                Operator::NotEqual => Some(if (l - r).abs() >= 1e-15 { 1.0 } else { 0.0 }),
                Operator::And => Some(if l != 0.0 && r != 0.0 { 1.0 } else { 0.0 }),
                Operator::Or => Some(if l != 0.0 || r != 0.0 { 1.0 } else { 0.0 }),
                _ => None,
            }
        }
        Expression::If(cond, t_expr, f_expr) => {
            let c = eval_const_expr_with_params(cond, params)?;
            if c != 0.0 {
                eval_const_expr_with_params(t_expr, params)
            } else {
                eval_const_expr_with_params(f_expr, params)
            }
        }
        Expression::Call(func, args) => {
            let func_lower = func.to_lowercase();
            let func_tail = func_lower.rsplit('.').next().unwrap_or(&func_lower);
            match func_tail {
                "sin" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.sin())
                }
                "cos" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.cos())
                }
                "tan" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.tan())
                }
                "asin" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.asin())
                }
                "acos" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.acos())
                }
                "atan" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.atan())
                }
                "atan2" if args.len() == 2 => {
                    let y = eval_const_expr_with_params(&args[0], params)?;
                    let x = eval_const_expr_with_params(&args[1], params)?;
                    Some(y.atan2(x))
                }
                "sqrt" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.sqrt())
                }
                "abs" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.abs())
                }
                "exp" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.exp())
                }
                "log" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.ln())
                }
                "log10" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.log10())
                }
                "pow" if args.len() == 2 => {
                    let base = eval_const_expr_with_params(&args[0], params)?;
                    let exp = eval_const_expr_with_params(&args[1], params)?;
                    Some(base.powf(exp))
                }
                "max" if args.len() == 2 => {
                    let a = eval_const_expr_with_params(&args[0], params)?;
                    let b = eval_const_expr_with_params(&args[1], params)?;
                    Some(a.max(b))
                }
                "min" if args.len() == 2 => {
                    let a = eval_const_expr_with_params(&args[0], params)?;
                    let b = eval_const_expr_with_params(&args[1], params)?;
                    Some(a.min(b))
                }
                "sign" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.signum())
                }
                "floor" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.floor())
                }
                "ceil" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.ceil())
                }
                "integer" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.floor())
                }
                "mod" if args.len() == 2 => {
                    let a = eval_const_expr_with_params(&args[0], params)?;
                    let b = eval_const_expr_with_params(&args[1], params)?;
                    if b != 0.0 { Some(a % b) } else { None }
                }
                "rem" if args.len() == 2 => {
                    let a = eval_const_expr_with_params(&args[0], params)?;
                    let b = eval_const_expr_with_params(&args[1], params)?;
                    if b != 0.0 { Some(a % b) } else { None }
                }
                "div" if args.len() == 2 => {
                    let a = eval_const_expr_with_params(&args[0], params)?;
                    let b = eval_const_expr_with_params(&args[1], params)?;
                    if b != 0.0 { Some((a / b).trunc()) } else { None }
                }
                "sinh" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.sinh())
                }
                "cosh" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.cosh())
                }
                "tanh" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params).map(|v| v.tanh())
                }
                "noevent" | "smooth" if !args.is_empty() => {
                    eval_const_expr_with_params(&args[0], params)
                }
                "homotopy" if args.len() >= 1 => {
                    eval_const_expr_with_params(&args[0], params)
                }
                "fill" if args.len() >= 1 => {
                    eval_const_expr_with_params(&args[0], params)
                }
                "scalar" if args.len() == 1 => {
                    eval_const_expr_with_params(&args[0], params)
                }
                _ => None,
            }
        }
        Expression::ArrayLiteral(items) => {
            if items.len() == 1 {
                eval_const_expr_with_params(&items[0], params)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Expand `zeros(n) = rhs` / `ones(n) = rhs` into n scalar equations `rhs[i] = 0|1`.
/// FixedTranslation force/torque balance is emitted this way; a Call LHS is skipped by JIT.
pub(crate) fn expand_fill_lhs_equations(eqs: Vec<Equation>) -> Vec<Equation> {
    let mut out = Vec::with_capacity(eqs.len());
    for eq in eqs {
        match eq {
            Equation::Simple(lhs, rhs) => {
                if let Some((n, fill)) = fill_lhs_size_and_value(&lhs) {
                    for i in 1..=n {
                        let rhs_i = flatten_const_index(index_expression(&rhs, i));
                        out.push(Equation::Simple(rhs_i, Expression::Number(fill)));
                    }
                } else {
                    out.push(Equation::Simple(lhs, rhs));
                }
            }
            Equation::If(c, then_eqs, elifs, else_eqs) => out.push(Equation::If(
                c,
                expand_fill_lhs_equations(then_eqs),
                elifs
                    .into_iter()
                    .map(|(c, b)| (c, expand_fill_lhs_equations(b)))
                    .collect(),
                else_eqs.map(expand_fill_lhs_equations),
            )),
            Equation::For(v, start, end, body) => out.push(Equation::For(
                v,
                start,
                end,
                expand_fill_lhs_equations(body),
            )),
            Equation::When(c, body, elsewhens) => out.push(Equation::When(
                c,
                expand_fill_lhs_equations(body),
                elsewhens
                    .into_iter()
                    .map(|(c, b)| (c, expand_fill_lhs_equations(b)))
                    .collect(),
            )),
            other => out.push(other),
        }
    }
    out
}

fn fill_lhs_size_and_value(lhs: &Expression) -> Option<(usize, f64)> {
    match lhs {
        Expression::Call(name, args) => {
            let tail = name.rsplit('.').next().unwrap_or(name.as_str());
            let fill = match tail {
                "zeros" => 0.0,
                "ones" => 1.0,
                "fill" => match args.get(1) {
                    Some(Expression::Number(v)) => *v,
                    _ => return None,
                },
                _ => return None,
            };
            let n = match args.first() {
                Some(Expression::Number(v)) if *v >= 1.0 => *v as usize,
                _ => return None,
            };
            Some((n, fill))
        }
        _ => None,
    }
}

fn flatten_const_index(expr: Expression) -> Expression {
    match expr {
        Expression::ArrayAccess(arr, idx) => {
            let arr = flatten_const_index(*arr);
            let idx = flatten_const_index(*idx);
            match (&arr, &idx) {
                (Expression::Variable(id), Expression::Number(n)) => {
                    Expression::var(&format!("{}_{}", crate::string_intern::resolve_id(*id), *n as i64))
                }
                _ => Expression::ArrayAccess(Box::new(arr), Box::new(idx)),
            }
        }
        Expression::BinaryOp(l, op, r) => Expression::BinaryOp(
            Box::new(flatten_const_index(*l)),
            op,
            Box::new(flatten_const_index(*r)),
        ),
        Expression::Call(n, args) => {
            Expression::Call(n, args.into_iter().map(flatten_const_index).collect())
        }
        Expression::Der(inner) => Expression::Der(Box::new(flatten_const_index(*inner))),
        Expression::If(c, t, f) => Expression::If(
            Box::new(flatten_const_index(*c)),
            Box::new(flatten_const_index(*t)),
            Box::new(flatten_const_index(*f)),
        ),
        other => other,
    }
}

/// `anyTrue(inPort.set)` flattens as `anyTrue(step_inPort_set)` but elements are
/// `step_inPort_1_set`. Expand to a scalar OR/AND so BLT and JIT see the pins.
pub fn expand_boolean_vector_reductions(flat: &mut crate::flatten::FlattenedModel) {
    let names: HashSet<String> = flat.declarations.iter().map(|d| d.name.clone()).collect();
    for eq in &mut flat.equations {
        expand_bool_vec_in_eq(eq, &names);
    }
    for eq in &mut flat.initial_equations {
        expand_bool_vec_in_eq(eq, &names);
    }
}

fn expand_bool_vec_in_eq(eq: &mut Equation, names: &HashSet<String>) {
    match eq {
        Equation::Simple(lhs, rhs) => {
            *lhs = expand_bool_vec_expr(lhs, names);
            *rhs = expand_bool_vec_expr(rhs, names);
        }
        Equation::If(c, then_eqs, elifs, else_eqs) => {
            *c = expand_bool_vec_expr(c, names);
            for e in then_eqs {
                expand_bool_vec_in_eq(e, names);
            }
            for (ec, body) in elifs {
                *ec = expand_bool_vec_expr(ec, names);
                for e in body {
                    expand_bool_vec_in_eq(e, names);
                }
            }
            if let Some(body) = else_eqs {
                for e in body {
                    expand_bool_vec_in_eq(e, names);
                }
            }
        }
        _ => {}
    }
}

fn bool_vec_elements(base: &str, names: &HashSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for i in 1..=64 {
        let a = format!("{}_{}", base, i);
        if names.contains(&a) {
            out.push(a);
            continue;
        }
        if let Some((prefix, member)) = base.rsplit_once('_') {
            let b = format!("{}_{}_{}", prefix, i, member);
            if names.contains(&b) {
                out.push(b);
                continue;
            }
        }
        break;
    }
    out
}

fn is_bool_vec_reduction(func: &str) -> Option<&'static str> {
    let tail = func.rsplit('.').next().unwrap_or(func);
    match tail {
        "anyTrue" => Some("any"),
        "allTrue" => Some("all"),
        _ => None,
    }
}

fn reduce_bool_vec(kind: &str, elems: &[String]) -> Expression {
    if elems.is_empty() {
        return Expression::Number(if kind == "all" { 1.0 } else { 0.0 });
    }
    let op = if kind == "all" {
        Operator::And
    } else {
        Operator::Or
    };
    let mut acc = Expression::var(&elems[0]);
    for e in elems.iter().skip(1) {
        acc = Expression::BinaryOp(Box::new(acc), op, Box::new(Expression::var(e)));
    }
    acc
}

fn expand_bool_vec_expr(expr: &Expression, names: &HashSet<String>) -> Expression {
    match expr {
        Expression::Call(func, args) => {
            let args: Vec<Expression> = args
                .iter()
                .map(|a| expand_bool_vec_expr(a, names))
                .collect();
            if let Some(kind) = is_bool_vec_reduction(func) {
                if args.len() == 1 {
                    if let Expression::Variable(id) = &args[0] {
                        let base = crate::string_intern::resolve_id(*id);
                        let elems = bool_vec_elements(&base, names);
                        if !elems.is_empty() {
                            return reduce_bool_vec(kind, &elems);
                        }
                    }
                }
            }
            Expression::Call(func.clone(), args)
        }
        Expression::BinaryOp(l, op, r) => Expression::BinaryOp(
            Box::new(expand_bool_vec_expr(l, names)),
            *op,
            Box::new(expand_bool_vec_expr(r, names)),
        ),
        Expression::If(c, t, f) => Expression::If(
            Box::new(expand_bool_vec_expr(c, names)),
            Box::new(expand_bool_vec_expr(t, names)),
            Box::new(expand_bool_vec_expr(f, names)),
        ),
        Expression::Der(inner) => Expression::Der(Box::new(expand_bool_vec_expr(inner, names))),
        Expression::ArrayAccess(a, i) => Expression::ArrayAccess(
            Box::new(expand_bool_vec_expr(a, names)),
            Box::new(expand_bool_vec_expr(i, names)),
        ),
        Expression::ArrayLiteral(es) => {
            Expression::ArrayLiteral(es.iter().map(|e| expand_bool_vec_expr(e, names)).collect())
        }
        other => other.clone(),
    }
}

pub fn expr_to_path(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Variable(id) => Some(crate::string_intern::resolve_id(*id)),
        Expression::Dot(base, member) => {
            if let Some(base_path) = expr_to_path(base) {
                Some(format!("{}.{}", base_path, member))
            } else {
                None
            }
        }
        _ => None,
    }
}
