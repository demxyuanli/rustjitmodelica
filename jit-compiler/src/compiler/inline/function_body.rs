use crate::ast::{AlgorithmStatement, Expression, Model};
use std::collections::{HashMap, HashSet};

fn stmts_has_side_effects_one(
    stmts: &[AlgorithmStatement],
    output_names: &[String],
    local_names: &HashSet<String>,
    allowed: &HashSet<&String>,
) -> bool {
    let _ = (output_names, local_names);
    for stmt in stmts {
        match stmt {
            AlgorithmStatement::Reinit(_, _)
            | AlgorithmStatement::Assert(_, _)
            | AlgorithmStatement::Terminate(_) => return true,
            AlgorithmStatement::CallStmt(_) => return true,
            AlgorithmStatement::NoOp => {}
            AlgorithmStatement::Assignment(lhs, _) => {
                if let Expression::Variable(id) = lhs {
                    let name = crate::string_intern::resolve_id(*id);
                    if !allowed.contains(&&name) {
                        return true;
                    }
                }
            }
            AlgorithmStatement::MultiAssign(lhss, _) => {
                for lhs in lhss {
                    if let Expression::Variable(id) = lhs {
                        let name = crate::string_intern::resolve_id(*id);
                        if !allowed.contains(&&name) {
                            return true;
                        }
                    }
                }
            }
            AlgorithmStatement::If(_, then_s, else_if, else_s) => {
                if stmts_has_side_effects_one(then_s, output_names, local_names, allowed) {
                    return true;
                }
                for (_, s) in else_if {
                    if stmts_has_side_effects_one(s, output_names, local_names, allowed) {
                        return true;
                    }
                }
                if let Some(s) = else_s {
                    if stmts_has_side_effects_one(s, output_names, local_names, allowed) {
                        return true;
                    }
                }
            }
            AlgorithmStatement::For(_, _, s) | AlgorithmStatement::While(_, s) => {
                if stmts_has_side_effects_one(s, output_names, local_names, allowed) {
                    return true;
                }
            }
            AlgorithmStatement::When(_, s, elsewhen_list) => {
                if stmts_has_side_effects_one(s, output_names, local_names, allowed) {
                    return true;
                }
                for (_, s) in elsewhen_list {
                    if stmts_has_side_effects_one(s, output_names, local_names, allowed) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub(super) fn function_has_side_effects(
    model: &Model,
    output_names: &[String],
    local_names: &HashSet<String>,
) -> bool {
    let allowed: HashSet<&String> = output_names.iter().chain(local_names.iter()).collect();
    stmts_has_side_effects_one(&model.algorithms, output_names, local_names, &allowed)
}

pub(crate) fn get_function_body(model: &Model) -> Option<(Vec<String>, Vec<(String, Expression)>)> {
    if !model.is_function {
        return None;
    }
    if model.external_info.is_some() {
        return None;
    }
    let input_names: Vec<String> = model
        .declarations
        .iter()
        .filter(|d| d.is_input)
        .map(|d| d.name.clone())
        .collect();
    let output_names: Vec<String> = model
        .declarations
        .iter()
        .filter(|d| d.is_output)
        .map(|d| d.name.clone())
        .collect();
    if output_names.is_empty() {
        return None;
    }
    let local_names: HashSet<String> = model
        .declarations
        .iter()
        .filter(|d| !d.is_input && !d.is_output)
        .map(|d| d.name.clone())
        .collect();
    if function_has_side_effects(model, &output_names, &local_names) {
        return None;
    }
    let mut out_exprs: HashMap<String, Expression> = HashMap::new();
    for stmt in &model.algorithms {
        if let AlgorithmStatement::Assignment(lhs, rhs) = stmt {
            if let Expression::Variable(id) = lhs {
                let v = crate::string_intern::resolve_id(*id);
                if output_names.contains(&v) {
                    out_exprs.insert(v, rhs.clone());
                }
            }
        }
    }
    let ordered: Vec<(String, Expression)> = output_names
        .into_iter()
        .filter_map(|name| out_exprs.remove(&name).map(|e| (name, e)))
        .collect();
    if ordered.is_empty() {
        return None;
    }
    Some((input_names, ordered))
}
