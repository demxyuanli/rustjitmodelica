impl crate::flatten::Flattener {
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
                    if let Some(indices) = cache::expand_range_indices(start, step, end) {
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
                    if let Some(indices) = cache::expand_range_indices(start, step, end) {
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
        let key = ((cache::hash_expr_bincode(expr) as u128) << 64) | (cache::hash_context(context) as u128);
        if let Ok(g) = cache::global_subst_cache().read() {
            if let Some(v) = g.get(&key) {
                return v.clone();
            }
        }
        let mut visiting = std::collections::HashSet::new();
        let out = self.substitute_cached_inner(expr, context, &mut visiting, cache);
        if let Ok(mut g) = cache::global_subst_cache().write() {
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
