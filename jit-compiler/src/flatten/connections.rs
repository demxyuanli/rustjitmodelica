use super::structures::FlattenedModel;
use super::utils::are_types_compatible;
use super::FlattenError;
use crate::ast::{Equation, Expression, Operator};
use crate::diag::SourceLocation;
use crate::loader::ModelLoader;
use std::collections::{HashMap, HashSet};

fn equations_for_connections(
    flat: &FlattenedModel,
    connections: &[(String, String)],
) -> Vec<Equation> {
    let mut potential_eqs = Vec::new();
    let mut flow_adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut flow_vars = HashSet::new();
    for (a_path, b_path) in connections {
        if let Some(_type_name) = flat.instances.get(a_path) {
            let prefix_a = format!("{}_", a_path);
            let prefix_b = format!("{}_", b_path);
            for decl in &flat.declarations {
                if decl.name.starts_with(&prefix_a) {
                    if let Some(suffix) = decl.name.strip_prefix(&prefix_a) {
                        let target_name = format!("{}{}", prefix_b, suffix);
                        if decl.is_flow {
                            flow_adj
                                .entry(decl.name.clone())
                                .or_default()
                                .push(target_name.clone());
                            flow_adj
                                .entry(target_name.clone())
                                .or_default()
                                .push(decl.name.clone());
                            flow_vars.insert(decl.name.clone());
                            flow_vars.insert(target_name);
                        } else {
                            potential_eqs.push(Equation::Simple(
                                Expression::Variable(decl.name.clone()),
                                Expression::Variable(target_name),
                            ));
                        }
                    }
                }
            }
        } else {
            let mut found = false;
            for decl in &flat.declarations {
                if decl.name == *a_path {
                    found = true;
                    if decl.is_flow {
                        flow_adj
                            .entry(a_path.clone())
                            .or_default()
                            .push(b_path.clone());
                        flow_adj
                            .entry(b_path.clone())
                            .or_default()
                            .push(a_path.clone());
                        flow_vars.insert(a_path.clone());
                        flow_vars.insert(b_path.clone());
                    } else {
                        potential_eqs.push(Equation::Simple(
                            Expression::Variable(a_path.clone()),
                            Expression::Variable(b_path.clone()),
                        ));
                    }
                    break;
                }
            }
            if !found {
                potential_eqs.push(Equation::Simple(
                    Expression::Variable(a_path.clone()),
                    Expression::Variable(b_path.clone()),
                ));
            }
        }
    }
    let mut out = potential_eqs;
    let mut visited = HashSet::new();
    for var in &flow_vars {
        if visited.contains(var) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![var.clone()];
        visited.insert(var.clone());
        while let Some(curr) = stack.pop() {
            component.push(curr.clone());
            if let Some(neighbors) = flow_adj.get(&curr) {
                for n in neighbors {
                    if !visited.contains(n) {
                        visited.insert(n.clone());
                        stack.push(n.clone());
                    }
                }
            }
        }
        if !component.is_empty() {
            let mut expr = Expression::Variable(component[0].clone());
            for i in 1..component.len() {
                expr = Expression::BinaryOp(
                    Box::new(expr),
                    Operator::Add,
                    Box::new(Expression::Variable(component[i].clone())),
                );
            }
            out.push(Equation::Simple(expr, Expression::Number(0.0)));
        }
    }
    out
}

