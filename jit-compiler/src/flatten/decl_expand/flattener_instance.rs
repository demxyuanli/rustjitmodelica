impl crate::flatten::Flattener {
    /// Inheritance cache, modifiers, then enqueue child Process / equations.
    /// Logic is unchanged from the previous inline tail of `expand_declarations_with_mode`.
    fn finish_composite_instance(
        &mut self,
        stack: &mut Vec<ExpandDeclTask>,
        flat: &mut crate::flatten::FlattenedModel,
        model: &Model,
        decl: &Declaration,
        mut sub_model: Arc<Model>,
        resolved_type: String,
        full_path: String,
        current_qualified: &str,
        msl_import_context: &str,
        each_start: &Option<Expression>,
        context: &HashMap<String, Expression>,
        local_array_sizes: &HashMap<String, usize>,
        is_array: bool,
        i: usize,
        resolved_state_select: Option<crate::ast::StateSelect>,
        mode: ExpandDeclMode,
        expanded_types: &mut HashSet<String>,
    ) -> Result<(), FlattenError> {
        if let Some(cached) = self.inheritance_flat_template_cache.get(resolved_type.as_str()) {
            crate::query_db::perf_record_add("inherit_flat_template_cache_hit", 1);
            sub_model = Arc::clone(cached);
        } else {
            crate::query_db::perf_record_add("inherit_flat_template_cache_miss", 1);
            let decl_expand_flatten_inheritance_t0 = std::time::Instant::now();
            self.flatten_inheritance(&mut sub_model, &resolved_type)?;
            crate::query_db::perf_record_us(
                "decl_expand_flatten_inheritance_us",
                decl_expand_flatten_inheritance_t0.elapsed().as_micros() as u64,
            );
            self.inheritance_flat_template_cache
                .insert(resolved_type.clone(), Arc::clone(&sub_model));
        }
        let mod_ctx = ModifyContext::for_declaration_expand(
            current_qualified,
            msl_import_context,
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

        // type Orientation extends QuaternionBase (= Real[4]): after inheritance.
        if let Some((prim, arr_n)) = effective_primitive_alias(sub_model.as_ref()) {
            let start_value =
                self.elem_start_value(each_start, &decl.start_value, context, is_array, i);
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
            return Ok(());
        }

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
                    let subbed = self.substitute(v, context);
                    let folded = match eval_const_expr_with_param_exprs(
                        &subbed,
                        context,
                        local_array_sizes,
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

        flat.instances
            .insert(full_path.clone(), resolved_type.clone());
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
        // Replaceable components are exempt: their partial default type is a
        // legal interface placeholder until redeclared.
        if sub_model.is_partial && !decl.replaceable {
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
            self.inner_declarations
                .insert(format!("__inner_type_{}", raw_type), full_path.clone());
        }

        if mode == ExpandDeclMode::DeclAndSubEq {
            stack.push(ExpandDeclTask::ExpandEquations {
                model: Arc::clone(&sub_model),
                prefix: full_path.clone(),
            });
        }
        // Default: expand each type once (avoids FullRobot codegen blow-up).
        // Always re-Process Body/Joint/Frame/Orientation instances so
        // StateSelect and DOF declarations exist per instance.
        let expand_instance = type_must_expand_every_instance(&resolved_type)
            || expanded_types.insert(resolved_type.clone());
        if expand_instance {
            stack.push(ExpandDeclTask::Process {
                model: sub_model,
                prefix: full_path,
                current_model_name: Some(resolved_type),
                msl_import_context: msl_import_context.to_string(),
            });
        }
        Ok(())
    }
}
