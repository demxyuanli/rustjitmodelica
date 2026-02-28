
use std::collections::{HashSet, HashMap};
use crate::ast::{Equation, Expression, Operator};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::tarjan_scc;

// Re-export or define missing functions if they are not available
// Assuming these are private within analysis.rs, we need to make sure they exist.
// If they were removed or not public, we must restore them.

/// F2-1: Expand der(linear combination of variables) to linear combination of der(x).
/// Returns None if expr is not linear in variables (then caller keeps Der(inner)).
fn expand_der_linear(inner: &Expression) -> Option<Expression> {
    use Expression::*;
    match inner {
        Variable(name) => Some(Variable(format!("der_{}", name))),
        BinaryOp(l, Operator::Add, r) => {
            let dl = expand_der_linear(l)?;
            let dr = expand_der_linear(r)?;
            Some(BinaryOp(Box::new(dl), Operator::Add, Box::new(dr)))
        }
        BinaryOp(l, Operator::Sub, r) => {
            let dl = expand_der_linear(l)?;
            let dr = expand_der_linear(r)?;
            Some(BinaryOp(Box::new(dl), Operator::Sub, Box::new(dr)))
        }
        BinaryOp(l, Operator::Mul, r) => {
            match (&**l, &**r) {
                (Number(c), Variable(name)) => Some(BinaryOp(Box::new(Number(*c)), Operator::Mul, Box::new(Variable(format!("der_{}", name))))),
                (Variable(name), Number(c)) => Some(BinaryOp(Box::new(Number(*c)), Operator::Mul, Box::new(Variable(format!("der_{}", name))))),
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn normalize_der(expr: &Expression) -> Expression {
    match expr {
        Expression::Der(inner) => {
            if let Some(expanded) = expand_der_linear(inner) {
                normalize_der(&expanded)
            } else if let Expression::Variable(name) = &**inner {
                Expression::Variable(format!("der_{}", name))
            } else {
                Expression::Der(Box::new(normalize_der(inner)))
            }
        },
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(Box::new(normalize_der(lhs)), *op, Box::new(normalize_der(rhs))),
        Expression::Call(func, args) => Expression::Call(func.clone(), args.iter().map(|a| normalize_der(a)).collect()),
        Expression::ArrayAccess(arr, idx) => Expression::ArrayAccess(Box::new(normalize_der(arr)), Box::new(normalize_der(idx))),
        Expression::If(c, t, f) => Expression::If(Box::new(normalize_der(c)), Box::new(normalize_der(t)), Box::new(normalize_der(f))),
        _ => expr.clone()
    }
}

pub fn collect_states_from_eq(eq: &Equation, states: &mut HashSet<String>) {
    match eq {
        Equation::Simple(lhs, rhs) => {
            collect_states_from_expr(lhs, states);
            collect_states_from_expr(rhs, states);
        }
        Equation::For(_, s, e, body) => {
            collect_states_from_expr(s, states);
            collect_states_from_expr(e, states);
            for sub_eq in body {
                collect_states_from_eq(sub_eq, states);
            }
        }
        Equation::When(c, b, e) => {
            collect_states_from_expr(c, states);
            for sub_eq in b {
                collect_states_from_eq(sub_eq, states);
            }
            for (ec, eb) in e {
                collect_states_from_expr(ec, states);
                for sub_eq in eb {
                    collect_states_from_eq(sub_eq, states);
                }
            }
        }
        Equation::If(c, then_eqs, elseif_list, else_eqs) => {
            collect_states_from_expr(c, states);
            for eq in then_eqs {
                collect_states_from_eq(eq, states);
            }
            for (ec, eb) in elseif_list {
                collect_states_from_expr(ec, states);
                for eq in eb {
                    collect_states_from_eq(eq, states);
                }
            }
            if let Some(eqs) = else_eqs {
                for eq in eqs {
                    collect_states_from_eq(eq, states);
                }
            }
        }
        Equation::Assert(cond, msg) => {
            collect_states_from_expr(cond, states);
            collect_states_from_expr(msg, states);
        }
        Equation::Terminate(msg) => {
            collect_states_from_expr(msg, states);
        }
        _ => {}
    }
}

fn collect_vars_in_expr(expr: &Expression, out: &mut HashSet<String>) {
    match expr {
        Expression::Variable(name) => {
            out.insert(name.clone());
        }
        Expression::BinaryOp(lhs, _, rhs) => {
            collect_vars_in_expr(lhs, out);
            collect_vars_in_expr(rhs, out);
        }
        Expression::Call(_, args) => {
            for a in args {
                collect_vars_in_expr(a, out);
            }
        }
        Expression::If(c, t, f) => {
            collect_vars_in_expr(c, out);
            collect_vars_in_expr(t, out);
            collect_vars_in_expr(f, out);
        }
        Expression::Der(inner) => collect_vars_in_expr(inner, out),
        _ => {}
    }
}

fn collect_states_from_expr(expr: &Expression, states: &mut HashSet<String>) {
    match expr {
        Expression::Der(inner) => {
            collect_vars_in_expr(inner, states);
        }
        Expression::BinaryOp(lhs, _, rhs) => {
            collect_states_from_expr(lhs, states);
            collect_states_from_expr(rhs, states);
        }
        Expression::Call(_, args) => {
            for arg in args {
                collect_states_from_expr(arg, states);
            }
        }
        Expression::If(c, t, f) => {
            collect_states_from_expr(c, states);
            collect_states_from_expr(t, states);
            collect_states_from_expr(f, states);
        }
        _ => {}
    }
}

#[derive(Clone, Default)]
pub struct AnalysisOptions {
    #[allow(dead_code)]
    pub index_reduction_method: String,
    pub tearing_method: String,
}

/// Select tearing variable from block unknowns by method. OMC-style: first, maxEquation (most equations), minCellier (heuristic).
fn select_tearing_variable(
    block_unknowns: &[String],
    block_eqs: &[Equation],
    _unknown_map: &HashMap<String, usize>,
    method: &str,
) -> Option<String> {
    if block_unknowns.is_empty() {
        return None;
    }
    match method {
        "maxEquation" => {
            let mut best = block_unknowns[0].clone();
            let mut best_count = 0usize;
            for u in block_unknowns {
                let count = block_eqs.iter().filter(|eq| equation_contains_var(eq, u)).count();
                if count > best_count {
                    best_count = count;
                    best = u.clone();
                }
            }
            Some(best)
        }
        "minCellier" => {
            let mut best = block_unknowns[0].clone();
            let mut best_score = usize::MAX;
            for u in block_unknowns {
                let count = block_eqs.iter().filter(|eq| equation_contains_var(eq, u)).count();
                if count < best_score {
                    best_score = count;
                    best = u.clone();
                }
            }
            Some(best)
        }
        _ => block_unknowns.first().cloned(),
    }
}

fn equation_contains_var(eq: &Equation, var: &str) -> bool {
    let mut vars = HashSet::new();
    collect_vars_eq(eq, &mut vars);
    vars.contains(var)
}

/// Result of initial equation system analysis (IR3-3). Used for consistent initialization.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InitialSystemInfo {
    pub equation_count: usize,
    pub variable_count: usize,
    pub alias_eliminated_count: usize,
    pub is_underdetermined: bool,
    pub is_overdetermined: bool,
}

/// Analyze initial equations: count equations and variables, run alias elimination, detect over/under-determination.
/// known_at_initial: typically parameters and time (t=0). Variables not in this set are unknowns.
pub fn analyze_initial_equations(
    initial_equations: &[Equation],
    known_at_initial: &HashSet<String>,
) -> InitialSystemInfo {
    let (eqs_after_alias, alias_map) = eliminate_aliases(initial_equations);
    let mut var_set = HashSet::new();
    for eq in &eqs_after_alias {
        collect_vars_eq(eq, &mut var_set);
    }
    for v in alias_map.keys() {
        var_set.insert(v.clone());
    }
    let unknown_set: HashSet<String> = var_set
        .difference(known_at_initial)
        .cloned()
        .collect();
    let variable_count = unknown_set.len();
    let equation_count = eqs_after_alias.len();
    let is_underdetermined = equation_count < variable_count;
    let is_overdetermined = equation_count > variable_count;
    InitialSystemInfo {
        equation_count,
        variable_count,
        alias_eliminated_count: alias_map.len(),
        is_underdetermined,
        is_overdetermined,
    }
}

/// Result of sort_algebraic_equations: sorted equations, differential index, and constraint count (IR3-1, IR3-2).
#[derive(Debug, Clone)]
pub struct SortAlgebraicResult {
    pub sorted_equations: Vec<Equation>,
    pub differential_index: u32,
    /// Number of equations not assigned to any variable (constraint equations) when index > 1.
    pub constraint_equation_count: usize,
}

/// Sorts algebraic equations using variable-equation bipartite matching (IR2-1) and
/// Block Lower Triangular (BLT) ordering (IR2-2). Returns sorted equations,
/// structural differential index, and constraint equation count.
pub fn sort_algebraic_equations(
    equations: &[Equation],
    known_vars: &HashSet<String>,
    params: &[String],
    options: &AnalysisOptions,
) -> SortAlgebraicResult {
    // 0. Prepare known variables set (including parameters and time)
    let mut current_known = known_vars.clone();
    for p in params {
        current_known.insert(p.clone());
    }
    current_known.insert("time".to_string());

    // 0.5. Eliminate Aliases (Simple equations like a = b)
    let (equations, alias_map) = eliminate_aliases(&equations);
    
    println!("{}", crate::i18n::msg("aliases_eliminated", &[&alias_map.len()]));
    println!("{}", crate::i18n::msg("remaining_equations", &[&equations.len()]));
    // for (i, eq) in equations.iter().enumerate() {
    //     println!("    [{}]: {:?}", i, eq);
    // }

    // 1. Mutable equations for index reduction loop
    let mut equations = equations;

    let mut state_set = HashSet::new();
    for eq in equations.iter() {
        collect_states_from_eq(eq, &mut state_set);
    }
    let state_vars: Vec<String> = state_set.into_iter().collect();
    if options.index_reduction_method == "debugPrint" && !state_vars.is_empty() {
        for eq in equations.iter() {
            if let Equation::Simple(lhs, rhs) = eq {
                let is_ode = matches!(lhs, Expression::Der(_));
                if !is_ode {
                    let residual = Expression::BinaryOp(
                        Box::new(lhs.clone()),
                        Operator::Sub,
                        Box::new(rhs.clone()),
                    );
                    let dt = time_derivative(&residual, &state_vars);
                    eprintln!("[debugPrint] time_derivative of constraint residual: {:?}", dt);
                    break;
                }
            }
        }
    }

    struct EqInfo {
        original_idx: usize,
        unknowns: Vec<String>,
    }

    fn dfs_iter(
        u_start: usize,
        adj: &[Vec<usize>],
        assigned_eq: &mut [Option<usize>],
        visited: &mut [bool],
    ) -> bool {
        let mut stack: Vec<(usize, usize, Option<usize>)> = vec![(u_start, 0, None)];
        while let Some((u, idx, var_opt)) = stack.last_mut() {
            if *idx >= adj[*u].len() {
                if let Some(v) = *var_opt {
                    visited[v] = false;
                }
                stack.pop();
                continue;
            }
            let v = adj[*u][*idx];
            *idx += 1;
            if visited[v] {
                continue;
            }
            visited[v] = true;
            if assigned_eq[v].is_none() {
                assigned_eq[v] = Some(*u);
                while let Some((eq, _, pop_var)) = stack.pop() {
                    if let Some(var) = pop_var {
                        assigned_eq[var] = Some(eq);
                    }
                }
                return true;
            }
            let next_eq = assigned_eq[v].unwrap();
            stack.push((next_eq, 0, Some(v)));
        }
        false
    }

    let mut differential_index: u32;
    let mut eq_infos = Vec::new();
    let mut unknown_list: Vec<String>;
    let mut unknown_map: HashMap<String, usize>;
    let mut assigned_var: Vec<Option<usize>>;
    let mut assigned_eq: Vec<Option<usize>>;

    loop {
        eq_infos.clear();
        let mut all_unknowns = HashSet::new();
        for (i, eq) in equations.iter().enumerate() {
            let unknowns = extract_unknowns(eq, &current_known);
            for u in &unknowns {
                all_unknowns.insert(u.clone());
            }
            eq_infos.push(EqInfo {
                original_idx: i,
                unknowns,
            });
        }

        unknown_list = all_unknowns.into_iter().collect();
        unknown_map = unknown_list.iter().enumerate().map(|(i, u)| (u.clone(), i)).collect();

        assigned_var = vec![None; eq_infos.len()];
        assigned_eq = vec![None; unknown_list.len()];

        for (i, info) in eq_infos.iter().enumerate() {
            let eq = &equations[info.original_idx];
            if let Equation::Simple(lhs, _) = eq {
                let mut target_var = None;
                if let Expression::Variable(v) = lhs {
                    target_var = Some(v.clone());
                } else if let Expression::Der(inner) = lhs {
                    if let Expression::Variable(v) = &**inner {
                        target_var = Some(format!("der_{}", v));
                    }
                }
                if let Some(v) = target_var {
                    if let Some(&v_idx) = unknown_map.get(&v) {
                        if assigned_eq[v_idx].is_none() {
                            assigned_eq[v_idx] = Some(i);
                            assigned_var[i] = Some(v_idx);
                        }
                    }
                }
            }
        }

        let mut adj = vec![Vec::new(); eq_infos.len()];
        for (i, info) in eq_infos.iter().enumerate() {
            for u in &info.unknowns {
                if let Some(&v_idx) = unknown_map.get(u) {
                    adj[i].push(v_idx);
                }
            }
        }

        for i in 0..eq_infos.len() {
            if assigned_var[i].is_some() {
                continue;
            }
            let mut visited = vec![false; unknown_list.len()];
            if dfs_iter(i, &adj, &mut assigned_eq, &mut visited) {
                // assigned_eq updated
            }
        }

        for (v_idx, eq_opt) in assigned_eq.iter().enumerate() {
            if let Some(eq_idx) = eq_opt {
                assigned_var[*eq_idx] = Some(v_idx);
            }
        }

        differential_index = if assigned_var.iter().any(|o| o.is_none()) { 2 } else { 1 };
        if differential_index == 1 {
            break;
        }
        if options.index_reduction_method == "none" {
            break;
        }
        if let Some(new_eqs) = try_index_reduction(
            &equations,
            &assigned_var,
            &assigned_eq,
            &unknown_list,
            &state_vars,
        ) {
            equations = new_eqs;
        } else {
            break;
        }
    }

    let constraint_equation_count = assigned_var.iter().filter(|o| o.is_none()).count();

    // 4. Build Dependency Graph
    let mut dep_graph = DiGraph::<usize, ()>::new();
    let node_indices: Vec<NodeIndex> = (0..eq_infos.len()).map(|i| dep_graph.add_node(i)).collect();

    for (i, info) in eq_infos.iter().enumerate() {
        for u in &info.unknowns {
            if let Some(&v_idx) = unknown_map.get(u) {
                if Some(v_idx) == assigned_var[i] {
                    continue;
                }
                if let Some(solver_eq_idx) = assigned_eq[v_idx] {
                    dep_graph.add_edge(node_indices[i], node_indices[solver_eq_idx], ());
                }
            }
        }
    }

    // 5. Find SCCs
    let sccs = tarjan_scc(&dep_graph);

    let mut sorted_equations = Vec::new();

    for scc in sccs {
        let block_indices: Vec<usize> = scc.iter().map(|n| dep_graph[*n]).collect();
        
        if block_indices.is_empty() { continue; }

        if block_indices.len() == 1 {
            let idx = block_indices[0];
            let eq = &equations[eq_infos[idx].original_idx];
            
            if let Some(var_idx) = assigned_var[idx] {
                let var_name = &unknown_list[var_idx];
                if let Some(expr) = solve_for_variable(eq, var_name) {
                    current_known.insert(var_name.clone());
                    sorted_equations.push(Equation::Simple(
                        Expression::Variable(var_name.clone()),
                        expr
                    ));
                } else {
                    let tearing_var = select_tearing_variable(
                        &[var_name.clone()],
                        &[eq.clone()],
                        &unknown_map,
                        &options.tearing_method,
                    );
                    sorted_equations.push(Equation::SolvableBlock {
                        unknowns: vec![var_name.clone()],
                        tearing_var,
                        equations: vec![],
                        residuals: vec![make_residual(eq)]
                    });
                    current_known.insert(var_name.clone());
                }
            } else {
                 // Preserve structural equations (e.g. If) that have no single assigned variable
                 sorted_equations.push(eq.clone());
            }
        } else {
            let block_eqs: Vec<Equation> = block_indices.iter()
                .map(|&idx| equations[eq_infos[idx].original_idx].clone())
                .collect();

            let block_unknowns: Vec<String> = block_indices.iter()
                .filter_map(|&idx| assigned_var[idx].map(|v_idx| unknown_list[v_idx].clone()))
                .collect();

            let tearing_var = select_tearing_variable(
                &block_unknowns,
                &block_eqs,
                &unknown_map,
                &options.tearing_method,
            );

            sorted_equations.push(Equation::SolvableBlock {
                unknowns: block_unknowns.clone(),
                tearing_var,
                equations: vec![],
                residuals: block_eqs.iter().map(|eq| make_residual(eq)).collect()
            });

            for u in block_unknowns {
                current_known.insert(u);
            }
        }
    }

    SortAlgebraicResult {
        sorted_equations,
        differential_index,
        constraint_equation_count,
    }
}

fn extract_unknowns(eq: &Equation, knowns: &HashSet<String>) -> Vec<String> {
    let mut vars = HashSet::new();
    collect_vars_eq(eq, &mut vars);
    let mut unknowns = Vec::new();
    for v in vars {
        if !knowns.contains(&v) {
             unknowns.push(v);
        }
    }
    unknowns
}

fn collect_vars_eq(eq: &Equation, vars: &mut HashSet<String>) {
    match eq {
        Equation::Simple(l, r) => { collect_vars_expr(l, vars); collect_vars_expr(r, vars); }
        Equation::For(_, s, e, b) => { 
             collect_vars_expr(s, vars); collect_vars_expr(e, vars);
             for sub in b { collect_vars_eq(sub, vars); }
        }
        Equation::When(c, b, e) => {
             collect_vars_expr(c, vars);
             for sub in b { collect_vars_eq(sub, vars); }
             for (ec, eb) in e {
                 collect_vars_expr(ec, vars);
                 for sub in eb { collect_vars_eq(sub, vars); }
             }
         }
        Equation::If(c, then_eqs, elseif_list, else_eqs) => {
             collect_vars_expr(c, vars);
             for sub in then_eqs { collect_vars_eq(sub, vars); }
             for (ec, eb) in elseif_list {
                 collect_vars_expr(ec, vars);
                 for sub in eb { collect_vars_eq(sub, vars); }
             }
             if let Some(eqs) = else_eqs { for sub in eqs { collect_vars_eq(sub, vars); } }
         }
        Equation::Assert(cond, msg) => { collect_vars_expr(cond, vars); collect_vars_expr(msg, vars); }
        Equation::Terminate(msg) => { collect_vars_expr(msg, vars); }
        _ => {}
    }
}

fn collect_vars_expr(expr: &Expression, vars: &mut HashSet<String>) {
    match expr {
        Expression::Variable(n) => { vars.insert(n.clone()); }
        Expression::Der(e) => { 
             // Treat der(x) as variable "der_x" if normalized, or just recurse?
             // If normalized, it should be Variable("der_x").
             // If not, we might see Der(Variable("x")).
             if let Expression::Variable(n) = &**e {
                 vars.insert(format!("der_{}", n));
             } else {
                 collect_vars_expr(e, vars);
             }
        }
        Expression::BinaryOp(l, _, r) => { collect_vars_expr(l, vars); collect_vars_expr(r, vars); }
        Expression::Call(_, args) => { for a in args { collect_vars_expr(a, vars); } }
        Expression::ArrayAccess(a, i) => { collect_vars_expr(a, vars); collect_vars_expr(i, vars); }
        Expression::If(c, t, f) => { collect_vars_expr(c, vars); collect_vars_expr(t, vars); collect_vars_expr(f, vars); }
        Expression::ArrayLiteral(es) => { for e in es { collect_vars_expr(e, vars); } }
        Expression::Dot(b, _) => { collect_vars_expr(b, vars); } // Simplification
        _ => {}
    }
}

fn solve_for_variable(eq: &Equation, var: &str) -> Option<Expression> {
    if let Equation::Simple(lhs, rhs) = eq {
        // Case: var = expr
        if let Expression::Variable(v) = lhs {
            if v == var && !contains_var(rhs, var) {
                return Some(rhs.clone());
            }
        }
        // Case: expr = var
        if let Expression::Variable(v) = rhs {
            if v == var && !contains_var(lhs, var) {
                return Some(lhs.clone());
            }
        }
        // Case: var + a = b => var = b - a
        // Case: a * var = b => var = b / a
        // (Simple linear inversion)
        // TODO: Implement more robust solver
    }
    None
}

fn make_residual(eq: &Equation) -> Expression {
    match eq {
        Equation::Simple(lhs, rhs) => make_binary(lhs.clone(), Operator::Sub, rhs.clone()),
        _ => make_num(0.0) // Placeholder
    }
}

/// Substitute der_x -> expr in expression using the given map (from ODE equations).
fn substitute_der_in_expr(expr: &Expression, der_map: &HashMap<String, Expression>) -> Expression {
    match expr {
        Expression::Variable(name) => {
            if name.starts_with("der_") {
                der_map.get(name).cloned().unwrap_or_else(|| expr.clone())
            } else {
                expr.clone()
            }
        }
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
            Box::new(substitute_der_in_expr(lhs, der_map)),
            *op,
            Box::new(substitute_der_in_expr(rhs, der_map)),
        ),
        Expression::Call(n, args) => Expression::Call(
            n.clone(),
            args.iter().map(|a| substitute_der_in_expr(a, der_map)).collect(),
        ),
        Expression::Der(inner) => Expression::Der(Box::new(substitute_der_in_expr(inner, der_map))),
        Expression::If(c, t, f) => Expression::If(
            Box::new(substitute_der_in_expr(c, der_map)),
            Box::new(substitute_der_in_expr(t, der_map)),
            Box::new(substitute_der_in_expr(f, der_map)),
        ),
        _ => expr.clone(),
    }
}

/// Solve residual expr = 0 for var when expr is linear in var: expr = rest - coeff*var => var = rest/coeff.
fn solve_residual_linear(expr: &Expression, var: &str) -> Option<Expression> {
    if !contains_var(expr, var) {
        return None;
    }
    if let Expression::BinaryOp(lhs, op, rhs) = expr {
        let (rest, coeff) = match (op, lhs.as_ref(), rhs.as_ref()) {
            (Operator::Sub, rest, Expression::BinaryOp(mul_l, Operator::Mul, mul_r)) => {
                let coeff = if let Expression::Variable(vn) = mul_r.as_ref() {
                    if vn == var && !contains_var(rest, var) && !contains_var(mul_l, var) {
                        mul_l.clone()
                    } else if let Expression::Variable(vn) = mul_l.as_ref() {
                        if vn == var && !contains_var(rest, var) && !contains_var(mul_r, var) {
                            mul_r.clone()
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                };
                (rest.clone(), coeff)
            }
            (Operator::Sub, Expression::BinaryOp(mul_l, Operator::Mul, mul_r), rest) => {
                let coeff = if let Expression::Variable(vn) = mul_r.as_ref() {
                    if vn == var && !contains_var(rest, var) && !contains_var(mul_l, var) {
                        mul_l.clone()
                    } else if let Expression::Variable(vn) = mul_l.as_ref() {
                        if vn == var && !contains_var(rest, var) && !contains_var(mul_r, var) {
                            mul_r.clone()
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                };
                (rest.clone(), coeff)
            }
            _ => return None,
        };
        Some(make_binary(rest, Operator::Div, *coeff))
    } else {
        None
    }
}

/// Try to reduce differential index by differentiating constraint equations and solving for algebraic variables.
/// Returns new equation list if one constraint was replaced by an explicit equation for an algebraic.
fn try_index_reduction(
    equations: &[Equation],
    assigned_var: &[Option<usize>],
    _assigned_eq: &[Option<usize>],
    unknown_list: &[String],
    state_vars: &[String],
) -> Option<Vec<Equation>> {
    let mut der_map: HashMap<String, Expression> = HashMap::new();
    for (eq_idx, &var_idx_opt) in assigned_var.iter().enumerate() {
        if let Some(var_idx) = var_idx_opt {
            let var_name = &unknown_list[var_idx];
            if var_name.starts_with("der_") {
                let eq = &equations[eq_idx];
                if let Equation::Simple(Expression::Variable(lhs), rhs) = eq {
                    if *lhs == *var_name {
                        der_map.insert(var_name.clone(), rhs.clone());
                    }
                } else if let Equation::Simple(Expression::Der(inner), rhs) = eq {
                    if let Expression::Variable(v) = inner.as_ref() {
                        let der_name = format!("der_{}", v);
                        if der_name == *var_name {
                            der_map.insert(var_name.clone(), rhs.clone());
                        }
                    }
                }
            }
        }
    }
    let unassigned: Vec<usize> = assigned_var
        .iter()
        .enumerate()
        .filter_map(|(i, o)| if o.is_none() { Some(i) } else { None })
        .collect();
    for eq_idx in unassigned {
        let eq = &equations[eq_idx];
        let (is_constraint, residual) = match eq {
            Equation::Simple(lhs, rhs) => {
                let lhs_has_der = matches!(lhs, Expression::Der(_))
                    || (if let Expression::Variable(n) = lhs { n.starts_with("der_") } else { false });
                (!lhs_has_der, make_binary(lhs.clone(), Operator::Sub, rhs.clone()))
            }
            _ => continue,
        };
        if !is_constraint {
            continue;
        }
        let mut diff_expr = time_derivative(&residual, state_vars);
        diff_expr = substitute_der_in_expr(&diff_expr, &der_map);
        let alg_vars: Vec<&String> = unknown_list
            .iter()
            .filter(|u| !u.starts_with("der_"))
            .collect();
        if let Some(alg_var) = alg_vars.first() {
            if let Some(sol) = solve_residual_linear(&diff_expr, alg_var) {
                let mut new_eqs = equations.to_vec();
                new_eqs[eq_idx] =
                    Equation::Simple(Expression::Variable((*alg_var).clone()), sol);
                return Some(new_eqs);
            }
        }
        let diff2 = time_derivative(&diff_expr, state_vars);
        let diff2_sub = substitute_der_in_expr(&diff2, &der_map);
        for alg_var in &alg_vars {
            if let Some(sol) = solve_residual_linear(&diff2_sub, alg_var) {
                let mut new_eqs = equations.to_vec();
                new_eqs[eq_idx] =
                    Equation::Simple(Expression::Variable((*alg_var).clone()), sol);
                return Some(new_eqs);
            }
        }
    }
    None
}


// Alias Elimination
// Simplifies the system by removing equations of the form:
//  a = b
//  a = -b
//  a = constant
// And substituting 'a' with the RHS in all other equations.

fn eliminate_aliases(equations: &[Equation]) -> (Vec<Equation>, HashMap<String, Expression>) {
    let mut alias_map: HashMap<String, Expression> = HashMap::new();
    let mut current_eqs = equations.to_vec();
    let mut changed = true;
    
    // Iteratively find and substitute aliases until no more changes
    while changed {
        changed = false;
        let mut next_eqs = Vec::with_capacity(current_eqs.len());
        
        // 1. Find new aliases in this pass
        for eq in &current_eqs {
            let mut is_alias = false;
            
            if let Equation::Simple(lhs, rhs) = eq {
                // Check for: Variable = Expression
                if let Expression::Variable(v) = lhs {
                    // Avoid circular or trivial aliases
                    if *lhs != *rhs && !contains_var(rhs, v) {
                        // Heuristic: Prefer to keep variables that look like "der_x" or user variables, 
                        // and eliminate intermediate connection variables if possible. 
                        
                        // IMPORTANT: Do NOT eliminate derivatives! They are needed for JIT state update.
                        if !v.starts_with("der_") {
                            // Check if 'v' is already aliased? (Shouldn't be if we substitute eagerly)
                            if !alias_map.contains_key(v) {
                                 // Debug
                                 // println!("Eliminating alias: {} = {:?}", v, rhs);
                                alias_map.insert(v.clone(), rhs.clone());
                                changed = true;
                                is_alias = true;
                            }
                        }
                    }
                } 
                
                // Check for: Expression = Variable (flip)  e.g. der_h = v => would alias v -> der_h
                if !is_alias {
                    if let Expression::Variable(v) = rhs {
                        if *lhs != *rhs && !contains_var(lhs, v) {
                            // Do not eliminate if RHS is derivative
                            if !v.starts_with("der_") {
                                // Do not create alias when LHS is a derivative (e.g. der_h = v).
                                // State v and derivative der_h are different; removing der_h = v would lose the ODE.
                                let lhs_is_der = if let Expression::Variable(l) = lhs {
                                    l.starts_with("der_")
                                } else {
                                    false
                                };
                                if !lhs_is_der && !alias_map.contains_key(v) {
                                    alias_map.insert(v.clone(), lhs.clone());
                                    changed = true;
                                    is_alias = true;
                                }
                            }
                        }
                    }
                }
                
                // Check for: -a = b  => a = -b
                if !is_alias {
                    if let Expression::BinaryOp(l, Operator::Sub, r) = lhs {
                        if let Expression::Number(n) = &**l {
                            if n.abs() < 1e-10 {
                                 if let Expression::Variable(v) = &**r {
                                     // -v = rhs => v = -rhs
                                     if !alias_map.contains_key(v) && !contains_var(rhs, v) {
                                         // Don't eliminate der vars here either (though unlikely to be -der)
                                         if !v.starts_with("der_") {
                                             let neg_rhs = make_mul(make_num(-1.0), rhs.clone());
                                             alias_map.insert(v.clone(), neg_rhs);
                                             changed = true;
                                             is_alias = true;
                                         }
                                     }
                                 }
                            }
                        }
                    }
                }
            }
            
            if !is_alias {
                next_eqs.push(eq.clone());
            }
        }
        
        // 2. Substitute aliases into remaining equations
        if changed {
            // Update the alias map itself (chain substitutions: a=b, b=c => a=c)
            // If map is {A:B, B:C}, one pass gives {A:C, B:C}.
            // If {A:B, B:C, C:D}, one pass gives {A:C, B:D, C:D}.
            // A still points to C (which points to D).
            // So we need multiple passes if chains are long?
            // Or resolve fully in substitute?
            // substitute only does one level lookup!
            // "if let Some(val) = map.get(name) { val.clone() }"
            
            // Fix: substitute should recurse on the result!
            // But we need to avoid infinite recursion.
            
            // Better approach: Topological sort or loop until stable.
             // Given small size, loop 10 times?
            let mut new_alias_map = alias_map.clone();
            for _ in 0..10 {
                let mut map_changed = false;
                let keys: Vec<String> = new_alias_map.keys().cloned().collect();
                for k in keys {
                     let val = new_alias_map[&k].clone();
                     let new_val = substitute_aliases_in_expr(&val, &new_alias_map);
                    if val != new_val {
                        new_alias_map.insert(k, new_val);
                        map_changed = true;
                    }
                }
                if !map_changed {
                    break;
                }
            }
            alias_map = new_alias_map;

            let mut substituted_eqs = Vec::new();
            for eq in next_eqs {
                let new_eq = substitute_aliases_in_eq(&eq, &alias_map);
                substituted_eqs.push(new_eq);
            }
            current_eqs = substituted_eqs;
        } else {
            current_eqs = next_eqs;
        }
    }
    
    (current_eqs, alias_map)
}

fn substitute_aliases_in_eq(eq: &Equation, map: &HashMap<String, Expression>) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(
            substitute_aliases_in_expr(lhs, map),
            substitute_aliases_in_expr(rhs, map)
        ),
        Equation::For(v, s, e, body) => Equation::For(
            v.clone(),
            Box::new(substitute_aliases_in_expr(s, map)),
            Box::new(substitute_aliases_in_expr(e, map)),
            body.iter().map(|b_eq| substitute_aliases_in_eq(b_eq, map)).collect()
        ),
        Equation::When(cond, body, else_whens) => Equation::When(
            substitute_aliases_in_expr(cond, map),
            body.iter().map(|b_eq| substitute_aliases_in_eq(b_eq, map)).collect(),
            else_whens.iter().map(|(ec, eb)| (
                substitute_aliases_in_expr(ec, map),
                eb.iter().map(|b_eq| substitute_aliases_in_eq(b_eq, map)).collect(),
            )).collect(),
        ),
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => Equation::If(
            substitute_aliases_in_expr(cond, map),
            then_eqs.iter().map(|e| substitute_aliases_in_eq(e, map)).collect(),
            elseif_list.iter().map(|(c, eb)| (
                substitute_aliases_in_expr(c, map),
                eb.iter().map(|e| substitute_aliases_in_eq(e, map)).collect(),
            )).collect(),
            else_eqs.as_ref().map(|eqs| eqs.iter().map(|e| substitute_aliases_in_eq(e, map)).collect()),
        ),
        Equation::Connect(_, _) | Equation::Reinit(_, _) | Equation::Assert(_, _) | Equation::Terminate(_) | Equation::SolvableBlock { .. } => eq.clone(),
    }
}

fn substitute_aliases_in_expr(expr: &Expression, map: &HashMap<String, Expression>) -> Expression {
    match expr {
        Expression::Variable(name) => {
            if let Some(val) = map.get(name) {
                // Avoid infinite recursion if map has cycles (though construction tries to prevent it)
                // Ideally, map should be fully resolved. 
                // For safety, we could limit depth, but here we assume map is DAG.
                val.clone() 
            } else {
                expr.clone()
            }
        }
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
            Box::new(substitute_aliases_in_expr(lhs, map)),
            *op,
            Box::new(substitute_aliases_in_expr(rhs, map))
        ),
        Expression::Call(name, args) => Expression::Call(
            name.clone(),
            args.iter().map(|a| substitute_aliases_in_expr(a, map)).collect()
        ),
        Expression::Der(arg) => Expression::Der(
             Box::new(substitute_aliases_in_expr(arg, map))
        ),
        Expression::ArrayAccess(arr, idx) => Expression::ArrayAccess(
            Box::new(substitute_aliases_in_expr(arr, map)),
            Box::new(substitute_aliases_in_expr(idx, map))
        ),
        Expression::If(c, t, f) => Expression::If(
            Box::new(substitute_aliases_in_expr(c, map)),
            Box::new(substitute_aliases_in_expr(t, map)),
            Box::new(substitute_aliases_in_expr(f, map))
        ),
        Expression::ArrayLiteral(es) => Expression::ArrayLiteral(
            es.iter().map(|e| substitute_aliases_in_expr(e, map)).collect()
        ),
        Expression::Dot(base, member) => Expression::Dot(
            Box::new(substitute_aliases_in_expr(base, map)),
            member.clone()
        ),
        Expression::Range(start, step, end) => Expression::Range(
            Box::new(substitute_aliases_in_expr(start, map)),
            Box::new(substitute_aliases_in_expr(step, map)),
            Box::new(substitute_aliases_in_expr(end, map))
        ),
        _ => expr.clone()
    }
}

fn contains_var(expr: &Expression, var_name: &str) -> bool {
    match expr {
        Expression::Variable(name) => name == var_name,
        Expression::BinaryOp(lhs, _, rhs) => contains_var(lhs, var_name) || contains_var(rhs, var_name),
        Expression::Call(_, args) => args.iter().any(|arg| contains_var(arg, var_name)),
        Expression::Der(arg) => contains_var(arg, var_name),
        Expression::ArrayAccess(arr, idx) => contains_var(arr, var_name) || contains_var(idx, var_name),
        Expression::Dot(base, _) => contains_var(base, var_name),
        Expression::If(c, t, f) => contains_var(c, var_name) || contains_var(t, var_name) || contains_var(f, var_name),
        Expression::ArrayLiteral(es) => es.iter().any(|e| contains_var(e, var_name)),
        Expression::Range(start, step, end) => contains_var(start, var_name) || contains_var(step, var_name) || contains_var(end, var_name),
        _ => false,
    }
}

pub fn make_num(n: f64) -> Expression {
    Expression::Number(n)
}

pub fn make_mul(lhs: Expression, rhs: Expression) -> Expression {
    Expression::BinaryOp(Box::new(lhs), Operator::Mul, Box::new(rhs))
}

#[allow(dead_code)]
pub fn make_div(lhs: Expression, rhs: Expression) -> Expression {
    Expression::BinaryOp(Box::new(lhs), Operator::Div, Box::new(rhs))
}

#[allow(dead_code)]
pub fn make_add(lhs: Expression, rhs: Expression) -> Expression {
    Expression::BinaryOp(Box::new(lhs), Operator::Add, Box::new(rhs))
}

pub fn make_binary(lhs: Expression, op: Operator, rhs: Expression) -> Expression {
    Expression::BinaryOp(Box::new(lhs), op, Box::new(rhs))
}

/// Partial derivative of expression w.r.t. variable (symbolic). Used for Jacobian and index reduction.
pub fn partial_derivative(expr: &Expression, var: &str) -> Expression {
    use crate::ast::Operator;
    match expr {
        Expression::Variable(name) => {
            if name == var {
                Expression::Number(1.0)
            } else {
                Expression::Number(0.0)
            }
        }
        Expression::Number(_) => Expression::Number(0.0),
        Expression::BinaryOp(lhs, op, rhs) => {
            let dl = partial_derivative(lhs, var);
            let dr = partial_derivative(rhs, var);
            match op {
                Operator::Add | Operator::Sub => {
                    let r = if *op == Operator::Add { Operator::Add } else { Operator::Sub };
                    Expression::BinaryOp(Box::new(dl), r, Box::new(dr))
                }
                Operator::Mul => {
                    let term1 = Expression::BinaryOp(Box::new(dl.clone()), Operator::Mul, rhs.clone());
                    let term2 = Expression::BinaryOp(Box::new((**lhs).clone()), Operator::Mul, Box::new(dr));
                    Expression::BinaryOp(Box::new(term1), Operator::Add, Box::new(term2))
                }
                Operator::Div => {
                    let num = Expression::BinaryOp(
                        Box::new(Expression::BinaryOp(Box::new(dl.clone()), Operator::Mul, rhs.clone())),
                        Operator::Sub,
                        Box::new(Expression::BinaryOp(Box::new((**lhs).clone()), Operator::Mul, Box::new(dr.clone()))),
                    );
                    let r = (**rhs).clone();
                    let den = Expression::BinaryOp(Box::new(r.clone()), Operator::Mul, Box::new(r));
                    Expression::BinaryOp(Box::new(num), Operator::Div, Box::new(den))
                }
                _ => Expression::Number(0.0),
            }
        }
        Expression::Der(inner) => {
            if contains_var(inner, var) {
                Expression::Der(Box::new(partial_derivative(inner, var)))
            } else {
                Expression::Number(0.0)
            }
        }
        Expression::Call(_, _) | Expression::If(_, _, _) | Expression::ArrayAccess(_, _)
        | Expression::Dot(_, _) | Expression::Range(_, _, _) | Expression::ArrayLiteral(_) => Expression::Number(0.0),
    }
}

/// Time derivative of expression using chain rule: d/dt expr = sum over state x of (d expr/d x * der(x)).
/// Used for index reduction (Pantelides) when differentiating constraint equations.
pub fn time_derivative(expr: &Expression, state_vars: &[String]) -> Expression {
    let mut sum: Option<Expression> = None;
    for x in state_vars {
        let pd = partial_derivative(expr, x);
        if let Expression::Number(n) = &pd {
            if n.abs() < 1e-15 {
                continue;
            }
        }
        let der_x = Expression::Variable(format!("der_{}", x));
        let term = Expression::BinaryOp(Box::new(pd), Operator::Mul, Box::new(der_x));
        sum = Some(match sum {
            None => term,
            Some(s) => Expression::BinaryOp(Box::new(s), Operator::Add, Box::new(term)),
        });
    }
    sum.unwrap_or_else(|| Expression::Number(0.0))
}
