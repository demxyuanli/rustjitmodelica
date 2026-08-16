/// `y = pre(x)` plus `x = f(y, ...)` is a discrete delay register (StateGraph newActive).
/// Connect aliases `u = other.y` with `pre(u)` (UnitDelay) are not registers.
fn previous_arg_name(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Previous(inner) => match inner.as_ref() {
            Expression::Variable(id) => Some(crate::string_intern::resolve_id(*id)),
            other => previous_arg_name(other),
        },
        Expression::Call(name, args) if name == "pre" && args.len() == 1 => {
            previous_arg_name(&args[0])
        }
        _ => None,
    }
}

fn expr_mentions_any(expr: &Expression, names: &[String]) -> bool {
    if names.is_empty() {
        return false;
    }
    let mut vars = HashSet::new();
    collect_vars_expr(expr, &mut vars);
    names.iter().any(|n| vars.contains(n))
}

fn collect_pre_register_discrete(equations: &[Equation]) -> HashSet<String> {
    let mut holders: HashMap<String, Vec<String>> = HashMap::new();
    let mut assignments: Vec<(String, Expression)> = Vec::new();
    for eq in equations {
        let Equation::Simple(lhs, rhs) = eq else {
            continue;
        };
        let Expression::Variable(id) = lhs else {
            continue;
        };
        let lhs_name = crate::string_intern::resolve_id(*id);
        if let Some(src) = previous_arg_name(rhs) {
            holders.entry(src).or_default().push(lhs_name.clone());
        }
        assignments.push((lhs_name, rhs.clone()));
    }
    let mut out = HashSet::new();
    for (lhs, rhs) in &assignments {
        let Some(hs) = holders.get(lhs) else {
            continue;
        };
        if expr_mentions_any(rhs, hs) || previous_arg_name(rhs).as_ref() == Some(lhs) {
            out.insert(lhs.clone());
        }
    }
    out
}

