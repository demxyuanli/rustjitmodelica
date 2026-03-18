use crate::ast::{AlgorithmStatement, Equation, Expression, Model, Modification};
use std::collections::{HashMap, HashSet};

/// F3-3: Get function inputs and output (name, expr) list from model. Used for multi-output expand.
pub fn get_function_outputs(model: &Model) -> Option<(Vec<String>, Vec<(String, Expression)>)> {
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
    matches!(type_name, "Real" | "Integer" | "Boolean" | "String")
        || type_name.starts_with("Modelica.SIunits.")
        || type_name.starts_with("Modelica.Units.SI.")
        || type_name.starts_with("Modelica.Units.NonSI.")
        || type_name == "Modelica.StateSelect"
        || type_name.ends_with("ExternalObject")
}

/// Connector compatibility matrix: same physical domain can connect.
/// Rules grouped by domain (Blocks, Electrical, Rotational, HeatTransfer, Fluid, MultiBody).
pub fn are_types_compatible(t1: &str, t2: &str) -> bool {
    if t1 == t2 {
        return true;
    }
    let a = |s: &str, suffix: &str| s.ends_with(suffix);
    let both = |s1: &str, s2: &str, x: &str, y: &str| (a(s1, x) && a(s2, y)) || (a(s1, y) && a(s2, x));

    // --- Blocks.Interfaces: signal connectors (Real/Boolean/Integer Input/Output) ---
    if both(t1, t2, "RealInput", "RealOutput") {
        return true;
    }
    if both(t1, t2, "BooleanInput", "BooleanOutput") {
        return true;
    }
    if both(t1, t2, "IntegerInput", "IntegerOutput") {
        return true;
    }
    let t1_short = t1.split('.').last().unwrap_or(t1);
    let t2_short = t2.split('.').last().unwrap_or(t2);
    if (t1_short == "RealInput" && t2_short == "RealOutput")
        || (t1_short == "RealOutput" && t2_short == "RealInput")
    {
        return true;
    }

    // --- Electrical.Analog: Pin, PositivePin, NegativePin (same electrical domain) ---
    if a(t1, "Pin") && (a(t2, "Pin") || a(t2, "PositivePin") || a(t2, "NegativePin")) {
        return true;
    }
    if a(t2, "Pin") && (a(t1, "PositivePin") || a(t1, "NegativePin")) {
        return true;
    }
    if both(t1, t2, "PositivePin", "NegativePin") {
        return true;
    }
    // --- Electrical.Polyphase: PositivePlug, NegativePlug, Plug (treat as compatible) ---
    if (a(t1, "Plug") && a(t2, "Plug"))
        || both(t1, t2, "PositivePlug", "NegativePlug")
        || (a(t1, "Plug") && (a(t2, "PositivePlug") || a(t2, "NegativePlug")))
        || (a(t2, "Plug") && (a(t1, "PositivePlug") || a(t1, "NegativePlug")))
    {
        return true;
    }

    // --- Mechanics.Rotational: Flange_a, Flange_b, Support ---
    if both(t1, t2, "Flange_a", "Flange_b") {
        return true;
    }
    if (a(t1, "Support") && (a(t2, "Flange_a") || a(t2, "Flange_b")))
        || (a(t2, "Support") && (a(t1, "Flange_a") || a(t1, "Flange_b")))
    {
        return true;
    }

    // --- Thermal.HeatTransfer: HeatPort_a, HeatPort_b ---
    if both(t1, t2, "HeatPort_a", "HeatPort_b") {
        return true;
    }
    // --- Thermal: HeatPort_b <-> HeatPorts_a (array heat port compatibility) ---
    if both(t1, t2, "HeatPort_b", "HeatPorts_a") {
        return true;
    }

    // --- Fluid: FluidPort_a, FluidPort_b (Modelica.Fluid.Interfaces) ---
    if both(t1, t2, "FluidPort_a", "FluidPort_b") {
        return true;
    }
    if a(t1, "FluidPort") && a(t2, "FluidPort") {
        return true;
    }

    // --- Mechanics.MultiBody: Frame_a, Frame_b ---
    if both(t1, t2, "Frame_a", "Frame_b") {
        return true;
    }
    if both(t1, t2, "Frame_resolve", "Frame_a") || both(t1, t2, "Frame_resolve", "Frame_b") {
        return true;
    }
    if a(t1, "Frame") && a(t2, "Frame") {
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
    // Inherit inner classes (e.g. replaceable model FlowModel) so that short names can be resolved
    // relative to the derived class context.
    for inner in &base.inner_classes {
        if !child.inner_classes.iter().any(|c| c.name == inner.name) {
            child.inner_classes.push(inner.clone());
        }
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
        Equation::CallStmt(expr) => AlgorithmStatement::CallStmt(expr),
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
