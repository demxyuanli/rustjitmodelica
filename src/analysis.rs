
use std::collections::{HashSet, HashMap};
use crate::ast::{Equation, Expression, Operator};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::tarjan_scc;

// Re-export or define missing functions if they are not available
// Assuming these are private within analysis.rs, we need to make sure they exist.
// If they were removed or not public, we must restore them.

pub fn normalize_der(expr: &Expression) -> Expression {
    match expr {
        Expression::Der(inner) => {
            if let Expression::Variable(name) = &**inner {
                Expression::Variable(format!("der_{}", name))
            } else {
                // If der(complex_expr), we might need to handle it or error out.
                // For now, recursively normalize.
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
        _ => {}
    }
}

fn collect_states_from_expr(expr: &Expression, states: &mut HashSet<String>) {
    match expr {
        Expression::Der(inner) => {
             if let Expression::Variable(name) = &**inner {
                 states.insert(name.clone());
             }
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

/// Sorts algebraic equations using Block Lower Triangular (BLT) transformation.
/// Returns (sorted_equations, structural_differential_index).
pub fn sort_algebraic_equations(
    equations: &[Equation],
    known_vars: &HashSet<String>,
    params: &[String],
    options: &AnalysisOptions,
) -> (Vec<Equation>, u32) {
    // 0. Prepare known variables set (including parameters and time)
    let mut current_known = known_vars.clone();
    for p in params {
        current_known.insert(p.clone());
    }
    current_known.insert("time".to_string());

    // 0.5. Eliminate Aliases (Simple equations like a = b)
    let (equations, alias_map) = eliminate_aliases(&equations);
    
    println!("  Aliases eliminated: {}", alias_map.len());
    // Debug
    println!("  Remaining equations after alias elim: {}", equations.len());
    // for (i, eq) in equations.iter().enumerate() {
    //     println!("    [{}]: {:?}", i, eq);
    // }

    // 1. Index reduction is applied after matching (see differential_index below).
    let equations = equations;

    // 2. Analyze equations to find unknowns
    struct EqInfo {
        original_idx: usize,
        unknowns: Vec<String>,
    }

    let mut eq_infos = Vec::new();
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

    let unknown_list: Vec<String> = all_unknowns.into_iter().collect();
    // Fix closure type inference
    let unknown_map: HashMap<String, usize> = unknown_list.iter().enumerate().map(|(i, u): (usize, &String)| (u.clone(), i)).collect();

    // 3. Perform Matching (Equation -> Variable)
    let mut assigned_var = vec![None; eq_infos.len()];
    let mut assigned_eq = vec![None; unknown_list.len()];
    
    // First pass: Try to assign equations to variables greedily
    // Especially "der(x) = ..." to "der_x"
    for (i, info) in eq_infos.iter().enumerate() {
        let eq = &equations[info.original_idx];
        if let Equation::Simple(lhs, _) = eq {
            // Check if LHS is der(x) or der_x
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
    
    // Augmenting path DFS for bipartite matching; iterative to avoid stack overflow.
    // Frame: (eq_index, next_adj_index, var_chosen_for_next_level)
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
            // assigned_eq updated in dfs_iter
        }
    }

    // Update assigned_var based on final assigned_eq
    for (v_idx, eq_opt) in assigned_eq.iter().enumerate() {
        if let Some(eq_idx) = eq_opt {
            assigned_var[*eq_idx] = Some(v_idx);
        }
    }

    let differential_index = if assigned_var.iter().any(|o| o.is_none()) { 2 } else { 1 };

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
                 // Equation not assigned to any variable (redundant or check?)
                 // For now, just include it as residual check? or ignore?
                 // Usually means over-determined or redundant.
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

    (sorted_equations, differential_index)
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
        let mut next_eqs = Vec::new();
        
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
            else_whens.iter().map(|(c, b)| (
                substitute_aliases_in_expr(c, map),
                b.iter().map(|b_eq| substitute_aliases_in_eq(b_eq, map)).collect()
            )).collect()
        ),
        _ => eq.clone()
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
#[allow(dead_code)]
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
