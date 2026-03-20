use crate::ast::{flat_index_suffix_for_scalar_name, Expression, Operator};
use std::collections::HashMap;

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
    if let Expression::Variable(name) = arr_flat {
        if let Some(v) = eval_const_expr(idx_flat) {
            let n_int = v.round() as i64;
            return Some(Expression::Variable(format!("{}_{}", name, n_int)));
        }
        if let Some(suf) = flat_index_suffix_for_scalar_name(idx_flat) {
            return Some(Expression::Variable(format!("{}_{}", name, suf)));
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
        Expression::Variable(name) => {
            if member == "signal" {
                Expression::Variable(name)
            } else {
                Expression::Variable(format!("{}_{}", name, member))
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
        Expression::Variable(name) => {
            if name == "time" {
                return expr.clone();
            }
            let flat_name = name.replace('.', "_");
            Expression::Variable(format!("{}{}", prefix_str, flat_name))
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
        Expression::StringLiteral(s) => Expression::StringLiteral(s.clone()),
    }
}

pub fn index_expression(expr: &Expression, idx: usize) -> Expression {
    match expr {
        Expression::Variable(_name) => Expression::ArrayAccess(
            Box::new(expr.clone()),
            Box::new(Expression::Number(idx as f64)),
        ),
        Expression::Number(_) => expr.clone(),
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
            Box::new(index_expression(lhs, idx)),
            *op,
            Box::new(index_expression(rhs, idx)),
        ),
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
        Expression::Call(func, args) => Expression::Call(
            func.clone(),
            args.iter().map(|arg| index_expression(arg, idx)).collect(),
        ),
        Expression::Der(arg) => Expression::Der(Box::new(index_expression(arg, idx))),
        Expression::ArrayAccess(_arr, _) => expr.clone(),
        _ => expr.clone(),
    }
}

pub fn eval_const_expr(expr: &Expression) -> Option<f64> {
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

pub fn eval_const_expr_with_array_sizes(
    expr: &Expression,
    array_sizes: &HashMap<String, usize>,
) -> Option<f64> {
    match expr {
        Expression::Number(n) => Some(*n),
        Expression::Variable(name) => array_sizes.get(name).map(|v| *v as f64),
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = eval_const_expr_with_array_sizes(lhs, array_sizes)?;
            let r = eval_const_expr_with_array_sizes(rhs, array_sizes)?;
            match op {
                Operator::Add => Some(l + r),
                Operator::Sub => Some(l - r),
                Operator::Mul => Some(l * r),
                Operator::Div => Some(l / r),
                _ => None,
            }
        }
        Expression::If(cond, t_expr, f_expr) => {
            let c = eval_const_expr_with_array_sizes(cond, array_sizes)?;
            if c != 0.0 {
                eval_const_expr_with_array_sizes(t_expr, array_sizes)
            } else {
                eval_const_expr_with_array_sizes(f_expr, array_sizes)
            }
        }
        Expression::Call(func, args) if func == "size" => {
            let first = args.first()?;
            match first {
                Expression::Variable(name) => array_sizes.get(name).map(|v| *v as f64),
                Expression::ArrayLiteral(items) => Some(items.len() as f64),
                Expression::Number(_) => Some(1.0),
                _ => None,
            }
        }
        _ => eval_const_expr(expr),
    }
}

pub fn expr_to_path(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Variable(name) => Some(name.clone()),
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