pub(crate) fn classify_variables(
    flat_model: &FlattenedModel,
    quiet: bool,
    stage_trace: bool,
) -> VariableLayout {
    if stage_trace {
        eprintln!("[stage] classify_vars");
    }
    let started_at = Instant::now();

    let mut state_vars = HashSet::new();
    let mut discrete_vars = HashSet::new();
    fn collect_ref_root_vars(expr: &Expression, out: &mut HashSet<String>) {
        match expr {
            Expression::Variable(id) => {
                out.insert(crate::string_intern::resolve_id(*id));
            }
            Expression::ArrayAccess(base, _) => collect_ref_root_vars(base, out),
            Expression::Dot(base, _) => collect_ref_root_vars(base, out),
            _ => {}
        }
    }
    fn collect_previous_vars_expr(expr: &Expression, out: &mut HashSet<String>) {
        match expr {
            Expression::Previous(inner) => {
                collect_ref_root_vars(inner, out);
                collect_previous_vars_expr(inner, out);
            }
            Expression::BinaryOp(l, _, r) => {
                collect_previous_vars_expr(l, out);
                collect_previous_vars_expr(r, out);
            }
            Expression::Call(name, args) => {
                if name == "pre" {
                    for a in args {
                        collect_ref_root_vars(a, out);
                        collect_previous_vars_expr(a, out);
                    }
                } else {
                    for a in args {
                        collect_previous_vars_expr(a, out);
                    }
                }
            }
            Expression::ArrayLiteral(args) => {
                for a in args {
                    collect_previous_vars_expr(a, out);
                }
            }
            Expression::ArrayAccess(base, idx) => {
                collect_previous_vars_expr(base, out);
                collect_previous_vars_expr(idx, out);
            }
            Expression::Dot(base, _) => collect_previous_vars_expr(base, out),
            Expression::If(c, t, f) => {
                collect_previous_vars_expr(c, out);
                collect_previous_vars_expr(t, out);
                collect_previous_vars_expr(f, out);
            }
            Expression::Range(a, b, c) => {
                collect_previous_vars_expr(a, out);
                collect_previous_vars_expr(b, out);
                collect_previous_vars_expr(c, out);
            }
            Expression::Sample(inner)
            | Expression::Interval(inner)
            | Expression::Hold(inner)
            | Expression::Der(inner) => collect_previous_vars_expr(inner, out),
            Expression::SubSample(c, n)
            | Expression::SuperSample(c, n)
            | Expression::ShiftSample(c, n)
            | Expression::BackSample(c, n) => {
                collect_previous_vars_expr(c, out);
                collect_previous_vars_expr(n, out);
            }
            Expression::ArrayComprehension { expr, iter_range, .. } => {
                collect_previous_vars_expr(expr, out);
                collect_previous_vars_expr(iter_range, out);
            }
            Expression::Variable(_) | Expression::Number(_) | Expression::StringLiteral(_) => {}
        }
    }
    fn collect_previous_vars_eq(eq: &Equation, out: &mut HashSet<String>) {
        match eq {
            Equation::Simple(lhs, rhs) => {
                collect_previous_vars_expr(lhs, out);
                collect_previous_vars_expr(rhs, out);
            }
            Equation::MultiAssign(lhss, rhs) => {
                for lhs in lhss {
                    collect_previous_vars_expr(lhs, out);
                }
                collect_previous_vars_expr(rhs, out);
            }
            Equation::For(_, _, _, body) => {
                for e in body {
                    collect_previous_vars_eq(e, out);
                }
            }
            Equation::When(c, body, else_whens) => {
                collect_previous_vars_expr(c, out);
                for e in body {
                    collect_previous_vars_eq(e, out);
                }
                for (cond, branch) in else_whens {
                    collect_previous_vars_expr(cond, out);
                    for e in branch {
                        collect_previous_vars_eq(e, out);
                    }
                }
            }
            Equation::If(c, t, else_ifs, e) => {
                collect_previous_vars_expr(c, out);
                for eq in t {
                    collect_previous_vars_eq(eq, out);
                }
                for (cond, branch) in else_ifs {
                    collect_previous_vars_expr(cond, out);
                    for eq in branch {
                        collect_previous_vars_eq(eq, out);
                    }
                }
                if let Some(branch) = e {
                    for eq in branch {
                        collect_previous_vars_eq(eq, out);
                    }
                }
            }
            Equation::Connect(a, b) => {
                collect_previous_vars_expr(a, out);
                collect_previous_vars_expr(b, out);
            }
            Equation::Reinit(_, e) | Equation::Assert(e, _) | Equation::Terminate(e) => {
                collect_previous_vars_expr(e, out);
            }
            Equation::CallStmt(e) => collect_previous_vars_expr(e, out),
            Equation::SolvableBlock { equations, residuals, .. } => {
                for eq in equations {
                    collect_previous_vars_eq(eq, out);
                }
                for r in residuals {
                    collect_previous_vars_expr(r, out);
                }
            }
        }
    }

    fn collect_previous_vars_alg(stmt: &AlgorithmStatement, out: &mut HashSet<String>) {
        match stmt {
            AlgorithmStatement::Assignment(lhs, rhs) => {
                collect_previous_vars_expr(lhs, out);
                collect_previous_vars_expr(rhs, out);
            }
            AlgorithmStatement::MultiAssign(lhss, rhs) => {
                for l in lhss {
                    collect_previous_vars_expr(l, out);
                }
                collect_previous_vars_expr(rhs, out);
            }
            AlgorithmStatement::CallStmt(e) => collect_previous_vars_expr(e, out),
            AlgorithmStatement::NoOp => {}
            AlgorithmStatement::Break => {}
            AlgorithmStatement::Return(v) => {
                if let Some(expr) = v {
                    collect_previous_vars_expr(expr, out);
                }
            }
            AlgorithmStatement::If(c, t, elseifs, els) => {
                collect_previous_vars_expr(c, out);
                for s in t {
                    collect_previous_vars_alg(s, out);
                }
                for (cond, body) in elseifs {
                    collect_previous_vars_expr(cond, out);
                    for s in body {
                        collect_previous_vars_alg(s, out);
                    }
                }
                if let Some(body) = els {
                    for s in body {
                        collect_previous_vars_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::For(_, range, body) => {
                collect_previous_vars_expr(range, out);
                for s in body {
                    collect_previous_vars_alg(s, out);
                }
            }
            AlgorithmStatement::While(cond, body) => {
                collect_previous_vars_expr(cond, out);
                for s in body {
                    collect_previous_vars_alg(s, out);
                }
            }
            AlgorithmStatement::When(cond, body, else_whens) => {
                collect_previous_vars_expr(cond, out);
                for s in body {
                    collect_previous_vars_alg(s, out);
                }
                for (c, branch) in else_whens {
                    collect_previous_vars_expr(c, out);
                    for s in branch {
                        collect_previous_vars_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::Reinit(_, e) | AlgorithmStatement::Assert(e, _) | AlgorithmStatement::Terminate(e) => {
                collect_previous_vars_expr(e, out);
            }
        }
    }

    fn collect_simple_lhs_vars(eq: &Equation, out: &mut HashSet<String>) {
        match eq {
            Equation::Simple(lhs, _) => collect_ref_root_vars(lhs, out),
            Equation::MultiAssign(lhss, _) => {
                for lhs in lhss {
                    collect_ref_root_vars(lhs, out);
                }
            }
            _ => {}
        }
    }

    fn collect_alg_lhs_vars(stmt: &AlgorithmStatement, out: &mut HashSet<String>) {
        match stmt {
            AlgorithmStatement::Assignment(lhs, _) => collect_ref_root_vars(lhs, out),
            AlgorithmStatement::MultiAssign(lhss, _) => {
                for l in lhss {
                    collect_ref_root_vars(l, out);
                }
            }
            AlgorithmStatement::Reinit(var, _) => {
                out.insert(var.clone());
            }
            _ => {}
        }
    }

    fn collect_when_assigned_eq(eq: &Equation, out: &mut HashSet<String>) {
        match eq {
            Equation::When(_, body, else_whens) => {
                for e in body {
                    collect_simple_lhs_vars(e, out);
                    collect_when_assigned_eq(e, out);
                }
                for (_, branch) in else_whens {
                    for e in branch {
                        collect_simple_lhs_vars(e, out);
                        collect_when_assigned_eq(e, out);
                    }
                }
            }
            Equation::For(_, _, _, body) => {
                for e in body {
                    collect_when_assigned_eq(e, out);
                }
            }
            Equation::If(_, then_eqs, else_ifs, else_eqs) => {
                for e in then_eqs {
                    collect_when_assigned_eq(e, out);
                }
                for (_, branch) in else_ifs {
                    for e in branch {
                        collect_when_assigned_eq(e, out);
                    }
                }
                if let Some(branch) = else_eqs {
                    for e in branch {
                        collect_when_assigned_eq(e, out);
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_when_assigned_alg(stmt: &AlgorithmStatement, out: &mut HashSet<String>) {
        match stmt {
            AlgorithmStatement::When(_, body, else_whens) => {
                for s in body {
                    collect_alg_lhs_vars(s, out);
                    collect_when_assigned_alg(s, out);
                }
                for (_, branch) in else_whens {
                    for s in branch {
                        collect_alg_lhs_vars(s, out);
                        collect_when_assigned_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::If(_, then_stmts, else_ifs, else_stmts) => {
                for s in then_stmts {
                    collect_when_assigned_alg(s, out);
                }
                for (_, branch) in else_ifs {
                    for s in branch {
                        collect_when_assigned_alg(s, out);
                    }
                }
                if let Some(branch) = else_stmts {
                    for s in branch {
                        collect_when_assigned_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::While(_, body) | AlgorithmStatement::For(_, _, body) => {
                for s in body {
                    collect_when_assigned_alg(s, out);
                }
            }
            _ => {}
        }
    }

    let mut param_vars = Vec::new();
    let mut output_vars = Vec::new();
    let mut params = Vec::new();
    let mut states = Vec::new();
    let mut discrete_vals = Vec::new();

    for eq in &flat_model.equations {
        collect_states_from_eq(eq, &mut state_vars);
    }
    let state_select_ranks =
        crate::state_select::build_state_select_map(&flat_model.declarations);
    demote_equated_connector_states(
        &flat_model.equations,
        &mut state_vars,
        &state_select_ranks,
    );
    demote_dependent_relative_states(&flat_model.equations, &mut state_vars);
    demote_scaled_absolute_pairs(
        &flat_model.equations,
        &mut state_vars,
        &state_select_ranks,
    );
    demote_redundant_sensor_velocity_states(&flat_model.equations, &mut state_vars);
    let mb_demoted_states = demote_multibody_absolute_body_states(
        &flat_model.equations,
        &flat_model.declarations,
        &flat_model.instances,
        &mut state_vars,
    );

    let mut param_value_map: HashMap<String, f64> = HashMap::new();

    for decl in &flat_model.declarations {
        if decl.is_parameter {
            param_vars.push(decl.name.clone());
            let val = decl
                .start_value
                .as_ref()
                .and_then(|e| eval_const_expr_with_params(e, &param_value_map))
                .unwrap_or(0.0);
            param_value_map.insert(decl.name.clone(), val);
            params.push(val);
        } else if decl.is_discrete || flat_model.clocked_var_names.contains(&decl.name) {
            discrete_vars.insert(decl.name.clone());
        }
    }

    // Second pass: re-evaluate parameters that depend on later-declared parameters
    let mut changed = true;
    for _round in 0..5 {
        if !changed {
            break;
        }
        changed = false;
        for (i, decl) in flat_model.declarations.iter().enumerate() {
            if !decl.is_parameter {
                continue;
            }
            if let Some(expr) = &decl.start_value {
                if let Some(new_val) = eval_const_expr_with_params(expr, &param_value_map) {
                    let old_val = param_value_map.get(&decl.name).copied().unwrap_or(0.0);
                    if (new_val - old_val).abs() > 1e-15 {
                        param_value_map.insert(decl.name.clone(), new_val);
                        let param_idx = param_vars.iter().position(|n| *n == decl.name);
                        if let Some(pi) = param_idx {
                            params[pi] = new_val;
                        }
                        changed = true;
                    }
                }
            }
            let _ = i;
        }
    }
    let mut previous_vars = HashSet::new();
    for eq in &flat_model.equations {
        collect_previous_vars_eq(eq, &mut previous_vars);
    }
    for eq in &flat_model.initial_equations {
        collect_previous_vars_eq(eq, &mut previous_vars);
    }
    for stmt in &flat_model.algorithms {
        collect_previous_vars_alg(stmt, &mut previous_vars);
    }
    // Variables assigned inside `when` are discrete. `pre(x)` / `previous(x)` alone must
    // NOT mark continuous inputs as discrete (UnitDelay `y = pre(u)` would otherwise
    // freeze `u` at 0 and break connect(gain.y, ud.u)).
    let mut when_assigned = HashSet::new();
    for eq in &flat_model.equations {
        collect_when_assigned_eq(eq, &mut when_assigned);
    }
    for stmt in &flat_model.algorithms {
        collect_when_assigned_alg(stmt, &mut when_assigned);
    }
    for name in when_assigned.iter().chain(previous_vars.iter().filter(|n| when_assigned.contains(*n))) {
        if !state_vars.contains(name) {
            discrete_vars.insert(name.clone());
        }
    }
    for name in collect_pre_register_discrete(&flat_model.equations) {
        if !state_vars.contains(&name) {
            discrete_vars.insert(name);
        }
    }

    let decl_index: HashMap<String, usize> = flat_model
        .declarations
        .iter()
        .enumerate()
        .map(|(i, d)| (d.name.clone(), i))
        .collect();

    if stage_trace {
        eprintln!("[stage] referenced_vars");
    }

    let empty_knowns: HashSet<String> = HashSet::new();
    let mut referenced_in_equations = HashSet::new();
    for eq in &flat_model.equations {
        referenced_in_equations.extend(extract_unknowns(eq, &empty_knowns));
    }
    for stmt in &flat_model.algorithms {
        referenced_in_equations.extend(extract_unknowns_from_algorithm(stmt, &empty_knowns));
    }
    for eq in &flat_model.initial_equations {
        referenced_in_equations.extend(extract_unknowns(eq, &empty_knowns));
    }
    for stmt in &flat_model.initial_algorithms {
        referenced_in_equations.extend(extract_unknowns_from_algorithm(stmt, &empty_knowns));
    }

    let mut param_set: HashSet<String> = param_vars.iter().cloned().collect();
    // Connector pins / expand artifacts may appear in equations without a matching
    // declaration. Promoting them to parameters freezes them at 0 and makes BLT skip
    // the driving equations (e.g. SineVoltage: vsine_p_v / vsine_n_v). Keep them as
    // algebraic outputs instead so the circuit can be solved.
    let mut undeclared_algebraic: Vec<String> = Vec::new();
    for var in &referenced_in_equations {
        if state_vars.contains(var)
            || discrete_vars.contains(var)
            || param_set.contains(var)
            || var.starts_with("der_")
            || decl_index.contains_key(var)
            || var == "time"
            || var == "t"
        {
            continue;
        }
        undeclared_algebraic.push(var.clone());
    }
    undeclared_algebraic.sort();
    undeclared_algebraic.dedup();

    let mut state_vars_sorted: Vec<String> = state_vars.into_iter().collect();
    state_vars_sorted.sort();

    let mut discrete_vars_sorted: Vec<String> = discrete_vars.into_iter().collect();
    discrete_vars_sorted.sort();

    let state_set: HashSet<String> = state_vars_sorted.iter().cloned().collect();
    let discrete_set: HashSet<String> = discrete_vars_sorted.iter().cloned().collect();

    let start_value_for = |name: &str| -> f64 {
        if let Some(&v) = param_value_map.get(name) {
            return v;
        }
        if let Some(v) = decl_index
            .get(name)
            .and_then(|&idx| flat_model.declarations[idx].start_value.as_ref())
            .and_then(|e| eval_const_expr_with_params(e, &param_value_map))
        {
            return v;
        }
        geometric_default_for_name(name)
    };

    for var in &state_vars_sorted {
        states.push(start_value_for(var));
    }
    for var in &discrete_vars_sorted {
        discrete_vals.push(start_value_for(var));
    }

    let mut output_var_index = HashMap::new();
    let mut output_start_vals = Vec::new();
    for decl in &flat_model.declarations {
        if decl.is_parameter || discrete_set.contains(&decl.name) || state_set.contains(&decl.name) {
            continue;
        }
        let idx = output_vars.len();
        output_var_index.insert(decl.name.clone(), idx);
        output_vars.push(decl.name.clone());
        output_start_vals.push(start_value_for(&decl.name));
    }
    for var in &undeclared_algebraic {
        if output_var_index.contains_key(var)
            || state_set.contains(var)
            || discrete_set.contains(var)
            || param_set.contains(var)
        {
            continue;
        }
        let idx = output_vars.len();
        output_var_index.insert(var.clone(), idx);
        output_vars.push(var.clone());
        output_start_vals.push(start_value_for(var));
    }
    for var in &state_vars_sorted {
        let der_var = format!("der_{}", var);
        if !output_var_index.contains_key(&der_var) {
            let idx = output_vars.len();
            output_var_index.insert(der_var.clone(), idx);
            output_vars.push(der_var);
            output_start_vals.push(0.0);
        }
    }
    // Multibody body states demoted to algebraics still appear under der(...);
    // keep the variable and der_* as outputs so JIT/BLT can resolve them.
    for var in &mb_demoted_states {
        if !output_var_index.contains_key(var)
            && !state_set.contains(var)
            && !discrete_set.contains(var)
            && !param_set.contains(var)
        {
            let idx = output_vars.len();
            output_var_index.insert(var.clone(), idx);
            output_vars.push(var.clone());
            output_start_vals.push(start_value_for(var));
        }
        let der_var = format!("der_{}", var);
        if !output_var_index.contains_key(&der_var) {
            let idx = output_vars.len();
            output_var_index.insert(der_var.clone(), idx);
            output_vars.push(der_var);
            output_start_vals.push(0.0);
        }
    }

    let input_var_names = flat_model
        .declarations
        .iter()
        .filter(|d| d.is_input)
        .map(|d| d.name.clone())
        .collect();

    let state_var_index: HashMap<String, usize> = state_vars_sorted
        .iter()
        .enumerate()
        .map(|(i, s)| (s.clone(), i))
        .collect();
    let discrete_var_index: HashMap<String, usize> = discrete_vars_sorted
        .iter()
        .enumerate()
        .map(|(i, s)| (s.clone(), i))
        .collect();
    let param_var_index: HashMap<String, usize> = param_vars
        .iter()
        .enumerate()
        .map(|(i, s)| (s.clone(), i))
        .collect();

    initial_conditions::apply_initial_conditions(
        flat_model,
        &mut states,
        &mut discrete_vals,
        &mut params,
        &state_var_index,
        &discrete_var_index,
        &param_var_index,
        &param_value_map,
        quiet,
    );

    let mut array_info = HashMap::new();
    for (name, size) in &flat_model.array_sizes {
        let first_elem = format!("{}_1", name);
        let array_type_and_start = state_var_index
            .get(&first_elem)
            .copied()
            .map(|start_index| (ArrayType::State, start_index))
            .or_else(|| {
                discrete_var_index
                    .get(&first_elem)
                    .copied()
                    .map(|start_index| (ArrayType::Discrete, start_index))
            })
            .or_else(|| {
                param_var_index
                    .get(&first_elem)
                    .copied()
                    .map(|start_index| (ArrayType::Parameter, start_index))
            })
            .or_else(|| {
                output_var_index
                    .get(&first_elem)
                    .copied()
                    .map(|start_index| (ArrayType::Output, start_index))
            })
            .or_else(|| {
                state_var_index
                    .get(name)
                    .copied()
                    .map(|start_index| (ArrayType::State, start_index))
            })
            .or_else(|| {
                discrete_var_index
                    .get(name)
                    .copied()
                    .map(|start_index| (ArrayType::Discrete, start_index))
            })
            .or_else(|| {
                param_var_index
                    .get(name)
                    .copied()
                    .map(|start_index| (ArrayType::Parameter, start_index))
            })
            .or_else(|| {
                output_var_index
                    .get(name)
                    .copied()
                    .map(|start_index| (ArrayType::Output, start_index))
            });

        if let Some((array_type, start_index)) = array_type_and_start {
            array_info.insert(
                name.clone(),
                ArrayInfo {
                    array_type,
                    start_index,
                    size: *size,
                },
            );
        }
    }

    log_stage_timing(stage_trace, "classify_vars", started_at);

    VariableLayout {
        states,
        discrete_vals,
        params,
        state_vars: state_vars_sorted,
        discrete_vars: discrete_vars_sorted,
        param_vars,
        output_vars,
        output_start_vals,
        input_var_names,
        state_var_index,
        param_var_index,
        output_var_index,
        array_info,
    }
}
