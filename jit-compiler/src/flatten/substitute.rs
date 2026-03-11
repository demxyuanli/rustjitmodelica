use std::collections::HashMap;
use crate::ast::Expression;

use super::expressions::expr_to_path;

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
                    val
                } else {
                    expr.clone()
                }
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
                    let idx = *n as usize;
                    if idx > 0 && idx <= elements.len() {
                        elements[idx - 1].clone()
                    } else {
                        eprintln!(
                            "Index out of bounds in substitution: {} (len {})",
                            idx,
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
            Expression::Sample(inner) => Expression::Sample(Box::new(self.substitute_stack(inner, context_stack))),
            Expression::Interval(inner) => Expression::Interval(Box::new(self.substitute_stack(inner, context_stack))),
            Expression::Hold(inner) => Expression::Hold(Box::new(self.substitute_stack(inner, context_stack))),
            Expression::Previous(inner) => Expression::Previous(Box::new(self.substitute_stack(inner, context_stack))),
            Expression::SubSample(c, n) => Expression::SubSample(Box::new(self.substitute_stack(c, context_stack)), Box::new(self.substitute_stack(n, context_stack))),
            Expression::SuperSample(c, n) => Expression::SuperSample(Box::new(self.substitute_stack(c, context_stack)), Box::new(self.substitute_stack(n, context_stack))),
            Expression::ShiftSample(c, n) => Expression::ShiftSample(Box::new(self.substitute_stack(c, context_stack)), Box::new(self.substitute_stack(n, context_stack))),
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
                    let idx = *n as usize;
                    if idx > 0 && idx <= elements.len() {
                        elements[idx - 1].clone()
                    } else {
                        eprintln!(
                            "Index out of bounds in substitution: {} (len {})",
                            idx,
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
                exprs
                    .iter()
                    .map(|e| self.substitute(e, context))
                    .collect(),
            ),
            Expression::Sample(inner) => Expression::Sample(Box::new(self.substitute(inner, context))),
            Expression::Interval(inner) => Expression::Interval(Box::new(self.substitute(inner, context))),
            Expression::Hold(inner) => Expression::Hold(Box::new(self.substitute(inner, context))),
            Expression::Previous(inner) => Expression::Previous(Box::new(self.substitute(inner, context))),
            Expression::SubSample(c, n) => Expression::SubSample(Box::new(self.substitute(c, context)), Box::new(self.substitute(n, context))),
            Expression::SuperSample(c, n) => Expression::SuperSample(Box::new(self.substitute(c, context)), Box::new(self.substitute(n, context))),
            Expression::ShiftSample(c, n) => Expression::ShiftSample(Box::new(self.substitute(c, context)), Box::new(self.substitute(n, context))),
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
}
