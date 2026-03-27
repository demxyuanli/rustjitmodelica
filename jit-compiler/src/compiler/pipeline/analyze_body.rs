pub(crate) fn analyze_equations(
    flat_model: &FlattenedModel,
    layout: &mut VariableLayout,
    opts: &CompilerOptions,
    stage_trace: bool,
) -> AnalysisStage {
    fn collect_symbolic_index_summary(equations: &[Equation]) -> (usize, usize) {
        fn visit_expr(expr: &Expression, has_der: &mut bool) {
            match expr {
                Expression::Der(_) => *has_der = true,
                Expression::Variable(id) => {
                    let v = crate::string_intern::resolve_id(*id);
                    if v.starts_with("der_") {
                        *has_der = true;
                    }
                }
                Expression::BinaryOp(l, _, r) => {
                    visit_expr(l, has_der);
                    visit_expr(r, has_der);
                }
                Expression::Call(_, args) | Expression::ArrayLiteral(args) => {
                    for a in args {
                        visit_expr(a, has_der);
                    }
                }
                Expression::ArrayAccess(a, i) => {
                    visit_expr(a, has_der);
                    visit_expr(i, has_der);
                }
                Expression::If(c, t, f) => {
                    visit_expr(c, has_der);
                    visit_expr(t, has_der);
                    visit_expr(f, has_der);
                }
                Expression::Dot(base, _) => visit_expr(base, has_der),
                Expression::Range(s, st, e) => {
                    visit_expr(s, has_der);
                    visit_expr(st, has_der);
                    visit_expr(e, has_der);
                }
                _ => {}
            }
        }
        let mut signals = 0usize;
        let mut implicit_constraints = 0usize;
        for eq in equations {
            if let Equation::Simple(lhs, rhs) = eq {
                let mut lhs_has_der = false;
                let mut rhs_has_der = false;
                visit_expr(lhs, &mut lhs_has_der);
                visit_expr(rhs, &mut rhs_has_der);
                if lhs_has_der || rhs_has_der {
                    signals += 1;
                }
                if !lhs_has_der && rhs_has_der {
                    implicit_constraints += 1;
                }
            }
        }
        (signals, implicit_constraints)
    }
    if stage_trace {
        eprintln!("[stage] normalize_derivatives");
    }
    let normalize_started_at = Instant::now();
    let normalized_eqs = normalize_equations(&flat_model.equations);
    ensure_derivative_outputs(layout);
    let diff_equations = build_diff_equations(layout);
    log_stage_timing(stage_trace, "normalize_derivatives", normalize_started_at);

    if stage_trace {
        eprintln!("[stage] structure_analysis");
    }
    let structure_started_at = Instant::now();
    let discrete_var_set: HashSet<String> = layout.discrete_vars.iter().cloned().collect();
    let lhs_root_var = |e: &Expression| -> Option<String> {
        match e {
            Expression::Variable(id) => Some(crate::string_intern::resolve_id(*id)),
            Expression::ArrayAccess(base, _) => match &**base {
                Expression::Variable(id) => Some(crate::string_intern::resolve_id(*id)),
                _ => None,
            },
            Expression::Dot(base, _) => match &**base {
                Expression::Variable(id) => Some(crate::string_intern::resolve_id(*id)),
                _ => None,
            },
            _ => None,
        }
    };
    let continuous_eqs: Vec<Equation> = normalized_eqs
        .into_iter()
        .filter(|eq| {
            !matches!(
                eq,
                Equation::When(_, _, _)
                    | Equation::Reinit(_, _)
                    | Equation::If(_, _, _, _)
                    | Equation::Assert(_, _)
                    | Equation::Terminate(_)
            )
                && !matches!(
                    eq,
                    Equation::Simple(lhs, _)
                        if lhs_root_var(lhs)
                            .map(|n| discrete_var_set.contains(&n))
                            .unwrap_or(false)
                )
        })
        .collect();

    let mut known_vars: HashSet<String> = layout.state_vars.iter().cloned().collect();
    known_vars.extend(layout.discrete_vars.iter().cloned());

    let analysis_opts = AnalysisOptions {
        index_reduction_method: opts.index_reduction_method.clone(),
        tearing_method: opts.tearing_method.clone(),
        quiet: opts.quiet,
    };
    if stage_trace {
        eprintln!("[stage] sort_equations");
    }
    let sort_started_at = Instant::now();
    let sort_result = sort_algebraic_equations(
        continuous_eqs,
        &known_vars,
        &layout.param_vars,
        &analysis_opts,
    );
    log_stage_timing(stage_trace, "sort_equations", sort_started_at);

    let mut alg_equations = sort_result.sorted_equations;
    if stage_trace && sort_result.index_reduction_rounds > 0 {
        eprintln!(
            "[stage] index_reduction rounds={} dummy_derivatives={}",
            sort_result.index_reduction_rounds, sort_result.dummy_derivative_equation_count
        );
    }

    // Register dummy derivative variables created by index reduction as output (algebraic) vars
    for eq in &alg_equations {
        if let Equation::Simple(Expression::Variable(id), _) = eq {
            let name = crate::string_intern::resolve_id(*id);
            if name.starts_with("$dummy_") && !layout.output_var_index.contains_key(&name) {
                let idx = layout.output_vars.len();
                layout.output_var_index.insert(name.clone(), idx);
                layout.output_vars.push(name);
                layout.output_start_vals.push(0.0);
            }
        }
    }

    for out_var in &layout.output_vars {
        if let Some(alias_expr) = sort_result.alias_map.get(out_var) {
            alg_equations.push(Equation::Simple(
                Expression::var(out_var),
                alias_expr.clone(),
            ));
        }
    }

    let numeric_ode_jacobian =
        opts.generate_dynamic_jacobian == "numeric" || opts.generate_dynamic_jacobian == "both";
    let symbolic_ode_jacobian_enabled =
        opts.generate_dynamic_jacobian == "symbolic" || opts.generate_dynamic_jacobian == "both";
    let ode_jacobian_sparse = if symbolic_ode_jacobian_enabled {
        jacobian::build_ode_jacobian_sparse(
            &layout.state_vars,
            &alg_equations,
            &layout.state_var_index,
        )
    } else {
        None
    };
    let symbolic_ode_jacobian_matrix = ode_jacobian_sparse
        .as_ref()
        .map(jacobian::SparseOdeJacobian::to_dense)
        .or_else(|| {
            if symbolic_ode_jacobian_enabled {
                jacobian::build_ode_jacobian_expressions(
                    &layout.state_vars,
                    &alg_equations,
                    &layout.state_var_index,
                )
            } else {
                None
            }
        });

    log_stage_timing(stage_trace, "structure_analysis", structure_started_at);
    let (symbolic_index_signal_count, implicit_derivative_constraint_count) =
        collect_symbolic_index_summary(&alg_equations);

    AnalysisStage {
        alg_equations,
        diff_equations,
        differential_index: sort_result.differential_index,
        constraint_equation_count: sort_result.constraint_equation_count,
        constant_conflict_count: sort_result.constant_conflict_count,
        numeric_ode_jacobian,
        symbolic_ode_jacobian_matrix,
        ode_jacobian_sparse,
        block_causality: sort_result.block_causality,
        symbolic_index_signal_count,
        implicit_derivative_constraint_count,
        blt_degrade_guard_triggered: sort_result.blt_degrade_guard_triggered,
        blt_degrade_guard_limit: sort_result.blt_degrade_guard_limit,
        blt_degrade_guard_equation_count: sort_result.blt_degrade_guard_equation_count,
    }
}
