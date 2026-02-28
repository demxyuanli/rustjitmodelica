use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::{AlgorithmStatement, Equation, Expression, Model};
use crate::flatten::FlattenedModel;
use crate::loader::ModelLoader;

fn substitute_expr(expr: &Expression, subst: &HashMap<String, Expression>) -> Expression {
    use Expression::*;
    match expr {
        Variable(name) => subst.get(name).cloned().unwrap_or_else(|| Variable(name.clone())),
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
        ArrayLiteral(items) => ArrayLiteral(items.iter().map(|e| substitute_expr(e, subst)).collect()),
        Dot(base, member) => Dot(Box::new(substitute_expr(base, subst)), member.clone()),
    }
}

pub(crate) fn get_function_body(model: &Model) -> Option<(Vec<String>, Expression)> {
    if !model.is_function {
        return None;
    }
    let input_names: Vec<String> = model.declarations.iter().filter(|d| d.is_input).map(|d| d.name.clone()).collect();
    let output_names: Vec<String> = model.declarations.iter().filter(|d| d.is_output).map(|d| d.name.clone()).collect();
    if output_names.len() != 1 {
        return None;
    }
    let output_name = &output_names[0];
    for stmt in &model.algorithms {
        if let AlgorithmStatement::Assignment(lhs, rhs) = stmt {
            if let Expression::Variable(v) = lhs {
                if v == output_name {
                    return Some((input_names, rhs.clone()));
                }
            }
        }
    }
    None
}

fn is_builtin_function(name: &str) -> bool {
    matches!(name,
        "abs" | "sign" | "sqrt" | "min" | "max" | "mod" | "rem" | "div" | "integer"
        | "ceil" | "floor" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2"
        | "sinh" | "cosh" | "tanh" | "exp" | "log" | "log10"
        | "pre" | "edge" | "change" | "noEvent" | "initial" | "terminal"
    ) || name.starts_with("Modelica.Math.")
}

fn inline_expr(expr: &Expression, loader: &mut ModelLoader, cache: &mut HashMap<String, Arc<Model>>) -> Expression {
    use Expression::*;
    match expr {
        Call(name, args) => {
            let name = name.as_str();
            let func = if is_builtin_function(name) {
                None
            } else {
                cache.get(name).cloned().or_else(|| loader.load_model(name).ok())
            };
            if let Some(func_model) = func {
                if let Some((input_names, output_expr)) = get_function_body(func_model.as_ref()) {
                    if input_names.len() == args.len() {
                        cache.insert(name.to_string(), Arc::clone(&func_model));
                        let args_inlined: Vec<Expression> = args.iter().map(|a| inline_expr(a, loader, cache)).collect();
                        let mut subst = HashMap::new();
                        for (i, in_name) in input_names.iter().enumerate() {
                            if i < args_inlined.len() {
                                subst.insert(in_name.clone(), args_inlined[i].clone());
                            }
                        }
                        return substitute_expr(&output_expr, &subst);
                    }
                }
            }
            Call(name.to_string(), args.iter().map(|a| inline_expr(a, loader, cache)).collect())
        }
        Variable(_) | Number(_) => expr.clone(),
        BinaryOp(lhs, op, rhs) => BinaryOp(
            Box::new(inline_expr(lhs, loader, cache)),
            *op,
            Box::new(inline_expr(rhs, loader, cache)),
        ),
        Der(inner) => Der(Box::new(inline_expr(inner, loader, cache))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(inline_expr(arr, loader, cache)),
            Box::new(inline_expr(idx, loader, cache)),
        ),
        If(cond, t, f) => If(
            Box::new(inline_expr(cond, loader, cache)),
            Box::new(inline_expr(t, loader, cache)),
            Box::new(inline_expr(f, loader, cache)),
        ),
        Range(start, step, end) => Range(
            Box::new(inline_expr(start, loader, cache)),
            Box::new(inline_expr(step, loader, cache)),
            Box::new(inline_expr(end, loader, cache)),
        ),
        ArrayLiteral(items) => ArrayLiteral(items.iter().map(|e| inline_expr(e, loader, cache)).collect()),
        Dot(base, member) => Dot(Box::new(inline_expr(base, loader, cache)), member.clone()),
    }
}

