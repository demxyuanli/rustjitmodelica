fn scope_for_type_candidates<'a>(current_qualified: &'a str, msl_import_context: &'a str) -> &'a str {
    if current_qualified.is_empty() {
        msl_import_context
    } else if !msl_import_context.is_empty()
        && msl_import_context.starts_with("Modelica.")
        && !current_qualified.starts_with("Modelica.")
    {
        msl_import_context
    } else {
        current_qualified
    }
}

impl crate::flatten::Flattener {
    /// Load a composite type, or recover a primitive alias / Dynamics fallback.
    /// `Ok(None)` means a leaf was already pushed and the caller should `continue`.
    fn load_or_recover_sub_model(
        &mut self,
        model: &Model,
        decl: &Declaration,
        resolved_type: String,
        full_path: &str,
        current_qualified: &str,
        msl_import_context: &str,
        loc: Option<SourceLocation>,
        context: &HashMap<String, Expression>,
        is_array: bool,
        i: usize,
        resolved_state_select: Option<crate::ast::StateSelect>,
        flat: &mut crate::flatten::FlattenedModel,
    ) -> Result<Option<(String, Arc<Model>)>, FlattenError> {
        // Use lexical scope for candidate qualification.
        // Some MSL libraries reference siblings via relative prefixes (e.g. `Utilities.*`,
        // `Analog.*`) and the loader may report `current_qualified` as a relative name.
        // When that happens, prefer the root import context as the scope anchor so
        // upward qualification can reach `Modelica.*`.
        let scope_for_candidates = scope_for_type_candidates(current_qualified, msl_import_context);
        let load_candidates = Self::build_load_candidates(&resolved_type, scope_for_candidates);
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
            model,
            &resolved_type,
            scope_for_candidates,
            &load_candidates,
        );
        crate::query_db::perf_record_us(
            "decl_expand_try_load_sub_model_us",
            decl_expand_try_load_sub_model_t0.elapsed().as_micros() as u64,
        );

        match loaded_type {
            Some((resolved_candidate, m)) => {
                // Replaceable components may declare a partial default type;
                // they are legal as long as usage stays within the declared
                // interface (redeclare supplies the concrete type).
                if m.is_partial && !decl.replaceable {
                    return Err(FlattenError::PartialModelInstantiated {
                        partial_type: resolved_candidate.clone(),
                        instance_path: full_path.to_string(),
                    });
                }
                Ok(Some((resolved_candidate, m)))
            }
            None => {
                let e = last_err.unwrap_or_else(|| LoadError::NotFound(resolved_type.clone()));
                if matches!(&e, LoadError::NotFound(_)) {
                    // Try every scope candidate, not just the literal prefix:
                    // `Types.PortFlowDirection` inside Fluid must find the
                    // alias on owner `Modelica.Fluid.Types` even though the
                    // bare prefix `Types` is not loadable on its own.
                    let mut alias_base: Option<String> = None;
                    for cand in load_candidates.iter() {
                        let Some((prefix_type, suffix_type)) = cand.rsplit_once('.') else {
                            continue;
                        };
                        if let Ok(owner) = self.loader.load_model_silent(prefix_type, true) {
                            if let Some((_, base)) =
                                owner.type_aliases.iter().find(|(a, _)| a == suffix_type)
                            {
                                let base = resolve_type_alias(&owner.type_aliases, base);
                                if is_primitive(&base) {
                                    alias_base = Some(base);
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(base) = alias_base {
                        let start_value =
                            self.elem_start_value(&None, &decl.start_value, context, is_array, i);
                        if let Some((prim, arr_n @ Some(_))) = split_primitive_array_type(&base) {
                            push_primitive_alias_components(
                                flat,
                                full_path,
                                &prim,
                                arr_n,
                                decl,
                                start_value,
                                resolved_state_select,
                                if is_array { Some(i as usize) } else { None },
                            );
                            return Ok(None);
                        }
                        flat.declarations.push(leaf_declaration(
                            base.clone(),
                            full_path.to_string(),
                            decl,
                            start_value,
                            resolved_state_select,
                            decl.is_discrete,
                        ));
                        flat.register_inst_path(
                            full_path.to_string(),
                            base,
                            if is_array { Some(i as usize) } else { None },
                        );
                        return Ok(None);
                    }
                    if !resolved_type.contains('.') {
                        for (_alias, qual) in &model.imports {
                            if qual.is_empty() {
                                continue;
                            }
                            let candidate = format!("{}.{}", qual, resolved_type);
                            if let Some((prefix_type, suffix_type)) = candidate.rsplit_once('.') {
                                if let Ok(owner) = self.loader.load_model(prefix_type) {
                                    if let Some((_, base)) = owner
                                        .type_aliases
                                        .iter()
                                        .find(|(a, _)| a == suffix_type)
                                    {
                                        let base = resolve_type_alias(&owner.type_aliases, base);
                                        if is_primitive(&base) {
                                            flat.declarations.push(leaf_declaration(
                                                base.clone(),
                                                full_path.to_string(),
                                                decl,
                                                self.elem_start_value(
                                                    &None,
                                                    &decl.start_value,
                                                    context,
                                                    is_array,
                                                    i,
                                                ),
                                                resolved_state_select,
                                                decl.is_discrete,
                                            ));
                                            flat.register_inst_path(
                                                full_path.to_string(),
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
                    flat.declarations.push(leaf_declaration(
                        fallback_type_name.to_string(),
                        full_path.to_string(),
                        decl,
                        if let Some(val) = &decl.start_value {
                            Some(self.substitute(val, context))
                        } else {
                            Some(Expression::Number(0.0))
                        },
                        resolved_state_select,
                        decl.is_discrete,
                    ));
                    flat.register_inst_path(
                        full_path.to_string(),
                        fallback_type_name.to_string(),
                        if is_array { Some(i as usize) } else { None },
                    );
                    return Ok(None);
                }
                if resolved_type == "Types.Dynamics"
                    || resolved_type.ends_with(".Types.Dynamics")
                {
                    flat.declarations.push(leaf_declaration(
                        "Integer".to_string(),
                        full_path.to_string(),
                        decl,
                        Some(Expression::Number(0.0)),
                        resolved_state_select,
                        true,
                    ));
                    flat.register_inst_path(
                        full_path.to_string(),
                        "Integer".to_string(),
                        if is_array { Some(i as usize) } else { None },
                    );
                    return Ok(None);
                }
                match e {
                    LoadError::NotFound(_) => Err(FlattenError::UnknownType(
                        resolved_type.clone(),
                        full_path.to_string(),
                        loc,
                    )),
                    _ => Err(FlattenError::Load(e)),
                }
            }
        }
    }
}
