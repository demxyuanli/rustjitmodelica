use super::{eval_const_expr, eval_const_expr_with_array_sizes, index_expression, FlattenError, Flattener};
use crate::ast::{Declaration, Expression, Model};
use crate::diag::SourceLocation;
use crate::loader::LoadError;
use crate::flatten::utils::{apply_modification, is_primitive, resolve_inner_class_alias, resolve_type_alias};
use std::collections::HashMap;
use std::sync::Arc;

impl Flattener {
    pub(super) fn expand_declarations(
        &mut self,
        model: Arc<Model>,
        prefix: &str,
        flat: &mut crate::flatten::FlattenedModel,
        current_model_name: Option<&str>,
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

                    for decl in &model.declarations {
                        if let Some(ref cond_expr) = decl.condition {
                            let cond_sub = self.substitute(cond_expr, &context);
                            if let Some(v) = eval_const_expr(&cond_sub) {
                                if v == 0.0 {
                                    continue;
                                }
                            }
                        }

                        let array_len = if let Some(size_expr) = &decl.array_size {
                            let sub_expr = self.substitute(size_expr, &context);
                            if let Some(val) = eval_const_expr(&sub_expr) {
                                Some(val as usize)
                            } else if let Some(val) =
                                eval_const_expr_with_array_sizes(&sub_expr, &local_array_sizes)
                            {
                                Some(val as usize)
                            } else {
                                eprintln!("Warning: Could not evaluate array size for '{}'", decl.name);
                                None
                            }
                        } else {
                            None
                        };

                        let count = array_len.unwrap_or(1);
                        let is_array = array_len.is_some();

                        let base_name = if prefix.is_empty() {
                            decl.name.clone()
                        } else {
                            format!("{}_{}", prefix, decl.name)
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
                            resolved_type = resolve_inner_class_alias(&model, &resolved_type);
                            resolved_type = Self::resolve_import_scoped_type(
                                model.as_ref(),
                                &resolved_type,
                                current_qualified,
                                &msl_import_context,
                            );
                            resolved_type =
                                Self::normalize_decl_type_name(resolved_type, &pre_inner_alias);

                            if is_primitive(&resolved_type) {
                                flat.declarations.push(Declaration {
                                    type_name: resolved_type.clone(),
                                    name: full_path.clone(),
                                    replaceable: decl.replaceable,
                                    is_parameter: decl.is_parameter,
                                    is_flow: decl.is_flow,
                                    is_discrete: decl.is_discrete,
                                    is_input: decl.is_input,
                                    is_output: decl.is_output,
                                    start_value: if let Some(val) = &decl.start_value {
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
                                continue;
                            }

                            let load_candidates =
                                Self::build_load_candidates(&resolved_type, current_qualified);
                            let (loaded_type, last_err) = self.try_load_sub_model(
                                model.as_ref(),
                                &resolved_type,
                                current_qualified,
                                &load_candidates,
                            );

                            let mut sub_model = match loaded_type {
                                Some((resolved_candidate, m)) => {
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
                                                            type_name: base,
                                                            name: full_path.clone(),
                                                            replaceable: decl.replaceable,
                                                            is_parameter: decl.is_parameter,
                                                            is_flow: decl.is_flow,
                                                            is_discrete: decl.is_discrete,
                                                            is_input: decl.is_input,
                                                            is_output: decl.is_output,
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
                                                                    type_name: base,
                                                                    name: full_path.clone(),
                                                                    replaceable: decl.replaceable,
                                                                    is_parameter: decl.is_parameter,
                                                                    is_flow: decl.is_flow,
                                                                    is_discrete: decl.is_discrete,
                                                                    is_input: decl.is_input,
                                                                    is_output: decl.is_output,
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
                                            || resolved_type.ends_with(".Distribution"))
                                    {
                                        flat.declarations.push(Declaration {
                                            type_name: "Real".to_string(),
                                            name: full_path.clone(),
                                            replaceable: decl.replaceable,
                                            is_parameter: decl.is_parameter,
                                            is_flow: decl.is_flow,
                                            is_discrete: decl.is_discrete,
                                            is_input: decl.is_input,
                                            is_output: decl.is_output,
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
                                        type_name: base,
                                        name: full_path.clone(),
                                        replaceable: decl.replaceable,
                                        is_parameter: decl.is_parameter,
                                        is_flow: decl.is_flow,
                                        is_discrete: decl.is_discrete,
                                        is_input: decl.is_input,
                                        is_output: decl.is_output,
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
                                    continue;
                                }
                            }

                            self.flatten_inheritance(&mut sub_model, &resolved_type)?;
                            for modification in &decl.modifications {
                                apply_modification(Arc::make_mut(&mut sub_model), modification);
                            }

                            flat.instances.insert(full_path.clone(), resolved_type.clone());

                            stack.push(Task::ExpandEquations {
                                model: Arc::clone(&sub_model),
                                prefix: full_path.clone(),
                            });
                            stack.push(Task::Process {
                                model: sub_model,
                                prefix: full_path,
                                current_model_name: Some(resolved_type),
                                msl_import_context: msl_import_context.clone(),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
