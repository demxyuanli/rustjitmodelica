use crate::analysis::collect_vars_expr;
use crate::ast::Equation;
use crate::flatten::utils::convert_eq_to_alg;
use std::collections::HashSet;

use crate::jit::context::TranslationContext;
use crate::solvable_limits::MAX_SOLVABLE_RESIDUALS;

use crate::jit::translator::algorithm::compile_algorithm_stmt;
use super::assign::{compile_for_equation, compile_simple_equation};
use super::solvable::compile_solvable_block_general_n;
use super::solvable_tearing::compile_single_unknown_or_tearing_solvable_block;

fn compile_single_residual_solvable_block(
    unknowns: &[String],
    tearing_var: &Option<String>,
    inner_eqs: &[Equation],
    residuals: &[crate::ast::Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let mut u = unknowns.to_vec();
                if u.is_empty() {
        if let Some(t) = tearing_var {
                        u.push(t.clone());
                    } else {
                        let mut hs = HashSet::new();
                        collect_vars_expr(&residuals[0], &mut hs);
                        let mut vars: Vec<String> = hs.into_iter().collect();
                        vars.sort();
                        if let Some(p) = vars
                            .iter()
                            .find(|v| !v.starts_with("__dummy"))
                            .cloned()
                            .or_else(|| vars.first().cloned())
                        {
                            u.push(p);
                        }
                    }
                }
                if u.len() == 1 {
                    compile_solvable_block_general_n(&u, residuals, ctx, builder)?;
                } else if u.is_empty() {
                    for ieq in inner_eqs {
                        compile_equation(ieq, ctx, builder)?;
                    }
                } else {
                    return Err(format!(
                        "SolvableBlock with 1 residual needs one unknown (synthesized len {})",
                        u.len()
                    ));
                }
    Ok(())
}

fn compile_solvable_block_dispatch(
    unknowns: &[String],
    tearing_var: &Option<String>,
    inner_eqs: &[Equation],
    residuals: &[crate::ast::Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    if residuals.len() >= 2
        && residuals.len() <= MAX_SOLVABLE_RESIDUALS
        && unknowns.len() >= residuals.len()
    {
        let r = residuals.len();
        compile_solvable_block_general_n(&unknowns[..r], residuals, ctx, builder)?;
    } else if (residuals.len() == 1 && (tearing_var.is_some() || !unknowns.is_empty()))
        || (residuals.len() >= 2
            && residuals.len() <= MAX_SOLVABLE_RESIDUALS
            && unknowns.len() == 1)
    {
        compile_single_unknown_or_tearing_solvable_block(
            unknowns, tearing_var, inner_eqs, residuals, ctx, builder,
        )?;
    } else if residuals.len() == 1 {
        compile_single_residual_solvable_block(
            unknowns, tearing_var, inner_eqs, residuals, ctx, builder,
        )?;
            } else {
                return Err(format!(
                    "SolvableBlock with {} residuals is not supported (1 to {} allowed)",
                    residuals.len(),
                    MAX_SOLVABLE_RESIDUALS
                ));
            }
    Ok(())
}

pub fn compile_equation(
    eq: &Equation,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    match eq {
        Equation::CallStmt(_) => {}
        Equation::Simple(lhs, rhs) => {
            compile_simple_equation(lhs, rhs, ctx, builder)?;
        }
        Equation::For(loop_var, start_expr, end_expr, body) => {
            compile_for_equation(loop_var, start_expr, end_expr, body, ctx, builder)?;
        }
        Equation::SolvableBlock {
            unknowns,
            tearing_var,
            equations: inner_eqs,
            residuals,
        } => {
            compile_solvable_block_dispatch(
                unknowns, tearing_var, inner_eqs, residuals, ctx, builder,
            )?;
        }
        Equation::If(..)
        | Equation::When(..)
        | Equation::Reinit(..)
        | Equation::Assert(..)
        | Equation::Terminate(..) => {
            let alg = convert_eq_to_alg(eq.clone());
            compile_algorithm_stmt(&alg, ctx, builder)?;
        }
        Equation::MultiAssign(_, _) => {
            return Err("MultiAssign should not reach JIT (expand in flatten)".to_string());
        }
        _ => {}
    }
    Ok(())
}
