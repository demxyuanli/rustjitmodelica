use super::context::{array_layout_macro_name, CCodegenContext};
use super::expr_emit::expr_to_c;
use super::external::emit_extern_declarations;
use super::solvable_emit;
use super::ArgKind;
use crate::ast::{Equation, Expression};
use crate::compiler::equation_convert::{expr_substitute_all_array_indices, parse_array_index};
use std::collections::{HashMap, HashSet};
use std::io::Write;

fn array_size_from_ctx(ctx: &CCodegenContext, base: &str) -> Option<usize> {
    ctx.state_array_layout
        .iter()
        .flat_map(|l| l.iter())
        .chain(ctx.output_array_layout.iter().flat_map(|l| l.iter()))
        .chain(ctx.param_array_layout.iter().flat_map(|l| l.iter()))
        .find(|(n, _, _)| n == base)
        .map(|(_, _, sz)| *sz)
}

fn array_lhs_loop_c(ctx: &CCodegenContext, base: &str, loop_var: &str) -> Option<String> {
    let mac = array_layout_macro_name(base);
    if ctx
        .state_array_layout
        .map_or(false, |l| l.iter().any(|(n, _, _)| n == base))
    {
        return Some(format!("x[{}_START + {}]", mac, loop_var));
    }
    if ctx
        .output_array_layout
        .map_or(false, |l| l.iter().any(|(n, _, _)| n == base))
    {
        return Some(format!("y[Y_{}_START + {}]", mac, loop_var));
    }
    if ctx
        .param_array_layout
        .map_or(false, |l| l.iter().any(|(n, _, _)| n == base))
    {
        return Some(format!("p[P_{}_START + {}]", mac, loop_var));
    }
    None
}

fn take_array_run(eqs: &[Equation], ctx: &CCodegenContext) -> Option<(String, usize, Expression)> {
    let first = eqs.first()?;
    let (lhs, rhs) = match first {
        Equation::Simple(lhs, rhs) => (lhs, rhs),
        _ => return None,
    };
    let name = match lhs {
        Expression::Variable(id) => crate::string_intern::resolve_id(*id),
        _ => return None,
    };
    let (base, idx1) = parse_array_index(&name)?;
    if idx1 != 1 {
        return None;
    }
    let size = array_size_from_ctx(ctx, &base)?;
    if size < 2 || eqs.len() < size {
        return None;
    }
    for k in 0..size {
        let eq = &eqs[k];
        let (lhs_k, rhs_k) = match eq {
            Equation::Simple(l, r) => (l, r),
            _ => return None,
        };
        let name_k = match lhs_k {
            Expression::Variable(id) => crate::string_intern::resolve_id(*id),
            _ => return None,
        };
        let (b, idx) = parse_array_index(&name_k)?;
        if b != base || idx != k + 1 {
            return None;
        }
        let expected_rhs = expr_substitute_all_array_indices(rhs, k);
        if rhs_k != &expected_rhs {
            return None;
        }
    }
    Some((base, size, rhs.clone()))
}

pub(super) fn emit_one_equation(
    lhs: &Expression,
    rhs_c: &str,
    ctx: &CCodegenContext<'_>,
    out: &mut dyn Write,
) -> Result<(), String> {
    let lhs_str = match lhs {
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            if name.starts_with("der_") {
                let base = &name[4..];
                if let Some(&i) = ctx.state_index.get(base) {
                    format!("xdot[{}]", i)
                } else {
                    return Err(format!(
                        "C codegen: der_ variable '{}' not in state set",
                        name
                    ));
                }
            } else if let Some(ov) = ctx.var_overrides.get(&name) {
                ov.clone()
            } else if let Some(&i) = ctx.output_index.get(&name) {
                format!("y[{}]", i)
            } else {
                return Err(format!(
                    "C codegen: LHS variable '{}' not der_ or output",
                    name
                ));
            }
        }
        _ => return Err("C codegen: LHS must be variable".to_string()),
    };
    writeln!(out, "  {} = {};", lhs_str, rhs_c).map_err(|e| e.to_string())?;
    Ok(())
}

