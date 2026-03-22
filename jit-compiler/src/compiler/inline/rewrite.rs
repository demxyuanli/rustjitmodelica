use crate::ast::{AlgorithmStatement, Expression, Model, Operator};
use crate::loader::ModelLoader;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::builtin::is_builtin_function;
use super::function_body::get_function_body;
use super::record_access::{
    extract_named_record_field, try_extract_record_constructor_dot_field,
};
use super::subst::substitute_expr;

pub(super) fn subst_merge_params_and_locals(
    params: &HashMap<String, Expression>,
    locals: &HashMap<String, Expression>,
) -> HashMap<String, Expression> {
    let mut m = params.clone();
    for (k, v) in locals {
        m.insert(k.clone(), v.clone());
    }
    m
}

pub(super) fn function_resolution_candidates(name: &str) -> Vec<String> {
    let mut out = vec![name.to_string()];
    if let Some(suffix) = name.strip_prefix("Frames.") {
        if !suffix.is_empty() {
            out.push(format!("Modelica.Mechanics.MultiBody.Frames.{suffix}"));
        }
    }
    if let Some(suffix) = name.strip_prefix("FluxTubes.") {
        if !suffix.is_empty() {
            out.push(format!("Modelica.Magnetic.FluxTubes.{suffix}"));
        }
    }
    out
}

