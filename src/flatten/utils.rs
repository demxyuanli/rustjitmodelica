use std::collections::HashSet;
use crate::ast::{Model, Modification, Equation, AlgorithmStatement, Expression};

pub fn is_primitive(type_name: &str) -> bool {
    matches!(type_name, "Real" | "Integer" | "Boolean")
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
                });
                return;
            }
        }
    } else {
        for decl in &mut model.declarations {
            if decl.name == modification.name {
                decl.start_value = modification.value.clone();
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
        Equation::Connect(_, _) => panic!("Connect statements not supported inside When/Algorithm"),
        Equation::SolvableBlock { .. } => panic!("SolvableBlock not supported inside When/Algorithm"),
    }
}
