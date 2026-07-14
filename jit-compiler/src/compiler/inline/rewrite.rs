use crate::ast::{AlgorithmStatement, Expression, Model, Operator};
use crate::loader::ModelLoader;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

use super::builtin::is_builtin_function;
use super::function_body::get_function_body;
use super::record_access::{
    extract_named_record_field, try_extract_record_constructor_dot_field,
};
use super::subst::substitute_expr;

#[derive(Clone, Debug)]
pub(super) enum ResolveMemoEntry {
    Resolved(String),
    NoInline,
}

fn failed_model_loads() -> &'static RwLock<HashSet<String>> {
    static FAILED: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    FAILED.get_or_init(|| RwLock::new(HashSet::new()))
}

pub(super) fn load_model_inline_cached(
    cand: &str,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    mut no_inline: Option<&mut HashSet<String>>,
) -> Option<Arc<Model>> {
    if let Some(m) = cache.get(cand).cloned() {
        return Some(m);
    }
    if let Some(m) = loader.peek_loaded_model(cand) {
        cache.insert(cand.to_string(), Arc::clone(&m));
        return Some(m);
    }
    if failed_model_loads()
        .read()
        .map(|s| s.contains(cand))
        .unwrap_or(false)
    {
        if let Some(ref mut ni) = no_inline {
            ni.insert(cand.to_string());
        }
        return None;
    }
    let t0 = Instant::now();
    let loaded = loader.load_model(cand).ok();
    crate::query_db::perf_record_us("inline_load_model_us", t0.elapsed().as_micros() as u64);
    match loaded {
        Some(m) => {
            cache.insert(cand.to_string(), Arc::clone(&m));
            if let Ok(mut s) = failed_model_loads().write() {
                s.remove(cand);
            }
            Some(m)
        }
        None => {
            if let Ok(mut s) = failed_model_loads().write() {
                s.insert(cand.to_string());
            }
            if let Some(ref mut ni) = no_inline {
                ni.insert(cand.to_string());
            }
            None
        }
    }
}

fn record_candidate_probe_bucket(probes: u64) {
    let key = if probes <= 1 {
        "inline_resolve_probe_1"
    } else if probes == 2 {
        "inline_resolve_probe_2"
    } else if probes == 3 {
        "inline_resolve_probe_3"
    } else if probes == 4 {
        "inline_resolve_probe_4"
    } else {
        "inline_resolve_probe_ge5"
    };
    crate::query_db::perf_record_add(key, 1);
}

