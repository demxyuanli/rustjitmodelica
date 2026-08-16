impl crate::flatten::Flattener {
    pub(super) fn expand_declarations(
        &mut self,
        model: Arc<Model>,
        prefix: &str,
        flat: &mut crate::flatten::FlattenedModel,
        current_model_name: Option<&str>,
    ) -> Result<(), FlattenError> {
        self.expand_declarations_with_mode(
            model,
            prefix,
            flat,
            current_model_name,
            ExpandDeclMode::DeclAndSubEq,
        )
    }

    pub(super) fn expand_declarations_with_mode(
        &mut self,
        model: Arc<Model>,
        prefix: &str,
        flat: &mut crate::flatten::FlattenedModel,
        current_model_name: Option<&str>,
        mode: ExpandDeclMode,
    ) -> Result<(), FlattenError> {
        let msl_ctx = current_model_name.unwrap_or("").to_string();
        let mut stack: Vec<ExpandDeclTask> = vec![ExpandDeclTask::Process {
            model,
            prefix: prefix.to_string(),
            current_model_name: current_model_name.map(|s| s.to_string()),
            msl_import_context: msl_ctx,
        }];
        let mut array_size_warned_names: HashSet<String> = HashSet::new();
        let mut expanded_types: HashSet<String> = HashSet::new();

        while let Some(task) = stack.pop() {
            match task {
                ExpandDeclTask::ExpandEquations { model, prefix } => {
                    self.expand_equations(model.as_ref(), &prefix, flat);
                    self.expand_algorithms(model.as_ref(), &prefix, flat);
                    self.expand_initial_equations(model.as_ref(), &prefix, flat);
                    self.expand_initial_algorithms(model.as_ref(), &prefix, flat);
                }
                ExpandDeclTask::Process {
                    model,
                    prefix,
                    current_model_name,
                    msl_import_context,
                } => {
                    crate::query_db::perf_record_add("decl_expand_process_tasks", 1);
                    let decl_parallel_enabled = env::flatten_decl_parallel_enabled();
                    let decl_parallel_min_items = env::flatten_decl_parallel_min_items();
                    let current_qualified = current_model_name.as_deref().unwrap_or("");
                    let mut context: HashMap<String, Expression> = HashMap::new();
                    let mut local_array_sizes: HashMap<String, usize> = HashMap::new();
                    for decl in &model.declarations {
                        if decl.is_parameter {
                            if let Some(val) = &decl.start_value {
                                context.insert(decl.name.clone(), val.clone());
                            }
                        }
                    }

                    let decl_expand_param_pass_t0 = std::time::Instant::now();
                    // Parameter propagation is a hot path in validate mode; use a fast optimizer
                    // first, then fall back to the legacy fixed-point loop if not converged.
                    let mut param_opt = param_pass::ParamPassOptimizer::default();
                    let fast_passes =
                        param_opt.optimize_param_passes(self, &model, &mut context, &local_array_sizes);
                    let perf = env::perf_trace_enabled();
                    if perf && fast_passes >= 16 {
                        eprintln!(
                            "[perf] param_passes_fast={} total_params={} stable_params={}",
                            fast_passes,
                            param_opt.param_deps.len(),
                            param_opt.stable_params.len()
                        );
                    }
                    const MAX_PARAM_PASSES_TOTAL: usize = 128;
                    if self.validation_mode == crate::flatten::ValidationMode::Full && fast_passes >= 32 {
                        for _ in fast_passes..MAX_PARAM_PASSES_TOTAL {
                            let mut changed = false;
                            for decl in &model.declarations {
                                if decl.is_parameter {
                                    if let Some(val) = &decl.start_value {
                                        let sub = self.substitute(val, &context);
                                        if let Some(n) = eval_const_expr_with_param_exprs(
                                            &sub,
                                            &context,
                                            &local_array_sizes,
                                        ) {
                                            let update = match context.get(&decl.name) {
                                                None => true,
                                                Some(Expression::Number(p)) => (n - p).abs() > 1e-12,
                                                Some(Expression::ArrayLiteral(items)) if items.len() != 1 => false,
                                                Some(_) => true,
                                            };
                                            if update {
                                                context.insert(decl.name.clone(), Expression::Number(n));
                                                changed = true;
                                            }
                                        }
                                    }
                                }
                            }
                            if !changed {
                                break;
                            }
                        }
                    }
                    crate::query_db::perf_record_us(
                        "decl_expand_param_pass_us",
                        decl_expand_param_pass_t0.elapsed().as_micros() as u64,
                    );

                    let decl_expand_array_dim_t0 = std::time::Instant::now();
                    // Array dimension inference is another validate hot path; use a fast optimizer
                    // then fall back to the legacy fixed-point loop if needed.
                    let mut arr_opt = array_dim::ArrayDimensionOptimizer::default();
                    let dim_fast_passes =
                        arr_opt.optimize_array_dims(self, &model, &context, &mut local_array_sizes);
                    if env::perf_trace_enabled() && dim_fast_passes >= 8 {
                        eprintln!(
                            "[perf] array_dim_passes_fast={} computed={} uncalculable={}",
                            dim_fast_passes,
                            arr_opt.computed_dims.len(),
                            arr_opt.uncalculable.len()
                        );
                    }
                    const MAX_ARRAY_DIM_PASSES_TOTAL: usize = 64;
                    if self.validation_mode == crate::flatten::ValidationMode::Full && dim_fast_passes >= 16 {
                        for _ in dim_fast_passes..MAX_ARRAY_DIM_PASSES_TOTAL {
                            let mut dim_changed = false;
                            for decl in &model.declarations {
                                if let Some(ref cond_expr) = decl.condition {
                                    let cond_sub = self.substitute(cond_expr, &context);
                                    if let Some(v) = eval_const_expr_with_param_exprs(
                                        &cond_sub,
                                        &context,
                                        &local_array_sizes,
                                    ) {
                                        if v == 0.0 {
                                            continue;
                                        }
                                    }
                                }
                                let Some(size_expr) = decl.array_size.as_ref() else {
                                    continue;
                                };
                                if local_array_sizes.contains_key(&decl.name) {
                                    continue;
                                }
                                let sub_expr = self.substitute(size_expr, &context);
                                if let Some(val) = eval_const_expr_with_param_exprs(
                                    &sub_expr,
                                    &context,
                                    &local_array_sizes,
                                ) {
                                    let n = val as usize;
                                    if n > 0 {
                                        local_array_sizes.insert(decl.name.clone(), n);
                                        dim_changed = true;
                                    }
                                }
                            }
                            if !dim_changed {
                                break;
                            }
                        }
                    }
                    crate::query_db::perf_record_us(
                        "decl_expand_array_dim_us",
                        decl_expand_array_dim_t0.elapsed().as_micros() as u64,
                    );

                    let decl_expand_decl_loop_t0 = std::time::Instant::now();
                    for decl in &model.declarations {
                        if let Some(ref cond_expr) = decl.condition {
                            let cond_sub = self.substitute(cond_expr, &context);
                            if let Some(v) =
                                eval_const_expr_with_param_exprs(&cond_sub, &context, &local_array_sizes)
                            {
                                if v == 0.0 {
                                    continue;
                                }
                            }
                        }

                        let base_name = if prefix.is_empty() {
                            decl.name.clone()
                        } else {
                            format!("{}_{}", prefix, decl.name)
                        };

                        let array_len = if let Some(size_expr) = &decl.array_size {
                            let sub_expr = self.substitute(size_expr, &context);
                            if let Some(val) = eval_const_expr_with_param_exprs(
                                &sub_expr,
                                &context,
                                &local_array_sizes,
                            ) {
                                Some(val as usize)
                            } else if let Some(&n) = self.external_array_sizes.get(&base_name) {
                                Some(n)
                            } else {
                                let fail_flatten = matches!(
                                    self.array_size_policy,
                                    crate::flatten::ArraySizePolicy::Strict
                                ) || self.warnings_level == "error";
                                if fail_flatten {
                                    return Err(FlattenError::UnevaluatedArraySize {
                                        flat_base_name: base_name.clone(),
                                    });
                                }
                                if self.warnings_level != "none"
                                    && array_size_warned_names.insert(decl.name.clone())
                                {
                                    eprintln!(
                                        "{}",
                                        crate::i18n::msg(
                                            "warning_array_size",
                                            &[&base_name as &dyn std::fmt::Display],
                                        )
                                    );
                                }
                                None
                            }
                        } else {
                            None
                        };

                        let count = array_len.unwrap_or(1);
                        let is_array = array_len.is_some();

                        let mut each_start: Option<Expression> = None;
                        let mut each_state_select: Option<Expression> = None;
                        for m in &decl.modifications {
                            if m.each && m.name == "start" {
                                each_start = m.value.clone();
                            }
                            if m.each && m.name == "stateSelect" {
                                each_state_select = m.value.clone();
                            }
                        }
                        let resolved_state_select: Option<crate::ast::StateSelect> = {
                            let raw = each_state_select
                                .as_ref()
                                .or_else(|| {
                                    crate::state_select::state_select_mod_value(&decl.modifications)
                                });
                            raw.map(|e| self.substitute(e, &context))
                                .and_then(|e| crate::state_select::parse_state_select_expr(&e))
                                .or(decl.state_select)
                        };

                        if is_array {
                            flat.array_sizes.insert(base_name.clone(), count);
                            local_array_sizes.insert(decl.name.clone(), count);
                            if !decl.is_parameter || decl.start_value.is_none() {
                                context.insert(
                                    decl.name.clone(),
                                    Expression::ArrayLiteral(vec![Expression::Number(0.0); count]),
                                );
                            }
                        }

                        for i in 1..=count {
                            let name_suffix = if is_array { format!("_{}", i) } else { "".to_string() };
                            let local_name = format!("{}{}", decl.name, name_suffix);
                            let full_path = if prefix.is_empty() {
                                local_name.clone()
                            } else {
                                format!("{}_{}", prefix, local_name)
                            };

                            let loc = current_model_name
                                .as_deref()
                                .and_then(|n| self.loader.get_path_for_model(n))
                                .map(|p| SourceLocation {
                                    file: p.display().to_string(),
                                    line: 0,
                                    column: 0,
                                });

                            let mut resolved_type = resolve_type_alias(&model.type_aliases, &decl.type_name);
                            let pre_inner_alias = resolved_type.clone();
                            // Outer reference: search parent scope for matching inner declaration.
                            // Modelica semantics: every `outer X x` must find an `inner X x` in an
                            // enclosing scope. The inner instance owns the actual storage; the outer
                            // declaration references it by path.
                            let outer_inner_path: Option<String> = if decl.is_outer {
                                let key = format!("__inner_{}_{}", resolved_type, decl.name);
                                self.inner_declarations
                                    .get(&key)
                                    .cloned()
                                    .or_else(|| {
                                        // Modelica allows different names: search by type only as fallback.
                                        let type_key = format!("__inner_type_{}", resolved_type);
                                        self.inner_declarations.get(&type_key).cloned()
                                    })
                            } else {
                                None
                            };
                            if decl.is_outer && outer_inner_path.is_none() {
                                // outer without matching inner: Modelica spec requires this to be an error.
                                // For now, emit a warning and treat as regular instance (graceful degradation).
                                eprintln!(
                                    "[flatten] Warning: outer declaration '{}.{}' has no matching inner in parent scope",
                                    current_qualified, decl.name,
                                );
                            }
                            resolved_type = resolve_inner_class_alias(&model, &resolved_type);
                            resolved_type = Self::resolve_import_scoped_type(
                                model.as_ref(),
                                &resolved_type,
                                current_qualified,
                                &msl_import_context,
                            );
                            resolved_type =
                                Self::normalize_decl_type_name(resolved_type, &pre_inner_alias);

                            // Outer with matching inner: register as alias, skip instance creation.
                            if let Some(ref inner_path) = outer_inner_path {
                                flat.instances.insert(full_path.clone(), resolved_type.clone());
                                flat.register_inst_path(
                                    full_path.clone(),
                                    resolved_type.clone(),
                                    if is_array { Some(i as usize) } else { None },
                                );
                                // Copy equations/algorithms from inner scope if present
                                // (the inner instance handles the actual storage).
                                continue;
                            }

                            if i == 1
                                && is_array
                                && count >= decl_parallel_min_items
                                && decl_parallel_enabled
                                && is_primitive(&resolved_type)
                            {
                                crate::query_db::perf_record_add("flatten_parallel_poc_enabled", 1);
                                let start_template: Option<Expression> = if let Some(ev) = &each_start {
                                    Some(self.substitute(ev, &context))
                                } else if let Some(val) = &decl.start_value {
                                    Some(self.substitute(val, &context))
                                } else {
                                    None
                                };
                                let resolved_type_clone = resolved_type.clone();
                                let state_select_template = resolved_state_select;
                                let entries: Vec<(Declaration, String, usize)> = (1..=count)
                                    .into_par_iter()
                                    .map(|idx| {
                                        let idx_suffix = format!("_{}", idx);
                                        let idx_local_name = format!("{}{}", decl.name, idx_suffix);
                                        let idx_full_path = if prefix.is_empty() {
                                            idx_local_name
                                        } else {
                                            format!("{}_{}", prefix, idx_local_name)
                                        };
                                        let start_value = start_template
                                            .as_ref()
                                            .map(|sub| index_expression(sub, idx));
                                        let d = Declaration {
                                            type_name: resolved_type_clone.clone(),
                                            name: idx_full_path.clone(),
                                            replaceable: decl.replaceable,
                                            constrainedby_type: decl.constrainedby_type.clone(),
                                            is_parameter: decl.is_parameter,
                                            is_flow: decl.is_flow,
                                            is_stream: decl.is_stream,
                                            is_discrete: decl.is_discrete,
                                            is_input: decl.is_input,
                                            is_output: decl.is_output,
                                            is_inner: decl.is_inner,
                                            is_outer: decl.is_outer,
                                            is_public: decl.is_public,
                                            is_protected: decl.is_protected,
                                            start_value,
                                            array_size: None,
                                            modifications: Vec::new(),
                                            is_rest: decl.is_rest,
                                            annotation: None,
                                            condition: None,
                                            state_select: state_select_template,
                                        };
                                        (d, idx_full_path, idx)
                                    })
                                    .collect();
                                for (d, idx_full_path, idx) in entries {
                                    flat.declarations.push(d);
                                    flat.register_inst_path(
                                        idx_full_path,
                                        resolved_type.clone(),
                                        Some(idx),
                                    );
                                }
                                break;
                            }

                            // SuperFast mode: skip recursive sub-model loading for non-primitive types
                            if mode == ExpandDeclMode::SuperFast && !is_primitive(&resolved_type) {
                                // Just register the instance without loading the sub-model
                                flat.register_inst_path(
                                    full_path.clone(),
                                    resolved_type.clone(),
                                    if is_array { Some(i as usize) } else { None },
                                );
                                flat.instances.insert(full_path.clone(), resolved_type.clone());
                                continue;
                            }

                            if let Some((prim, arr_n @ Some(_))) =
                                split_primitive_array_type(&resolved_type)
                            {
                                let start_value = self.elem_start_value(
                                    &each_start,
                                    &decl.start_value,
                                    &context,
                                    is_array,
                                    i,
                                );
                                push_primitive_alias_components(
                                    flat,
                                    &full_path,
                                    &prim,
                                    arr_n,
                                    decl,
                                    start_value,
                                    resolved_state_select,
                                    if is_array { Some(i as usize) } else { None },
                                );
                                continue;
                            }

                            if is_primitive(&resolved_type) {
                                let start_value = initial_step_start(
                                    current_model_name.as_deref(),
                                    &decl.name,
                                    self.elem_start_value(
                                        &each_start,
                                        &decl.start_value,
                                        &context,
                                        is_array,
                                        i,
                                    ),
                                );
                                push_primitive_leaf(
                                    flat,
                                    &full_path,
                                    &prefix,
                                    &resolved_type,
                                    decl,
                                    start_value,
                                    resolved_state_select,
                                    if is_array { Some(i as usize) } else { None },
                                );
                                continue;
                            }

                            let Some((resolved_type, sub_model)) = self.load_or_recover_sub_model(
                                model.as_ref(),
                                decl,
                                resolved_type,
                                &full_path,
                                current_qualified,
                                &msl_import_context,
                                loc,
                                &context,
                                is_array,
                                i,
                                resolved_state_select,
                                flat,
                            )?
                            else {
                                continue;
                            };

                            // type X = Real[n] (and name-matching aliases): expand as primitive.
                            if let Some((prim, arr_n)) = effective_primitive_alias(sub_model.as_ref())
                                .or_else(|| {
                                    let resolved =
                                        resolve_type_alias(&sub_model.type_aliases, &sub_model.name);
                                    split_primitive_array_type(&resolved)
                                })
                            {
                                let start_value = self.elem_start_value(
                                    &each_start,
                                    &decl.start_value,
                                    &context,
                                    is_array,
                                    i,
                                );
                                push_primitive_alias_components(
                                    flat,
                                    &full_path,
                                    &prim,
                                    arr_n,
                                    decl,
                                    start_value,
                                    resolved_state_select,
                                    if is_array { Some(i as usize) } else { None },
                                );
                                continue;
                            }

                            self.finish_composite_instance(
                                &mut stack,
                                flat,
                                model.as_ref(),
                                decl,
                                sub_model,
                                resolved_type,
                                full_path,
                                current_qualified,
                                &msl_import_context,
                                &each_start,
                                &context,
                                &local_array_sizes,
                                is_array,
                                i,
                                resolved_state_select,
                                mode,
                                &mut expanded_types,
                            )?;
                        }
                    }
                    crate::query_db::perf_record_us(
                        "decl_expand_decl_loop_us",
                        decl_expand_decl_loop_t0.elapsed().as_micros() as u64,
                    );
                }
            }
        }

        Ok(())
    }

    fn elem_start_value(
        &mut self,
        preferred: &Option<Expression>,
        fallback: &Option<Expression>,
        context: &HashMap<String, Expression>,
        is_array: bool,
        i: usize,
    ) -> Option<Expression> {
        let raw = preferred.as_ref().or(fallback.as_ref())?;
        let sub = self.substitute(raw, context);
        Some(if is_array {
            index_expression(&sub, i)
        } else {
            sub
        })
    }
}
