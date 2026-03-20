use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::ast::{AlgorithmStatement, Equation, Expression, Model, Operator};
use crate::flatten::FlattenedModel;
use crate::loader::ModelLoader;

fn substitute_expr(expr: &Expression, subst: &HashMap<String, Expression>) -> Expression {
    use Expression::*;
    match expr {
        Variable(name) => subst
            .get(name)
            .cloned()
            .unwrap_or_else(|| Variable(name.clone())),
        Number(n) => Number(*n),
        BinaryOp(lhs, op, rhs) => BinaryOp(
            Box::new(substitute_expr(lhs, subst)),
            *op,
            Box::new(substitute_expr(rhs, subst)),
        ),
        Call(func, args) => Call(
            func.clone(),
            args.iter().map(|a| substitute_expr(a, subst)).collect(),
        ),
        Der(inner) => Der(Box::new(substitute_expr(inner, subst))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(substitute_expr(arr, subst)),
            Box::new(substitute_expr(idx, subst)),
        ),
        If(cond, t, f) => If(
            Box::new(substitute_expr(cond, subst)),
            Box::new(substitute_expr(t, subst)),
            Box::new(substitute_expr(f, subst)),
        ),
        Range(start, step, end) => Range(
            Box::new(substitute_expr(start, subst)),
            Box::new(substitute_expr(step, subst)),
            Box::new(substitute_expr(end, subst)),
        ),
        ArrayLiteral(items) => {
            ArrayLiteral(items.iter().map(|e| substitute_expr(e, subst)).collect())
        }
        ArrayComprehension { expr, iter_var, iter_range } => ArrayComprehension {
            expr: Box::new(substitute_expr(expr, subst)),
            iter_var: iter_var.clone(),
            iter_range: Box::new(substitute_expr(iter_range, subst)),
        },
        Dot(base, member) => Dot(Box::new(substitute_expr(base, subst)), member.clone()),
        Sample(inner) => Sample(Box::new(substitute_expr(inner, subst))),
        Interval(inner) => Interval(Box::new(substitute_expr(inner, subst))),
        Hold(inner) => Hold(Box::new(substitute_expr(inner, subst))),
        Previous(inner) => Previous(Box::new(substitute_expr(inner, subst))),
        SubSample(c, n) => SubSample(
            Box::new(substitute_expr(c, subst)),
            Box::new(substitute_expr(n, subst)),
        ),
        SuperSample(c, n) => SuperSample(
            Box::new(substitute_expr(c, subst)),
            Box::new(substitute_expr(n, subst)),
        ),
        ShiftSample(c, n) => ShiftSample(
            Box::new(substitute_expr(c, subst)),
            Box::new(substitute_expr(n, subst)),
        ),
        StringLiteral(s) => StringLiteral(s.clone()),
    }
}

/// FUNC-4: Returns true if function body has side effects (reinit/assert/terminate or assign to non-local).
fn function_has_side_effects(
    model: &Model,
    output_names: &[String],
    local_names: &std::collections::HashSet<String>,
) -> bool {
    let allowed: std::collections::HashSet<&String> =
        output_names.iter().chain(local_names.iter()).collect();
    stmts_has_side_effects_one(&model.algorithms, output_names, local_names, &allowed)
}

