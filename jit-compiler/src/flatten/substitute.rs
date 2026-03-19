use crate::ast::Expression;
use std::collections::HashMap;

use super::expressions::{eval_const_expr, expr_to_path};

fn expand_range_indices(start: &Expression, step: &Expression, end: &Expression) -> Option<Vec<i64>> {
    let start_val = eval_const_expr(start)? as i64;
    let step_val = eval_const_expr(step)? as i64;
    let end_val = eval_const_expr(end)? as i64;
    if step_val == 0 {
        return None;
    }
    let mut values = Vec::new();
    let mut curr = start_val;
    let max_len = 100_000;
    while (step_val > 0 && curr <= end_val) || (step_val < 0 && curr >= end_val) {
        values.push(curr);
        if values.len() >= max_len {
            break;
        }
        curr += step_val;
    }
    Some(values)
}

impl super::Flattener {
    pub(crate) fn lookup_context_stack(
        context_stack: &[HashMap<String, Expression>],
        name: &str,
    ) -> Option<Expression> {
        for map in context_stack.iter().rev() {
            if let Some(val) = map.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    fn substitute_stack_inner(
        &mut self,
        expr: &Expression,
        context_stack: &[HashMap<String, Expression>],
        visiting: &mut std::collections::HashSet<String>,
    ) -> Expression {
        let substituted = match expr {
            Expression::Variable(name) => {
                if visiting.contains(name) {
                    // Break simple substitution cycles like a=b, b=a.
                    return expr.clone();
                }
                if let Some(val) = Self::lookup_context_stack(context_stack, name) {
                    if matches!(&val, Expression::Variable(inner) if inner == name) {
                        return val;
                    }
                    visiting.insert(name.clone());
                    let out = self.substitute_stack_inner(&val, context_stack, visiting);
                    visiting.remove(name);
                    return out;
                }
                // Resolve global constants so JIT sees Number instead of opaque variable names.
                let path = if name.contains('.') {
                    name.clone()
                } else if name.starts_with("Modelica_") {
                    name.replace("Modelica_", "Modelica.").replace("Constants_", "Constants.")
                } else {
                    String::new()
                };
                if !path.is_empty() {
                    if let Some(val) = self.resolve_global_constant(&path) {
                        return val;
                    }
                }
                expr.clone()
            }
            Expression::Number(_) => expr.clone(),
            Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
                Box::new(self.substitute_stack_inner(lhs, context_stack, visiting)),
                op.clone(),
                Box::new(self.substitute_stack_inner(rhs, context_stack, visiting)),
            ),
            Expression::Call(func, args) => Expression::Call(
                func.clone(),
                args.iter()
                    .map(|arg| self.substitute_stack_inner(arg, context_stack, visiting))
                    .collect(),
            ),
            Expression::Der(arg) => {
                Expression::Der(Box::new(self.substitute_stack_inner(arg, context_stack, visiting)))
            }
            Expression::ArrayAccess(arr, idx) => {
                let new_arr = self.substitute_stack_inner(arr, context_stack, visiting);
                let new_idx = self.substitute_stack_inner(idx, context_stack, visiting);
                if let (Expression::Variable(name), Expression::Number(n)) = (&new_arr, &new_idx) {
                    let n_int = *n as i64;
                    Expression::Variable(format!("{}_{}", name, n_int))
                } else if let (Expression::Variable(name), Expression::Range(start, step, end)) =
                    (&new_arr, &new_idx)
                {
                    if let Some(indices) = expand_range_indices(start, step, end) {
                        Expression::ArrayLiteral(
                            indices
                                .into_iter()
                                .map(|n| Expression::Variable(format!("{}_{}", name, n)))
                                .collect(),
                        )
                    } else {
                        Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                    }
                } else if let (Expression::ArrayLiteral(elements), Expression::Number(n)) =
                    (&new_arr, &new_idx)
                {
                    let n_usize = *n as usize;
                    let idx0 = if n_usize == 0 && !elements.is_empty() {
                        0
                    } else if n_usize >= 1 && n_usize <= elements.len() {
                        n_usize - 1
                    } else {
                        elements.len()
                    };
                    if idx0 < elements.len() {
                        elements[idx0].clone()
                    } else {
                        eprintln!(
                            "Index out of bounds in substitution: {} (len {})",
                            n_usize,
                            elements.len()
                        );
                        Expression::Number(0.0)
                    }
                } else if let (Expression::ArrayLiteral(elements), Expression::Range(start, step, end)) =
                    (&new_arr, &new_idx)
                {
                    if let Some(indices) = expand_range_indices(start, step, end) {
                        let mut out = Vec::new();
                        for n in indices {
                            let n_usize = n as usize;
                            if n_usize >= 1 && n_usize <= elements.len() {
                                out.push(elements[n_usize - 1].clone());
                            }
                        }
                        Expression::ArrayLiteral(out)
                    } else {
                        Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                    }
                } else {
                    Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                }
            }
            Expression::Dot(base, member) => {
                let new_base = self.substitute_stack_inner(base, context_stack, visiting);
                if let Some(base_path) = expr_to_path(&new_base) {
                    let full_path = format!("{}.{}", base_path, member);
                    if let Some(val) = self.resolve_global_constant(&full_path) {
                        return val;
                    }
                }
                Expression::Dot(Box::new(new_base), member.clone())
            }
            Expression::If(cond, t_expr, f_expr) => Expression::If(
                Box::new(self.substitute_stack_inner(cond, context_stack, visiting)),
                Box::new(self.substitute_stack_inner(t_expr, context_stack, visiting)),
                Box::new(self.substitute_stack_inner(f_expr, context_stack, visiting)),
            ),
            Expression::Range(start, step, end) => Expression::Range(
                Box::new(self.substitute_stack_inner(start, context_stack, visiting)),
                Box::new(self.substitute_stack_inner(step, context_stack, visiting)),
                Box::new(self.substitute_stack_inner(end, context_stack, visiting)),
            ),
            Expression::ArrayLiteral(exprs) => Expression::ArrayLiteral(
                exprs
                    .iter()
                    .map(|e| self.substitute_stack_inner(e, context_stack, visiting))
                    .collect(),
            ),
            Expression::ArrayComprehension { expr, iter_var, iter_range } => {
                let range_sub = self.substitute_stack_inner(iter_range, context_stack, visiting);
                let expr_sub = self.substitute_stack_inner(expr, context_stack, visiting);
                if let Some(expanded) = self.expand_comprehension_to_literal(
                    &expr_sub,
                    iter_var,
                    &range_sub,
                    context_stack,
                ) {
                    expanded
                } else {
                    Expression::ArrayComprehension {
                        expr: Box::new(expr_sub),
                        iter_var: iter_var.clone(),
                        iter_range: Box::new(range_sub),
                    }
                }
            }
            Expression::Sample(inner) => {
                Expression::Sample(Box::new(self.substitute_stack_inner(inner, context_stack, visiting)))
            }
            Expression::Interval(inner) => {
                Expression::Interval(Box::new(self.substitute_stack_inner(inner, context_stack, visiting)))
            }
            Expression::Hold(inner) => {
                Expression::Hold(Box::new(self.substitute_stack_inner(inner, context_stack, visiting)))
            }
            Expression::Previous(inner) => {
                Expression::Previous(Box::new(self.substitute_stack_inner(inner, context_stack, visiting)))
            }
            Expression::SubSample(c, n) => Expression::SubSample(
                Box::new(self.substitute_stack_inner(c, context_stack, visiting)),
                Box::new(self.substitute_stack_inner(n, context_stack, visiting)),
            ),
            Expression::SuperSample(c, n) => Expression::SuperSample(
                Box::new(self.substitute_stack_inner(c, context_stack, visiting)),
                Box::new(self.substitute_stack_inner(n, context_stack, visiting)),
            ),
            Expression::ShiftSample(c, n) => Expression::ShiftSample(
                Box::new(self.substitute_stack_inner(c, context_stack, visiting)),
                Box::new(self.substitute_stack_inner(n, context_stack, visiting)),
            ),
            Expression::StringLiteral(s) => Expression::StringLiteral(s.clone()),
        };
        match substituted {
            Expression::Call(func, args) if func == "size" => {
                if let Some(first) = args.first() {
                    match first {
                        Expression::ArrayLiteral(items) => Expression::Number(items.len() as f64),
                        Expression::Number(_) => Expression::Number(1.0),
                        _ => Expression::Call(func, args),
                    }
                } else {
                    Expression::Call(func, args)
                }
            }
            other => other,
        }
    }

    pub(crate) fn substitute_stack(
        &mut self,
        expr: &Expression,
        context_stack: &[HashMap<String, Expression>],
    ) -> Expression {
        let mut visiting = std::collections::HashSet::new();
        self.substitute_stack_inner(expr, context_stack, &mut visiting)
    }

    pub(crate) fn substitute(
        &mut self,
        expr: &Expression,
        context: &HashMap<String, Expression>,
    ) -> Expression {
        fn inner(
            fl: &mut super::Flattener,
            expr: &Expression,
            context: &HashMap<String, Expression>,
            visiting: &mut std::collections::HashSet<String>,
        ) -> Expression {
            let substituted = match expr {
                Expression::Variable(name) => {
                    if visiting.contains(name) {
                        return expr.clone();
                    }
                    if let Some(val) = context.get(name) {
                        if matches!(val, Expression::Variable(inner_name) if inner_name == name) {
                            val.clone()
                        } else {
                            visiting.insert(name.clone());
                            let out = inner(fl, val, context, visiting);
                            visiting.remove(name);
                            out
                        }
                    } else {
                        expr.clone()
                    }
                }
                Expression::Number(_) => expr.clone(),
                Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
                    Box::new(inner(fl, lhs, context, visiting)),
                    op.clone(),
                    Box::new(inner(fl, rhs, context, visiting)),
                ),
                Expression::Call(func, args) => Expression::Call(
                    func.clone(),
                    args.iter()
                        .map(|arg| inner(fl, arg, context, visiting))
                        .collect(),
                ),
                Expression::Der(arg) => Expression::Der(Box::new(inner(fl, arg, context, visiting))),
                Expression::ArrayAccess(arr, idx) => {
                    let new_arr = inner(fl, arr, context, visiting);
                    let new_idx = inner(fl, idx, context, visiting);

                    if let (Expression::Variable(name), Expression::Number(n)) = (&new_arr, &new_idx) {
                        let n_int = *n as i64;
                        Expression::Variable(format!("{}_{}", name, n_int))
                    } else if let (Expression::Variable(name), Expression::Range(start, step, end)) =
                        (&new_arr, &new_idx)
                    {
                        if let Some(indices) = expand_range_indices(start, step, end) {
                            Expression::ArrayLiteral(
                                indices
                                    .into_iter()
                                    .map(|n| Expression::Variable(format!("{}_{}", name, n)))
                                    .collect(),
                            )
                        } else {
                            Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                        }
                    } else if let (Expression::ArrayLiteral(elements), Expression::Number(n)) =
                        (&new_arr, &new_idx)
                    {
                        let n_usize = *n as usize;
                        let idx0 = if n_usize == 0 && !elements.is_empty() {
                            0
                        } else if n_usize >= 1 && n_usize <= elements.len() {
                            n_usize - 1
                        } else {
                            elements.len()
                        };
                        if idx0 < elements.len() {
                            elements[idx0].clone()
                        } else {
                            eprintln!(
                                "Index out of bounds in substitution: {} (len {})",
                                n_usize,
                                elements.len()
                            );
                            Expression::Number(0.0)
                        }
                    } else if let (Expression::ArrayLiteral(elements), Expression::Range(start, step, end)) =
                        (&new_arr, &new_idx)
                    {
                        if let Some(indices) = expand_range_indices(start, step, end) {
                            let mut out = Vec::new();
                            for n in indices {
                                let n_usize = n as usize;
                                if n_usize >= 1 && n_usize <= elements.len() {
                                    out.push(elements[n_usize - 1].clone());
                                }
                            }
                            Expression::ArrayLiteral(out)
                        } else {
                            Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                        }
                    } else {
                        Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                    }
                }
                Expression::Dot(base, member) => {
                    let new_base = inner(fl, base, context, visiting);

                    if let Some(base_path) = expr_to_path(&new_base) {
                        let full_path = format!("{}.{}", base_path, member);
                        if let Some(val) = fl.resolve_global_constant(&full_path) {
                            return val;
                        }
                    }

                    Expression::Dot(Box::new(new_base), member.clone())
                }
                Expression::If(cond, t_expr, f_expr) => Expression::If(
                    Box::new(inner(fl, cond, context, visiting)),
                    Box::new(inner(fl, t_expr, context, visiting)),
                    Box::new(inner(fl, f_expr, context, visiting)),
                ),
                Expression::Range(start, step, end) => Expression::Range(
                    Box::new(inner(fl, start, context, visiting)),
                    Box::new(inner(fl, step, context, visiting)),
                    Box::new(inner(fl, end, context, visiting)),
                ),
                Expression::ArrayLiteral(exprs) => Expression::ArrayLiteral(
                    exprs.iter().map(|e| inner(fl, e, context, visiting)).collect(),
                ),
                Expression::ArrayComprehension { expr, iter_var, iter_range } => {
                    let range_sub = inner(fl, iter_range, context, visiting);
                    let expr_sub = inner(fl, expr, context, visiting);
                    if let Some(expanded) =
                        fl.expand_comprehension_to_literal_flat(&expr_sub, iter_var, &range_sub, context)
                    {
                        expanded
                    } else {
                        Expression::ArrayComprehension {
                            expr: Box::new(expr_sub),
                            iter_var: iter_var.clone(),
                            iter_range: Box::new(range_sub),
                        }
                    }
                }
                Expression::Sample(inner0) => {
                    Expression::Sample(Box::new(inner(fl, inner0, context, visiting)))
                }
                Expression::Interval(inner0) => {
                    Expression::Interval(Box::new(inner(fl, inner0, context, visiting)))
                }
                Expression::Hold(inner0) => {
                    Expression::Hold(Box::new(inner(fl, inner0, context, visiting)))
                }
                Expression::Previous(inner0) => {
                    Expression::Previous(Box::new(inner(fl, inner0, context, visiting)))
                }
                Expression::SubSample(c, n) => Expression::SubSample(
                    Box::new(inner(fl, c, context, visiting)),
                    Box::new(inner(fl, n, context, visiting)),
                ),
                Expression::SuperSample(c, n) => Expression::SuperSample(
                    Box::new(inner(fl, c, context, visiting)),
                    Box::new(inner(fl, n, context, visiting)),
                ),
                Expression::ShiftSample(c, n) => Expression::ShiftSample(
                    Box::new(inner(fl, c, context, visiting)),
                    Box::new(inner(fl, n, context, visiting)),
                ),
                Expression::StringLiteral(s) => Expression::StringLiteral(s.clone()),
            };
            match substituted {
                Expression::Call(func, args) if func == "size" => {
                    if let Some(first) = args.first() {
                        match first {
                            Expression::ArrayLiteral(items) => Expression::Number(items.len() as f64),
                            Expression::Number(_) => Expression::Number(1.0),
                            _ => Expression::Call(func, args),
                        }
                    } else {
                        Expression::Call(func, args)
                    }
                }
                other => other,
            }
        }
        let mut visiting = std::collections::HashSet::new();
        inner(self, expr, context, &mut visiting)
    }

