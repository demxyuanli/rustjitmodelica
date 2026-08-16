//! Main fixed/adaptive simulation driver orchestration (`run_simulation`).

use crate::ast::Expression;
use crate::compiler::ClockPartitionScheduleEntry;
use crate::diag::fallback_counter;
use crate::i18n;
use crate::jit::deopt::DeoptSimPerfSummary;
use crate::jit::{native, CalcDerivsFunc};
use crate::solver::{AdaptiveRK45Solver, BackwardEulerSolver, RungeKutta4Solver};
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{self, Write};

use super::fill_deopt_sim_perf_out;
use super::sched;
use super::sundials_dispatch;
use super::super::events::{perf_enabled, perf_reset_counters, perf_snapshot};
use super::super::newton_recovery::is_ideal_switch_complementarity_name;
use super::super::sim_io::{flush_writer, write_csv_line};
use super::super::control::SimControl;
use super::super::types::{EventQueue, ResultCollector};

pub(super) const CSV_ROWS_PER_FLUSH: u32 = 64;

#[path = "run_step.rs"]
mod run_step;
use run_step::{run_simulation_step, SimStepOutcome};

pub fn run_simulation(
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    crossings_count: usize,
    mut states: Vec<f64>,
    mut discrete_vals: Vec<f64>,
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
    #[cfg_attr(not(feature = "sundials"), allow(unused_variables))]
    differential_index: u32,
    #[cfg_attr(not(feature = "sundials"), allow(unused_variables))]
    ida_component_id: &[f64],
    solver: &str,
    output_interval: f64,
    result_file: Option<&str>,
    clock_partition_schedule: &[ClockPartitionScheduleEntry],
    mut result_collector: Option<&mut ResultCollector>,
    mut deopt_sim_perf: Option<&mut DeoptSimPerfSummary>,
    model_name: &str,
    lib_paths: Vec<std::path::PathBuf>,
    mut control: Option<&mut SimControl>,
) -> Result<(), String> {
    if perf_enabled() {
        perf_reset_counters();
    }
    let mut time = 0.0;
    let mut derivs = vec![0.0; states.len()];
    let mut outputs = if output_start_vals.len() == output_vars.len() {
        output_start_vals.to_vec()
    } else {
        vec![0.0; output_vars.len()]
    };
    let mut when_states = vec![0.0; when_count * 2];
    let mut crossings = vec![0.0; crossings_count];
    let mut pre_states = vec![0.0; states.len()];
    let mut pre_discrete_vals = vec![0.0; discrete_vals.len()];
    // Pre-allocated scratch buffers reused per-step via System borrow (no per-step Vec::new()).
    let mut step_buf_discrete = Vec::<f64>::new();
    let mut step_buf_when = Vec::<f64>::new();
    let mut step_buf_crossings = Vec::<f64>::new();
    let mut step_buf_outputs = Vec::<f64>::new();
    let mut step_buf_guess = Vec::<f64>::new();
    let mut homotopy_lambda: f64 = 1.0;

    let wants_cvode = solver == "cvode";
    let wants_ida = solver == "ida";
    if wants_cvode || wants_ida {
        if let Some(ctrl) = control.as_mut() {
            ctrl.poll_step(0.0, t_end, 0)?;
        }
        return sundials_dispatch::dispatch_sundials(
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
            solver,
            output_interval,
            result_file,
            result_collector,
            deopt_sim_perf,
        );
    }

    // Use adaptive RK45 when solver is rk45 and no when-clauses (zero-crossings are
    // safe because the outer loop handles them via save/restore + linear interpolation
    // independently of the solver step method).
    let use_adaptive = solver == "rk45" && when_count == 0;
    let use_implicit = solver == "implicit";
    let use_radau = solver == "radau";
    let use_qss = solver == "qss";
    let mut rk4_solver = RungeKutta4Solver::new(states.len());
    let mut rk45_solver = AdaptiveRK45Solver::new(states.len(), atol, rtol);
    let mut backward_euler_solver = BackwardEulerSolver::new(states.len());
    let mut radau_solver = crate::solver::radau::RadauSolver::new(states.len(), atol, rtol);
    let mut qss_solver = crate::solver::qss::QssSolver::new(states.len(), atol, rtol);
    // Scratch warm-start helps large output vectors (e.g. EngineV6) where Newton metadata may be empty.
    let use_scratch_outputs_for_solver =
        !newton_tearing_var_names.is_empty() || output_vars.len() >= 4096;

    let mut out: Box<dyn Write> = if result_collector.is_some() {
        Box::new(io::sink())
    } else if let Some(path) = result_file {
        let f = File::create(path)
            .map_err(|e| format!("Failed to create result file {}: {}", path, e))?;
        Box::new(std::io::BufWriter::new(f))
    } else {
        Box::new(std::io::BufWriter::new(io::stdout()))
    };

    let w = &mut out;

    let est_cols = 1 + state_vars.len() + discrete_vars.len() + output_vars.len();
    let mut header = String::with_capacity(est_cols * 16);
    header.push_str(i18n::msg0("time"));
    for var in state_vars {
        write!(&mut header, ", {}", var).map_err(|e| e.to_string())?;
    }
    for var in discrete_vars {
        write!(&mut header, ", {}", var).map_err(|e| e.to_string())?;
    }
    for var in output_vars {
        write!(&mut header, ", {}", var).map_err(|e| e.to_string())?;
    }
    write_csv_line(w, &header)?;
    flush_writer(w)?;

    let mut csv_row = String::with_capacity(est_cols * 24);
    let mut rows_since_flush: u32 = 0;

    let print_interval = output_interval;
    let mut next_print = 0.0;
    let epsilon = 1e-5;
    let mut adaptive_step_count: u64 = 0;
    let hotspot_threshold = std::env::var("RUSTMODLICA_HOTSPOT_THRESHOLD")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1000);
    let stack_scratch_enabled = std::env::var("RUSTMODLICA_JIT_STACK_SCRATCH")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(true);
    let mut diag_state = vec![0.0_f64; states.len()];
    let mut scratch_outputs_for_step = vec![0.0_f64; output_vars.len()];
    let mut prev_outputs = vec![0.0; output_vars.len()];
    let mut event_queue = EventQueue::default();
    let mut save_states = vec![0.0_f64; states.len()];
    let mut save_discrete = vec![0.0_f64; discrete_vals.len()];
    let mut save_crossings = vec![0.0_f64; crossings_count];
    let mut save_outputs = vec![0.0_f64; output_vars.len()];
    let mut trial_discrete = vec![0.0_f64; discrete_vals.len()];
    let mut trial_when_states = vec![0.0_f64; when_count * 2];
    let mut switch_discontinuity_nudges: u32 = 0;
    // Cap consecutive stuck nudges at one discontinuity; reset whenever time advances.
    // Near PWM/ideal-switch events, keep a reduced step so we do not re-fail every
    // nominal dt and thrash through recovery (can take O(1e5) micro-steps otherwise).
    let mut dt_active = dt;
    let mut switch_recover_streak: u32 = 0;
    let model_has_ideal_switch = output_vars
        .iter()
        .any(|n| is_ideal_switch_complementarity_name(n));
    // RK4 stages straddle PWM edges; prefer BE near switches from the start when the
    // model carries ideal-switch complementarity unknowns (Chopper / HBridge / ...).
    let mut prefer_implicit_near_switch =
        model_has_ideal_switch && !use_implicit && !use_radau && !use_adaptive && !use_qss;

    native::reset_terminate_flag();
    native::reset_assert_counter();

    let training_run_active = std::env::var("RUSTMODLICA_TRAINING_RUN")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false);
    let eq_count = state_vars.len() + crossings_count;
    let mut profile_collector: Option<crate::condenser::profile_data::ProfileCollector> = if training_run_active {
        Some(crate::condenser::profile_data::ProfileCollector::new(model_name, eq_count))
    } else {
        None
    };

    let mut deopt_manager = sched::build_deopt_manager(calc_derivs, model_name);
    let mut tiered_scheduler = sched::build_tiered_scheduler(
        calc_derivs,
        model_name,
        lib_paths.clone(),
        state_vars.len(),
        discrete_vals.len(),
        output_vars.len(),
        params.len(),
        crossings_count,
    );

    let mut active_calc_derivs = calc_derivs;
    let mut _step_count: u64 = 0;
    let mut tiered_locked_by_deopt = false;

    match run_simulation_step(
        time,
        t_end,
        epsilon,
        control,
        &mut profile_collector,
        state_vars,
        states,
        eq_count,
        use_implicit,
        &mut deopt_manager,
        tiered_scheduler,
        active_calc_derivs,
        _step_count,
        tiered_locked_by_deopt,
        pre_states,
        pre_discrete_vals,
        discrete_vals,
        newton_tearing_var_names,
        when_count,
        derivs,
        params,
        outputs,
        when_states,
        crossings,
        homotopy_lambda,
        output_start_vals,
        output_vars,
        prev_outputs,
        clock_partition_schedule,
        w,
        event_queue,
        deopt_sim_perf.as_deref_mut(),
        numeric_ode_jacobian,
        symbolic_ode_jacobian,
        state_var_index,
        csv_row,
        next_print,
        print_interval,
        result_collector,
        rows_since_flush,
        dt,
        save_states,
        save_discrete,
        save_crossings,
        save_outputs,
        scratch_outputs_for_step,
        dt_active,
        prefer_implicit_near_switch,
        use_adaptive,
        use_radau,
        use_qss,
        backward_euler_solver,
        rk45_solver,
        radau_solver,
        qss_solver,
        rk4_solver,
        &mut adaptive_step_count,
        switch_recover_streak,
        model_has_ideal_switch,
        switch_discontinuity_nudges,
        step_buf_discrete,
        step_buf_when,
        step_buf_crossings,
        step_buf_outputs,
        step_buf_guess,
        use_scratch_outputs_for_solver,
        hotspot_threshold,
        stack_scratch_enabled,
        crossings_count,
        trial_discrete,
        trial_when_states,
        diag_state,
    )? {
        SimStepOutcome::Continue => {}
        SimStepOutcome::Finished => return Ok(()),
    }
    if let Some(collector) = profile_collector.take() {
        let wall_us = 0u64;
        let final_profile = collector.finalize(wall_us);
        if let Some(cache_root) = crate::flatten::flatten_cache_dir() {
            let profiles_dir = cache_root.join("profiles");
            let _ = std::fs::create_dir_all(&profiles_dir);
            let safe_name = final_profile.model_name.replace('.', "_").replace('/', "_");
            let path = profiles_dir.join(format!("{}.profile.bin", safe_name));
            let _ = final_profile.write_to_file(&path);
        }
    }
    if use_adaptive {
        println!(
            "{}",
            i18n::msg("adaptive_rk45_steps", &[&adaptive_step_count])
        );
    }
    if perf_enabled() {
        let (event_iter_total, clock_dispatch_total) = perf_snapshot();
        let fallback = fallback_counter::snapshot();
        eprintln!(
            "[perf] event_iter_total={} clock_dispatch_total={}",
            event_iter_total, clock_dispatch_total
        );
        eprintln!(
            "[perf] fallback_total={} jit_builtin={} jit_variable={} jit_derivative={} jit_equation_skip={} jit_multi_assign={} newton_init_accept={} newton_event_accept={} switch_fail_open={} clock_degrade={}",
            fallback_counter::total(&fallback),
            fallback.jit_builtin,
            fallback.jit_variable,
            fallback.jit_derivative,
            fallback.jit_equation_skip,
            fallback.jit_multi_assign,
            fallback.newton_init_accept,
            fallback.newton_event_accept,
            fallback.switch_fail_open,
            fallback.clock_degrade
        );
    }
    fill_deopt_sim_perf_out(&deopt_manager, deopt_sim_perf);
    flush_writer(w)?;
    Ok(())
}

