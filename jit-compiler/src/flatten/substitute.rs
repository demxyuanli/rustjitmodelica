use crate::ast::{flat_index_suffix_for_scalar_name, Expression};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::hash::{Hash, Hasher};

use super::expressions::{eval_const_expr, expr_to_path};

#[derive(Debug)]
pub struct SubstituteCache {
    cache: HashMap<*const Expression, Expression>,
    max_size: usize,
    order: Vec<*const Expression>,
}

impl SubstituteCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size.min(4096)),
            max_size,
            order: Vec::with_capacity(max_size.min(4096)),
        }
    }
}

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

#[allow(dead_code)]
fn global_subst_cache() -> &'static RwLock<HashMap<u128, Expression>> {
    static C: OnceLock<RwLock<HashMap<u128, Expression>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(HashMap::new()))
}

#[allow(dead_code)]
fn hash_expr_bincode(expr: &Expression) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Ok(bytes) = bincode::serialize(expr) {
        bytes.hash(&mut h);
    } else {
        std::mem::discriminant(expr).hash(&mut h);
    }
    h.finish()
}

#[allow(dead_code)]
fn hash_context(context: &HashMap<String, Expression>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut keys: Vec<&String> = context.keys().collect();
    keys.sort();
    for k in keys {
        k.hash(&mut h);
        if let Some(v) = context.get(k) {
            hash_expr_bincode(v).hash(&mut h);
        }
    }
    h.finish()
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
            Expression::Variable(id) => {
                let name = crate::string_intern::resolve_id(*id);
                if visiting.contains(&name) {
                    return expr.clone();
                }
                if let Some(val) = Self::lookup_context_stack(context_stack, &name) {
                    if matches!(&val, Expression::Variable(inner_id) if *inner_id == *id) {
                        return val;
                    }
                    visiting.insert(name.clone());
                    let out = self.substitute_stack_inner(&val, context_stack, visiting);
                    visiting.remove(&name);
                    return out;
                }
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
                if let (Expression::Variable(id), Expression::Number(n)) = (&new_arr, &new_idx) {
                    let name = crate::string_intern::resolve_id(*id);
                    let n_int = *n as i64;
                    Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, n_int)))
                } else if let (Expression::Variable(id), Expression::Range(start, step, end)) =
                    (&new_arr, &new_idx)
                {
                    let name = crate::string_intern::resolve_id(*id);
                    if let Some(indices) = expand_range_indices(start, step, end) {
                        Expression::ArrayLiteral(
                            indices
                                .into_iter()
                                .map(|n| Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, n))))
                                .collect(),
                        )
                    } else {
                        Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                    }
                } else if let Expression::Variable(id) = &new_arr {
                    if let Some(suf) = flat_index_suffix_for_scalar_name(&new_idx) {
                        let name = crate::string_intern::resolve_id(*id);
                        Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, suf)))
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
            Expression::BackSample(c, n) => Expression::BackSample(
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

    #[allow(dead_code)]
    fn substitute_cached_inner(
        &mut self,
        expr: &Expression,
        context: &HashMap<String, Expression>,
        visiting: &mut std::collections::HashSet<String>,
        cache: &mut SubstituteCache,
    ) -> Expression {
        let ptr = expr as *const Expression;
        if let Some(v) = cache.cache.get(&ptr) {
            return v.clone();
        }
        let out = match expr {
            Expression::Variable(id) => {
                let name = crate::string_intern::resolve_id(*id);
                if visiting.contains(&name) {
                    expr.clone()
                } else if let Some(val) = context.get(&name) {
                    if matches!(val, Expression::Variable(inner_id) if *inner_id == *id) {
                        val.clone()
                    } else {
                        visiting.insert(name.clone());
                        let v = self.substitute_cached_inner(val, context, visiting, cache);
                        visiting.remove(&name);
                        v
                    }
                } else {
                    let path = if name.contains('.') {
                        name.clone()
                    } else if name.starts_with("Modelica_") {
                        name.replace("Modelica_", "Modelica.")
                            .replace("Constants_", "Constants.")
                    } else {
                        String::new()
                    };
                    if !path.is_empty() {
                        if let Some(val) = self.resolve_global_constant(&path) {
                            val
                        } else {
                            expr.clone()
                        }
                    } else {
                        expr.clone()
                    }
                }
            }
            Expression::Number(_) => expr.clone(),
            Expression::StringLiteral(_) => expr.clone(),
            Expression::BinaryOp(lhs, op, rhs) => {
                let new_lhs = self.substitute_cached_inner(lhs, context, visiting, cache);
                let new_rhs = self.substitute_cached_inner(rhs, context, visiting, cache);
                if new_lhs == **lhs && new_rhs == **rhs {
                    expr.clone()
                } else {
                    Expression::BinaryOp(Box::new(new_lhs), op.clone(), Box::new(new_rhs))
                }
            }
            Expression::Call(func, args) => {
                let mut changed = false;
                let mut new_args: Vec<Expression> = Vec::with_capacity(args.len());
                for (i, a) in args.iter().enumerate() {
                    let na = self.substitute_cached_inner(a, context, visiting, cache);
                    if na != args[i] {
                        changed = true;
                    }
                    new_args.push(na);
                }
                if !changed {
                    expr.clone()
                } else {
                    Expression::Call(func.clone(), new_args)
                }
            }
            Expression::Der(arg) => {
                let new_arg = self.substitute_cached_inner(arg, context, visiting, cache);
                if new_arg == **arg {
                    expr.clone()
                } else {
                    Expression::Der(Box::new(new_arg))
                }
            }
            Expression::ArrayAccess(arr, idx) => {
                let new_arr = self.substitute_cached_inner(arr, context, visiting, cache);
                let new_idx = self.substitute_cached_inner(idx, context, visiting, cache);
                if let (Expression::Variable(id), Expression::Number(n)) = (&new_arr, &new_idx) {
                    let name = crate::string_intern::resolve_id(*id);
                    let n_int = *n as i64;
                    Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, n_int)))
                } else if let (Expression::Variable(id), Expression::Range(start, step, end)) =
                    (&new_arr, &new_idx)
                {
                    let name = crate::string_intern::resolve_id(*id);
                    if let Some(indices) = expand_range_indices(start, step, end) {
                        Expression::ArrayLiteral(
                            indices
                                .into_iter()
                                .map(|n| {
                                    Expression::Variable(crate::string_intern::intern(&format!(
                                        "{}_{}",
                                        name, n
                                    )))
                                })
                                .collect(),
                        )
                    } else {
                        Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                    }
                } else if let Expression::Variable(id) = &new_arr {
                    if let Some(suf) = flat_index_suffix_for_scalar_name(&new_idx) {
                        let name = crate::string_intern::resolve_id(*id);
                        Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, suf)))
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
                let new_base = self.substitute_cached_inner(base, context, visiting, cache);
                if let Some(base_path) = expr_to_path(&new_base) {
                    let full_path = format!("{}.{}", base_path, member);
                    if let Some(val) = self.resolve_global_constant(&full_path) {
                        return val;
                    }
                }
                if new_base == **base {
                    expr.clone()
                } else {
                    Expression::Dot(Box::new(new_base), member.clone())
                }
            }
            Expression::If(cond, t_expr, f_expr) => {
                let nc = self.substitute_cached_inner(cond, context, visiting, cache);
                let nt = self.substitute_cached_inner(t_expr, context, visiting, cache);
                let nf = self.substitute_cached_inner(f_expr, context, visiting, cache);
                if nc == **cond && nt == **t_expr && nf == **f_expr {
                    expr.clone()
                } else {
                    Expression::If(Box::new(nc), Box::new(nt), Box::new(nf))
                }
            }
            Expression::Range(start, step, end) => {
                let ns = self.substitute_cached_inner(start, context, visiting, cache);
                let nst = self.substitute_cached_inner(step, context, visiting, cache);
                let ne = self.substitute_cached_inner(end, context, visiting, cache);
                if ns == **start && nst == **step && ne == **end {
                    expr.clone()
                } else {
                    Expression::Range(Box::new(ns), Box::new(nst), Box::new(ne))
                }
            }
            Expression::ArrayLiteral(exprs) => {
                let mut changed = false;
                let mut out_v = Vec::with_capacity(exprs.len());
                for (i, e) in exprs.iter().enumerate() {
                    let ne = self.substitute_cached_inner(e, context, visiting, cache);
                    if ne != exprs[i] {
                        changed = true;
                    }
                    out_v.push(ne);
                }
                if !changed {
                    expr.clone()
                } else {
                    Expression::ArrayLiteral(out_v)
                }
            }
            Expression::ArrayComprehension {
                expr: inner_expr,
                iter_var,
                iter_range,
            } => {
                let range_sub =
                    self.substitute_cached_inner(iter_range, context, visiting, cache);
                let expr_sub = self.substitute_cached_inner(inner_expr, context, visiting, cache);
                // Keep cached substitution conservative: do not expand comprehensions here.
                // Comprehension expansion is handled by the main substitute paths where a full
                // context stack is available.
                if range_sub == **iter_range && expr_sub == **inner_expr {
                    expr.clone()
                } else {
                    Expression::ArrayComprehension {
                        expr: Box::new(expr_sub),
                        iter_var: iter_var.clone(),
                        iter_range: Box::new(range_sub),
                    }
                }
            }
            Expression::Sample(inner) => {
                let ni = self.substitute_cached_inner(inner, context, visiting, cache);
                if ni == **inner { expr.clone() } else { Expression::Sample(Box::new(ni)) }
            }
            Expression::Interval(inner) => {
                let ni = self.substitute_cached_inner(inner, context, visiting, cache);
                if ni == **inner { expr.clone() } else { Expression::Interval(Box::new(ni)) }
            }
            Expression::Hold(inner) => {
                let ni = self.substitute_cached_inner(inner, context, visiting, cache);
                if ni == **inner { expr.clone() } else { Expression::Hold(Box::new(ni)) }
            }
            Expression::Previous(inner) => {
                let ni = self.substitute_cached_inner(inner, context, visiting, cache);
                if ni == **inner { expr.clone() } else { Expression::Previous(Box::new(ni)) }
            }
            Expression::SubSample(c, n) => Expression::SubSample(
                {
                    let nc = self.substitute_cached_inner(c, context, visiting, cache);
                    if nc == **c { c.clone() } else { Box::new(nc) }
                },
                {
                    let nn = self.substitute_cached_inner(n, context, visiting, cache);
                    if nn == **n { n.clone() } else { Box::new(nn) }
                },
            ),
            Expression::SuperSample(c, n) => Expression::SuperSample(
                {
                    let nc = self.substitute_cached_inner(c, context, visiting, cache);
                    if nc == **c { c.clone() } else { Box::new(nc) }
                },
                {
                    let nn = self.substitute_cached_inner(n, context, visiting, cache);
                    if nn == **n { n.clone() } else { Box::new(nn) }
                },
            ),
            Expression::ShiftSample(c, n) => Expression::ShiftSample(
                {
                    let nc = self.substitute_cached_inner(c, context, visiting, cache);
                    if nc == **c { c.clone() } else { Box::new(nc) }
                },
                {
                    let nn = self.substitute_cached_inner(n, context, visiting, cache);
                    if nn == **n { n.clone() } else { Box::new(nn) }
                },
            ),
            Expression::BackSample(c, n) => Expression::BackSample(
                {
                    let nc = self.substitute_cached_inner(c, context, visiting, cache);
                    if nc == **c { c.clone() } else { Box::new(nc) }
                },
                {
                    let nn = self.substitute_cached_inner(n, context, visiting, cache);
                    if nn == **n { n.clone() } else { Box::new(nn) }
                },
            ),
        };

        if cache.max_size > 0 {
            if cache.cache.len() >= cache.max_size && !cache.order.is_empty() {
                // FIFO eviction (good enough to bound memory; avoids unbounded growth in IDE sessions).
                let old = cache.order.remove(0);
                cache.cache.remove(&old);
            }
            if cache.cache.len() < cache.max_size {
                cache.cache.insert(ptr, out.clone());
                cache.order.push(ptr);
            }
        }
        out
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
                Expression::Variable(id) => {
                    let name = crate::string_intern::resolve_id(*id);
                    if visiting.contains(&name) {
                        return expr.clone();
                    }
                    if let Some(val) = context.get(&name) {
                        if matches!(val, Expression::Variable(inner_id) if *inner_id == *id) {
                            val.clone()
                        } else {
                            visiting.insert(name.clone());
                            let out = inner(fl, val, context, visiting);
                            visiting.remove(&name);
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

                    if let (Expression::Variable(id), Expression::Number(n)) = (&new_arr, &new_idx) {
                        let name = crate::string_intern::resolve_id(*id);
                        let n_int = *n as i64;
                        Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, n_int)))
                    } else if let (Expression::Variable(id), Expression::Range(start, step, end)) =
                        (&new_arr, &new_idx)
                    {
                        let name = crate::string_intern::resolve_id(*id);
                        if let Some(indices) = expand_range_indices(start, step, end) {
                            Expression::ArrayLiteral(
                                indices
                                    .into_iter()
                                    .map(|n| Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, n))))
                                    .collect(),
                            )
                        } else {
                            Expression::ArrayAccess(Box::new(new_arr), Box::new(new_idx))
                        }
                    } else if let Expression::Variable(id) = &new_arr {
                        if let Some(suf) = flat_index_suffix_for_scalar_name(&new_idx) {
                            let name = crate::string_intern::resolve_id(*id);
                            Expression::Variable(crate::string_intern::intern(&format!("{}_{}", name, suf)))
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
                Expression::BackSample(c, n) => Expression::BackSample(
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

    fn substitute_cached_inner_cow<'a>(
        &mut self,
        expr: &'a Expression,
        context: &HashMap<String, Expression>,
        visiting: &mut std::collections::HashSet<String>,
        cache: &mut SubstituteCache,
    ) -> Cow<'a, Expression> {
        let key_ptr = expr as *const Expression;
        if let Some(v) = cache.cache.get(&key_ptr) {
            if v == expr {
                return Cow::Borrowed(expr);
            }
            return Cow::Owned(v.clone());
        }

        use crate::ast::Expression as E;
        let out: Cow<'a, Expression> = match expr {
            E::Variable(id) => {
                let name = crate::string_intern::resolve_id(*id);
                if visiting.contains(&name) {
                    Cow::Borrowed(expr)
                } else if let Some(val) = context.get(&name) {
                    if matches!(val, E::Variable(inner_id) if *inner_id == *id) {
                        Cow::Borrowed(expr)
                    } else {
                        visiting.insert(name.clone());
                        let sub = self.substitute_cached_inner_cow(val, context, visiting, cache);
                        visiting.remove(&name);
                        Cow::Owned(sub.into_owned())
                    }
                } else {
                    // Preserve legacy global constant resolution behavior.
                    let path = if name.contains('.') {
                        name.clone()
                    } else if name.starts_with("Modelica_") {
                        name.replace("Modelica_", "Modelica.")
                            .replace("Constants_", "Constants.")
                    } else {
                        String::new()
                    };
                    if !path.is_empty() {
                        if let Some(val) = self.resolve_global_constant(&path) {
                            Cow::Owned(val)
                        } else {
                            Cow::Borrowed(expr)
                        }
                    } else {
                        Cow::Borrowed(expr)
                    }
                }
            }
            E::Number(_) | E::StringLiteral(_) => Cow::Borrowed(expr),
            E::BinaryOp(lhs, op, rhs) => {
                let nl = self.substitute_cached_inner_cow(lhs, context, visiting, cache);
                let nr = self.substitute_cached_inner_cow(rhs, context, visiting, cache);
                if nl.as_ref() == lhs.as_ref() && nr.as_ref() == rhs.as_ref() {
                    Cow::Borrowed(expr)
                } else {
                    Cow::Owned(E::BinaryOp(
                        Box::new(nl.into_owned()),
                        op.clone(),
                        Box::new(nr.into_owned()),
                    ))
                }
            }
            E::Call(func, args) => {
                let mut changed = false;
                let mut new_args: Vec<Expression> = Vec::with_capacity(args.len());
                for a in args {
                    let na = self.substitute_cached_inner_cow(a, context, visiting, cache);
                    if na.as_ref() != a {
                        changed = true;
                    }
                    new_args.push(na.into_owned());
                }
                if !changed {
                    Cow::Borrowed(expr)
                } else {
                    Cow::Owned(E::Call(func.clone(), new_args))
                }
            }
            E::Der(inner) => {
                let ni = self.substitute_cached_inner_cow(inner, context, visiting, cache);
                if ni.as_ref() == inner.as_ref() {
                    Cow::Borrowed(expr)
                } else {
                    Cow::Owned(E::Der(Box::new(ni.into_owned())))
                }
            }
            E::If(cond, t_expr, f_expr) => {
                let nc = self.substitute_cached_inner_cow(cond, context, visiting, cache);
                let nt = self.substitute_cached_inner_cow(t_expr, context, visiting, cache);
                let nf = self.substitute_cached_inner_cow(f_expr, context, visiting, cache);
                if nc.as_ref() == cond.as_ref()
                    && nt.as_ref() == t_expr.as_ref()
                    && nf.as_ref() == f_expr.as_ref()
                {
                    Cow::Borrowed(expr)
                } else {
                    Cow::Owned(E::If(
                        Box::new(nc.into_owned()),
                        Box::new(nt.into_owned()),
                        Box::new(nf.into_owned()),
                    ))
                }
            }
            E::Range(start, step, end) => {
                let ns = self.substitute_cached_inner_cow(start, context, visiting, cache);
                let nst = self.substitute_cached_inner_cow(step, context, visiting, cache);
                let ne = self.substitute_cached_inner_cow(end, context, visiting, cache);
                if ns.as_ref() == start.as_ref()
                    && nst.as_ref() == step.as_ref()
                    && ne.as_ref() == end.as_ref()
                {
                    Cow::Borrowed(expr)
                } else {
                    Cow::Owned(E::Range(
                        Box::new(ns.into_owned()),
                        Box::new(nst.into_owned()),
                        Box::new(ne.into_owned()),
                    ))
                }
            }
            E::ArrayLiteral(items) => {
                let mut changed = false;
                let mut out_items: Vec<Expression> = Vec::with_capacity(items.len());
                for it in items {
                    let nit = self.substitute_cached_inner_cow(it, context, visiting, cache);
                    if nit.as_ref() != it {
                        changed = true;
                    }
                    out_items.push(nit.into_owned());
                }
                if !changed {
                    Cow::Borrowed(expr)
                } else {
                    Cow::Owned(E::ArrayLiteral(out_items))
                }
            }
            E::Dot(base, member) => {
                let nb = self.substitute_cached_inner_cow(base, context, visiting, cache);
                if let Some(base_path) = expr_to_path(nb.as_ref()) {
                    let full_path = format!("{}.{}", base_path, member);
                    if let Some(val) = self.resolve_global_constant(&full_path) {
                        Cow::Owned(val)
                    } else if nb.as_ref() == base.as_ref() {
                        Cow::Borrowed(expr)
                    } else {
                        Cow::Owned(E::Dot(Box::new(nb.into_owned()), member.clone()))
                    }
                } else if nb.as_ref() == base.as_ref() {
                    Cow::Borrowed(expr)
                } else {
                    Cow::Owned(E::Dot(Box::new(nb.into_owned()), member.clone()))
                }
            }
            E::ArrayAccess(arr, idx) => {
                let new_arr = self.substitute_cached_inner_cow(arr, context, visiting, cache);
                let new_idx = self.substitute_cached_inner_cow(idx, context, visiting, cache);
                let new_arr_ref = new_arr.as_ref();
                let new_idx_ref = new_idx.as_ref();

                // Keep legacy scalarization behavior.
                if let (E::Variable(id), E::Number(n)) = (new_arr_ref, new_idx_ref) {
                    let name = crate::string_intern::resolve_id(*id);
                    let n_int = *n as i64;
                    Cow::Owned(E::Variable(crate::string_intern::intern(&format!("{}_{}", name, n_int))))
                } else if let (E::Variable(id), E::Range(start, step, end)) = (new_arr_ref, new_idx_ref) {
                    let name = crate::string_intern::resolve_id(*id);
                    if let Some(indices) = expand_range_indices(start, step, end) {
                        Cow::Owned(E::ArrayLiteral(
                            indices
                                .into_iter()
                                .map(|n| E::Variable(crate::string_intern::intern(&format!("{}_{}", name, n))))
                                .collect(),
                        ))
                    } else if new_arr_ref == arr.as_ref() && new_idx_ref == idx.as_ref() {
                        Cow::Borrowed(expr)
                    } else {
                        Cow::Owned(E::ArrayAccess(
                            Box::new(new_arr.into_owned()),
                            Box::new(new_idx.into_owned()),
                        ))
                    }
                } else if let E::Variable(id) = new_arr_ref {
                    if let Some(suf) = flat_index_suffix_for_scalar_name(new_idx_ref) {
                        let name = crate::string_intern::resolve_id(*id);
                        Cow::Owned(E::Variable(crate::string_intern::intern(&format!("{}_{}", name, suf))))
                    } else if new_arr_ref == arr.as_ref() && new_idx_ref == idx.as_ref() {
                        Cow::Borrowed(expr)
                    } else {
                        Cow::Owned(E::ArrayAccess(
                            Box::new(new_arr.into_owned()),
                            Box::new(new_idx.into_owned()),
                        ))
                    }
                } else if let (E::ArrayLiteral(elements), E::Number(n)) = (new_arr_ref, new_idx_ref) {
                    let n_usize = *n as usize;
                    let idx0 = if n_usize == 0 && !elements.is_empty() {
                        0
                    } else if n_usize >= 1 && n_usize <= elements.len() {
                        n_usize - 1
                    } else {
                        elements.len()
                    };
                    if idx0 < elements.len() {
                        Cow::Owned(elements[idx0].clone())
                    } else {
                        Cow::Owned(E::Number(0.0))
                    }
                } else if let (E::ArrayLiteral(elements), E::Range(start, step, end)) = (new_arr_ref, new_idx_ref) {
                    if let Some(indices) = expand_range_indices(start, step, end) {
                        let mut out = Vec::new();
                        for n in indices {
                            let n_usize = n as usize;
                            if n_usize >= 1 && n_usize <= elements.len() {
                                out.push(elements[n_usize - 1].clone());
                            }
                        }
                        Cow::Owned(E::ArrayLiteral(out))
                    } else if new_arr_ref == arr.as_ref() && new_idx_ref == idx.as_ref() {
                        Cow::Borrowed(expr)
                    } else {
                        Cow::Owned(E::ArrayAccess(
                            Box::new(new_arr.into_owned()),
                            Box::new(new_idx.into_owned()),
                        ))
                    }
                } else if new_arr_ref == arr.as_ref() && new_idx_ref == idx.as_ref() {
                    Cow::Borrowed(expr)
                } else {
                    Cow::Owned(E::ArrayAccess(
                        Box::new(new_arr.into_owned()),
                        Box::new(new_idx.into_owned()),
                    ))
                }
            }
            _ => Cow::Owned(self.substitute_stack_inner(expr, &[context.clone()], visiting)),
        };

        // Only cache owned results. Borrowed results are identical to input and do not need caching.
        if let Cow::Owned(ref owned) = out {
            if cache.cache.len() >= cache.max_size && !cache.cache.contains_key(&key_ptr) {
                if let Some(old) = cache.order.first().copied() {
                    cache.cache.remove(&old);
                }
                if !cache.order.is_empty() {
                    cache.order.remove(0);
                }
            }
            cache.order.push(key_ptr);
            cache.cache.insert(key_ptr, owned.clone());
        }
        out
    }

    pub(crate) fn substitute_cached_cow<'a>(
        &mut self,
        expr: &'a Expression,
        context: &HashMap<String, Expression>,
        cache: &mut SubstituteCache,
    ) -> Cow<'a, Expression> {
        let mut visiting = std::collections::HashSet::new();
        self.substitute_cached_inner_cow(expr, context, &mut visiting, cache)
    }

    #[allow(dead_code)]
    pub(crate) fn substitute_cached(
        &mut self,
        expr: &Expression,
        context: &HashMap<String, Expression>,
        cache: &mut SubstituteCache,
    ) -> Expression {
        // Hot cache across calls in long-lived IDE processes.
        // Key is derived from expression + context semantic content (not pointers).
        let key = ((hash_expr_bincode(expr) as u128) << 64) | (hash_context(context) as u128);
        if let Ok(g) = global_subst_cache().read() {
            if let Some(v) = g.get(&key) {
                return v.clone();
            }
        }
        let mut visiting = std::collections::HashSet::new();
        let out = self.substitute_cached_inner(expr, context, &mut visiting, cache);
        if let Ok(mut g) = global_subst_cache().write() {
            const MAX: usize = 4096;
            if g.len() >= MAX && !g.contains_key(&key) {
                if let Some(k) = g.keys().next().cloned() {
                    g.remove(&k);
                }
            }
            g.insert(key, out.clone());
        }
        out
    }
}
