use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use crate::analysis::{
    collect_states_from_eq, extract_unknowns, normalize_der, sort_algebraic_equations,
    AnalysisOptions,
};
use crate::ast::{AlgorithmStatement, Equation, Expression, Model};
use crate::compiler::{equation_convert, initial_conditions, jacobian, CompilerOptions};
use crate::flatten::{eval_const_expr, FlattenedModel, Flattener};
use crate::jit::{ArrayInfo, ArrayType};
use crate::loader::ModelLoader;

pub(super) type CompilerResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(super) struct FrontendStage {
    pub flat_model: FlattenedModel,
    pub total_equations: usize,
    pub total_declarations: usize,
}

pub(super) struct VariableLayout {
    pub states: Vec<f64>,
    pub discrete_vals: Vec<f64>,
    pub params: Vec<f64>,
    pub state_vars: Vec<String>,
    pub discrete_vars: Vec<String>,
    pub param_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub input_var_names: Vec<String>,
    pub state_var_index: HashMap<String, usize>,
    pub param_var_index: HashMap<String, usize>,
    pub output_var_index: HashMap<String, usize>,
    pub array_info: HashMap<String, ArrayInfo>,
}

pub(super) struct AnalysisStage {
    pub alg_equations: Vec<Equation>,
    pub diff_equations: Vec<Equation>,
    pub differential_index: u32,
    pub constraint_equation_count: usize,
    pub constant_conflict_count: usize,
    pub numeric_ode_jacobian: bool,
    pub symbolic_ode_jacobian_matrix: Option<Vec<Vec<Expression>>>,
    pub ode_jacobian_sparse: Option<jacobian::SparseOdeJacobian>,
}

