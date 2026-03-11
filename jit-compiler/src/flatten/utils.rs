use std::collections::{HashMap, HashSet};
use crate::ast::{Model, Modification, Equation, AlgorithmStatement, Expression};

/// F3-3: Get function inputs and output (name, expr) list from model. Used for multi-output expand.
pub fn get_function_outputs(model: &Model) -> Option<(Vec<String>, Vec<(String, Expression)>)> {
    if !model.is_function {
        return None;
    }
    if model.external_info.is_some() {
        return None;
    }
    let input_names: Vec<String> = model.declarations.iter().filter(|d| d.is_input).map(|d| d.name.clone()).collect();
    let output_names: Vec<String> = model.declarations.iter().filter(|d| d.is_output).map(|d| d.name.clone()).collect();
    if output_names.is_empty() {
        return None;
    }
    let mut out_exprs: HashMap<String, Expression> = HashMap::new();
    for stmt in &model.algorithms {
        if let AlgorithmStatement::Assignment(lhs, rhs) = stmt {
            if let Expression::Variable(v) = lhs {
                if output_names.contains(v) {
                    out_exprs.insert(v.clone(), rhs.clone());
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

/// F1-4: Resolve type alias through type_aliases (e.g. type MyReal = Real;). Returns final type name; avoids cycles.
pub fn resolve_type_alias(type_aliases: &[(String, String)], name: &str) -> String {
    let mut current = name.to_string();
    let mut visited = std::collections::HashSet::new();
    while visited.insert(current.clone()) {
        if let Some((_, base)) = type_aliases.iter().find(|(n, _)| n == &current) {
            current = base.clone();
        } else {
            break;
        }
    }
    current
}

/// MSL-4: SIunits types (Modelica.SIunits.Time, etc.) are resolved as Real; units parsed but not enforced.
pub fn is_primitive(type_name: &str) -> bool {
    matches!(type_name, "Real" | "Integer" | "Boolean")
        || type_name.starts_with("Modelica.SIunits.")
}

pub fn are_types_compatible(t1: &str, t2: &str) -> bool {
    if t1 == t2 {
        return true;
    }
    // Allow RealInput and RealOutput to connect (both are Real signals)
    // In Modelica, they are "connector RealInput = input Real;" and "connector RealOutput = output Real;"
    // They are compatible for connection.
    if (t1.ends_with("RealInput") && t2.ends_with("RealOutput")) || 
       (t1.ends_with("RealOutput") && t2.ends_with("RealInput")) {
        return true;
    }
    
    // Also check if they are fully qualified
    let t1_short = t1.split('.').last().unwrap_or(t1);
    let t2_short = t2.split('.').last().unwrap_or(t2);
    
    if (t1_short == "RealInput" && t2_short == "RealOutput") ||
       (t1_short == "RealOutput" && t2_short == "RealInput") {
           return true;
    }

    false
}

pub fn apply_modification(model: &mut Model, modification: &Modification) {
    if let Some((head, tail)) = modification.name.split_once('.') {
        for decl in &mut model.declarations {
            if decl.name == head {
                decl.modifications.push(Modification {
                    name: tail.to_string(),
                    value: modification.value.clone(),
                    each: modification.each,
                    redeclare: modification.redeclare,
                    redeclare_type: modification.redeclare_type.clone(),
                });
                return;
            }
        }
    } else {
        for decl in &mut model.declarations {
            if decl.name == modification.name {
                if modification.redeclare {
                    if let Some(ref t) = modification.redeclare_type {
                        decl.type_name = t.clone();
                    }
                    decl.start_value = modification.value.clone();
                } else {
                    decl.start_value = modification.value.clone();
                }
                return;
            }
        }
    }
}

pub fn merge_models(child: &mut Model, base: &Model) {
    let mut existing_vars = HashSet::new();
    for decl in &child.declarations {
        existing_vars.insert(decl.name.clone());
    }
    for decl in &base.declarations {
        if !existing_vars.contains(&decl.name) {
            child.declarations.push(decl.clone());
        }
    }
    for eq in &base.equations {
        child.equations.push(eq.clone());
    }
    for (name, base_type) in &base.type_aliases {
        if !child.type_aliases.iter().any(|(n, _)| n == name) {
            child.type_aliases.push((name.clone(), base_type.clone()));
        }
    }
}

pub fn convert_eq_to_alg(eq: Equation) -> AlgorithmStatement {
    match eq {
        Equation::Simple(lhs, rhs) => AlgorithmStatement::Assignment(lhs, rhs),
        Equation::Reinit(var, val) => AlgorithmStatement::Reinit(var, val),
        Equation::For(var, start, end, body) => {
            let alg_body = body.into_iter().map(convert_eq_to_alg).collect();
            let range = Expression::Range(start, Box::new(Expression::Number(1.0)), end);
            AlgorithmStatement::For(var, Box::new(range), alg_body)
        }
        Equation::When(cond, body, else_whens) => {
             let alg_body = body.into_iter().map(convert_eq_to_alg).collect();
             let alg_else_whens = else_whens.into_iter().map(|(c, b)| (c, b.into_iter().map(convert_eq_to_alg).collect())).collect();
             AlgorithmStatement::When(cond, alg_body, alg_else_whens)
        }
        Equation::Assert(cond, msg) => AlgorithmStatement::Assert(cond, msg),
        Equation::Terminate(msg) => AlgorithmStatement::Terminate(msg),
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => {
            let then_alg = then_eqs.into_iter().map(convert_eq_to_alg).collect();
            let elseif_alg = elseif_list.into_iter()
                .map(|(c, eb)| (c, eb.into_iter().map(convert_eq_to_alg).collect()))
                .collect();
            let else_alg = else_eqs.map(|eqs| eqs.into_iter().map(convert_eq_to_alg).collect());
            AlgorithmStatement::If(cond, then_alg, elseif_alg, else_alg)
        }
        Equation::Connect(_, _) => panic!("F4-2: connect() inside when/algorithm is not supported; use equation section for connections"),
        Equation::SolvableBlock { .. } => panic!("F4-2: SolvableBlock (algebraic loop) inside when/algorithm is not supported; put equations in the equation section instead"),
        Equation::MultiAssign(_, _) => panic!("F3-3: (a,b,...)=f(x) in when/algorithm is not supported; use equation section"),
    }
}