fn substitute_expr_inline_prof(expr: &Expression, subst: &HashMap<String, Expression>) -> Expression {
    let t0 = Instant::now();
    let out = substitute_expr(expr, subst).unwrap_or_else(|| expr.clone());
    crate::query_db::perf_record_us(
        "inline_substitute_us",
        t0.elapsed().as_micros() as u64,
    );
    out
}

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
    if name == "valveCharacteristic" {
        out.push("Modelica.Fluid.Valves.BaseClasses.ValveCharacteristics.linear".to_string());
    }
    if matches!(name, "regRoot" | "regRoot2" | "regSquare2" | "regFun3" | "regStep" | "spliceFunction")
    {
        out.push(format!("Modelica.Fluid.Utilities.{name}"));
        out.push(format!("Modelica.Utilities.Math.{name}"));
    }
    if let Some(suffix) = name.strip_prefix("Utilities.") {
        if !suffix.is_empty() {
            out.push(format!("Modelica.Fluid.Utilities.{suffix}"));
            out.push(format!("Modelica.Utilities.{suffix}"));
        }
    }
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
    let mut seen = std::collections::HashSet::new();
    out.into_iter().filter(|n| seen.insert(n.clone())).collect()
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
    no_inline: &mut HashSet<String>,
    resolve_memo: &mut HashMap<String, ResolveMemoEntry>,
    depth: u32,
    max_depth: u32,
) -> Option<Expression> {
    if depth > max_depth {
        return None;
    }
    let mut func_model: Option<Arc<Model>> = None;
    let mut resolved_name: Option<String> = None;
    crate::query_db::perf_record_add("inline_resolve_calls", 1);
    let candidates: Vec<String> = match resolve_memo.get(func_name) {
        Some(ResolveMemoEntry::Resolved(name)) => vec![name.clone()],
        Some(ResolveMemoEntry::NoInline) => return None,
        None => function_resolution_candidates(func_name),
    };
    crate::query_db::perf_record_add("inline_resolve_candidates_total", candidates.len() as u64);
    let mut probes: u64 = 0;
    for (idx, cand) in candidates.into_iter().enumerate() {
        if no_inline.contains(&cand) {
            continue;
        }
        probes += 1;
        if let Some(m) = cache.get(&cand).cloned() {
            func_model = Some(m);
            resolved_name = Some(cand);
            if idx == 0 {
                crate::query_db::perf_record_add("inline_resolve_first_hit", 1);
            }
            break;
        }
        if let Some(m) = load_model_inline_cached(&cand, loader, cache, Some(no_inline)) {
            func_model = Some(m);
            resolved_name = Some(cand);
            if idx == 0 {
                crate::query_db::perf_record_add("inline_resolve_first_hit", 1);
            }
            break;
        }
    }
    crate::query_db::perf_record_add("inline_resolve_probes_total", probes);
    if probes > 0 {
        record_candidate_probe_bucket(probes);
    }
    let func_model = func_model?;
    let resolved = resolved_name?;
    if !func_model.is_function || func_model.external_info.is_some() {
        no_inline.insert(resolved);
        resolve_memo.insert(func_name.to_string(), ResolveMemoEntry::NoInline);
        return None;
    }
    resolve_memo.insert(
        func_name.to_string(),
        ResolveMemoEntry::Resolved(resolved.clone()),
    );
    cache.insert(resolved.clone(), Arc::clone(&func_model));
    let input_names: Vec<String> = func_model
        .declarations
        .iter()
        .filter(|d| d.is_input)
        .map(|d| d.name.clone())
        .collect();
    if input_names.len() != args.len() {
        no_inline.insert(resolved);
        resolve_memo.insert(func_name.to_string(), ResolveMemoEntry::NoInline);
        return None;
    }
    let args_inlined: Vec<Expression> = args
        .iter()
        .map(|a| inline_expr(a, loader, cache, no_inline, resolve_memo, depth + 1, max_depth))
        .collect();
    let mut param_subst: HashMap<String, Expression> = HashMap::with_capacity(input_names.len());
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
                        let rhs_sub = substitute_expr_inline_prof(rhs, &ctx);
                        if let Some(e) = extract_named_record_field(&rhs_sub, field) {
                            return Some(e);
                        }
                    } else {
                        let ctx = subst_merge_params_and_locals(&param_subst, &locals);
                        let rhs_sub = substitute_expr_inline_prof(rhs, &ctx);
                        locals.insert(name, rhs_sub);
                    }
                }
                Expression::Dot(inner, fld) if fld == field => {
                    if let Expression::Variable(id) = inner.as_ref() {
                        let out = crate::string_intern::resolve_id(*id);
                        if outputs.contains(&out) {
                            let ctx = subst_merge_params_and_locals(&param_subst, &locals);
                            return Some(substitute_expr_inline_prof(rhs, &ctx));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    no_inline.insert(resolved);
    resolve_memo.insert(func_name.to_string(), ResolveMemoEntry::NoInline);
    None
}

pub(super) fn inline_expr(
    expr: &Expression,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    no_inline: &mut HashSet<String>,
    resolve_memo: &mut HashMap<String, ResolveMemoEntry>,
    depth: u32,
    max_depth: u32,
) -> Expression {
    use Expression::*;
    match expr {
        Call(name, args) => {
            crate::query_db::perf_record_add("inline_call_sites", 1);
            let name = name.as_str();
            if depth > max_depth {
                return Call(
                    name.to_string(),
                    args.iter()
                        .map(|a| inline_expr(a, loader, cache, no_inline, resolve_memo, depth + 1, max_depth))
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
                let inlined_arg =
                    inline_expr(&args[0], loader, cache, no_inline, resolve_memo, depth + 1, max_depth);
                if let StringLiteral(uri) = &inlined_arg {
                    return StringLiteral(resolve_modelica_uri(uri, &loader.library_paths));
                }
                return inlined_arg;
            }
            if name == "powlin" || name.ends_with(".powlin") {
                if args.len() == 2 {
                    let u = inline_expr(&args[0], loader, cache, no_inline, resolve_memo, depth + 1, max_depth);
                    let me = inline_expr(&args[1], loader, cache, no_inline, resolve_memo, depth + 1, max_depth);
                    let thresh = Number(-1.0 + 1e-12);
                    let cond = BinaryOp(Box::new(u.clone()), Operator::Greater, Box::new(thresh));
                    let base = BinaryOp(Box::new(Number(1.0)), Operator::Add, Box::new(u));
                    let pow_call = Call("pow".to_string(), vec![base, me]);
                    return If(Box::new(cond), Box::new(pow_call), Box::new(Number(0.0)));
                }
            }
            let mut args_inlined: Vec<Expression> = Vec::with_capacity(args.len());
            for a in args {
                args_inlined.push(inline_expr(
                    a,
                    loader,
                    cache,
                    no_inline,
                    resolve_memo,
                    depth + 1,
                    max_depth,
                ));
            }
            let func = if is_builtin_function(name) {
                None
            } else if no_inline.contains(name) {
                None
            } else if let Some(m) = cache.get(name).cloned() {
                Some(m)
            } else {
                load_model_inline_cached(name, loader, cache, Some(no_inline))
            };
            if let Some(func_model) = func {
                if let Some((input_names, outputs)) = get_function_body(func_model.as_ref()) {
                    if input_names.len() == args_inlined.len() && outputs.len() == 1 {
                        cache.insert(name.to_string(), Arc::clone(&func_model));
                        let mut subst = HashMap::with_capacity(input_names.len());
                        for (in_name, arg) in input_names.into_iter().zip(args_inlined.into_iter()) {
                            subst.insert(in_name, arg);
                        }
                        crate::query_db::perf_record_add("inline_single_output_inlines", 1);
                        return substitute_expr_inline_prof(&outputs[0].1, &subst);
                    }
                }
                no_inline.insert(name.to_string());
            }
            Call(
                name.to_string(),
                args_inlined,
            )
        }
        Variable(_) | Number(_) => expr.clone(),
        BinaryOp(lhs, op, rhs) => BinaryOp(
            Box::new(inline_expr(lhs, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            *op,
            Box::new(inline_expr(rhs, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
        ),
        Der(inner) => Der(Box::new(inline_expr(
            inner,
            loader,
            cache,
            no_inline,
            resolve_memo,
            depth + 1,
            max_depth,
        ))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(inline_expr(arr, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            Box::new(inline_expr(idx, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
        ),
        If(cond, t, f) => If(
            Box::new(inline_expr(cond, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            Box::new(inline_expr(t, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            Box::new(inline_expr(f, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
        ),
        Range(start, step, end) => Range(
            Box::new(inline_expr(start, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            Box::new(inline_expr(step, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            Box::new(inline_expr(end, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
        ),
        ArrayLiteral(items) => ArrayLiteral(
            items
                .iter()
                .map(|e| inline_expr(e, loader, cache, no_inline, resolve_memo, depth + 1, max_depth))
                .collect(),
        ),
        ArrayComprehension { expr, iter_var, iter_range } => ArrayComprehension {
            expr: Box::new(inline_expr(expr, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            iter_var: iter_var.clone(),
            iter_range: Box::new(inline_expr(
                iter_range,
                loader,
                cache,
                no_inline,
                resolve_memo,
                depth + 1,
                max_depth,
            )),
        },
        Dot(base, member) => {
            let member = member.clone();
            match base.as_ref() {
                If(c, t, f) => {
                    return If(
                        Box::new(inline_expr(c, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
                        Box::new(inline_expr(
                            &Expression::Dot(t.clone(), member.clone()),
                            loader,
                            cache,
                            no_inline,
                            resolve_memo,
                            depth + 1,
                            max_depth,
                        )),
                        Box::new(inline_expr(
                            &Expression::Dot(f.clone(), member),
                            loader,
                            cache,
                            no_inline,
                            resolve_memo,
                            depth + 1,
                            max_depth,
                        )),
                    );
                }
                Call(fname, args) => {
                    if let Some(e) = try_extract_function_output_dot_field(
                        fname, args, &member, loader, cache, no_inline, resolve_memo, depth, max_depth,
                    ) {
                        return inline_expr(&e, loader, cache, no_inline, resolve_memo, depth + 1, max_depth);
                    }
                    if let Some(e) =
                        try_extract_record_constructor_dot_field(fname, args, &member, loader, cache)
                    {
                        return inline_expr(&e, loader, cache, no_inline, resolve_memo, depth + 1, max_depth);
                    }
                }
                _ => {}
            }
            let b = inline_expr(base, loader, cache, no_inline, resolve_memo, depth + 1, max_depth);
            match b {
                If(c, t, f) => If(
                    c,
                    Box::new(inline_expr(
                        &Expression::Dot(t, member.clone()),
                        loader,
                        cache,
                        no_inline,
                        resolve_memo,
                        depth + 1,
                        max_depth,
                    )),
                    Box::new(inline_expr(
                        &Expression::Dot(f, member.clone()),
                        loader,
                        cache,
                        no_inline,
                        resolve_memo,
                        depth + 1,
                        max_depth,
                    )),
                ),
                Call(fname, args) => {
                    if let Some(e) = try_extract_function_output_dot_field(
                        &fname,
                        &args,
                        &member,
                        loader,
                        cache,
                        no_inline,
                        resolve_memo,
                        depth,
                        max_depth,
                    ) {
                        return inline_expr(&e, loader, cache, no_inline, resolve_memo, depth + 1, max_depth);
                    }
                    if let Some(e) = try_extract_record_constructor_dot_field(
                        &fname, &args, &member, loader, cache,
                    ) {
                        return inline_expr(&e, loader, cache, no_inline, resolve_memo, depth + 1, max_depth);
                    }
                    Dot(Box::new(Call(fname, args)), member)
                }
                other => Dot(Box::new(other), member),
            }
        }
        Sample(inner) => Sample(Box::new(inline_expr(
            inner,
            loader,
            cache,
            no_inline,
            resolve_memo,
            depth + 1,
            max_depth,
        ))),
        Interval(inner) => {
            Interval(Box::new(inline_expr(
                inner,
                loader,
                cache,
                no_inline,
                resolve_memo,
                depth + 1,
                max_depth,
            )))
        }
        Hold(inner) => Hold(Box::new(inline_expr(
            inner,
            loader,
            cache,
            no_inline,
            resolve_memo,
            depth + 1,
            max_depth,
        ))),
        Previous(inner) => {
            Previous(Box::new(inline_expr(
                inner,
                loader,
                cache,
                no_inline,
                resolve_memo,
                depth + 1,
                max_depth,
            )))
        }
        SubSample(c, n) => SubSample(
            Box::new(inline_expr(c, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            Box::new(inline_expr(n, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
        ),
        SuperSample(c, n) => SuperSample(
            Box::new(inline_expr(c, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            Box::new(inline_expr(n, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
        ),
        ShiftSample(c, n) => ShiftSample(
            Box::new(inline_expr(c, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            Box::new(inline_expr(n, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
        ),
        BackSample(c, n) => BackSample(
            Box::new(inline_expr(c, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
            Box::new(inline_expr(n, loader, cache, no_inline, resolve_memo, depth + 1, max_depth)),
        ),
        StringLiteral(s) => StringLiteral(s.clone()),
    }
}