pub(super) fn resolve_modelica_uri(uri: &str, library_paths: &[PathBuf]) -> String {
    if let Some(rest) = uri.strip_prefix("modelica://") {
        let fs_rel = rest.replace('/', std::path::MAIN_SEPARATOR_STR);
        for lib in library_paths {
            let candidate = lib.join(&fs_rel);
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    uri.to_string()
}

pub(super) fn try_extract_function_output_dot_field(
    func_name: &str,
    args: &[Expression],
    field: &str,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    depth: u32,
    max_depth: u32,
) -> Option<Expression> {
    if depth > max_depth {
        return None;
    }
    let mut func_model: Option<Arc<Model>> = None;
    let mut resolved_name: Option<String> = None;
    for cand in function_resolution_candidates(func_name) {
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
        .map(|a| inline_expr(a, loader, cache, depth + 1, max_depth))
        .collect();
    let mut param_subst: HashMap<String, Expression> = HashMap::new();
    for (i, in_name) in input_names.iter().enumerate() {
        if i < args_inlined.len() {
            param_subst.insert(in_name.clone(), args_inlined[i].clone());
        }
    }
    let outputs: std::collections::HashSet<String> = func_model
        .declarations
        .iter()
        .filter(|d| d.is_output)
        .map(|d| d.name.clone())
        .collect();
    let mut locals: HashMap<String, Expression> = HashMap::new();
    for stmt in &func_model.algorithms {
        if let AlgorithmStatement::Assignment(lhs, rhs) = stmt {
            match lhs {
                Expression::Variable(id) => {
                    let name = crate::string_intern::resolve_id(*id);
                    if outputs.contains(&name) {
                        let ctx = subst_merge_params_and_locals(&param_subst, &locals);
                        let rhs_sub = substitute_expr(rhs, &ctx);
                        if let Some(e) = extract_named_record_field(&rhs_sub, field) {
                            return Some(e);
                        }
                    } else {
                        let ctx = subst_merge_params_and_locals(&param_subst, &locals);
                        let rhs_sub = substitute_expr(rhs, &ctx);
                        locals.insert(name, rhs_sub);
                    }
                }
                Expression::Dot(inner, fld) if fld == field => {
                    if let Expression::Variable(id) = inner.as_ref() {
                        let out = crate::string_intern::resolve_id(*id);
                        if outputs.contains(&out) {
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

pub(super) fn inline_expr(
    expr: &Expression,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    depth: u32,
    max_depth: u32,
) -> Expression {
    use Expression::*;
    match expr {
        Call(name, args) => {
            let name = name.as_str();
            if depth > max_depth {
                return Call(
                    name.to_string(),
                    args.iter()
                        .map(|a| inline_expr(a, loader, cache, depth + 1, max_depth))
                        .collect(),
                );
            }
            if (name == "loadResource"
                || name == "Modelica.Utilities.Files.loadResource"
                || name.ends_with(".loadResource"))
                && args.len() == 1
            {
                if let StringLiteral(uri) = &args[0] {
                    return StringLiteral(resolve_modelica_uri(uri, &loader.library_paths));
                }
                let inlined_arg = inline_expr(&args[0], loader, cache, depth + 1, max_depth);
                if let StringLiteral(uri) = &inlined_arg {
                    return StringLiteral(resolve_modelica_uri(uri, &loader.library_paths));
                }
                return inlined_arg;
            }
            if name == "powlin" || name.ends_with(".powlin") {
                if args.len() == 2 {
                    let u = inline_expr(&args[0], loader, cache, depth + 1, max_depth);
                    let me = inline_expr(&args[1], loader, cache, depth + 1, max_depth);
                    let thresh = Number(-1.0 + 1e-12);
                    let cond = BinaryOp(Box::new(u.clone()), Operator::Greater, Box::new(thresh));
                    let base = BinaryOp(Box::new(Number(1.0)), Operator::Add, Box::new(u));
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
                            .map(|a| inline_expr(a, loader, cache, depth + 1, max_depth))
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
                    .map(|a| inline_expr(a, loader, cache, depth + 1, max_depth))
                    .collect(),
            )
        }
        Variable(_) | Number(_) => expr.clone(),
        BinaryOp(lhs, op, rhs) => BinaryOp(
            Box::new(inline_expr(lhs, loader, cache, depth + 1, max_depth)),
            *op,
            Box::new(inline_expr(rhs, loader, cache, depth + 1, max_depth)),
        ),
        Der(inner) => Der(Box::new(inline_expr(inner, loader, cache, depth + 1, max_depth))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(inline_expr(arr, loader, cache, depth + 1, max_depth)),
            Box::new(inline_expr(idx, loader, cache, depth + 1, max_depth)),
        ),
        If(cond, t, f) => If(
            Box::new(inline_expr(cond, loader, cache, depth + 1, max_depth)),
            Box::new(inline_expr(t, loader, cache, depth + 1, max_depth)),
            Box::new(inline_expr(f, loader, cache, depth + 1, max_depth)),
        ),
        Range(start, step, end) => Range(
            Box::new(inline_expr(start, loader, cache, depth + 1, max_depth)),
            Box::new(inline_expr(step, loader, cache, depth + 1, max_depth)),
            Box::new(inline_expr(end, loader, cache, depth + 1, max_depth)),
        ),
        ArrayLiteral(items) => ArrayLiteral(
            items
                .iter()
                .map(|e| inline_expr(e, loader, cache, depth + 1, max_depth))
                .collect(),
        ),
        ArrayComprehension { expr, iter_var, iter_range } => ArrayComprehension {
            expr: Box::new(inline_expr(expr, loader, cache, depth + 1, max_depth)),
            iter_var: iter_var.clone(),
            iter_range: Box::new(inline_expr(iter_range, loader, cache, depth + 1, max_depth)),
        },
        Dot(base, member) => {
            let member = member.clone();
            match base.as_ref() {
                If(c, t, f) => {
                    return If(
                        Box::new(inline_expr(c, loader, cache, depth + 1, max_depth)),
                        Box::new(inline_expr(
                            &Expression::Dot(t.clone(), member.clone()),
                            loader,
                            cache,
                            depth + 1,
                            max_depth,
                        )),
                        Box::new(inline_expr(
                            &Expression::Dot(f.clone(), member),
                            loader,
                            cache,
                            depth + 1,
                            max_depth,
                        )),
                    );
                }
                Call(fname, args) => {
                    if let Some(e) = try_extract_function_output_dot_field(
                        fname, args, &member, loader, cache, depth, max_depth,
                    ) {
                        return inline_expr(&e, loader, cache, depth + 1, max_depth);
                    }
                    if let Some(e) =
                        try_extract_record_constructor_dot_field(fname, args, &member, loader, cache)
                    {
                        return inline_expr(&e, loader, cache, depth + 1, max_depth);
                    }
                }
                _ => {}
            }
            let b = inline_expr(base, loader, cache, depth + 1, max_depth);
            match b {
                If(c, t, f) => If(
                    c,
                    Box::new(inline_expr(
                        &Expression::Dot(t, member.clone()),
                        loader,
                        cache,
                        depth + 1,
                        max_depth,
                    )),
                    Box::new(inline_expr(
                        &Expression::Dot(f, member.clone()),
                        loader,
                        cache,
                        depth + 1,
                        max_depth,
                    )),
                ),
                Call(fname, args) => {
                    if let Some(e) = try_extract_function_output_dot_field(
                        &fname, &args, &member, loader, cache, depth, max_depth,
                    ) {
                        return inline_expr(&e, loader, cache, depth + 1, max_depth);
                    }
                    if let Some(e) = try_extract_record_constructor_dot_field(
                        &fname, &args, &member, loader, cache,
                    ) {
                        return inline_expr(&e, loader, cache, depth + 1, max_depth);
                    }
                    Dot(Box::new(Call(fname, args)), member)
                }
                other => Dot(Box::new(other), member),
            }
        }
        Sample(inner) => Sample(Box::new(inline_expr(inner, loader, cache, depth + 1, max_depth))),
        Interval(inner) => {
            Interval(Box::new(inline_expr(inner, loader, cache, depth + 1, max_depth)))
        }
        Hold(inner) => Hold(Box::new(inline_expr(inner, loader, cache, depth + 1, max_depth))),
        Previous(inner) => {
            Previous(Box::new(inline_expr(inner, loader, cache, depth + 1, max_depth)))
        }
        SubSample(c, n) => SubSample(
            Box::new(inline_expr(c, loader, cache, depth + 1, max_depth)),
            Box::new(inline_expr(n, loader, cache, depth + 1, max_depth)),
        ),
        SuperSample(c, n) => SuperSample(
            Box::new(inline_expr(c, loader, cache, depth + 1, max_depth)),
            Box::new(inline_expr(n, loader, cache, depth + 1, max_depth)),
        ),
        ShiftSample(c, n) => ShiftSample(
            Box::new(inline_expr(c, loader, cache, depth + 1, max_depth)),
            Box::new(inline_expr(n, loader, cache, depth + 1, max_depth)),
        ),
        StringLiteral(s) => StringLiteral(s.clone()),
    }
}
