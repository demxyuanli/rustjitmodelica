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
        #[derive(Clone)]
        enum Task {
            Process {
                model: Arc<Model>,
                prefix: String,
                current_model_name: Option<String>,
                msl_import_context: String,
            },
            ExpandEquations {
                model: Arc<Model>,
                prefix: String,
            },
        }

        let msl_ctx = current_model_name.unwrap_or("").to_string();
        let mut stack: Vec<Task> = vec![Task::Process {
            model,
            prefix: prefix.to_string(),
            current_model_name: current_model_name.map(|s| s.to_string()),
            msl_import_context: msl_ctx,
        }];
        let mut array_size_warned_names: HashSet<String> = HashSet::new();
        let mut expanded_types: HashSet<String> = HashSet::new();

        while let Some(task) = stack.pop() {
            match task {
                Task::ExpandEquations { model, prefix } => {
                    self.expand_equations(model.as_ref(), &prefix, flat);
                }
                Task::Process {
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
                        for m in &decl.modifications {
                            if m.each && m.name == "start" {
                                each_start = m.value.clone();
                            }
                        }

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

                            if is_primitive(&resolved_type) {
                                flat.declarations.push(Declaration {
                                    type_name: resolved_type.clone(),
                                    name: full_path.clone(),
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
                                    start_value: if let Some(ev) = &each_start {
                                        let sub = self.substitute(ev, &context);
                                        if is_array {
                                            Some(index_expression(&sub, i))
                                        } else {
                                            Some(sub)
                                        }
                                    } else if let Some(val) = &decl.start_value {
                                        let sub = self.substitute(val, &context);
                                        if is_array { Some(index_expression(&sub, i)) } else { Some(sub) }
                                    } else {
                                        None
                                    },
                                    array_size: None,
                                    modifications: Vec::new(),
                                    is_rest: decl.is_rest,
                                    annotation: None,
                                    condition: None,
                                });
                                flat.register_inst_path(
                                    full_path.clone(),
                                    resolved_type.clone(),
                                    if is_array { Some(i as usize) } else { None },
                                );
                                continue;
                            }

                            // Use lexical scope for candidate qualification.
                            // Some MSL libraries reference siblings via relative prefixes (e.g. `Utilities.*`,
                            // `Analog.*`) and the loader may report `current_qualified` as a relative name.
                            // When that happens, prefer the root import context as the scope anchor so
                            // upward qualification can reach `Modelica.*`.
                            let scope_for_candidates = if current_qualified.is_empty() {
                                msl_import_context.as_str()
                            } else if !msl_import_context.is_empty()
                                && msl_import_context.starts_with("Modelica.")
                                && !current_qualified.starts_with("Modelica.")
                            {
                                msl_import_context.as_str()
                            } else {
                                current_qualified
                            };
                            let load_candidates =
                                Self::build_load_candidates(&resolved_type, scope_for_candidates);
                            if env::perf_trace_enabled()
                                && (resolved_type == "Analog.Interfaces.NegativePin"
                                    || resolved_type.ends_with(".Analog.Interfaces.NegativePin"))
                            {
                                eprintln!(
                                    "[perf] type_load_probe resolved_type={} current={} msl_ctx={} scope={} candidates={:?}",
                                    resolved_type,
                                    current_qualified,
                                    msl_import_context,
                                    scope_for_candidates,
                                    load_candidates
                                );
                            }
                            let decl_expand_try_load_sub_model_t0 = std::time::Instant::now();
                            let (loaded_type, last_err) = self.try_load_sub_model(
                                model.as_ref(),
                                &resolved_type,
                                scope_for_candidates,
                                &load_candidates,
                            );
                            crate::query_db::perf_record_us(
                                "decl_expand_try_load_sub_model_us",
                                decl_expand_try_load_sub_model_t0.elapsed().as_micros() as u64,
                            );

                            let mut sub_model = match loaded_type {
                                Some((resolved_candidate, m)) => {
                                    if m.is_partial {
                                        return Err(FlattenError::PartialModelInstantiated {
                                            partial_type: resolved_candidate.clone(),
                                            instance_path: full_path.clone(),
                                        });
                                    }
                                    resolved_type = resolved_candidate;
                                    m
                                }
                                None => {
                                    let e = last_err.unwrap_or_else(|| LoadError::NotFound(resolved_type.clone()));
                                    if matches!(&e, LoadError::NotFound(_)) {
                                        if let Some((prefix_type, suffix_type)) = resolved_type.rsplit_once('.') {
                                            if let Ok(owner) = self.loader.load_model(prefix_type) {
                                                if let Some((_, base)) =
                                                    owner.type_aliases.iter().find(|(a, _)| a == suffix_type)
                                                {
                                                    let base = resolve_type_alias(&owner.type_aliases, base);
                                                    if is_primitive(&base) {
                                                        flat.declarations.push(Declaration {
                                                            type_name: base.clone(),
                                                            name: full_path.clone(),
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
                                                            start_value: if let Some(val) = &decl.start_value {
                                                                let sub = self.substitute(val, &context);
                                                                if is_array {
                                                                    Some(index_expression(&sub, i))
                                                                } else {
                                                                    Some(sub)
                                                                }
                                                            } else {
                                                                None
                                                            },
                                                            array_size: None,
                                                            modifications: Vec::new(),
                                                            is_rest: decl.is_rest,
                                                            annotation: None,
                                                            condition: None,
                                                        });
                                                        flat.register_inst_path(
                                                            full_path.clone(),
                                                            base,
                                                            if is_array { Some(i as usize) } else { None },
                                                        );
                                                        continue;
                                                    }
                                                }
                                            }
                                        }
                                        if !resolved_type.contains('.') {
                                            for (_alias, qual) in &model.imports {
                                                if qual.is_empty() {
                                                    continue;
                                                }
                                                let candidate = format!("{}.{}", qual, resolved_type);
                                                if let Some((prefix_type, suffix_type)) = candidate.rsplit_once('.')
                                                {
                                                    if let Ok(owner) = self.loader.load_model(prefix_type) {
                                                        if let Some((_, base)) = owner
                                                            .type_aliases
                                                            .iter()
                                                            .find(|(a, _)| a == suffix_type)
                                                        {
                                                            let base =
                                                                resolve_type_alias(&owner.type_aliases, base);
                                                            if is_primitive(&base) {
                                                                flat.declarations.push(Declaration {
                                                                    type_name: base.clone(),
                                                                    name: full_path.clone(),
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
                                                                    start_value: if let Some(val) = &decl.start_value {
                                                                        let sub = self.substitute(val, &context);
                                                                        if is_array {
                                                                            Some(index_expression(&sub, i))
                                                                        } else {
                                                                            Some(sub)
                                                                        }
                                                                    } else {
                                                                        None
                                                                    },
                                                                    array_size: None,
                                                                    modifications: Vec::new(),
                                                                    is_rest: decl.is_rest,
                                                                    annotation: None,
                                                                    condition: None,
                                                                });
                                                                flat.register_inst_path(
                                                                    full_path.clone(),
                                                                    base,
                                                                    if is_array { Some(i as usize) } else { None },
                                                                );
                                                                continue;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if decl.is_parameter
                                        && (resolved_type.eq_ignore_ascii_case("distribution")
                                            || resolved_type.contains("PartialDistribution")
                                            || resolved_type.contains(".Distributions.Interfaces.")
                                            || resolved_type.ends_with(".Distribution")
                                            || resolved_type == "Types.Dynamics"
                                            || resolved_type.ends_with(".Types.Dynamics"))
                                    {
                                        let fallback_type_name = if resolved_type == "Types.Dynamics"
                                            || resolved_type.ends_with(".Types.Dynamics")
                                        {
                                            "Integer"
                                        } else {
                                            "Real"
                                        };
                                        flat.declarations.push(Declaration {
                                            type_name: fallback_type_name.to_string(),
                                            name: full_path.clone(),
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
                                            start_value: if let Some(val) = &decl.start_value {
                                                Some(self.substitute(val, &context))
                                            } else {
                                                Some(Expression::Number(0.0))
                                            },
                                            array_size: None,
                                            modifications: Vec::new(),
                                            is_rest: decl.is_rest,
                                            annotation: None,
                                            condition: None,
                                        });
                                        flat.register_inst_path(
                                            full_path.clone(),
                                            fallback_type_name.to_string(),
                                            if is_array { Some(i as usize) } else { None },
                                        );
                                        continue;
                                    }
                                    if resolved_type == "Types.Dynamics"
                                        || resolved_type.ends_with(".Types.Dynamics")
                                    {
                                        flat.declarations.push(Declaration {
                                            type_name: "Integer".to_string(),
                                            name: full_path.clone(),
                                            replaceable: decl.replaceable,
                                            constrainedby_type: decl.constrainedby_type.clone(),
                                            is_parameter: decl.is_parameter,
                                            is_flow: decl.is_flow,
                                            is_stream: decl.is_stream,
                                            is_discrete: true,
                                            is_input: decl.is_input,
                                            is_output: decl.is_output,
                                            is_inner: decl.is_inner,
                                            is_outer: decl.is_outer,
                                            is_public: decl.is_public,
                                            is_protected: decl.is_protected,
                                            start_value: Some(Expression::Number(0.0)),
                                            array_size: None,
                                            modifications: Vec::new(),
                                            is_rest: decl.is_rest,
                                            annotation: None,
                                            condition: None,
                                        });
                                        flat.register_inst_path(
                                            full_path.clone(),
                                            "Integer".to_string(),
                                            if is_array { Some(i as usize) } else { None },
                                        );
                                        continue;
                                    }
                                    return match e {
                                        LoadError::NotFound(_) => Err(FlattenError::UnknownType(
                                            resolved_type.clone(),
                                            full_path.clone(),
                                            loc,
                                        )),
                                        _ => Err(FlattenError::Load(e)),
                                    };
                                }
                            };

                            if let Some((_, base)) = sub_model
                                .type_aliases
                                .iter()
                                .find(|(a, _)| a == &sub_model.name)
                            {
                                let base = resolve_type_alias(&sub_model.type_aliases, base);
                                if is_primitive(&base) {
                                    flat.declarations.push(Declaration {
                                        type_name: base.clone(),
                                        name: full_path.clone(),
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
                                        start_value: if let Some(val) = &decl.start_value {
                                            let sub = self.substitute(val, &context);
                                            if is_array {
                                                Some(index_expression(&sub, i))
                                            } else {
                                                Some(sub)
                                            }
                                        } else {
                                            None
                                        },
                                        array_size: None,
                                        modifications: Vec::new(),
                                        is_rest: decl.is_rest,
                                        annotation: None,
                                        condition: None,
                                    });
                                    flat.register_inst_path(
                                        full_path.clone(),
                                        base.clone(),
                                        if is_array { Some(i as usize) } else { None },
                                    );
                                    continue;
                                }
                            }

                            if let Some(cached) =
                                self.inheritance_flat_template_cache.get(resolved_type.as_str())
                            {
                                crate::query_db::perf_record_add(
                                    "inherit_flat_template_cache_hit",
                                    1,
                                );
                                sub_model = Arc::clone(cached);
                            } else {
                                crate::query_db::perf_record_add(
                                    "inherit_flat_template_cache_miss",
                                    1,
                                );
                                let decl_expand_flatten_inheritance_t0 = std::time::Instant::now();
                                self.flatten_inheritance(&mut sub_model, &resolved_type)?;
                                crate::query_db::perf_record_us(
                                    "decl_expand_flatten_inheritance_us",
                                    decl_expand_flatten_inheritance_t0.elapsed().as_micros() as u64,
                                );
                                self.inheritance_flat_template_cache.insert(
                                    resolved_type.clone(),
                                    Arc::clone(&sub_model),
                                );
                            }
                            let mod_ctx = ModifyContext::for_declaration_expand(
                                current_qualified,
                                &msl_import_context,
                                self.coarse_constrainedby_only,
                                self.validation_mode,
                                self.compile_stop_label.as_str(),
                            );
                            let decl_expand_apply_modification_t0 = std::time::Instant::now();
                            for modification in &decl.modifications {
                                apply_modification_to_model(
                                    Arc::make_mut(&mut sub_model),
                                    modification,
                                    &mod_ctx,
                                    Some(&mut self.loader),
                                )?;
                            }
                            crate::query_db::perf_record_us(
                                "decl_expand_apply_modification_us",
                                decl_expand_apply_modification_t0.elapsed().as_micros() as u64,
                            );

                            // Bindings like `PlugToPins_n plugToPins_n(final m=m)` store the RHS in the
                            // child parameter `start_value` as a bare `m` that refers to the enclosing
                            // component. Substitute in the parent's `context` so nested array sizes
                            // (e.g. polyphase `pin[m]`) can be evaluated before the child Process task.
                            // When the substituted expression is const-foldable (e.g. `mSystems` from
                            // `numberOfSymmetricBaseSystems(m)`), replace with `Expression::Number`.
                            let decl_expand_param_substitute_fold_t0 = std::time::Instant::now();
                            for d in Arc::make_mut(&mut sub_model).declarations.iter_mut() {
                                if d.is_parameter {
                                    if let Some(v) = &d.start_value {
                                        let subbed = self.substitute(v, &context);
                                        let folded =
                                            match eval_const_expr_with_param_exprs(
                                                &subbed,
                                                &context,
                                                &local_array_sizes,
                                            ) {
                                                Some(n) if n.is_finite() => Expression::Number(n),
                                                _ => subbed,
                                            };
                                        d.start_value = Some(folded);
                                    }
                                }
                            }
                            crate::query_db::perf_record_us(
                                "decl_expand_param_substitute_fold_us",
                                decl_expand_param_substitute_fold_t0.elapsed().as_micros() as u64,
                            );

                            flat.instances.insert(full_path.clone(), resolved_type.clone());
                            flat.register_inst_path(
                                full_path.clone(),
                                resolved_type.clone(),
                                if is_array { Some(i as usize) } else { None },
                            );

                            // Mark expandable connector instances for dynamic member injection.
                            if sub_model.is_expandable {
                                flat.expandable_instances.insert(full_path.clone());
                            }

                            // Reject instantiation of partial models (MLS 4.4.2).
                            if sub_model.is_partial {
                                return Err(FlattenError::PartialModelInstantiated {
                                    partial_type: resolved_type.clone(),
                                    instance_path: full_path.clone(),
                                });
                            }

                            // Register inner declarations so outer references can resolve.
                            if decl.is_inner {
                                let raw_type = resolve_type_alias(&model.type_aliases, &decl.type_name);
                                self.inner_declarations.insert(
                                    format!("__inner_{}_{}", raw_type, decl.name),
                                    full_path.clone(),
                                );
                                self.inner_declarations.insert(
                                    format!("__inner_type_{}", raw_type),
                                    full_path.clone(),
                                );
                            }

                            if mode == ExpandDeclMode::DeclAndSubEq {
                                stack.push(Task::ExpandEquations {
                                    model: Arc::clone(&sub_model),
                                    prefix: full_path.clone(),
                                });
                            }
                            // Only expand each distinct type once per flatten pass.
                            // Shared base classes (e.g. SISO for PID + TransferFunction)
                            // are already flattened; re-expanding them on every instance
                            // causes combinatorial task explosion.
                            if expanded_types.insert(resolved_type.clone()) {
                                stack.push(Task::Process {
                                    model: sub_model,
                                    prefix: full_path,
                                    current_model_name: Some(resolved_type),
                                    msl_import_context: msl_import_context.clone(),
                                });
                            }
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
}
