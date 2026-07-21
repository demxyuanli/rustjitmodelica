//! Early CVODE/IDA feature-gated dispatch from `run_simulation`.

use crate::ast::Expression;
use crate::jit::deopt::DeoptSimPerfSummary;
use crate::jit::CalcDerivsFunc;
use std::collections::HashMap;

#[cfg(feature = "sundials")]
use super::fill_deopt_sim_perf_out;
use super::super::types::ResultCollector;

/// Run CVODE/IDA when requested. Caller must only invoke this when
/// `solver == "cvode" || solver == "ida"`.
pub(super) fn dispatch_sundials(
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    crossings_count: usize,
    states: Vec<f64>,
    discrete_vals: Vec<f64>,
    params: Vec<f64>,
    state_vars: &[String],
    discrete_vars: &[String],
    output_vars: &[String],
    output_start_vals: &[f64],
    state_var_index: &HashMap<String, usize>,
    t_end: f64,
    dt: f64,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    newton_tearing_var_names: &[String],
    atol: f64,
    rtol: f64,
    #[cfg_attr(not(feature = "sundials"), allow(unused_variables))] differential_index: u32,
    #[cfg_attr(not(feature = "sundials"), allow(unused_variables))] ida_component_id: &[f64],
    solver: &str,
    output_interval: f64,
    result_file: Option<&str>,
    result_collector: Option<&mut ResultCollector>,
    deopt_sim_perf: Option<&mut DeoptSimPerfSummary>,
) -> Result<(), String> {
    let wants_cvode = solver == "cvode";
    let wants_ida = solver == "ida";
    debug_assert!(wants_cvode || wants_ida);
    #[cfg(not(feature = "sundials"))]
    {
        let _ = (
            calc_derivs,
            when_count,
            crossings_count,
            states,
            discrete_vals,
            params,
            state_vars,
            discrete_vars,
            output_vars,
            output_start_vals,
            state_var_index,
            t_end,
            dt,
            numeric_ode_jacobian,
            symbolic_ode_jacobian,
            newton_tearing_var_names,
            atol,
            rtol,
            output_interval,
            result_file,
            result_collector,
            deopt_sim_perf,
            wants_cvode,
            wants_ida,
        );
        return Err(
            "solver cvode/ida requires building rustmodlica with --features sundials (optional: sundials-vendor)"
                .to_string(),
        );
    }
    #[cfg(feature = "sundials")]
    {
        if !newton_tearing_var_names.is_empty() {
            return Err(
                "solver cvode/ida does not support models with Newton tearing".to_string(),
            );
        }
        if wants_cvode {
            fill_deopt_sim_perf_out(&None, deopt_sim_perf);
            return super::super::sundials::run_with_cvode(
                calc_derivs,
                when_count,
                crossings_count,
                states,
                discrete_vals,
                params,
                state_vars,
                discrete_vars,
                output_vars,
                output_start_vals,
                state_var_index,
                t_end,
                dt,
                numeric_ode_jacobian,
                symbolic_ode_jacobian,
                newton_tearing_var_names,
                atol,
                rtol,
                output_interval,
                result_file,
                result_collector,
            );
        }
        let _ = wants_ida;
        fill_deopt_sim_perf_out(&None, deopt_sim_perf);
        super::super::sundials::run_with_ida(
            calc_derivs,
            when_count,
            crossings_count,
            states,
            discrete_vals,
            params,
            state_vars,
            discrete_vars,
            output_vars,
            output_start_vals,
            state_var_index,
            t_end,
            dt,
            numeric_ode_jacobian,
            symbolic_ode_jacobian,
            newton_tearing_var_names,
            atol,
            rtol,
            differential_index,
            ida_component_id,
            output_interval,
            result_file,
            result_collector,
        )
    }
}
