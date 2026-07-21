//! Helpers to build `DeoptManager` and `TieredScheduler` for the simulation driver.

use crate::jit::CalcDerivsFunc;

/// Build optional deopt manager (verbatim setup from `run_simulation`).
pub(super) fn build_deopt_manager(
    calc_derivs: CalcDerivsFunc,
    model_name: &str,
) -> Option<crate::jit::deopt::DeoptManager> {
    let deopt_enabled = crate::jit::speculation::global_registry()
        .read()
        .map(|r| r.total_guard_count() > 0)
        .unwrap_or(false);
    let precompiled_generic = crate::jit::deopt::take_precompiled_generic();
    let deopt_dual_compile = precompiled_generic.is_some()
        || std::env::var("RUSTMODLICA_DUAL_COMPILE")
            .ok()
            .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
            .unwrap_or(false);
    if deopt_enabled {
        if let Some(generic_fn) = precompiled_generic {
            Some(crate::jit::deopt::DeoptManager::with_fallback(
                calc_derivs,
                generic_fn,
            ))
        } else if deopt_dual_compile {
            let mut compiler_for_generic = crate::Compiler::new();
            compiler_for_generic.options_mut().quiet = true;
            match compiler_for_generic.compile(model_name) {
                Ok(crate::CompileOutput::Simulation(generic_artifacts)) => {
                    Some(crate::jit::deopt::DeoptManager::with_fallback(
                        calc_derivs,
                        generic_artifacts.calc_derivs,
                    ))
                }
                _ => Some(crate::jit::deopt::DeoptManager::new(calc_derivs)),
            }
        } else {
            Some(crate::jit::deopt::DeoptManager::new(calc_derivs))
        }
    } else {
        None
    }
}

/// Build optional tiered scheduler (verbatim setup from `run_simulation`).
pub(super) fn build_tiered_scheduler(
    calc_derivs: CalcDerivsFunc,
    model_name: &str,
    lib_paths: Vec<std::path::PathBuf>,
    state_vars_len: usize,
    discrete_vals_len: usize,
    output_vars_len: usize,
    params_len: usize,
    crossings_count: usize,
) -> Option<crate::jit::tiered::TieredScheduler> {
    let tiered_enabled = std::env::var("RUSTMODLICA_TIERED_COMPILATION")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false);
    let total_eq_count = state_vars_len + crossings_count;
    if tiered_enabled {
        crate::jit::tiered::clear_tiered_events();
        let mut policy = crate::jit::tiered::TieringPolicy::for_equation_count(total_eq_count);
        if crate::cache::fold_benefit_record::tierup_skip_const_fold() {
            policy.force_adaptive_skip_const_fold = true;
            policy.force_adaptive_skip_eq_dce = true;
        }
        let initial_tier = policy.select_initial_tier(total_eq_count);
        let initial_func = if initial_tier == crate::jit::tiered::CompileTier::Interpreter
            && crate::jit::interpreter::is_context_installed_for(model_name)
        {
            crate::jit::interpreter::interpreter_trampoline as CalcDerivsFunc
        } else {
            calc_derivs
        };
        let mut sched = crate::jit::tiered::TieredScheduler::new(
            model_name,
            lib_paths,
            initial_tier,
            initial_func,
            policy,
        );
        sched.tiered_func.set_initial_layout(
            state_vars_len,
            discrete_vals_len,
            output_vars_len,
            params_len,
        );
        if let Some(profile) = crate::condenser::training_run::load_cached_profile(model_name) {
            sched = sched.with_profile(profile);
        }
        Some(sched)
    } else {
        None
    }
}
