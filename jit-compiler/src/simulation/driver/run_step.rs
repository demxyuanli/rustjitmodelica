//! One simulation-loop invocation (the `while time <= t_end` body and loop).

use crate::ast::Expression;
use crate::compiler::ClockPartitionScheduleEntry;
use crate::i18n;
use crate::jit::deopt::DeoptSimPerfSummary;
use crate::jit::{native, CalcDerivsFunc};
use crate::solver::{AdaptiveRK45Solver, BackwardEulerSolver, RungeKutta4Solver, Solver, System};
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::io::Write;

use super::super::crossing;
use super::super::fill_deopt_sim_perf_out;
use super::super::super::events::{
    commit_pre_after_event_and_reeval, perf_enabled, perf_inc_clock_dispatch,
    run_event_iteration_at_time, EventIterationOutcome,
};
use super::super::super::newton_recovery::{
    allow_zero_residual_newton, commit_solver_algebraic_guess, print_newton_diag,
};
use super::super::super::switch_drive::{
    recover_failed_solver_step, recover_failed_trial_eval, SwitchStepRecoverResult,
    SwitchTrialRecoverResult,
};
use super::super::super::sim_io::{flush_writer, write_csv_line};
use super::super::super::step::maybe_print_numeric_jacobian;
use super::super::super::control::SimControl;
use super::super::super::types::{EventQueue, QueuedEventKind, ResultCollector};

use super::CSV_ROWS_PER_FLUSH;

pub(super) enum SimStepOutcome {
    Continue,
    Finished,
}