pub fn resolve_connections(
    flat: &mut FlattenedModel,
    root_model_name: Option<&str>,
    loader: &ModelLoader,
) -> Result<(), FlattenError> {
    let mut potential_eqs = Vec::new();
    let mut flow_adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut flow_vars = HashSet::new();

    for (a_path, b_path) in &flat.connections {
        // Type Checking: Verify connector compatibility
        let type_a = find_connector_type(a_path, flat);
        let type_b = find_connector_type(b_path, flat);

        if let (Some(ta), Some(tb)) = (&type_a, &type_b) {
            if !are_types_compatible(ta, tb) {
                let loc = root_model_name
                    .and_then(|n| loader.get_path_for_model(n))
                    .map(|p| SourceLocation {
                        file: p.display().to_string(),
                        line: 0,
                        column: 0,
                    });
                return Err(FlattenError::IncompatibleConnector(
                    a_path.clone(),
                    b_path.clone(),
                    ta.clone(),
                    tb.clone(),
                    loc,
                ));
            }
            let heat_scalar_array = |t: &str| {
                t.ends_with("HeatPort_a") || t.ends_with("HeatPort_b") || t.ends_with("HeatPorts_a")
            };
            if heat_scalar_array(ta) && heat_scalar_array(tb) && ta != tb {
                continue;
            }
        } else {
            if !loader.quiet {
                if type_a.is_none() {
                    eprintln!(
                        "Warning: Could not determine type for connector '{}' (path in model)",
                        a_path
                    );
                }
                if type_b.is_none() {
                    eprintln!(
                        "Warning: Could not determine type for connector '{}' (path in model)",
                        b_path
                    );
                }
            }
        }

        if let Some(_type_name) = flat.instances.get(a_path) {
            let prefix_a = format!("{}_", a_path);
            let prefix_b = format!("{}_", b_path);

            for decl in &flat.declarations {
                if decl.name.starts_with(&prefix_a) {
                    if let Some(suffix) = decl.name.strip_prefix(&prefix_a) {
                        let target_name = format!("{}{}", prefix_b, suffix);
                        if decl.is_flow {
                            flow_adj
                                .entry(decl.name.clone())
                                .or_default()
                                .push(target_name.clone());
                            flow_adj
                                .entry(target_name.clone())
                                .or_default()
                                .push(decl.name.clone());
                            flow_vars.insert(decl.name.clone());
                            flow_vars.insert(target_name);
                        } else {
                            potential_eqs.push(Equation::Simple(
                                Expression::Variable(decl.name.clone()),
                                Expression::Variable(target_name),
                            ));
                        }
                    }
                }
            }
        } else {
            let mut found = false;
            for decl in &flat.declarations {
                if decl.name == *a_path {
                    found = true;
                    if decl.is_flow {
                        flow_adj
                            .entry(a_path.clone())
                            .or_default()
                            .push(b_path.clone());
                        flow_adj
                            .entry(b_path.clone())
                            .or_default()
                            .push(a_path.clone());
                        flow_vars.insert(a_path.clone());
                        flow_vars.insert(b_path.clone());
                    } else {
                        potential_eqs.push(Equation::Simple(
                            Expression::Variable(a_path.clone()),
                            Expression::Variable(b_path.clone()),
                        ));
                    }
                    break;
                }
            }
            if !found {
                if !loader.quiet {
                    eprintln!("Warning: Connect involving unknown variable '{}'. Assuming potential equality.", a_path);
                }
                potential_eqs.push(Equation::Simple(
                    Expression::Variable(a_path.clone()),
                    Expression::Variable(b_path.clone()),
                ));
            }
        }
    }

    flat.equations.extend(potential_eqs);

    let mut visited = HashSet::new();
    for var in &flow_vars {
        if !visited.contains(var) {
            let mut component = Vec::new();
            let mut stack = vec![var.clone()];
            visited.insert(var.clone());

            while let Some(curr) = stack.pop() {
                component.push(curr.clone());
                if let Some(neighbors) = flow_adj.get(&curr) {
                    for n in neighbors {
                        if !visited.contains(n) {
                            visited.insert(n.clone());
                            stack.push(n.clone());
                        }
                    }
                }
            }

            if component.len() > 0 {
                let mut expr = Expression::Variable(component[0].clone());
                for i in 1..component.len() {
                    expr = Expression::BinaryOp(
                        Box::new(expr),
                        Operator::Add,
                        Box::new(Expression::Variable(component[i].clone())),
                    );
                }
                flat.equations
                    .push(Equation::Simple(expr, Expression::Number(0.0)));
            }
        }
    }

    if !flat.conditional_connections.is_empty() {
        let mut groups: Vec<(Expression, Vec<(String, String)>)> = Vec::new();
        for (cond, conn) in &flat.conditional_connections {
            let type_a = find_connector_type(&conn.0, flat);
            let type_b = find_connector_type(&conn.1, flat);
            if let (Some(ref ta), Some(ref tb)) = (&type_a, &type_b) {
                if !are_types_compatible(ta, tb) {
                    let loc = root_model_name
                        .and_then(|n| loader.get_path_for_model(n))
                        .map(|p| SourceLocation {
                            file: p.display().to_string(),
                            line: 0,
                            column: 0,
                        });
                    return Err(FlattenError::IncompatibleConnector(
                        conn.0.clone(),
                        conn.1.clone(),
                        ta.clone(),
                        tb.clone(),
                        loc,
                    ));
                }
                let heat_scalar_array = |t: &str| {
                    t.ends_with("HeatPort_a") || t.ends_with("HeatPort_b") || t.ends_with("HeatPorts_a")
                };
                if heat_scalar_array(ta) && heat_scalar_array(tb) && ta != tb {
                    continue;
                }
            }
            if let Some((_, list)) = groups.iter_mut().find(|(c, _)| c == cond) {
                list.push(conn.clone());
            } else {
                groups.push((cond.clone(), vec![conn.clone()]));
            }
        }
        for (cond, conns) in groups {
            let eqs = equations_for_connections(flat, &conns);
            if !eqs.is_empty() {
                flat.equations.push(Equation::When(cond, eqs, Vec::new()));
            }
        }
    }
    Ok(())
}

fn find_connector_type(path: &str, flat: &FlattenedModel) -> Option<String> {
    // If path is in instances, return its type
    if let Some(type_name) = flat.instances.get(path) {
        return Some(type_name.clone());
    }
    // If path is a variable/component in declarations
    for decl in &flat.declarations {
        if decl.name == path {
            return Some(decl.type_name.clone());
        }
    }
    None
}