pub(super) fn stage_trace_enabled() -> bool {
    std::env::var("RUSTMODLICA_STAGE_TRACE")
        .ok()
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

fn log_stage_timing(stage_trace: bool, stage: &str, started_at: Instant) {
    if stage_trace {
        eprintln!("[stage][timing] {} {} ms", stage, started_at.elapsed().as_millis());
    }
}

pub(super) fn flatten_and_inline(
    root_model: &mut Arc<Model>,
    model_name: &str,
    loader: &mut ModelLoader,
    quiet: bool,
    stage_trace: bool,
) -> CompilerResult<FrontendStage> {
    let started_at = Instant::now();
    if stage_trace {
        eprintln!("[stage] flatten");
    }
    let mut flattener = Flattener::new();
    for path in &loader.library_paths {
        flattener.loader.add_path(path.clone());
    }
    if let Some(p) = loader.get_path_for_model(model_name) {
        flattener.loader.register_path(model_name, p);
    }
    flattener.loader.set_quiet(quiet);
    let mut flat_model = flattener.flatten(root_model, model_name)?;
    log_stage_timing(stage_trace, "flatten", started_at);

    if stage_trace {
        eprintln!("[stage] inline");
    }
    let inline_started_at = Instant::now();
    crate::compiler::inline::inline_function_calls(&mut flat_model, loader);
    log_stage_timing(stage_trace, "inline", inline_started_at);

    Ok(FrontendStage {
        total_equations: flat_model.equations.len(),
        total_declarations: flat_model.declarations.len(),
        flat_model,
    })
}

pub(super) fn classify_variables(
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
            Expression::Variable(name) => {
                out.insert(name.clone());
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
            Expression::Call(_, args) | Expression::ArrayLiteral(args) => {
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
            | Expression::ShiftSample(c, n) => {
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

    let mut param_vars = Vec::new();
    let mut output_vars = Vec::new();
    let mut params = Vec::new();
    let mut states = Vec::new();
    let mut discrete_vals = Vec::new();

    for eq in &flat_model.equations {
        collect_states_from_eq(eq, &mut state_vars);
    }

    for decl in &flat_model.declarations {
        if decl.is_parameter {
            param_vars.push(decl.name.clone());
            params.push(
                decl.start_value
                    .as_ref()
                    .and_then(eval_const_expr)
                    .unwrap_or(0.0),
            );
        } else if decl.is_discrete || flat_model.clocked_var_names.contains(&decl.name) {
            discrete_vars.insert(decl.name.clone());
        }
    }
    let mut previous_vars = HashSet::new();
    for eq in &flat_model.equations {
        collect_previous_vars_eq(eq, &mut previous_vars);
    }
    for eq in &flat_model.initial_equations {
        collect_previous_vars_eq(eq, &mut previous_vars);
    }
    for name in previous_vars {
        discrete_vars.insert(name);
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

    let mut param_set: HashSet<String> = param_vars.iter().cloned().collect();
    for var in referenced_in_equations {
        if state_vars.contains(&var)
            || discrete_vars.contains(&var)
            || param_set.contains(&var)
            || var.starts_with("der_")
            || decl_index.contains_key(&var)
        {
            continue;
        }
        param_set.insert(var.clone());
        param_vars.push(var.clone());
        params.push(
            decl_index
                .get(&var)
                .and_then(|&idx| flat_model.declarations[idx].start_value.as_ref())
                .and_then(eval_const_expr)
                .unwrap_or(0.0),
        );
    }

    let mut state_vars_sorted: Vec<String> = state_vars.into_iter().collect();
    state_vars_sorted.sort();

    let mut discrete_vars_sorted: Vec<String> = discrete_vars.into_iter().collect();
    discrete_vars_sorted.sort();

    let state_set: HashSet<String> = state_vars_sorted.iter().cloned().collect();
    let discrete_set: HashSet<String> = discrete_vars_sorted.iter().cloned().collect();

    let start_value_for = |name: &str| -> f64 {
        decl_index
            .get(name)
            .and_then(|&idx| flat_model.declarations[idx].start_value.as_ref())
            .and_then(eval_const_expr)
            .unwrap_or(0.0)
    };

    for var in &state_vars_sorted {
        states.push(start_value_for(var));
    }
    for var in &discrete_vars_sorted {
        discrete_vals.push(start_value_for(var));
    }

    let mut output_var_index = HashMap::new();
    for decl in &flat_model.declarations {
        if decl.is_parameter || discrete_set.contains(&decl.name) || state_set.contains(&decl.name) {
            continue;
        }
        let idx = output_vars.len();
        output_var_index.insert(decl.name.clone(), idx);
        output_vars.push(decl.name.clone());
    }
    for var in &state_vars_sorted {
        let der_var = format!("der_{}", var);
        if !output_var_index.contains_key(&der_var) {
            let idx = output_vars.len();
            output_var_index.insert(der_var.clone(), idx);
            output_vars.push(der_var);
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
        input_var_names,
        state_var_index,
        param_var_index,
        output_var_index,
        array_info,
    }
}

pub(super) fn analyze_equations(
    flat_model: &FlattenedModel,
    layout: &mut VariableLayout,
    opts: &CompilerOptions,
    stage_trace: bool,
) -> AnalysisStage {
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
            Expression::Variable(n) => Some(n.clone()),
            Expression::ArrayAccess(base, _) => match &**base {
                Expression::Variable(n) => Some(n.clone()),
                _ => None,
            },
            Expression::Dot(base, _) => match &**base {
                Expression::Variable(n) => Some(n.clone()),
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
    for out_var in &layout.output_vars {
        if let Some(alias_expr) = sort_result.alias_map.get(out_var) {
            alg_equations.push(Equation::Simple(
                Expression::Variable(out_var.clone()),
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

    AnalysisStage {
        alg_equations,
        diff_equations,
        differential_index: sort_result.differential_index,
        constraint_equation_count: sort_result.constraint_equation_count,
        constant_conflict_count: sort_result.constant_conflict_count,
        numeric_ode_jacobian,
        symbolic_ode_jacobian_matrix,
        ode_jacobian_sparse,
    }
}

pub(super) fn build_runtime_algorithms(
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
            Expression::Variable(n) => Some(n.clone()),
            Expression::ArrayAccess(base, _) => match &**base {
                Expression::Variable(n) => Some(n.clone()),
                _ => None,
            },
            Expression::Dot(base, _) => match &**base {
                Expression::Variable(n) => Some(n.clone()),
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

pub(super) fn collect_newton_tearing_var_names(alg_equations: &[Equation]) -> Vec<String> {
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

fn normalize_equations(equations: &[Equation]) -> Vec<Equation> {
    equations.iter().map(normalize_equation).collect()
}

fn normalize_equation(eq: &Equation) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(normalize_der(lhs), normalize_der(rhs)),
        Equation::For(var, start, end, body) => Equation::For(
            var.clone(),
            start.clone(),
            end.clone(),
            body.iter().map(normalize_simple_equation).collect(),
        ),
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => Equation::If(
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
        ),
        _ => eq.clone(),
    }
}

fn normalize_simple_equation(eq: &Equation) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(normalize_der(lhs), normalize_der(rhs)),
        _ => eq.clone(),
    }
}

fn ensure_derivative_outputs(layout: &mut VariableLayout) {
    for var in &layout.state_vars {
        let der_var = format!("der_{}", var);
        if !layout.output_var_index.contains_key(&der_var) {
            let pos = layout.output_vars.len();
            layout.output_var_index.insert(der_var.clone(), pos);
            layout.output_vars.push(der_var);
        }
    }

    let mut derived_array_entries = Vec::new();
    for (name, info) in &layout.array_info {
        if !matches!(info.array_type, ArrayType::State) {
            continue;
        }
        let der_name = format!("der_{}", name);
        if layout.array_info.contains_key(&der_name) {
            continue;
        }
        let first_der = format!("der_{}_1", name);
        if let Some(&start_index) = layout.output_var_index.get(&first_der) {
            derived_array_entries.push((
                der_name,
                ArrayInfo {
                    array_type: ArrayType::Output,
                    start_index,
                    size: info.size,
                },
            ));
        }
    }
    for (name, info) in derived_array_entries {
        layout.array_info.insert(name, info);
    }
}

fn build_diff_equations(layout: &VariableLayout) -> Vec<Equation> {
    layout
        .state_vars
        .iter()
        .map(|var| {
            let rhs_expr = if let Some((base, idx)) = equation_convert::parse_array_index(var) {
                if layout.array_info.contains_key(&base)
                    && layout.array_info.contains_key(&format!("der_{}", base))
                {
                    Expression::ArrayAccess(
                        Box::new(Expression::Variable(format!("der_{}", base))),
                        Box::new(Expression::Number(idx as f64)),
                    )
                } else {
                    Expression::Variable(format!("der_{}", var))
                }
            } else {
                Expression::Variable(format!("der_{}", var))
            };

            Equation::Simple(
                Expression::Der(Box::new(Expression::Variable(var.clone()))),
                rhs_expr,
            )
        })
        .collect()
}
