use super::context::{array_base_in_ctx, CCodegenContext};
use super::{is_c_builtin, ArgKind};
use crate::ast::{Equation, Expression};
use std::collections::{HashMap, HashSet};
use std::io::Write;

#[allow(dead_code)]
fn collect_calls_in_expr(expr: &Expression, out: &mut HashMap<String, usize>) {
    match expr {
        Expression::Call(name, args) => {
            let n = args.len();
            out.entry(name.clone())
                .and_modify(|m| *m = (*m).max(n))
                .or_insert(n);
        }
        Expression::BinaryOp(l, _, r) => {
            collect_calls_in_expr(l, out);
            collect_calls_in_expr(r, out);
        }
        Expression::Der(inner) => collect_calls_in_expr(inner, out),
        Expression::If(c, t, e) => {
            collect_calls_in_expr(c, out);
            collect_calls_in_expr(t, out);
            collect_calls_in_expr(e, out);
        }
        Expression::ArrayAccess(a, i) => {
            collect_calls_in_expr(a, out);
            collect_calls_in_expr(i, out);
        }
        Expression::Sample(inner)
        | Expression::Interval(inner)
        | Expression::Hold(inner)
        | Expression::Previous(inner) => collect_calls_in_expr(inner, out),
        Expression::SubSample(c, n)
        | Expression::SuperSample(c, n)
        | Expression::ShiftSample(c, n) => {
            collect_calls_in_expr(c, out);
            collect_calls_in_expr(n, out);
        }
        _ => {}
    }
}

pub(super) fn collect_external_calls_with_signature(
    eqs: &[Equation],
    ctx: &CCodegenContext,
) -> HashMap<String, Vec<ArgKind>> {
    use crate::ast::Expression::*;
    let mut out = HashMap::new();
    fn walk(expr: &Expression, ctx: &CCodegenContext, out: &mut HashMap<String, Vec<ArgKind>>) {
        match expr {
            Call(name, args) if !is_c_builtin(name) => {
                let kinds: Vec<ArgKind> = args
                    .iter()
                    .map(|a| {
                        if let Variable(id) = a {
                            let n = crate::string_intern::resolve_id(*id);
                            if array_base_in_ctx(ctx, &n) {
                                ArgKind::Array
                            } else {
                                ArgKind::Scalar
                            }
                        } else if matches!(a, crate::ast::Expression::StringLiteral(_)) {
                            ArgKind::String
                        } else {
                            ArgKind::Scalar
                        }
                    })
                    .collect();
                out.entry(name.clone()).or_insert(kinds);
            }
            BinaryOp(l, _, r) => {
                walk(l, ctx, out);
                walk(r, ctx, out);
            }
            Der(inner) | Hold(inner) | Previous(inner) | Sample(inner) | Interval(inner) => {
                walk(inner, ctx, out)
            }
            If(c, t, e) => {
                walk(c, ctx, out);
                walk(t, ctx, out);
                walk(e, ctx, out);
            }
            ArrayAccess(a, i) => {
                walk(a, ctx, out);
                walk(i, ctx, out);
            }
            SubSample(c, n) | SuperSample(c, n) | ShiftSample(c, n) => {
                walk(c, ctx, out);
                walk(n, ctx, out);
            }
            _ => {}
        }
    }
    for eq in eqs {
        match eq {
            Equation::Simple(lhs, rhs) => {
                walk(lhs, ctx, &mut out);
                walk(rhs, ctx, &mut out);
            }
            Equation::SolvableBlock {
                equations,
                residuals,
                ..
            } => {
                for e in equations {
                    if let Equation::Simple(l, r) = e {
                        walk(l, ctx, &mut out);
                        walk(r, ctx, &mut out);
                    }
                }
                for r in residuals {
                    walk(r, ctx, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

#[allow(dead_code)]
pub(super) fn collect_called_external_functions(eqs: &[Equation]) -> HashMap<String, usize> {
    let mut out = HashMap::new();
    for eq in eqs {
        match eq {
            Equation::Simple(lhs, rhs) => {
                collect_calls_in_expr(lhs, &mut out);
                collect_calls_in_expr(rhs, &mut out);
            }
            Equation::SolvableBlock {
                equations,
                residuals,
                ..
            } => {
                for e in equations {
                    if let Equation::Simple(l, r) = e {
                        collect_calls_in_expr(l, &mut out);
                        collect_calls_in_expr(r, &mut out);
                    }
                }
                for r in residuals {
                    collect_calls_in_expr(r, &mut out);
                }
            }
            _ => {}
        }
    }
    out.retain(|name, _| !is_c_builtin(name));
    out
}

/// EXT-5 / FUNC-7: Emit extern with scalar and array (ptr, size) params per signature.
/// FUNC-6: When external_names is Some, only emit extern for names in that set (user functions get static defs).
pub(super) fn emit_extern_declarations(
    external_sigs: &HashMap<String, Vec<ArgKind>>,
    external_c_names: Option<&HashMap<String, String>>,
    external_names: Option<&HashSet<String>>,
    out: &mut dyn Write,
) -> Result<(), String> {
    for (name, kinds) in external_sigs {
        if let Some(ext_set) = external_names {
            if !ext_set.contains(name) {
                continue;
            }
        }
        let c_name = external_c_names
            .and_then(|m| m.get(name))
            .map(String::as_str)
            .unwrap_or_else(|| name.as_str())
            .replace('.', "_");
        let params: Vec<String> = kinds
            .iter()
            .flat_map(|k| match k {
                ArgKind::Scalar => vec!["double".to_string()],
                ArgKind::Array => vec!["const double*".to_string(), "int".to_string()],
                ArgKind::String => vec!["const char*".to_string()],
            })
            .collect();
        writeln!(out, "extern double {}({});", c_name, params.join(", "))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
