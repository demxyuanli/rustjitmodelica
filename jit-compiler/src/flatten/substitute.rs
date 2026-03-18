use crate::ast::Expression;
use std::collections::HashMap;

use super::expressions::{eval_const_expr, expr_to_path};

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

    pub(crate) fn substitute_stack(
        &mut self,
        expr: &Expression,
        context_stack: &[HashMap<String, Expression>],
    ) -> Expression {
        match expr {
            Expression::Variable(name) => {
                if let Some(val) = Self::lookup_context_stack(context_stack, name) {
                    return val;
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
                Box::new(self.substitute_stack(lhs, context_stack)),
                op.clone(),
                Box::new(self.substitute_stack(rhs, context_stack)),
            ),
            Expression::Call(func, args) => Expression::Call(
                func.clone(),
                args.iter()
                    .map(|arg| self.substitute_stack(arg, context_stack))
                    .collect(),
            ),
            Expression::Der(arg) => {
                Expression::Der(Box::new(self.substitute_stack(arg, context_stack)))
            }
            Expression::ArrayAccess(arr, idx) => {
                let new_arr = self.substitute_stack(arr, context_stack);
                let new_idx = self.substitute_stack(idx, context_stack);
                if let (Expression::Variable(name), Expression::Number(n)) = (&new_arr, &new_idx) {
                    let n_int = *n as i64;
                    Expression::Variable(format!("{}_{}", name, n_int))
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
                } else {
                    Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                }
            }
            Expression::Dot(base, member) => {
                let new_base = self.substitute_stack(base, context_stack);
                if let Some(base_path) = expr_to_path(&new_base) {
                    let full_path = format!("{}.{}", base_path, member);
                    if let Some(val) = self.resolve_global_constant(&full_path) {
                        return val;
                    }
                }
                Expression::Dot(Box::new(new_base), member.clone())
            }
            Expression::If(cond, t_expr, f_expr) => Expression::If(
                Box::new(self.substitute_stack(cond, context_stack)),
                Box::new(self.substitute_stack(t_expr, context_stack)),
                Box::new(self.substitute_stack(f_expr, context_stack)),
            ),
            Expression::Range(start, step, end) => Expression::Range(
                Box::new(self.substitute_stack(start, context_stack)),
                Box::new(self.substitute_stack(step, context_stack)),
                Box::new(self.substitute_stack(end, context_stack)),
            ),
            Expression::ArrayLiteral(exprs) => Expression::ArrayLiteral(
                exprs
                    .iter()
                    .map(|e| self.substitute_stack(e, context_stack))
                    .collect(),
            ),
            Expression::ArrayComprehension { expr, iter_var, iter_range } => {
                let range_sub = self.substitute_stack(iter_range, context_stack);
                let expr_sub = self.substitute_stack(expr, context_stack);
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
                Expression::Sample(Box::new(self.substitute_stack(inner, context_stack)))
            }
            Expression::Interval(inner) => {
                Expression::Interval(Box::new(self.substitute_stack(inner, context_stack)))
            }
            Expression::Hold(inner) => {
                Expression::Hold(Box::new(self.substitute_stack(inner, context_stack)))
            }
            Expression::Previous(inner) => {
                Expression::Previous(Box::new(self.substitute_stack(inner, context_stack)))
            }
            Expression::SubSample(c, n) => Expression::SubSample(
                Box::new(self.substitute_stack(c, context_stack)),
                Box::new(self.substitute_stack(n, context_stack)),
            ),
            Expression::SuperSample(c, n) => Expression::SuperSample(
                Box::new(self.substitute_stack(c, context_stack)),
                Box::new(self.substitute_stack(n, context_stack)),
            ),
            Expression::ShiftSample(c, n) => Expression::ShiftSample(
                Box::new(self.substitute_stack(c, context_stack)),
                Box::new(self.substitute_stack(n, context_stack)),
            ),
            Expression::StringLiteral(s) => Expression::StringLiteral(s.clone()),
        }
    }

    pub(crate) fn substitute(
        &mut self,
        expr: &Expression,
        context: &HashMap<String, Expression>,
    ) -> Expression {
        match expr {
            Expression::Variable(name) => {
                if let Some(val) = context.get(name) {
                    val.clone()
                } else {
                    expr.clone()
                }
            }
            Expression::Number(_) => expr.clone(),
            Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
                Box::new(self.substitute(lhs, context)),
                op.clone(),
                Box::new(self.substitute(rhs, context)),
            ),
            Expression::Call(func, args) => Expression::Call(
                func.clone(),
                args.iter()
                    .map(|arg| self.substitute(arg, context))
                    .collect(),
            ),
            Expression::Der(arg) => Expression::Der(Box::new(self.substitute(arg, context))),
            Expression::ArrayAccess(arr, idx) => {
                let new_arr = self.substitute(arr, context);
                let new_idx = self.substitute(idx, context);

                if let (Expression::Variable(name), Expression::Number(n)) = (&new_arr, &new_idx) {
                    let n_int = *n as i64;
                    Expression::Variable(format!("{}_{}", name, n_int))
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
                } else {
                    Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                }
            }
            Expression::Dot(base, member) => {
                let new_base = self.substitute(base, context);

                if let Some(base_path) = expr_to_path(&new_base) {
                    let full_path = format!("{}.{}", base_path, member);
                    if let Some(val) = self.resolve_global_constant(&full_path) {
                        return val;
                    }
                }

                Expression::Dot(Box::new(new_base), member.clone())
            }
            Expression::If(cond, t_expr, f_expr) => Expression::If(
                Box::new(self.substitute(cond, context)),
                Box::new(self.substitute(t_expr, context)),
                Box::new(self.substitute(f_expr, context)),
            ),
            Expression::Range(start, step, end) => Expression::Range(
                Box::new(self.substitute(start, context)),
                Box::new(self.substitute(step, context)),
                Box::new(self.substitute(end, context)),
            ),
            Expression::ArrayLiteral(exprs) => Expression::ArrayLiteral(
                exprs.iter().map(|e| self.substitute(e, context)).collect(),
            ),
            Expression::ArrayComprehension { expr, iter_var, iter_range } => {
                let range_sub = self.substitute(iter_range, context);
                let expr_sub = self.substitute(expr, context);
                if let Some(expanded) =
                    self.expand_comprehension_to_literal_flat(&expr_sub, iter_var, &range_sub, context)
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
            Expression::Sample(inner) => {
                Expression::Sample(Box::new(self.substitute(inner, context)))
            }
            Expression::Interval(inner) => {
                Expression::Interval(Box::new(self.substitute(inner, context)))
            }
            Expression::Hold(inner) => Expression::Hold(Box::new(self.substitute(inner, context))),
            Expression::Previous(inner) => {
                Expression::Previous(Box::new(self.substitute(inner, context)))
            }
            Expression::SubSample(c, n) => Expression::SubSample(
                Box::new(self.substitute(c, context)),
                Box::new(self.substitute(n, context)),
            ),
            Expression::SuperSample(c, n) => Expression::SuperSample(
                Box::new(self.substitute(c, context)),
                Box::new(self.substitute(n, context)),
            ),
            Expression::ShiftSample(c, n) => Expression::ShiftSample(
                Box::new(self.substitute(c, context)),
                Box::new(self.substitute(n, context)),
            ),
            Expression::StringLiteral(s) => Expression::StringLiteral(s.clone()),
        }
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
