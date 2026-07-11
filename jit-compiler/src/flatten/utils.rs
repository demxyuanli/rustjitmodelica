use crate::ast::{AlgorithmStatement, Declaration, Equation, Expression, Model, Modification};
use crate::flatten::redeclare::{apply_modification_to_model, ModifyContext};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct FunctionOutputSpec {
    pub name: String,
    pub expr: Expression,
    pub decl: Declaration,
    pub resolved_type_name: String,
}

/// F3-3: Get function inputs and output specs from model. Used for multi-output expand.
pub fn get_function_outputs(model: &Model) -> Option<(Vec<String>, Vec<FunctionOutputSpec>)> {
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
    let output_decls: Vec<Declaration> = model
        .declarations
        .iter()
        .filter(|d| d.is_output)
        .cloned()
        .collect();
    if output_decls.is_empty() {
        return None;
    }
    let mut out_exprs: HashMap<String, Expression> = HashMap::new();
    for stmt in &model.algorithms {
        if let AlgorithmStatement::Assignment(lhs, rhs) = stmt {
            if let Expression::Variable(id) = lhs {
                let v = crate::string_intern::resolve_id(*id);
                if output_decls.iter().any(|d| d.name == v) {
                    out_exprs.insert(v, rhs.clone());
                }
            }
        }
    }
    let ordered: Vec<FunctionOutputSpec> = output_decls
        .into_iter()
        .map(|decl| {
            let expr = out_exprs
                .remove(&decl.name)
                .unwrap_or_else(|| Expression::var(&decl.name));
            let resolved_type_name = resolve_type_alias(&model.type_aliases, &decl.type_name);
            FunctionOutputSpec {
                name: decl.name.clone(),
                expr,
                decl,
                resolved_type_name,
            }
        })
        .collect();
    Some((input_names, ordered))
}

/// Resolve short package/model aliases declared as inner classes
/// (e.g. `package Medium2 = Modelica.Media.IdealGases.SingleGases.N2;`).
/// If the first segment of `name` matches an inner class whose sole extends clause
/// points to a base type, replace the prefix with that base type.
pub fn resolve_inner_class_alias(model: &Model, name: &str) -> String {
    let first_seg = name.split('.').next().unwrap_or(name);
    if let Some(ic) = model.find_inner_class(first_seg) {
        if ic.extends.len() == 1 && ic.declarations.is_empty() && ic.equations.is_empty() {
            let base = &ic.extends[0].model_name;
            if name.len() > first_seg.len() {
                return format!("{}{}", base, &name[first_seg.len()..]);
            }
            return base.clone();
        }
    }
    name.to_string()
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
    let short = type_name.rsplit('.').next().unwrap_or(type_name);
    let is_complex_connector = matches!(short, "ComplexInput" | "ComplexOutput");
    is_complex_connector
        || matches!(type_name, "Real" | "Integer" | "Boolean" | "String" | "Complex")
        || type_name == "Clock"
        || type_name.starts_with("Modelica.SIunits.")
        || type_name.starts_with("Modelica.Units.SI.")
        || type_name.starts_with("Modelica.Units.NonSI.")
        || type_name.starts_with("Modelica.Complex")
        || type_name == "Modelica.StateSelect"
        || type_name.ends_with("ExternalObject")
}