    pub(crate) fn resolve_global_constant(&mut self, path: &str) -> Option<Expression> {
        if let Some((model_name, var_name)) = path.rsplit_once('.') {
            if let Ok(model) = self.loader.load_model_silent(model_name, true) {
                for decl in &model.declarations {
                    if decl.name == var_name {
                        if let Some(val) = &decl.start_value {
                            return Some(val.clone());
                        }
                    }
                }
            }
        }
        None
    }

    fn expand_comprehension_to_literal_flat(
        &mut self,
        expr: &Expression,
        iter_var: &str,
        range_sub: &Expression,
        context: &HashMap<String, Expression>,
    ) -> Option<Expression> {
        let (start_val, step_val, end_val) = match range_sub {
            Expression::Range(start, step, end) => {
                let s = eval_const_expr(start)?;
                let st = eval_const_expr(step)?;
                let e = eval_const_expr(end)?;
                (s, st, e)
            }
            _ => return None,
        };
        let mut values = Vec::new();
        let max_len = 100_000;
        let mut v = start_val;
        loop {
            if (step_val > 0.0 && v <= end_val) || (step_val < 0.0 && v >= end_val) {
                values.push(v);
            }
            if step_val == 0.0 || values.len() >= max_len {
                break;
            }
            v += step_val;
            if (step_val > 0.0 && v > end_val) || (step_val < 0.0 && v < end_val) {
                break;
            }
        }
        let mut out = Vec::with_capacity(values.len());
        for val in values {
            let mut ctx = context.clone();
            ctx.insert(iter_var.to_string(), Expression::Number(val));
            out.push(self.substitute(expr, &ctx));
        }
        Some(Expression::ArrayLiteral(out))
    }