fn emit_user_function_statics(
    user_function_bodies: &HashMap<String, (Vec<String>, Expression)>,
    external_c_names: Option<&HashMap<String, String>>,
    out: &mut dyn Write,
) -> Result<(), String> {
    for (name, (input_names, output_expr)) in user_function_bodies {
        let c_name = external_c_names
            .and_then(|m| m.get(name))
            .map(String::as_str)
            .unwrap_or_else(|| name.as_str())
            .replace('.', "_");
        let params: Vec<String> = input_names
            .iter()
            .enumerate()
            .map(|(i, _)| format!("double arg_{}", i))
            .collect();
        let overrides: Vec<(String, String)> = input_names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), format!("arg_{}", i)))
            .collect();
        let fn_ctx = CCodegenContext::new(&[], &[], &[]).with_overrides(&overrides);
        let body_c = expr_to_c(output_expr, &fn_ctx)?;
        writeln!(
            out,
            "static double {}({}) {{ return ({}); }}",
            c_name,
            params.join(", "),
            body_c
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn emit_residual(
    _state_vars: &[String],
    _param_vars: &[String],
    _output_vars: &[String],
    sorted_eqs: &[Equation],
    ctx: &CCodegenContext<'_>,
    external_sigs: &HashMap<String, Vec<ArgKind>>,
    external_names: Option<&HashSet<String>>,
    user_function_bodies: Option<&HashMap<String, (Vec<String>, Expression)>>,
    out: &mut dyn Write,
) -> Result<(), String> {
    writeln!(out, "/* Generated by rustmodlica CG1-1. Do not edit. */")
        .map_err(|e| e.to_string())?;
    writeln!(out, "#include <math.h>").map_err(|e| e.to_string())?;
    if !external_sigs.is_empty() {
        emit_extern_declarations(
            external_sigs,
            ctx.external_c_names.as_ref(),
            external_names,
            out,
        )?;
    }
    if let Some(bodies) = user_function_bodies {
        if !bodies.is_empty() {
            emit_user_function_statics(bodies, ctx.external_c_names.as_ref(), out)?;
        }
    }

    if solvable_emit::sorted_eqs_need_solve_dense(sorted_eqs) {
        solvable_emit::emit_solve_dense_helper(out)?;
    }
    writeln!(
        out,
        "void residual(double t, const double* x, double* xdot, const double* p, double* y) {{"
    )
    .map_err(|e| e.to_string())?;

    let mut i = 0;
    while i < sorted_eqs.len() {
        if let Some((base, size, rhs_template)) = take_array_run(&sorted_eqs[i..], ctx) {
            let lhs_c = array_lhs_loop_c(ctx, &base, "i")
                .ok_or_else(|| format!("C codegen: array '{}' not in layout", base))?;
            let ctx_loop = ctx.clone().with_loop_context("i");
            let rhs_c = expr_to_c(&rhs_template, &ctx_loop)?;
            writeln!(out, "  for (int i = 0; i < {}; i++) {{", size).map_err(|e| e.to_string())?;
            writeln!(out, "    {} = {};", lhs_c, rhs_c).map_err(|e| e.to_string())?;
            writeln!(out, "  }}").map_err(|e| e.to_string())?;
            i += size;
            continue;
        }
        let eq = &sorted_eqs[i];
        match eq {
            Equation::Simple(lhs, rhs) => {
                if !matches!(lhs, Expression::Variable(_)) {
                    return Err("C codegen: equation LHS must be a variable (residual-form equations not supported as standalone; use JIT backend)".to_string());
                }
                let rhs_c = expr_to_c(rhs, &ctx)?;
                emit_one_equation(lhs, &rhs_c, &ctx, out)?;
                i += 1;
            }
            Equation::SolvableBlock { .. } => {
                solvable_emit::emit_solvable_block_residual(eq, ctx, out)?;
                i += 1;
            }
            Equation::For(_, _, _, body) => {
                for eq in body {
                    if let Equation::Simple(lhs, rhs) = eq {
                        let rhs_c = expr_to_c(rhs, &ctx)?;
                        emit_one_equation(lhs, &rhs_c, &ctx, out)?;
                    }
                }
                i += 1;
            }
            _ => {
                return Err(format!("C codegen: equation type not supported: {:?}", eq));
            }
        }
    }

    writeln!(out, "}}").map_err(|e| e.to_string())?;
    Ok(())
}