/// Connector compatibility matrix: same physical domain can connect.
/// Rules grouped by domain (Blocks, Electrical, Rotational, HeatTransfer, Fluid, MultiBody).
pub fn are_types_compatible(t1: &str, t2: &str) -> bool {
    let normalize_connector_type_name = |raw: &str| -> String {
        let t = raw.trim();
        match t {
            "HeatPort_a" | "Interfaces.HeatPort_a" | "HeatTransfer.Interfaces.HeatPort_a" => {
                "Modelica.Thermal.HeatTransfer.Interfaces.HeatPort_a".to_string()
            }
            "HeatPort_b" | "Interfaces.HeatPort_b" | "HeatTransfer.Interfaces.HeatPort_b" => {
                "Modelica.Thermal.HeatTransfer.Interfaces.HeatPort_b".to_string()
            }
            "HeatPorts_a" | "Interfaces.HeatPorts_a" => {
                "Modelica.Thermal.HeatTransfer.Interfaces.HeatPorts_a".to_string()
            }
            "HeatPorts_b" | "Interfaces.HeatPorts_b" => {
                "Modelica.Thermal.HeatTransfer.Interfaces.HeatPorts_b".to_string()
            }
            "FlowPort_a" | "Interfaces.FlowPort_a" => {
                "Modelica.Thermal.FluidHeatFlow.Interfaces.FlowPort_a".to_string()
            }
            "FlowPort_b" | "Interfaces.FlowPort_b" => {
                "Modelica.Thermal.FluidHeatFlow.Interfaces.FlowPort_b".to_string()
            }
            "FluidPort_a" | "Interfaces.FluidPort_a" => {
                "Modelica.Fluid.Interfaces.FluidPort_a".to_string()
            }
            "FluidPort_b" | "Interfaces.FluidPort_b" => {
                "Modelica.Fluid.Interfaces.FluidPort_b".to_string()
            }
            "FluidPorts_a" | "Interfaces.FluidPorts_a" => {
                "Modelica.Fluid.Interfaces.FluidPorts_a".to_string()
            }
            "FluidPorts_b" | "Interfaces.FluidPorts_b" => {
                "Modelica.Fluid.Interfaces.FluidPorts_b".to_string()
            }
            _ => t.to_string(),
        }
    };
    let t1n = normalize_connector_type_name(t1);
    let t2n = normalize_connector_type_name(t2);
    let t1 = t1n.as_str();
    let t2 = t2n.as_str();
    let t1 = if t1 == "AxisControlBus" {
        "Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.Utilities.AxisControlBus"
    } else if t1 == "ControlBus" {
        "Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.Utilities.ControlBus"
    } else {
        t1
    };
    let t2 = if t2 == "AxisControlBus" {
        "Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.Utilities.AxisControlBus"
    } else if t2 == "ControlBus" {
        "Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.Utilities.ControlBus"
    } else {
        t2
    };
    if t1 == t2 {
        return true;
    }
    let known_port_suffixes: &[&str] = &[
        "Port_a", "Port_b", "Ports_a", "Ports_b",
        "Pin", "Port", "Plug",
        "Frame", "Frame_a", "Frame_b", "Frame_resolve",
        "Flange", "Flange_a", "Flange_b",
        "Support", "Input", "Output",
        "Bus", "Connector",
    ];
    let short1 = t1.rsplit('.').next().unwrap_or(t1);
    let short2 = t2.rsplit('.').next().unwrap_or(t2);
    // Short names match exactly: same connector type in different packages.
    if short1 == short2
        && known_port_suffixes.iter().any(|suffix| short1.ends_with(suffix))
    {
        return true;
    }
    // Different short names but share a known port suffix: handle extends
    // (e.g. VesselFluidPorts_b extends FluidPorts_b).
    if short1 != short2 {
        for suffix in known_port_suffixes {
            if short1.ends_with(suffix) && short2.ends_with(suffix) {
                return true;
            }
        }
    }
    let is_likely_connector_type = |s: &str| {
        let short = s.split('.').last().unwrap_or(s);
        s.contains('.') && known_port_suffixes.iter().any(|suffix| short.ends_with(suffix))
    };
    let is_real_like = |s: &str| {
        s == "Real"
            || s.starts_with("Modelica.SIunits.")
            || s.starts_with("Modelica.Units.SI.")
            || s.starts_with("Modelica.Units.NonSI.")
    };
    let is_scalar_signal_like = |s: &str| {
        is_real_like(s)
            || s == "Integer"
            || s == "Boolean"
            || s.ends_with("IntegerInput")
            || s.ends_with("IntegerOutput")
            || s.ends_with("BooleanInput")
            || s.ends_with("BooleanOutput")
    };
    if (t1 == "connector" && is_scalar_signal_like(t2))
        || (t2 == "connector" && is_scalar_signal_like(t1))
    {
        return true;
    }
    if (t1 == "connector" && is_likely_connector_type(t2))
        || (t2 == "connector" && is_likely_connector_type(t1))
    {
        return true;
    }
    if (t1 == "connector" && t2 == "Clock") || (t2 == "connector" && t1 == "Clock") {
        return true;
    }
    if is_real_like(t1) && is_real_like(t2) {
        return true;
    }
    // Clocked logical helper blocks can transiently expose scalar ports as plain
    // `Real` on one side and `Boolean` on the other before full connector alias
    // typing converges. Treat this pair as connect-compatible.
    if (t1 == "Real" && t2 == "Boolean") || (t1 == "Boolean" && t2 == "Real") {
        return true;
    }
    let a = |s: &str, suffix: &str| s.ends_with(suffix);
    let both = |s1: &str, s2: &str, x: &str, y: &str| (a(s1, x) && a(s2, y)) || (a(s1, y) && a(s2, x));
    if both(t1, t2, "ClockInput", "ClockOutput") {
        return true;
    }

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
    if both(t1, t2, "ComplexInput", "ComplexOutput") {
        return true;
    }
    let t1_short = t1.split('.').last().unwrap_or(t1);
    let t2_short = t2.split('.').last().unwrap_or(t2);
    if (t1_short == "RealInput" && t2_short == "RealOutput")
        || (t1_short == "RealOutput" && t2_short == "RealInput")
    {
        return true;
    }
    if (t1_short == "ComplexInput" && t2_short == "ComplexOutput")
        || (t1_short == "ComplexOutput" && t2_short == "ComplexInput")
    {
        return true;
    }
    if (t1_short == "Complex" && t2_short == "ComplexInput")
        || (t1_short == "ComplexInput" && t2_short == "Complex")
        || (t1_short == "Complex" && t2_short == "ComplexOutput")
        || (t1_short == "ComplexOutput" && t2_short == "Complex")
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
    if both(t1, t2, "InductiveCouplePinOut", "InductiveCouplePinIn") {
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
    // --- Thermal.FluidHeatFlow: FlowPort_a, FlowPort_b ---
    if both(t1, t2, "FlowPort_a", "FlowPort_b") {
        return true;
    }
    // --- Thermal: HeatPort_b <-> HeatPorts_a (array heat port compatibility) ---
    if both(t1, t2, "HeatPort_b", "HeatPorts_a") {
        return true;
    }
    if both(t1, t2, "HeatPort_a", "HeatPorts_a") {
        return true;
    }

    // --- Fluid: FluidPort_a, FluidPort_b (Modelica.Fluid.Interfaces) ---
    if both(t1, t2, "FluidPort_a", "FluidPort_b") {
        return true;
    }
    if both(t1, t2, "FluidPort_a", "FluidPorts_b")
        || both(t1, t2, "FluidPort_b", "FluidPorts_a")
        || both(t1, t2, "FluidPorts_a", "FluidPorts_b")
        || both(t1, t2, "FluidPort_a", "FluidPorts_a")
        || both(t1, t2, "FluidPort_b", "FluidPorts_b")
    {
        return true;
    }
    if both(t1, t2, "VesselFluidPorts_a", "FluidPort_b")
        || both(t1, t2, "VesselFluidPorts_a", "FluidPort_a")
        || both(t1, t2, "VesselFluidPorts_b", "FluidPort_b")
        || both(t1, t2, "VesselFluidPorts_b", "FluidPort_a")
    {
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

    // --- Magnetic (FundamentalWave, FluxTubes, QuasiStatic): positive <-> negative magnetic ports ---
    if both(t1, t2, "PositiveMagneticPort", "NegativeMagneticPort") {
        return true;
    }
    if (t1_short == "PositiveMagneticPort" && t2_short == "NegativeMagneticPort")
        || (t1_short == "NegativeMagneticPort" && t2_short == "PositiveMagneticPort")
    {
        return true;
    }
    if both(t1, t2, "PositiveMagneticFluxPort", "NegativeMagneticFluxPort") {
        return true;
    }
    if (t1_short == "PositiveMagneticFluxPort" && t2_short == "NegativeMagneticFluxPort")
        || (t1_short == "NegativeMagneticFluxPort" && t2_short == "PositiveMagneticFluxPort")
    {
        return true;
    }

    // QS.FundamentalWave magnetic connectors often use the same short class names as static
    // FundamentalWave; MSL connects QS machine windings to static polyphase converter components.
    if t1_short == t2_short
        && t1.contains("Modelica.Magnetic.")
        && t2.contains("Modelica.Magnetic.")
        && matches!(
            t1_short,
            "PositiveMagneticPort"
                | "NegativeMagneticPort"
                | "MagneticPort"
                | "PositiveMagneticFluxPort"
                | "NegativeMagneticFluxPort"
        )
    {
        return true;
    }

    // --- StateGraph: paired directional connectors ---
    // Same-type pairs (Step_out↔Step_in, Transition_out↔Transition_in)
    if both(t1, t2, "Step_out", "Step_in") {
        return true;
    }
    if both(t1, t2, "Alternative_out", "Alternative_in") {
        return true;
    }
    if both(t1, t2, "Parallel_out", "Parallel_in") {
        return true;
    }
    if both(t1, t2, "Transition_out", "Transition_in") {
        return true;
    }
    // Cross-type pairs: Step ↔ Transition, Step/Transition ↔ Alternative/Parallel
    if both(t1, t2, "Step_out", "Transition_in") {
        return true;
    }
    // Alternative/Parallel split steps expose `Step_out_forAlternative` (MSL 4.x naming).
    if (a(t1, "Step_out_forAlternative") && a(t2, "Transition_in"))
        || (a(t2, "Step_out_forAlternative") && a(t1, "Transition_in"))
    {
        return true;
    }
    if (a(t1, "Step_out_forParallel") && a(t2, "Transition_in"))
        || (a(t2, "Step_out_forParallel") && a(t1, "Transition_in"))
    {
        return true;
    }
    if both(t1, t2, "Transition_out", "Step_in") {
        return true;
    }
    if both(t1, t2, "Step_out", "Alternative_in")
        || both(t1, t2, "Alternative_out", "Step_in")
        || both(t1, t2, "Step_out", "Parallel_in")
        || both(t1, t2, "Parallel_out", "Step_in")
    {
        return true;
    }
    if both(t1, t2, "Transition_out", "Alternative_in")
        || both(t1, t2, "Alternative_out", "Transition_in")
        || both(t1, t2, "Transition_out", "Parallel_in")
        || both(t1, t2, "Parallel_out", "Transition_in")
    {
        return true;
    }
    // CompositeStep resume/suspend connectors
    if both(t1, t2, "CompositeStep_resume", "Transition_in")
        || both(t1, t2, "Transition_out", "CompositeStep_resume")
        || both(t1, t2, "CompositeStep_suspend", "Transition_in")
        || both(t1, t2, "Transition_out", "CompositeStep_suspend")
    {
        return true;
    }
    if both(
        t1,
        t2,
        "CompositeStepStatePort_out",
        "CompositeStepStatePort_in",
    ) {
        return true;
    }
    if t1.contains("Modelica.StateGraph") && t2.contains("Modelica.StateGraph") {
        let has_suffix = |s: &str| {
            s.ends_with("_out") || s.ends_with("_in") || s.ends_with("_resume") || s.ends_with("_suspend")
        };
        if has_suffix(t1_short) && has_suffix(t2_short) {
            return true;
        }
    }

    // --- StateGraph.Examples.Utilities: paired tank/valve flow connectors (MSL ControlledTanks) ---
    if both(t1, t2, "Outflow1", "Outflow2") {
        return true;
    }
    if both(t1, t2, "Inflow1", "Inflow2") {
        return true;
    }

    false
}

/// Applies one modification using default scope (backward compatible; ignores strict errors).
#[allow(dead_code)]
pub fn apply_modification(model: &mut Model, modification: &Modification) {
    let ctx = ModifyContext::default();
    let _ = apply_modification_to_model(model, modification, &ctx, None);
}

pub fn merge_models(child: &mut Model, base: &Model) {
    if base.is_function && !child.is_function {
        child.is_function = true;
    }
    if base.is_operator_function && !child.is_operator_function {
        child.is_operator_function = true;
    }
    if base.is_operator_record && !child.is_operator_record {
        child.is_operator_record = true;
    }
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
    for stmt in &base.algorithms {
        child.algorithms.push(stmt.clone());
    }
    for eq in &base.initial_equations {
        child.initial_equations.push(eq.clone());
    }
    for stmt in &base.initial_algorithms {
        child.initial_algorithms.push(stmt.clone());
    }
    for inner in &base.inner_classes {
        if !child.inner_class_index.contains_key(&inner.name) {
            let idx = child.inner_classes.len();
            child.inner_class_index.insert(inner.name.clone(), idx);
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
        Equation::CallStmt(expr) => {
            if let Expression::Call(name, args) = &expr {
                let is_reinit = name == "reinit" || name.ends_with(".reinit");
                if is_reinit && args.len() == 2 {
                    if let Some(var_name) = crate::ast::expr_to_flat_scalar_prefix(&args[0]) {
                        return AlgorithmStatement::Reinit(var_name, args[1].clone());
                    }
                }
            }
            AlgorithmStatement::CallStmt(expr)
        }
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
        Equation::MultiAssign(lhss, rhs) => AlgorithmStatement::MultiAssign(lhss, rhs),
    }
}

/// Qualify short and relative type names in a model's declarations by walking
/// the base package scope. Shared between the flattener and query_db paths.
pub fn qualify_short_type_names(
    model: &mut Model,
    base_pkg: &str,
    exists: &mut dyn FnMut(&str) -> bool,
) {
    let inner_names: std::collections::HashSet<String> =
        model.inner_classes.iter().map(|ic| ic.name.clone()).collect();
    for decl in &mut model.declarations {
        if is_primitive(&decl.type_name) || inner_names.contains(&decl.type_name) {
            continue;
        }
        if !decl.type_name.contains('.') {
            let fqn = format!("{}.{}", base_pkg, decl.type_name);
            if exists(&fqn) {
                decl.type_name = fqn;
            }
        } else if !decl.type_name.starts_with("Modelica.")
            && !decl.type_name.starts_with("ModelicaTest.")
            && !decl.type_name.starts_with("ModelicaServices.") {
            let mut scope = base_pkg.to_string();
            while let Some((parent, _)) = scope.rsplit_once('.') {
                let candidate = format!("{}.{}", parent, decl.type_name);
                if exists(&candidate) {
                    decl.type_name = candidate;
                    break;
                }
                scope = parent.to_string();
            }
        }
    }
    for ic in &mut model.inner_classes {
        qualify_short_type_names(ic, base_pkg, exists);
    }
}