    fn expand_comprehension_to_literal(
        &mut self,
        expr: &Expression,
        iter_var: &str,
        range_sub: &Expression,
        context_stack: &[HashMap<String, Expression>],
    ) -> Option<Expression> {
        let (start_val, step_val, end_val) = match range_sub {
            Expression::Range(start, step, end) => {
                let s = eval_const_expr(start)?;
                let st = eval_const_expr(step)?;
                let e = eval_const_expr(end)?;
                (s, st, e)
            }
            _ => return None,
        };
        let mut values = Vec::new();
        let max_len = 100_000;
        let mut v = start_val;
        loop {
            if (step_val > 0.0 && v <= end_val) || (step_val < 0.0 && v >= end_val) {
                values.push(v);
            }
            if step_val == 0.0 || values.len() >= max_len {
                break;
            }
            v += step_val;
            if (step_val > 0.0 && v > end_val) || (step_val < 0.0 && v < end_val) {
                break;
            }
        }
        let mut out = Vec::with_capacity(values.len());
        for val in values {
            let mut new_stack = context_stack.to_vec();
            let mut frame = HashMap::new();
            frame.insert(iter_var.to_string(), Expression::Number(val));
            new_stack.push(frame);
            out.push(self.substitute_stack(expr, &new_stack));
        }
        Some(Expression::ArrayLiteral(out))
    }
}