fn inline_equation(eq: &Equation, loader: &mut ModelLoader, cache: &mut HashMap<String, Arc<Model>>) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(
            inline_expr(lhs, loader, cache),
            inline_expr(rhs, loader, cache),
        ),
        Equation::For(var, start, end, body) => Equation::For(
            var.clone(),
            Box::new(inline_expr(start, loader, cache)),
            Box::new(inline_expr(end, loader, cache)),
            body.iter().map(|e| inline_equation(e, loader, cache)).collect(),
        ),
        Equation::When(cond, body, elses) => Equation::When(
            inline_expr(cond, loader, cache),
            body.iter().map(|e| inline_equation(e, loader, cache)).collect(),
            elses.iter().map(|(c, b)| (inline_expr(c, loader, cache), b.iter().map(|e| inline_equation(e, loader, cache)).collect())).collect(),
        ),
        Equation::Reinit(var, e) => Equation::Reinit(var.clone(), inline_expr(e, loader, cache)),
        Equation::Connect(a, b) => Equation::Connect(inline_expr(a, loader, cache), inline_expr(b, loader, cache)),
        Equation::Assert(cond, msg) => Equation::Assert(inline_expr(cond, loader, cache), inline_expr(msg, loader, cache)),
        Equation::Terminate(msg) => Equation::Terminate(inline_expr(msg, loader, cache)),
        Equation::SolvableBlock { unknowns, tearing_var, equations, residuals } => Equation::SolvableBlock {
            unknowns: unknowns.clone(),
            tearing_var: tearing_var.clone(),
            equations: equations.iter().map(|e| inline_equation(e, loader, cache)).collect(),
            residuals: residuals.iter().map(|r| inline_expr(r, loader, cache)).collect(),
        },
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => Equation::If(
            inline_expr(cond, loader, cache),
            then_eqs.iter().map(|e| inline_equation(e, loader, cache)).collect(),
            elseif_list.iter().map(|(c, eb)| (
                inline_expr(c, loader, cache),
                eb.iter().map(|e| inline_equation(e, loader, cache)).collect(),
            )).collect(),
            else_eqs.as_ref().map(|eqs| eqs.iter().map(|e| inline_equation(e, loader, cache)).collect()),
        ),
    }
}

fn inline_algorithm(stmt: &AlgorithmStatement, loader: &mut ModelLoader, cache: &mut HashMap<String, Arc<Model>>) -> AlgorithmStatement {
    match stmt {
        AlgorithmStatement::Assignment(lhs, rhs) => AlgorithmStatement::Assignment(
            lhs.clone(),
            inline_expr(rhs, loader, cache),
        ),
        AlgorithmStatement::Reinit(var, e) => AlgorithmStatement::Reinit(var.clone(), inline_expr(e, loader, cache)),
        AlgorithmStatement::Assert(cond, msg) => AlgorithmStatement::Assert(
            inline_expr(cond, loader, cache),
            inline_expr(msg, loader, cache),
        ),
        AlgorithmStatement::Terminate(msg) => AlgorithmStatement::Terminate(inline_expr(msg, loader, cache)),
        AlgorithmStatement::If(cond, t, eifs, els) => AlgorithmStatement::If(
            inline_expr(cond, loader, cache),
            t.iter().map(|s| inline_algorithm(s, loader, cache)).collect(),
            eifs.iter().map(|(c, b)| (inline_expr(c, loader, cache), b.iter().map(|s| inline_algorithm(s, loader, cache)).collect())).collect(),
            els.as_ref().map(|b| b.iter().map(|s| inline_algorithm(s, loader, cache)).collect()),
        ),
        AlgorithmStatement::For(var, range, body) => AlgorithmStatement::For(
            var.clone(),
            Box::new(inline_expr(&*range, loader, cache)),
            body.iter().map(|s| inline_algorithm(s, loader, cache)).collect(),
        ),
        AlgorithmStatement::While(cond, body) => AlgorithmStatement::While(
            inline_expr(cond, loader, cache),
            body.iter().map(|s| inline_algorithm(s, loader, cache)).collect(),
        ),
        AlgorithmStatement::When(cond, body, elses) => AlgorithmStatement::When(
            inline_expr(cond, loader, cache),
            body.iter().map(|s| inline_algorithm(s, loader, cache)).collect(),
            elses.iter().map(|(c, b)| (inline_expr(c, loader, cache), b.iter().map(|s| inline_algorithm(s, loader, cache)).collect())).collect(),
        ),
    }
}

pub fn inline_function_calls(flat: &mut FlattenedModel, loader: &mut ModelLoader) {
    let mut cache: HashMap<String, Arc<Model>> = HashMap::new();
    flat.equations = flat.equations.iter().map(|e| inline_equation(e, loader, &mut cache)).collect();
    flat.initial_equations = flat.initial_equations.iter().map(|e| inline_equation(e, loader, &mut cache)).collect();
    flat.algorithms = flat.algorithms.iter().map(|s| inline_algorithm(s, loader, &mut cache)).collect();
    flat.initial_algorithms = flat.initial_algorithms.iter().map(|s| inline_algorithm(s, loader, &mut cache)).collect();
}