pub(super) fn run_simulation_step(
    mut time: f64,
    t_end: f64,
    epsilon: f64,
    mut control: Option<&mut SimControl>,
    profile_collector: &mut Option<crate::condenser::profile_data::ProfileCollector>,
    state_vars: &[String],
    mut states: Vec<f64>,
    eq_count: usize,
    use_implicit: bool,
    deopt_manager: &mut Option<crate::jit::deopt::DeoptManager>,
    mut tiered_scheduler: Option<crate::jit::tiered::TieredScheduler>,
    mut active_calc_derivs: CalcDerivsFunc,
    mut step_count: u64,
    mut tiered_locked_by_deopt: bool,
    mut pre_states: Vec<f64>,
    mut pre_discrete_vals: Vec<f64>,
    mut discrete_vals: Vec<f64>,
    newton_tearing_var_names: &[String],
    when_count: usize,
    mut derivs: Vec<f64>,
    params: Vec<f64>,
    mut outputs: Vec<f64>,
    mut when_states: Vec<f64>,
    mut crossings: Vec<f64>,
    mut homotopy_lambda: f64,
    output_start_vals: &[f64],
    output_vars: &[String],
    mut prev_outputs: Vec<f64>,
    clock_partition_schedule: &[ClockPartitionScheduleEntry],
    w: &mut Box<dyn Write>,
    mut event_queue: EventQueue,
    deopt_sim_perf: Option<&mut DeoptSimPerfSummary>,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    state_var_index: &HashMap<String, usize>,
    mut csv_row: String,
    mut next_print: f64,
    print_interval: f64,
    mut result_collector: Option<&mut ResultCollector>,
    mut rows_since_flush: u32,
    dt: f64,
    mut save_states: Vec<f64>,
    mut save_discrete: Vec<f64>,
    mut save_crossings: Vec<f64>,
    mut save_outputs: Vec<f64>,
    mut scratch_outputs_for_step: Vec<f64>,
    mut dt_active: f64,
    mut prefer_implicit_near_switch: bool,
    use_adaptive: bool,
    use_radau: bool,
    use_qss: bool,
    mut backward_euler_solver: BackwardEulerSolver,
    mut rk45_solver: AdaptiveRK45Solver,
    mut radau_solver: crate::solver::radau::RadauSolver,
    mut qss_solver: crate::solver::qss::QssSolver,
    mut rk4_solver: RungeKutta4Solver,
    adaptive_step_count: &mut u64,
    mut switch_recover_streak: u32,
    model_has_ideal_switch: bool,
    mut switch_discontinuity_nudges: u32,
    mut step_buf_discrete: Vec<f64>,
    mut step_buf_when: Vec<f64>,
    mut step_buf_crossings: Vec<f64>,
    mut step_buf_outputs: Vec<f64>,
    mut step_buf_guess: Vec<f64>,
    use_scratch_outputs_for_solver: bool,
    hotspot_threshold: u64,
    stack_scratch_enabled: bool,
    crossings_count: usize,
    mut trial_discrete: Vec<f64>,
    mut trial_when_states: Vec<f64>,
    mut diag_state: Vec<f64>,
) -> Result<SimStepOutcome, String> {
    let homotopy_lambda_ptr: *const f64 = &homotopy_lambda;
    let mut empty_steps: u64 = 0;
    while time <= t_end + epsilon {
        native::reset_assert_counter();

        // Snap onto the output grid before event iteration so discrete
        // `time >= t_start + waitTime` fires at the printed sample (empty-state
        // models advance with `time += dt` and can drift just below the grid).
        if time < next_print && next_print - time <= epsilon {
            time = next_print;
        }

        step_count += 1;
        if let Some(ctrl) = control.as_mut() {
            ctrl.poll_step(time, t_end, step_count)?;
        }
        if let Some(ref mut collector) = profile_collector.as_mut() {
            collector.record_step();
            for (i, name) in state_vars.iter().enumerate() {
                if i < states.len() {
                    collector.record_state_value(name, states[i]);
                }
            }
            for i in 0..eq_count {
                collector.record_equation_eval(i);
            }
            let path = if use_implicit {
                crate::condenser::profile_data::SolverPath::Dense
            } else {
                crate::condenser::profile_data::SolverPath::Scalar
            };
            collector.record_solver_branch(0, path);
        }
        crate::jit::speculation::validate_runtime_assumptions(state_vars, &states, &[]);

        let mut deopt_fired = false;
        if let Some(ref mut dm) = deopt_manager.as_mut() {
            if dm.check_and_apply() {
                active_calc_derivs = dm.active_func();
                deopt_fired = true;
                tiered_locked_by_deopt = true;
            }
        }

        if !deopt_fired && !tiered_locked_by_deopt {
            if let Some(ref mut sched) = tiered_scheduler {
                active_calc_derivs = sched.on_step();
            }
        }

        pre_states.copy_from_slice(&states);
        pre_discrete_vals.copy_from_slice(&discrete_vals);

        let (mut diag_residual, mut diag_x) = (0.0_f64, 0.0_f64);
        let diag_res_ptr = &mut diag_residual as *mut f64;
        let diag_x_ptr = &mut diag_x as *mut f64;
        let mut diag_call_index = 0u32;
        let mut diag_time = 0.0_f64;
        diag_state.fill(0.0);
        let (eval_call_index_ptr, last_eval_time_ptr, last_eval_state_ptr, last_eval_state_len) =
            if newton_tearing_var_names.is_empty() {
                (
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                )
            } else {
                (
                    &mut diag_call_index as *mut u32,
                    &mut diag_time as *mut f64,
                    diag_state.as_mut_ptr(),
                    diag_state.len(),
                )
            };

        match run_event_iteration_at_time(
            time,
            t_end,
            active_calc_derivs,
            when_count,
            &mut states,
            &mut discrete_vals,
            &mut derivs,
            &params,
            &mut outputs,
            &mut when_states,
            &mut crossings,
            &pre_states,
            &pre_discrete_vals,
            &mut homotopy_lambda,
            homotopy_lambda_ptr,
            newton_tearing_var_names,
            output_start_vals,
            output_vars,
            &mut diag_residual,
            &mut diag_x,
            diag_res_ptr,
            diag_x_ptr,
            &mut diag_call_index,
            &mut diag_time,
            &mut prev_outputs,
            clock_partition_schedule,
            &mut **w,
            Some(&mut event_queue),
        )? {
            EventIterationOutcome::TerminatedOk => {
                fill_deopt_sim_perf_out(deopt_manager, deopt_sim_perf);
                return Ok(SimStepOutcome::Finished);
            }
            EventIterationOutcome::Completed => {}
        }
        commit_pre_after_event_and_reeval(
            time,
            t_end,
            active_calc_derivs,
            when_count,
            &mut states,
            &mut discrete_vals,
            &mut derivs,
            &params,
            &mut outputs,
            &mut when_states,
            &mut crossings,
            &mut pre_states,
            &mut pre_discrete_vals,
            homotopy_lambda_ptr,
            &mut diag_residual,
            diag_res_ptr,
            diag_x_ptr,
        )?;
        let dispatched_events = event_queue.drain_sorted();
        if !dispatched_events.is_empty() {
            let trace_dispatch = std::env::var("RUSTMODLICA_EVENT_TRACE")
                .ok()
                .map(|v| {
                    let t = v.trim();
                    t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false);
            for ev in &dispatched_events {
                if let QueuedEventKind::ClockPartition(id) = &ev.kind {
                    if let Some(ref mut collector) = profile_collector.as_mut() {
                        let idx = id.parse::<usize>().unwrap_or(0);
                        collector.record_clock_activation(idx, 0);
                    }
                }
            }
            let activated_clock_partitions: Vec<usize> = dispatched_events
                .iter()
                .filter_map(|ev| match &ev.kind {
                    QueuedEventKind::ClockPartition(id) => id.parse::<usize>().ok(),
                    _ => None,
                })
                .collect();
            crate::jit::speculation::validate_runtime_assumptions(
                state_vars,
                &states,
                &activated_clock_partitions,
            );
            if trace_dispatch {
                for ev in &dispatched_events {
                    match &ev.kind {
                        QueuedEventKind::ClockPartition(id) => {
                            if perf_enabled() {
                                perf_inc_clock_dispatch();
                            }
                            eprintln!(
                                "[event-dispatch] t={:.6} kind=clock_partition id={}",
                                ev.time, id
                            );
                        }
                        QueuedEventKind::WhenEdge(idx) => {
                            eprintln!(
                                "[event-dispatch] t={:.6} kind=when_edge idx={}",
                                ev.time, idx
                            );
                        }
                        QueuedEventKind::ZeroCrossing(idx) => {
                            eprintln!(
                                "[event-dispatch] t={:.6} kind=zero_crossing idx={}",
                                ev.time, idx
                            );
                        }
                    }
                }
            }
        }
        maybe_print_numeric_jacobian(
            numeric_ode_jacobian,
            time,
            epsilon,
            &states,
            active_calc_derivs,
            &params,
            &mut discrete_vals,
            &mut outputs,
            &mut when_states,
            &mut crossings,
            &pre_states,
            &pre_discrete_vals,
            t_end,
            symbolic_ode_jacobian,
            state_var_index,
            homotopy_lambda_ptr,
        );

        if time >= next_print - epsilon {
            csv_row.clear();
            write!(&mut csv_row, "{:.4}", time).map_err(|e| e.to_string())?;
            for val in &states {
                write!(&mut csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
            }
            for val in &discrete_vals {
                write!(&mut csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
            }
            for val in &outputs {
                write!(&mut csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
            }
            write_csv_line(w, &csv_row)?;
            rows_since_flush = rows_since_flush.saturating_add(1);
            if rows_since_flush >= CSV_ROWS_PER_FLUSH {
                flush_writer(w)?;
                rows_since_flush = 0;
            }
            if let Some(ref mut c) = result_collector {
                c.push((time, states.clone(), discrete_vals.clone(), outputs.clone()));
            }
            if let Some(ctrl) = control.as_mut() {
                ctrl.emit_sample(time, &states, &discrete_vals, &outputs);
            }
            next_print += print_interval;
        }

        // 2. Integration Step (Variable Step for Zero-Crossing)
        if states.is_empty() {
            empty_steps += 1;
            time = (empty_steps as f64) * dt;
            continue;
        }

        save_states.copy_from_slice(&states);
        save_discrete.copy_from_slice(&discrete_vals);
        save_crossings.copy_from_slice(&crossings);
        save_outputs.copy_from_slice(&outputs);
        scratch_outputs_for_step.copy_from_slice(&outputs);
        let mut dt_step = dt_active;

        // Trial Step -- suppress assertions during intermediate solver evaluations;
        // assertions are only semantically valid at accepted time points (event iteration).
        native::suppress_assert_begin();
        {
            let mut system = System {
                calc_derivs: active_calc_derivs,
                params: &params,
                discrete: &mut discrete_vals,
                outputs: &mut outputs,
                when_states: &mut when_states,
                crossings: &mut crossings,
                pre_states: &pre_states,
                pre_discrete: &pre_discrete_vals,
                t_end,
                diag_residual: diag_res_ptr,
                diag_x: diag_x_ptr,
                eval_call_index: eval_call_index_ptr,
                last_eval_time: last_eval_time_ptr,
                last_eval_state: last_eval_state_ptr,
                last_eval_state_len,
                scratch_outputs: if use_scratch_outputs_for_solver {
                    Some(&mut scratch_outputs_for_step)
                } else {
                    None
                },
                homotopy_lambda_ptr,
                buf_discrete: &mut step_buf_discrete,
                buf_when: &mut step_buf_when,
                buf_crossings: &mut step_buf_crossings,
                buf_outputs: &mut step_buf_outputs,
                buf_guess: &mut step_buf_guess,
                eval_count: 0,
                hotspot_threshold,
                simd_step_hits: 0,
                simd_step_fallbacks: 0,
                stack_scratch_enabled,
            };
            let step_res = if prefer_implicit_near_switch {
                backward_euler_solver.step(&mut system, time, dt_step, &mut states)
            } else if use_adaptive {
                let r = rk45_solver.step(&mut system, time, dt_step, &mut states);
                if r.is_ok() {
                    *adaptive_step_count += 1;
                }
                r
            } else if use_implicit {
                backward_euler_solver.step(&mut system, time, dt_step, &mut states)
            } else if use_radau {
                radau_solver.step(&mut system, time, dt_step, &mut states)
            } else if use_qss {
                qss_solver.step(&mut system, time, dt_step, &mut states)
            } else {
                rk4_solver.step(&mut system, time, dt_step, &mut states)
            };
            if step_res.is_ok() {
                if let Some(ref mut collector) = profile_collector.as_mut() {
                    collector.record_newton_iteration(1, true);
                }
                // Grow step back toward nominal after a clean step.
                if switch_recover_streak > 0 {
                    switch_recover_streak = switch_recover_streak.saturating_sub(1);
                }
                if switch_recover_streak == 0 {
                    dt_active = (dt_active * 2.0).min(dt);
                    // Keep BE preference sticky for ideal-switch models: PWM edges recur.
                    if !model_has_ideal_switch {
                        prefer_implicit_near_switch = false;
                    }
                }
            }
            if let Err(status) = step_res {
                if let Some(ref mut collector) = profile_collector.as_mut() {
                    collector.record_newton_iteration(0, status != 2);
                }
                match recover_failed_solver_step(
                    status,
                    time,
                    dt,
                    dt_step,
                    dt_active,
                    switch_recover_streak,
                    prefer_implicit_near_switch,
                    switch_discontinuity_nudges,
                    model_has_ideal_switch,
                    active_calc_derivs,
                    when_count,
                    t_end,
                    &params,
                    &pre_states,
                    &pre_discrete_vals,
                    output_vars,
                    output_start_vals,
                    newton_tearing_var_names,
                    &mut states,
                    &mut discrete_vals,
                    &mut derivs,
                    &mut outputs,
                    &mut when_states,
                    &mut crossings,
                    &mut save_states,
                    &mut save_discrete,
                    &mut save_outputs,
                    &mut scratch_outputs_for_step,
                    &mut diag_residual,
                    &mut diag_x,
                    diag_res_ptr,
                    diag_x_ptr,
                    &mut diag_call_index,
                    &mut diag_time,
                    &diag_state,
                    &mut homotopy_lambda,
                    homotopy_lambda_ptr,
                    &mut backward_euler_solver,
                    &mut rk4_solver,
                    &mut step_buf_discrete,
                    &mut step_buf_when,
                    &mut step_buf_crossings,
                    &mut step_buf_outputs,
                    &mut step_buf_guess,
                    use_scratch_outputs_for_solver,
                    hotspot_threshold,
                    stack_scratch_enabled,
                    eval_call_index_ptr,
                    last_eval_time_ptr,
                    last_eval_state_ptr,
                    last_eval_state_len,
                    clock_partition_schedule,
                    &mut **w,
                    &mut event_queue,
                    &mut prev_outputs,
                ) {
                    SwitchStepRecoverResult::Recovered {
                        dt_step: recovered_dt_step,
                        dt_active: recovered_dt_active,
                        switch_recover_streak: recovered_streak,
                        prefer_implicit_near_switch: recovered_prefer_implicit,
                    } => {
                        dt_step = recovered_dt_step;
                        dt_active = recovered_dt_active;
                        switch_recover_streak = recovered_streak;
                        prefer_implicit_near_switch = recovered_prefer_implicit;
                    }
                    SwitchStepRecoverResult::Nudge {
                        time: nudged_time,
                        switch_discontinuity_nudges: nudges,
                    } => {
                        time = nudged_time;
                        switch_discontinuity_nudges = nudges;
                        native::suppress_assert_end();
                        continue;
                    }
                    SwitchStepRecoverResult::TerminatedOk => {
                        fill_deopt_sim_perf_out(deopt_manager, deopt_sim_perf);
                        return Ok(SimStepOutcome::Finished);
                    }
                    SwitchStepRecoverResult::Fatal { status } => {
                        return Err(format!("Solver step failed with status {}", status));
                    }
                }
            }
        }
        // Scratch-based solver stages write algebraics into scratch only; sync before
        // trial-eval so Newton is not warm-started from a prior fail-open |x|~1e8 guess.
        let _ = commit_solver_algebraic_guess(
            output_vars,
            &mut outputs,
            &scratch_outputs_for_step,
            use_scratch_outputs_for_solver,
        );
        let t_trial = time + dt_step;

        trial_discrete.copy_from_slice(&discrete_vals);
        trial_when_states.copy_from_slice(&when_states);
        unsafe {
            let status = (active_calc_derivs)(
                t_trial,
                states.as_mut_ptr(),
                trial_discrete.as_mut_ptr(),
                derivs.as_mut_ptr(),
                params.as_ptr(),
                outputs.as_mut_ptr(),
                trial_when_states.as_mut_ptr(),
                crossings.as_mut_ptr(),
                pre_states.as_ptr(),
                pre_discrete_vals.as_ptr(),
                t_end,
                diag_res_ptr,
                diag_x_ptr,
                homotopy_lambda_ptr,
            );
            if status != 0 {
                if allow_zero_residual_newton(status, diag_residual) {
                    native::suppress_assert_end();
                    continue;
                }
                let trial_recover = recover_failed_trial_eval(
                    status,
                    time,
                    dt,
                    dt_step,
                    dt_active,
                    switch_recover_streak,
                    prefer_implicit_near_switch,
                    switch_discontinuity_nudges,
                    model_has_ideal_switch,
                    t_trial,
                    active_calc_derivs,
                    t_end,
                    &params,
                    &pre_states,
                    &pre_discrete_vals,
                    output_vars,
                    &mut states,
                    &mut discrete_vals,
                    &mut derivs,
                    &mut outputs,
                    &mut when_states,
                    &mut crossings,
                    &save_states,
                    &save_discrete,
                    &save_outputs,
                    &mut scratch_outputs_for_step,
                    &mut diag_residual,
                    &mut homotopy_lambda,
                    homotopy_lambda_ptr,
                    diag_res_ptr,
                    diag_x_ptr,
                    use_scratch_outputs_for_solver,
                );
                let trial_recovered = match trial_recover {
                    SwitchTrialRecoverResult::Recovered {
                        prefer_implicit_near_switch: recovered_prefer_implicit,
                        switch_recover_streak: recovered_streak,
                        dt_active: recovered_dt_active,
                    } => {
                        prefer_implicit_near_switch = recovered_prefer_implicit;
                        switch_recover_streak = recovered_streak;
                        dt_active = recovered_dt_active;
                        true
                    }
                    SwitchTrialRecoverResult::Nudge {
                        time: nudged_time,
                        switch_discontinuity_nudges: nudges,
                    } => {
                        time = nudged_time;
                        switch_discontinuity_nudges = nudges;
                        native::suppress_assert_end();
                        continue;
                    }
                    SwitchTrialRecoverResult::NotRecovered => false,
                };
                if trial_recovered {
                    // Fall through to zero-crossing checks with sanitized algebraics.
                } else {
                let t_fmt = format!("{:.4}", t_trial);
                eprintln!(
                    "{}",
                    i18n::msg(
                        "simulation_failed_trial",
                        &[&t_fmt as &dyn std::fmt::Display, &status]
                    )
                );
                if status == 2 {
                    eprintln!("{}", i18n::msg0("newton_failure"));
                    print_newton_diag("trial-eval", diag_call_index, diag_time, diag_residual, diag_x);
                    eprintln!(
                        "[trial] time={:.6}, state={:?}, diag_residual={:.6e}, diag_x={:.6e}",
                        t_trial, states, diag_residual, diag_x
                    );
                    if !newton_tearing_var_names.is_empty() {
                        let names = newton_tearing_var_names.join(", ");
                        let res_fmt = format!("{:.6e}", diag_residual);
                        let val_fmt = format!("{:.6e}", diag_x);
                        eprintln!(
                            "{}",
                            i18n::msg(
                                "tearing_vars_residual",
                                &[
                                    &names as &dyn std::fmt::Display,
                                    &res_fmt as &dyn std::fmt::Display,
                                    &val_fmt as &dyn std::fmt::Display
                                ]
                            )
                        );
                    }
                }
                let _ = flush_writer(w);
                native::suppress_assert_end();
                return Err(format!(
                    "Simulation failed at t={:.4} (trial step) with status {}",
                    t_trial, status
                ));
                }
            }
        }
        native::suppress_assert_end();

        crossing::handle_zero_crossings(
            crossings_count,
            &save_crossings,
            &mut crossings,
            profile_collector,
            t_trial,
            &mut event_queue,
            dt_step,
            dt,
            &mut time,
            &mut switch_discontinuity_nudges,
            &mut states,
            &save_states,
            &mut discrete_vals,
            &save_discrete,
            &mut outputs,
            &save_outputs,
            &mut scratch_outputs_for_step,
            output_vars,
            active_calc_derivs,
            &params,
            &mut when_states,
            &pre_states,
            &pre_discrete_vals,
            t_end,
            diag_res_ptr,
            diag_x_ptr,
            eval_call_index_ptr,
            last_eval_time_ptr,
            last_eval_state_ptr,
            last_eval_state_len,
            use_scratch_outputs_for_solver,
            &mut homotopy_lambda,
            homotopy_lambda_ptr,
            &mut step_buf_discrete,
            &mut step_buf_when,
            &mut step_buf_crossings,
            &mut step_buf_outputs,
            &mut step_buf_guess,
            hotspot_threshold,
            stack_scratch_enabled,
            use_adaptive,
            prefer_implicit_near_switch,
            use_implicit,
            use_radau,
            use_qss,
            &mut rk45_solver,
            &mut backward_euler_solver,
            &mut radau_solver,
            &mut qss_solver,
            &mut rk4_solver,
            adaptive_step_count,
            &mut diag_residual,
            &mut diag_x,
            diag_call_index,
            diag_time,
            &diag_state,
            newton_tearing_var_names,
            &mut derivs,
            &mut **w,
        )?;
    }
    Ok(SimStepOutcome::Continue)
}
