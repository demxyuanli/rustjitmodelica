pub(crate) fn build_runtime_algorithms(
    flat_model: &FlattenedModel,
    stage_trace: bool,
) -> Vec<AlgorithmStatement> {
    if stage_trace {
        eprintln!("[stage] lower_event_algorithms");
    }
    let started_at = Instant::now();
    let mut algorithms = flat_model.algorithms.clone();
    let discrete_lhs_set: HashSet<String> = flat_model
        .declarations
        .iter()
        .filter(|d| d.is_discrete || flat_model.clocked_var_names.contains(&d.name))
        .map(|d| d.name.clone())
        .collect();
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
    for eq in &flat_model.equations {
        match eq {
            Equation::Simple(lhs, rhs)
                if lhs_root_var(lhs)
                    .map(|n| discrete_lhs_set.contains(&n))
                    .unwrap_or(false) =>
            {
                algorithms.push(AlgorithmStatement::Assignment(
                    normalize_der(lhs),
                    normalize_der(rhs),
                ));
            }
            Equation::When(cond, body, else_whens) => {
                let normalized = Equation::When(
                    normalize_der(cond),
                    body.iter().map(normalize_simple_equation).collect(),
                    else_whens.clone(),
                );
                algorithms.push(equation_convert::convert_eq_to_alg_stmt(normalized));
            }
            Equation::Reinit(var, expr) => algorithms.push(equation_convert::convert_eq_to_alg_stmt(
                Equation::Reinit(var.clone(), normalize_der(expr)),
            )),
            Equation::Assert(cond, msg) => algorithms.push(equation_convert::convert_eq_to_alg_stmt(
                Equation::Assert(normalize_der(cond), normalize_der(msg)),
            )),
            Equation::Terminate(msg) => algorithms.push(equation_convert::convert_eq_to_alg_stmt(
                Equation::Terminate(normalize_der(msg)),
            )),
            Equation::If(cond, then_eqs, elseif_list, else_eqs) => algorithms.push(
                equation_convert::convert_eq_to_alg_stmt(Equation::If(
                    normalize_der(cond),
                    then_eqs.iter().map(normalize_simple_equation).collect(),
                    elseif_list
                        .iter()
                        .map(|(elseif_cond, body)| {
                            (
                                normalize_der(elseif_cond),
                                body.iter().map(normalize_simple_equation).collect(),
                            )
                        })
                        .collect(),
                    else_eqs
                        .as_ref()
                        .map(|eqs| eqs.iter().map(normalize_simple_equation).collect()),
                )),
            ),
            _ => {}
        }
    }
    log_stage_timing(stage_trace, "lower_event_algorithms", started_at);
    algorithms
}

pub(crate) fn collect_newton_tearing_var_names(alg_equations: &[Equation]) -> Vec<String> {
    alg_equations
        .iter()
        .filter_map(|eq| {
            if let Equation::SolvableBlock {
                tearing_var: Some(tearing_var),
                residuals,
                ..
            } = eq
            {
                if residuals.len() == 1 {
                    return Some(tearing_var.clone());
                }
            }
            None
        })
        .collect()
}
