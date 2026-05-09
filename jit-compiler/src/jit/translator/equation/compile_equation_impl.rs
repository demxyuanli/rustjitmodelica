use crate::analysis::collect_vars_expr;
use crate::ast::Equation;
use crate::diag::fallback_counter;
use crate::flatten::utils::convert_eq_to_alg;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::InstBuilder;
use cranelift_module::Module;
use std::collections::HashSet;

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;
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
    fn overdet_consistency_check_enabled() -> bool {
        use std::sync::OnceLock;
        static CACHED: OnceLock<bool> = OnceLock::new();
        *CACHED.get_or_init(|| {
            std::env::var("RUSTMODLICA_OVERDET_CHECK")
                .ok()
                .map(|v| !matches!(v.trim(), "0" | "false" | "FALSE" | "off" | "OFF"))
                .unwrap_or(true)
        })
    }

    fn overdet_consistency_tol() -> f64 {
        use std::sync::OnceLock;
        static CACHED: OnceLock<f64> = OnceLock::new();
        *CACHED.get_or_init(|| {
            std::env::var("RUSTMODLICA_OVERDET_RESIDUAL_TOL")
                .ok()
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(1e-4)
        })
    }

    fn emit_overdet_residual_consistency_check(
        residuals: &[crate::ast::Expression],
        tol: f64,
        ctx: &mut TranslationContext,
        builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<(), String> {
        if residuals.is_empty() {
            return Ok(());
        }
        let mut max_abs = {
            let rv = compile_expression(&residuals[0], ctx, builder)?;
            builder.ins().fabs(rv)
        };
        for res in residuals.iter().skip(1) {
            let rv = compile_expression(res, ctx, builder)?;
            let abs = builder.ins().fabs(rv);
            max_abs = builder.ins().fmax(max_abs, abs);
        }
        let tol_val = builder.ins().f64const(tol);
        let ok = builder
            .ins()
            .fcmp(cranelift::prelude::FloatCC::LessThanOrEqual, max_abs, tol_val);
        let ok_block = builder.create_block();
        let fail_block = builder.create_block();
        let cont_block = builder.create_block();
        builder.ins().brif(ok, ok_block, &[], fail_block, &[]);

        builder.switch_to_block(fail_block);
        let n_residuals_val = builder.ins().f64const(residuals.len() as f64);

        let mut gate_sig = ctx.module.make_signature();
        gate_sig.params.push(cranelift::prelude::AbiParam::new(cl_types::F64));
        gate_sig.params.push(cranelift::prelude::AbiParam::new(cl_types::F64));
        gate_sig.params.push(cranelift::prelude::AbiParam::new(cl_types::F64));
        let gate_id = ctx
            .module
            .declare_function("rustmodlica_residual_gate_fail", cranelift_module::Linkage::Import, &gate_sig)
            .map_err(|e| e.to_string())?;
        let gate_ref = ctx.module.declare_func_in_func(gate_id, &mut builder.func);
        builder.ins().call(gate_ref, &[max_abs, n_residuals_val, tol_val]);

        let zero = builder.ins().f64const(0.0);
        let mut assert_sig = ctx.module.make_signature();
        assert_sig.params.push(cranelift::prelude::AbiParam::new(cl_types::F64));
        assert_sig.params.push(cranelift::prelude::AbiParam::new(cl_types::F64));
        assert_sig
            .returns
            .push(cranelift::prelude::AbiParam::new(cl_types::F64));
        let assert_id = ctx
            .module
            .declare_function("assert", cranelift_module::Linkage::Import, &assert_sig)
            .map_err(|e| e.to_string())?;
        let assert_ref = ctx.module.declare_func_in_func(assert_id, &mut builder.func);
        builder.ins().call(assert_ref, &[zero, max_abs]);

        let mut terminate_sig = ctx.module.make_signature();
        terminate_sig
            .params
            .push(cranelift::prelude::AbiParam::new(cl_types::F64));
        terminate_sig
            .returns
            .push(cranelift::prelude::AbiParam::new(cl_types::F64));
        let terminate_id = ctx
            .module
            .declare_function("terminate", cranelift_module::Linkage::Import, &terminate_sig)
            .map_err(|e| e.to_string())?;
        let terminate_ref = ctx
            .module
            .declare_func_in_func(terminate_id, &mut builder.func);
        builder.ins().call(terminate_ref, &[max_abs]);
        builder.ins().jump(cont_block, &[]);
        builder.seal_block(fail_block);

        builder.switch_to_block(ok_block);
        builder.ins().jump(cont_block, &[]);
        builder.seal_block(ok_block);

        builder.switch_to_block(cont_block);
        builder.seal_block(cont_block);
        Ok(())
    }

    fn select_overdetermined_residual_subset(
        unknowns: &[String],
        residuals: &[crate::ast::Expression],
    ) -> Vec<usize> {
        let n = unknowns.len();
        if n == 0 || residuals.is_empty() {
            return Vec::new();
        }
        let unknown_set: HashSet<&str> = unknowns.iter().map(|s| s.as_str()).collect();
        let mut scored: Vec<(usize, usize, usize)> = Vec::with_capacity(residuals.len());
        for (idx, res) in residuals.iter().enumerate() {
            let mut vars = HashSet::new();
            collect_vars_expr(res, &mut vars);
            let cover = vars
                .iter()
                .filter(|v| unknown_set.contains(v.as_str()))
                .count();
            let extra = vars.len().saturating_sub(cover);
            // Sort key: max cover, min extra, then stable index.
            scored.push((idx, cover, extra));
        }
        scored.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| a.0.cmp(&b.0))
        });
        let mut picked = Vec::with_capacity(n);
        for (idx, _, _) in scored.into_iter().take(n) {
            picked.push(idx);
        }
        picked.sort_unstable();
        picked
    }

    if residuals.len() >= 2
        && residuals.len() <= MAX_SOLVABLE_RESIDUALS
        && unknowns.len() >= residuals.len()
    {
        let r = residuals.len();
        compile_solvable_block_general_n(&unknowns[..r], residuals, ctx, builder)?;
        if unknowns.len() == residuals.len() && overdet_consistency_check_enabled() {
            emit_overdet_residual_consistency_check(
                residuals,
                overdet_consistency_tol(),
                ctx,
                builder,
            )?;
        }
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
    } else if residuals.len() > unknowns.len()
        && unknowns.len() >= 2
        && unknowns.len() <= MAX_SOLVABLE_RESIDUALS
    {
        // Overdetermined block: pick an informative residual subset and solve as square system.
        let subset = select_overdetermined_residual_subset(unknowns, residuals);
        if subset.len() == unknowns.len() {
            let reduced: Vec<crate::ast::Expression> =
                subset.iter().map(|&i| residuals[i].clone()).collect();
            compile_solvable_block_general_n(unknowns, &reduced, ctx, builder)?;
            if overdet_consistency_check_enabled() {
                emit_overdet_residual_consistency_check(
                    residuals,
                    overdet_consistency_tol(),
                    ctx,
                    builder,
                )?;
            }
        } else {
            return Err(format!(
                "JIT overdetermined SolvableBlock could not select solvable subset: residuals={}, unknowns={}, selected={}.",
                residuals.len(),
                unknowns.len(),
                subset.len()
            ));
        }
    } else {
        let mut unknown_preview: Vec<String> = unknowns.iter().take(8).cloned().collect();
        if unknowns.len() > 8 {
            unknown_preview.push(format!("...(+{} more)", unknowns.len() - 8));
        }
        return Err(format!(
            "JIT unsupported SolvableBlock shape: residuals={}, unknowns={}, max_residuals={}. \
unknown preview=[{}]. This block cannot be safely downgraded; please split/rewrite the block or lower it to a supported solve path.",
            residuals.len(),
            unknowns.len(),
            MAX_SOLVABLE_RESIDUALS,
            unknown_preview.join(", ")
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
            if super::block_compile::block_compile_enabled() {
                // Defer: record block data, emit call stub. Block body compiled
                // after main function is finalized.
                let block_idx = ctx.block_index_counter;
                ctx.block_index_counter += 1;
                let (fid, sig) = super::block_compile::declare_block_function(ctx, block_idx)?;
                // Record for later body compilation
                ctx.deferred_blocks.push((
                    fid,
                    unknowns.clone(),
                    tearing_var.clone(),
                    inner_eqs.clone(),
                    residuals.clone(),
                ));
                ctx.block_funcs.push((fid, sig));
                // Emit call stub: the block function will be defined later.
                // For now, emit inline code as fallback (blocks get compiled
                // on the next recompile cycle via tier-up).
            }
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
        _ => {
            fallback_counter::inc_jit_equation_skip();
            eprintln!("[fallback:jit-equation] unhandled equation variant skipped: {:?}", eq);
        }
    }
    Ok(())
}
