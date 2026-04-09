impl crate::flatten::Flattener {
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
                    if let Some(indices) = cache::expand_range_indices(start, step, end) {
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
                    if let Some(indices) = cache::expand_range_indices(start, step, end) {
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
                    if let Some(indices) = cache::expand_range_indices(start, step, end) {
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
                    if let Some(indices) = cache::expand_range_indices(start, step, end) {
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
                        if let Some(indices) = cache::expand_range_indices(start, step, end) {
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
                        if let Some(indices) = cache::expand_range_indices(start, step, end) {
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
}
