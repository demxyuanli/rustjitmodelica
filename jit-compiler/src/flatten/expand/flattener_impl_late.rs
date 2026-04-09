impl crate::flatten::Flattener {
    pub(crate) fn expand_algorithm_list(
        &mut self,
        algorithms: &[AlgorithmStatement],
        prefix: &str,
        target: &mut ExpandTarget,
        context_stack: &mut Vec<HashMap<String, Expression>>,
    ) {
        for stmt in algorithms {
            match stmt {
                AlgorithmStatement::Assignment(lhs, rhs) => {
                    let lhs_sub = self.substitute_stack(lhs, context_stack);
                    let rhs_sub = self.substitute_stack(rhs, context_stack);
                    target.algorithms.push(AlgorithmStatement::Assignment(
                        prefix_expression(&lhs_sub, prefix),
                        prefix_expression(&rhs_sub, prefix),
                    ));
                }
                AlgorithmStatement::MultiAssign(lhss, rhs) => {
                    let lhss_sub: Vec<Expression> = lhss
                        .iter()
                        .map(|e| self.substitute_stack(e, context_stack))
                        .collect();
                    let complex_lhs_targets = crate::flatten::expand::helpers::collect_complex_lhs_targets(&lhss_sub);
                    let rhs_sub = self.substitute_stack(rhs, context_stack);
                    let lhss_pre: Vec<Expression> = lhss_sub
                        .iter()
                        .map(|e| prefix_expression(e, prefix))
                        .collect();
                    let rhs_pre = prefix_expression(&rhs_sub, prefix);
                    // P4-2: getInterpolationCoefficients - expand to multiple coefficient equations
                    // getInterpolationCoefficients(table, u) returns array of coefficients [c1, c2, ..., cn]
                    // Each coefficient ci = frac(u) for linear interpolation
                    if let Expression::Call(name, args_pre) = &rhs_pre {
                        if name.ends_with("getInterpolationCoefficients") && args_pre.len() >= 2 {
                            // Compute h = frac(u) = u - floor(u) for linear interpolation
                            // For each LHS output, assign the same coefficient h
                            let u_expr = &args_pre[1];
                            let h_expr = Expression::BinaryOp(
                                Box::new(u_expr.clone()),
                                crate::ast::Operator::Sub,
                                Box::new(Expression::Call("floor".to_string(), vec![u_expr.clone()])),
                            );
                            for lhs in &lhss_pre {
                                target.equations.push(Equation::Simple(lhs.clone(), h_expr.clone()));
                            }
                            continue;
                        }
                        if !complex_lhs_targets.is_empty() {
                            eprintln!(
                                "Error: Algorithm MultiAssign in '{}' uses complex LHS target(s) [{}] like arr[i].field, which require field-store semantics and are treated as hard error in backend.",
                                name,
                                complex_lhs_targets.join(", ")
                            );
                            target
                                .algorithms
                                .push(AlgorithmStatement::MultiAssign(lhss_pre, rhs_pre));
                            continue;
                        }
                        // Try user/function model expansion first, even when the name resembles builtin.
                        if let Ok(func_model) = self.loader.load_model(name) {
                            if let Some((input_names, outputs)) =
                                get_function_outputs(func_model.as_ref())
                            {
                                if input_names.len() == args_pre.len()
                                    && outputs.len() == lhss_pre.len()
                                {
                                    let mut expanded_pairs: Vec<(Expression, Expression)> =
                                        Vec::with_capacity(lhss_pre.len());
                                    let mut shape_mismatch = false;
                                    let mut subst = HashMap::new();
                                    for (i, in_name) in input_names.iter().enumerate() {
                                        if i < args_pre.len() {
                                            subst.insert(in_name.clone(), args_pre[i].clone());
                                        }
                                    }
                                    for (i, out_spec) in outputs.iter().enumerate() {
                                        if i < lhss_pre.len() {
                                            let sub = self.substitute(&out_spec.expr, &subst);
                                            let lhs = &lhss_pre[i];
                                            let nested_depth = crate::flatten::expand::helpers::array_literal_depth(&sub);
                                            let has_comp = crate::flatten::expand::helpers::expr_contains_array_comprehension(&sub);
                                            let is_record_like =
                                                crate::flatten::expand::helpers::is_record_like_output_type(&out_spec.resolved_type_name);
                                            if crate::flatten::expand::helpers::is_complex_lhs_target(lhs) {
                                                shape_mismatch = true;
                                                eprintln!(
                                                    "Error: Algorithm MultiAssign output shape mismatch in '{}': complex LHS target {:?} requires field-store semantics (for example arr[i].field), which is unsupported in backend. This is treated as hard error.",
                                                    name, lhs
                                                );
                                                break;
                                            }
                                            if crate::flatten::expand::helpers::is_scalar_lhs_target(lhs) && crate::flatten::expand::helpers::is_array_like_output(&sub)
                                            {
                                                shape_mismatch = true;
                                                if nested_depth > 1 || has_comp {
                                                    eprintln!(
                                                        "Error: Algorithm MultiAssign output shape mismatch in '{}': output '{}' is multidimensional/comprehension-like and cannot bind to scalar-like LHS {:?}. This is treated as hard error in backend.",
                                                        name, out_spec.name, lhs
                                                    );
                                                } else {
                                                    eprintln!(
                                                        "Warning: Algorithm MultiAssign output shape mismatch in '{}': scalar-like LHS {:?} cannot receive 1D array output {:?}. Keeping MultiAssign for backend handling.",
                                                        name, lhs, sub
                                                    );
                                                }
                                                break;
                                            }
                                            if !crate::flatten::expand::helpers::is_scalar_lhs_target(lhs) && crate::flatten::expand::helpers::is_scalar_like_output(&sub)
                                            {
                                                shape_mismatch = true;
                                                eprintln!(
                                                    "Warning: Algorithm MultiAssign output shape mismatch in '{}': non-scalar-like LHS {:?} cannot receive scalar output {:?}",
                                                    name, lhs, sub
                                                );
                                                break;
                                            }
                                            if crate::flatten::expand::helpers::is_scalar_lhs_target(lhs) && is_record_like {
                                                shape_mismatch = true;
                                                eprintln!(
                                                    "Error: Algorithm MultiAssign output shape mismatch in '{}': output '{}' has record-like type '{}' and cannot bind to scalar-like LHS {:?}. This is treated as hard error in backend.",
                                                    name, out_spec.name, out_spec.resolved_type_name, lhs
                                                );
                                                break;
                                            }
                                            let sub_pre = prefix_expression(&sub, prefix);
                                            expanded_pairs.push((lhs.clone(), sub_pre));
                                        }
                                    }
                                    if !shape_mismatch {
                                        for (lhs_e, rhs_e) in expanded_pairs {
                                            target
                                                .algorithms
                                                .push(AlgorithmStatement::Assignment(lhs_e, rhs_e));
                                        }
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    target
                        .algorithms
                        .push(AlgorithmStatement::MultiAssign(lhss_pre, rhs_pre));
                }
                AlgorithmStatement::If(cond, true_stmts, else_ifs, else_stmts) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(true_stmts, prefix, &mut temp_target, context_stack);
                    let new_true = temp_alg;
                    let mut new_else_ifs = Vec::new();
                    for (c, s) in else_ifs {
                        let c_sub = self.substitute_stack(c, context_stack);
                        let mut t_eqs = Vec::new();
                        let mut t_alg = Vec::new();
                        let mut t_conn = Vec::new();
                        let mut t_target = ExpandTarget {
                            equations: &mut t_eqs,
                            algorithms: &mut t_alg,
                            connections: &mut t_conn,
                            conditional_connections: target.conditional_connections,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_algorithm_list(s, prefix, &mut t_target, context_stack);
                        new_else_ifs.push((prefix_expression(&c_sub, prefix), t_alg));
                    }
                    let mut new_else = None;
                    if let Some(s) = else_stmts {
                        let mut t_eqs = Vec::new();
                        let mut t_alg = Vec::new();
                        let mut t_conn = Vec::new();
                        let mut t_target = ExpandTarget {
                            equations: &mut t_eqs,
                            algorithms: &mut t_alg,
                            connections: &mut t_conn,
                            conditional_connections: target.conditional_connections,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_algorithm_list(s, prefix, &mut t_target, context_stack);
                        new_else = Some(t_alg);
                    }
                    target.algorithms.push(AlgorithmStatement::If(
                        prefix_expression(&cond_sub, prefix),
                        new_true,
                        new_else_ifs,
                        new_else,
                    ));
                }
                AlgorithmStatement::While(cond, body) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(body, prefix, &mut temp_target, context_stack);
                    target.algorithms.push(AlgorithmStatement::While(
                        prefix_expression(&cond_sub, prefix),
                        temp_alg,
                    ));
                }
                AlgorithmStatement::For(var_name, range, body) => {
                    let range_sub = self.substitute_stack(range, context_stack);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(body, prefix, &mut temp_target, context_stack);
                    target.algorithms.push(AlgorithmStatement::For(
                        var_name.clone(),
                        Box::new(prefix_expression(&range_sub, prefix)),
                        temp_alg,
                    ));
                }
                AlgorithmStatement::When(cond, body, else_whens) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let mut temp_eqs = Vec::new();
                    let mut temp_alg = Vec::new();
                    let mut temp_conn = Vec::new();
                    let mut temp_target = ExpandTarget {
                        equations: &mut temp_eqs,
                        algorithms: &mut temp_alg,
                        connections: &mut temp_conn,
                        conditional_connections: target.conditional_connections,
                        array_sizes: target.array_sizes,
                    };
                    self.expand_algorithm_list(body, prefix, &mut temp_target, context_stack);
                    let new_body = temp_alg;
                    let mut new_else_whens = Vec::new();
                    for (c, s) in else_whens {
                        let c_sub = self.substitute_stack(c, context_stack);
                        let mut t_eqs = Vec::new();
                        let mut t_alg = Vec::new();
                        let mut t_conn = Vec::new();
                        let mut t_target = ExpandTarget {
                            equations: &mut t_eqs,
                            algorithms: &mut t_alg,
                            connections: &mut t_conn,
                            conditional_connections: target.conditional_connections,
                            array_sizes: target.array_sizes,
                        };
                        self.expand_algorithm_list(s, prefix, &mut t_target, context_stack);
                        new_else_whens.push((prefix_expression(&c_sub, prefix), t_alg));
                    }
                    target.algorithms.push(AlgorithmStatement::When(
                        prefix_expression(&cond_sub, prefix),
                        new_body,
                        new_else_whens,
                    ));
                }
                AlgorithmStatement::Reinit(var, val) => {
                    let val_sub = self.substitute_stack(val, context_stack);
                    let var_pre = if prefix.is_empty() {
                        var.clone()
                    } else {
                        format!("{}_{}", prefix, var)
                    };
                    let var_flat = var_pre.replace('.', "_");
                    target.algorithms.push(AlgorithmStatement::Reinit(
                        var_flat,
                        prefix_expression(&val_sub, prefix),
                    ));
                }
                AlgorithmStatement::Assert(cond, msg) => {
                    let cond_sub = self.substitute_stack(cond, context_stack);
                    let msg_sub = self.substitute_stack(msg, context_stack);
                    target.algorithms.push(AlgorithmStatement::Assert(
                        prefix_expression(&cond_sub, prefix),
                        prefix_expression(&msg_sub, prefix),
                    ));
                }
                AlgorithmStatement::Terminate(msg) => {
                    let msg_sub = self.substitute_stack(msg, context_stack);
                    target
                        .algorithms
                        .push(AlgorithmStatement::Terminate(prefix_expression(
                            &msg_sub, prefix,
                        )));
                }
                AlgorithmStatement::CallStmt(expr) => {
                    let sub = self.substitute_stack(expr, context_stack);
                    target
                        .algorithms
                        .push(AlgorithmStatement::CallStmt(prefix_expression(&sub, prefix)));
                }
                AlgorithmStatement::NoOp => {}
                AlgorithmStatement::Break => {
                    target.algorithms.push(AlgorithmStatement::Break);
                }
                AlgorithmStatement::Return(v) => {
                    let mapped = v
                        .as_ref()
                        .map(|expr| prefix_expression(&self.substitute_stack(expr, context_stack), prefix));
                    target.algorithms.push(AlgorithmStatement::Return(mapped));
                }
            }
        }
    }
}