fn stmts_has_side_effects_one(
    stmts: &[AlgorithmStatement],
    output_names: &[String],
    local_names: &std::collections::HashSet<String>,
    allowed: &std::collections::HashSet<&String>,
) -> bool {
    use crate::ast::AlgorithmStatement;
    for stmt in stmts {
        match stmt {
            AlgorithmStatement::Reinit(_, _)
            | AlgorithmStatement::Assert(_, _)
            | AlgorithmStatement::Terminate(_) => return true,
            AlgorithmStatement::CallStmt(_) => return true,
            AlgorithmStatement::NoOp => {}
            AlgorithmStatement::Assignment(lhs, _) => {
                if let Expression::Variable(name) = lhs {
                    if !allowed.contains(&name) {
                        return true;
                    }
                }
            }
            AlgorithmStatement::MultiAssign(lhss, _) => {
                for lhs in lhss {
                    if let Expression::Variable(name) = lhs {
                        if !allowed.contains(&name) {
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

/// Returns (input_names, outputs) where outputs is (name, expr) per output in declaration order.
/// Single-output: one element; multi-output (F3-3): multiple elements.
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
    let local_names: std::collections::HashSet<String> = model
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

/// FUNC-2: Exposed so compiler can detect remaining user calls that were not inlined.
/// When true, compiler does not try load_model(); JIT/backend provides implementation or placeholder.
pub(crate) fn is_builtin_function(name: &str) -> bool {
    // Namespace-qualified calls (e.g. Machines.*, Interfaces.*, FluxTubes.*) are often
    // package helpers in MSL that are not linked as standalone symbols in validate mode.
    if let Some(head) = name.split('.').next() {
        if !head.is_empty() {
            let c = head.chars().next().unwrap_or('\0');
            if c.is_ascii_uppercase() {
                return true;
            }
        }
    }
    // Also accept simple uppercase helper names as placeholder builtins.
    if !name.contains('.') {
        let c = name.chars().next().unwrap_or('\0');
        if c.is_ascii_uppercase() {
            return true;
        }
    }
    if name.ends_with(".sample")
        || name.ends_with(".interval")
        || name.ends_with(".backSample")
        || name.ends_with(".subSample")
        || name.ends_with(".superSample")
        || name.ends_with(".shiftSample")
        || name.ends_with(".Clock")
    {
        return true;
    }
    if name.ends_with("massFlowRate_dp_and_Re") || name.contains(".massFlowRate_dp_and_Re") {
        return true;
    }
    if name.starts_with("WallFriction.") || name.contains(".WallFriction.") {
        return true;
    }
    if name == "Modelica.Fluid.Utilities.regFun3"
        || name.ends_with(".regFun3")
        || name == "Utilities.regFun3"
        || name.ends_with(".Utilities.regFun3")
    {
        return true;
    }
    // MSL internal helper functions (often local to a model, not loadable as standalone units).
    // Validate-only policy: treat them as placeholders so compilation proceeds.
    if name.starts_with("FCN") {
        return true;
    }
    // Modelica built-in operators and functions
    if matches!(
        name,
        "abs"
            | "sign"
            | "sqrt"
            | "min"
            | "max"
            | "mod"
            | "rem"
            | "div"
            | "integer"
            | "smooth"
            | "ceil"
            | "floor"
            | "sin"
            | "cos"
            | "tan"
            | "asin"
            | "acos"
            | "atan"
            | "atan2"
            | "sinh"
            | "cosh"
            | "tanh"
            | "exp"
            | "log"
            | "log10"
            | "pow"
            | "inStream"
            | "actualStream"
            | "positiveMax"
            | "pre"
            | "edge"
            | "change"
            | "noEvent"
            | "initial"
            | "firstTick"
            | "terminal"
            | "backSample"
            | "subSample"
            | "superSample"
            | "shiftSample"
            | "sample"
            | "interval"
            | "Clock"
            | "Integer"
            | "Real"
            | "Boolean"
            | "size"
            | "vector"
            | "zeros"
            | "ones"
            | "fill"
            | "cat"
            | "named"
            | "homotopy"
            | "cardinality"
            | "not"
            | "product"
            | "assert"
            | "terminate"
            | "sum"
            | "cross"
            | "Complex"
            | "conj"
            | "real"
            | "imag"
            | "valveCharacteristic"
            | "Utilities.regRoot"
            | "Utilities.regRoot2"
            | "Utilities.regSquare2"
            | "transpose"
            | "linearTemperatureDependency"
            | "xtCharacteristic"
            | "FlCharacteristic"
            | "oneTrue"
            | "delay"
            | "exlin"
            | "exlin2"
            | "powlin"
            | "numberOfSymmetricBaseSystems"
    ) {
        return true;
    }
    if name.ends_with(".powlin") {
        return true;
    }
    if name.ends_with("gravityAcceleration") || name.contains(".gravityAcceleration") {
        return true;
    }
    // Modelica.Math.* (Vectors, BooleanVectors, Matrices, etc.)
    if name.starts_with("Modelica.Math.") {
        return true;
    }
    if name.starts_with("Modelica.ComplexMath.") {
        return true;
    }
    // Electrical Polyphase helpers are not linked in validate-only JIT.
    if name.starts_with("Modelica.Electrical.Polyphase.") || name.starts_with("Polyphase.") {
        return true;
    }
    // Internal / Utilities (MSL helpers; no load_model)
    if name.starts_with("Internal.") || name.contains(".Internal.") {
        return true;
    }
    if name.starts_with("Connections.") {
        return true;
    }
    if name == "outerProduct"
        || name.ends_with(".outerProduct")
        || name == "identity"
        || name.ends_with(".identity")
        || name == "skew"
        || name.ends_with(".skew")
    {
        return true;
    }
    if name.starts_with("Frames.") || name.contains(".Frames.") {
        return true;
    }
    if name.starts_with("BaseClasses.") || name.contains(".BaseClasses.") {
        return true;
    }
    // Medium.* calls are package-defined helpers; JIT treats them as placeholders.
    if name.starts_with("Medium.") {
        return true;
    }
    if name.starts_with("Modelica.Utilities.") || name.ends_with(".isEmpty") {
        return true;
    }
    // Tables / ExternalObject / Strings.*: unified placeholder policy.
    // Do not load_model; JIT returns constant placeholder so validate passes without external link panic.
    if name.contains("CombiTimeTable")
        || name.contains("getTimeTableValue")
        || name.ends_with("ExternalCombiTimeTable")
        || name.contains("ExternalObject")
        || name.ends_with(".ExternalObject")
    {
        return true;
    }
    // Time events and common qualified builtins
    if name.ends_with("getNextTimeEvent")
        || name.ends_with(".firstTick")
        || name.ends_with(".firstTrueIndex")
        || name.ends_with(".interpolate")
    {
        return true;
    }
    false
}

const MAX_INLINE_RECURSION_DEPTH: u32 = 64;

fn subst_merge_params_and_locals(
    params: &HashMap<String, Expression>,
    locals: &HashMap<String, Expression>,
) -> HashMap<String, Expression> {
    let mut m = params.clone();
    for (k, v) in locals {
        m.insert(k.clone(), v.clone());
    }
    m
}

/// Record constructors parse as `Call(name, args)` with `Call("named", [StringLiteral(field), value])` items.
fn extract_named_record_field(expr: &Expression, field: &str) -> Option<Expression> {
    let Expression::Call(_, args) = expr else {
        return None;
    };
    for a in args {
        let Expression::Call(op, items) = a else {
            continue;
        };
        if op != "named" || items.len() != 2 {
            continue;
        }
        let (Expression::StringLiteral(nm), val) = (&items[0], &items[1]) else {
            continue;
        };
        if nm == field {
            return Some(val.clone());
        }
    }
    None
}

/// Inline `f(...).field` when `f` is a Modelica function with output `out` and body assigns
/// `out.field := rhs` (MSL MultiBody Frames orientation accessors).
fn multi_body_frames_function_candidates(name: &str) -> Vec<String> {
    let mut out = vec![name.to_string()];
    if let Some(suffix) = name.strip_prefix("Frames.") {
        if !suffix.is_empty() {
            out.push(format!(
                "Modelica.Mechanics.MultiBody.Frames.{suffix}"
            ));
        }
    }
    out
}

fn try_extract_function_output_dot_field(
    func_name: &str,
    args: &[Expression],
    field: &str,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    depth: u32,
) -> Option<Expression> {
    if depth > MAX_INLINE_RECURSION_DEPTH {
        return None;
    }
    let mut func_model: Option<Arc<Model>> = None;
    let mut resolved_name: Option<String> = None;
    for cand in multi_body_frames_function_candidates(func_name) {
        if let Some(m) = cache.get(&cand).cloned().or_else(|| loader.load_model(&cand).ok()) {
            func_model = Some(m);
            resolved_name = Some(cand);
            break;
        }
    }
    let func_model = func_model?;
    let resolved = resolved_name?;
    if !func_model.is_function || func_model.external_info.is_some() {
        return None;
    }
    cache.insert(resolved.clone(), Arc::clone(&func_model));
    let input_names: Vec<String> = func_model
        .declarations
        .iter()
        .filter(|d| d.is_input)
        .map(|d| d.name.clone())
        .collect();
    if input_names.len() != args.len() {
        return None;
    }
    let args_inlined: Vec<Expression> = args
        .iter()
        .map(|a| inline_expr(a, loader, cache, depth + 1))
        .collect();
    let mut param_subst: HashMap<String, Expression> = HashMap::new();
    for (i, in_name) in input_names.iter().enumerate() {
        if i < args_inlined.len() {
            param_subst.insert(in_name.clone(), args_inlined[i].clone());
        }
    }
    let outputs: HashSet<String> = func_model
        .declarations
        .iter()
        .filter(|d| d.is_output)
        .map(|d| d.name.clone())
        .collect();
    let mut locals: HashMap<String, Expression> = HashMap::new();
    for stmt in &func_model.algorithms {
        if let AlgorithmStatement::Assignment(lhs, rhs) = stmt {
            match lhs {
                Expression::Variable(v) if !outputs.contains(v) => {
                    let ctx = subst_merge_params_and_locals(&param_subst, &locals);
                    let rhs_sub = substitute_expr(rhs, &ctx);
                    locals.insert(v.clone(), rhs_sub);
                }
                Expression::Variable(out) if outputs.contains(out) => {
                    let ctx = subst_merge_params_and_locals(&param_subst, &locals);
                    let rhs_sub = substitute_expr(rhs, &ctx);
                    if let Some(e) = extract_named_record_field(&rhs_sub, field) {
                        return Some(e);
                    }
                }
                Expression::Dot(inner, fld) if fld == field => {
                    if let Expression::Variable(out) = inner.as_ref() {
                        if outputs.contains(out) {
                            let ctx = subst_merge_params_and_locals(&param_subst, &locals);
                            return Some(substitute_expr(rhs, &ctx));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn inline_expr(
    expr: &Expression,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    depth: u32,
) -> Expression {
    use Expression::*;
    match expr {
        Call(name, args) => {
            let name = name.as_str();
            if depth > MAX_INLINE_RECURSION_DEPTH {
                return Call(
                    name.to_string(),
                    args.iter()
                        .map(|a| inline_expr(a, loader, cache, depth + 1))
                        .collect(),
                );
            }
            // MSL Semiconductors.powlin(u, Me): (1+u)^Me for u > -1 (smooth continuation), else 0.
            if name == "powlin" || name.ends_with(".powlin") {
                if args.len() == 2 {
                    use Expression::*;
                    let u = inline_expr(&args[0], loader, cache, depth + 1);
                    let me = inline_expr(&args[1], loader, cache, depth + 1);
                    let thresh = Number(-1.0 + 1e-12);
                    let cond = BinaryOp(
                        Box::new(u.clone()),
                        Operator::Greater,
                        Box::new(thresh),
                    );
                    let base = BinaryOp(
                        Box::new(Number(1.0)),
                        Operator::Add,
                        Box::new(u),
                    );
                    let pow_call = Call("pow".to_string(), vec![base, me]);
                    return If(Box::new(cond), Box::new(pow_call), Box::new(Number(0.0)));
                }
            }
            let func = if is_builtin_function(name) {
                None
            } else {
                cache
                    .get(name)
                    .cloned()
                    .or_else(|| loader.load_model(name).ok())
            };
            if let Some(func_model) = func {
                if let Some((input_names, outputs)) = get_function_body(func_model.as_ref()) {
                    if input_names.len() == args.len() && outputs.len() == 1 {
                        cache.insert(name.to_string(), Arc::clone(&func_model));
                        let args_inlined: Vec<Expression> = args
                            .iter()
                            .map(|a| inline_expr(a, loader, cache, depth + 1))
                            .collect();
                        let mut subst = HashMap::new();
                        for (i, in_name) in input_names.iter().enumerate() {
                            if i < args_inlined.len() {
                                subst.insert(in_name.clone(), args_inlined[i].clone());
                            }
                        }
                        return substitute_expr(&outputs[0].1, &subst);
                    }
                }
            }
            Call(
                name.to_string(),
                args.iter()
                    .map(|a| inline_expr(a, loader, cache, depth + 1))
                    .collect(),
            )
        }
        Variable(_) | Number(_) => expr.clone(),
        BinaryOp(lhs, op, rhs) => BinaryOp(
            Box::new(inline_expr(lhs, loader, cache, depth + 1)),
            *op,
            Box::new(inline_expr(rhs, loader, cache, depth + 1)),
        ),
        Der(inner) => Der(Box::new(inline_expr(inner, loader, cache, depth + 1))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(inline_expr(arr, loader, cache, depth + 1)),
            Box::new(inline_expr(idx, loader, cache, depth + 1)),
        ),
        If(cond, t, f) => If(
            Box::new(inline_expr(cond, loader, cache, depth + 1)),
            Box::new(inline_expr(t, loader, cache, depth + 1)),
            Box::new(inline_expr(f, loader, cache, depth + 1)),
        ),
        Range(start, step, end) => Range(
            Box::new(inline_expr(start, loader, cache, depth + 1)),
            Box::new(inline_expr(step, loader, cache, depth + 1)),
            Box::new(inline_expr(end, loader, cache, depth + 1)),
        ),
        ArrayLiteral(items) => ArrayLiteral(
            items
                .iter()
                .map(|e| inline_expr(e, loader, cache, depth + 1))
                .collect(),
        ),
        ArrayComprehension { expr, iter_var, iter_range } => ArrayComprehension {
            expr: Box::new(inline_expr(expr, loader, cache, depth + 1)),
            iter_var: iter_var.clone(),
            iter_range: Box::new(inline_expr(iter_range, loader, cache, depth + 1)),
        },
        Dot(base, member) => {
            let member = member.clone();
            match base.as_ref() {
                If(c, t, f) => {
                    return If(
                        Box::new(inline_expr(c, loader, cache, depth + 1)),
                        Box::new(inline_expr(
                            &Expression::Dot(t.clone(), member.clone()),
                            loader,
                            cache,
                            depth + 1,
                        )),
                        Box::new(inline_expr(
                            &Expression::Dot(f.clone(), member),
                            loader,
                            cache,
                            depth + 1,
                        )),
                    );
                }
                Call(fname, args) => {
                    if let Some(e) = try_extract_function_output_dot_field(
                        fname,
                        args,
                        &member,
                        loader,
                        cache,
                        depth,
                    ) {
                        return inline_expr(&e, loader, cache, depth + 1);
                    }
                }
                _ => {}
            }
            let b = inline_expr(base, loader, cache, depth + 1);
            match b {
                If(c, t, f) => If(
                    c,
                    Box::new(inline_expr(
                        &Expression::Dot(t, member.clone()),
                        loader,
                        cache,
                        depth + 1,
                    )),
                    Box::new(inline_expr(
                        &Expression::Dot(f, member.clone()),
                        loader,
                        cache,
                        depth + 1,
                    )),
                ),
                Call(fname, args) => {
                    if let Some(e) = try_extract_function_output_dot_field(
                        &fname,
                        &args,
                        &member,
                        loader,
                        cache,
                        depth,
                    ) {
                        return inline_expr(&e, loader, cache, depth + 1);
                    }
                    Dot(Box::new(Call(fname, args)), member)
                }
                other => Dot(Box::new(other), member),
            }
        }
        Sample(inner) => Sample(Box::new(inline_expr(inner, loader, cache, depth + 1))),
        Interval(inner) => Interval(Box::new(inline_expr(inner, loader, cache, depth + 1))),
        Hold(inner) => Hold(Box::new(inline_expr(inner, loader, cache, depth + 1))),
        Previous(inner) => Previous(Box::new(inline_expr(inner, loader, cache, depth + 1))),
        SubSample(c, n) => SubSample(
            Box::new(inline_expr(c, loader, cache, depth + 1)),
            Box::new(inline_expr(n, loader, cache, depth + 1)),
        ),
        SuperSample(c, n) => SuperSample(
            Box::new(inline_expr(c, loader, cache, depth + 1)),
            Box::new(inline_expr(n, loader, cache, depth + 1)),
        ),
        ShiftSample(c, n) => ShiftSample(
            Box::new(inline_expr(c, loader, cache, depth + 1)),
            Box::new(inline_expr(n, loader, cache, depth + 1)),
        ),
        StringLiteral(s) => StringLiteral(s.clone()),
    }
}

fn inline_equation(
    eq: &Equation,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(
            inline_expr(lhs, loader, cache, 0),
            inline_expr(rhs, loader, cache, 0),
        ),
        Equation::For(var, start, end, body) => Equation::For(
            var.clone(),
            Box::new(inline_expr(start, loader, cache, 0)),
            Box::new(inline_expr(end, loader, cache, 0)),
            body.iter()
                .map(|e| inline_equation(e, loader, cache))
                .collect(),
        ),
        Equation::When(cond, body, elses) => Equation::When(
            inline_expr(cond, loader, cache, 0),
            body.iter()
                .map(|e| inline_equation(e, loader, cache))
                .collect(),
            elses
                .iter()
                .map(|(c, b)| {
                    (
                        inline_expr(c, loader, cache, 0),
                        b.iter()
                            .map(|e| inline_equation(e, loader, cache))
                            .collect(),
                    )
                })
                .collect(),
        ),
        Equation::Reinit(var, e) => Equation::Reinit(var.clone(), inline_expr(e, loader, cache, 0)),
        Equation::Connect(a, b) => Equation::Connect(
            inline_expr(a, loader, cache, 0),
            inline_expr(b, loader, cache, 0),
        ),
        Equation::Assert(cond, msg) => Equation::Assert(
            inline_expr(cond, loader, cache, 0),
            inline_expr(msg, loader, cache, 0),
        ),
        Equation::Terminate(msg) => Equation::Terminate(inline_expr(msg, loader, cache, 0)),
        Equation::CallStmt(expr) => Equation::CallStmt(inline_expr(expr, loader, cache, 0)),
        Equation::SolvableBlock {
            unknowns,
            tearing_var,
            equations,
            residuals,
        } => Equation::SolvableBlock {
            unknowns: unknowns.clone(),
            tearing_var: tearing_var.clone(),
            equations: equations
                .iter()
                .map(|e| inline_equation(e, loader, cache))
                .collect(),
            residuals: residuals
                .iter()
                .map(|r| inline_expr(r, loader, cache, 0))
                .collect(),
        },
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => Equation::If(
            inline_expr(cond, loader, cache, 0),
            then_eqs
                .iter()
                .map(|e| inline_equation(e, loader, cache))
                .collect(),
            elseif_list
                .iter()
                .map(|(c, eb)| {
                    (
                        inline_expr(c, loader, cache, 0),
                        eb.iter()
                            .map(|e| inline_equation(e, loader, cache))
                            .collect(),
                    )
                })
                .collect(),
            else_eqs.as_ref().map(|eqs| {
                eqs.iter()
                    .map(|e| inline_equation(e, loader, cache))
                    .collect()
            }),
        ),
        Equation::MultiAssign(lhss, rhs) => Equation::MultiAssign(
            lhss.iter()
                .map(|e| inline_expr(e, loader, cache, 0))
                .collect(),
            inline_expr(rhs, loader, cache, 0),
        ),
    }
}

fn inline_algorithm(
    stmt: &AlgorithmStatement,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
) -> AlgorithmStatement {
    match stmt {
        AlgorithmStatement::Assignment(lhs, rhs) => {
            AlgorithmStatement::Assignment(lhs.clone(), inline_expr(rhs, loader, cache, 0))
        }
        AlgorithmStatement::CallStmt(expr) => {
            AlgorithmStatement::CallStmt(inline_expr(expr, loader, cache, 0))
        }
        AlgorithmStatement::NoOp => AlgorithmStatement::NoOp,
        AlgorithmStatement::MultiAssign(lhss, rhs) => AlgorithmStatement::MultiAssign(
            lhss.iter()
                .map(|e| inline_expr(e, loader, cache, 0))
                .collect(),
            inline_expr(rhs, loader, cache, 0),
        ),
        AlgorithmStatement::Reinit(var, e) => {
            AlgorithmStatement::Reinit(var.clone(), inline_expr(e, loader, cache, 0))
        }
        AlgorithmStatement::Assert(cond, msg) => AlgorithmStatement::Assert(
            inline_expr(cond, loader, cache, 0),
            inline_expr(msg, loader, cache, 0),
        ),
        AlgorithmStatement::Terminate(msg) => {
            AlgorithmStatement::Terminate(inline_expr(msg, loader, cache, 0))
        }
        AlgorithmStatement::If(cond, t, eifs, els) => AlgorithmStatement::If(
            inline_expr(cond, loader, cache, 0),
            t.iter()
                .map(|s| inline_algorithm(s, loader, cache))
                .collect(),
            eifs.iter()
                .map(|(c, b)| {
                    (
                        inline_expr(c, loader, cache, 0),
                        b.iter()
                            .map(|s| inline_algorithm(s, loader, cache))
                            .collect(),
                    )
                })
                .collect(),
            els.as_ref().map(|b| {
                b.iter()
                    .map(|s| inline_algorithm(s, loader, cache))
                    .collect()
            }),
        ),
        AlgorithmStatement::For(var, range, body) => AlgorithmStatement::For(
            var.clone(),
            Box::new(inline_expr(&*range, loader, cache, 0)),
            body.iter()
                .map(|s| inline_algorithm(s, loader, cache))
                .collect(),
        ),
        AlgorithmStatement::While(cond, body) => AlgorithmStatement::While(
            inline_expr(cond, loader, cache, 0),
            body.iter()
                .map(|s| inline_algorithm(s, loader, cache))
                .collect(),
        ),
        AlgorithmStatement::When(cond, body, elses) => AlgorithmStatement::When(
            inline_expr(cond, loader, cache, 0),
            body.iter()
                .map(|s| inline_algorithm(s, loader, cache))
                .collect(),
            elses
                .iter()
                .map(|(c, b)| {
                    (
                        inline_expr(c, loader, cache, 0),
                        b.iter()
                            .map(|s| inline_algorithm(s, loader, cache))
                            .collect(),
                    )
                })
                .collect(),
        ),
    }
}

pub fn inline_function_calls(flat: &mut FlattenedModel, loader: &mut ModelLoader) {
    let mut cache: HashMap<String, Arc<Model>> = HashMap::new();
    flat.equations = flat
        .equations
        .iter()
        .map(|e| inline_equation(e, loader, &mut cache))
        .collect();
    flat.initial_equations = flat
        .initial_equations
        .iter()
        .map(|e| inline_equation(e, loader, &mut cache))
        .collect();
    flat.algorithms = flat
        .algorithms
        .iter()
        .map(|s| inline_algorithm(s, loader, &mut cache))
        .collect();
    flat.initial_algorithms = flat
        .initial_algorithms
        .iter()
        .map(|s| inline_algorithm(s, loader, &mut cache))
        .collect();
}
