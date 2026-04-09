use super::*;

mod event_logic;

use event_logic::{
    handle_crossing_refine_event, handle_root_triggered_event,
    post_step_evaluate_and_detect_events,
};

fn write_output_row(
    time: f64,
    states: &[f64],
    discrete_vals: &[f64],
    outputs: &[f64],
    crossings: &[f64],
    tail_crossing_deadband: f64,
    tail_height_deadband: f64,
    tail_velocity_deadband: f64,
    csv_row: &mut String,
    w: &mut dyn Write,
    rows_since_flush: &mut u32,
    result_collector: &mut Option<&mut ResultCollector>,
) -> Result<(), String> {
    csv_row.clear();
    write!(csv_row, "{:.4}", time).map_err(|e| e.to_string())?;
    let mut display_states: Option<Vec<f64>> = None;
    if states.len() >= 2
        && !crossings.is_empty()
        && crossings[0].abs() < tail_crossing_deadband
        && states[0].abs() < tail_height_deadband
        && states[1].abs() < tail_velocity_deadband
    {
        let mut ds = states.to_vec();
        ds[1] = 0.0;
        display_states = Some(ds);
    }
    let state_view: &[f64] = match display_states.as_ref() {
        Some(v) => v.as_slice(),
        None => states,
    };
    for val in state_view.iter() {
        write!(csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
    }
    for val in discrete_vals.iter() {
        write!(csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
    }
    for val in outputs.iter() {
        write!(csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
    }
    write_csv_line(w, csv_row)?;
    *rows_since_flush = rows_since_flush.saturating_add(1);
    if *rows_since_flush >= CSV_ROWS_PER_FLUSH {
        flush_writer(w)?;
        *rows_since_flush = 0;
    }
    if let Some(ref mut c) = result_collector {
        c.push((time, states.to_vec(), discrete_vals.to_vec(), outputs.to_vec()));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
unsafe fn force_advance_reinit(
    kind: SundialsKind,
    mem: *mut c_void,
    y: N_Vector,
    yp: N_Vector,
    t_force: f64,
    calc_derivs: CalcDerivsFunc,
    states: &mut Vec<f64>,
    discrete_vals: &mut Vec<f64>,
    derivs: &mut [f64],
    params: &[f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    pre_states: &mut [f64],
    pre_discrete_vals: &mut [f64],
    t_end: f64,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    homotopy_lambda_ptr: *const f64,
    err_ctx: &str,
) -> Result<(), String> {
    let force_code = match kind {
        SundialsKind::Cvode => CVodeReInit(mem, t_force as sunrealtype, y),
        SundialsKind::Ida => {
            native::suppress_assert_begin();
            let st0 = (calc_derivs)(
                t_force,
                states.as_mut_ptr(),
                discrete_vals.as_mut_ptr(),
                derivs.as_mut_ptr(),
                params.as_ptr(),
                outputs.as_mut_ptr(),
                when_states.as_mut_ptr(),
                crossings.as_mut_ptr(),
                pre_states.as_ptr(),
                pre_discrete_vals.as_ptr(),
                t_end,
                diag_res_ptr,
                diag_x_ptr,
                homotopy_lambda_ptr,
            );
            native::suppress_assert_end();
            if st0 != 0 {
                return Err(format!("IDA force-advance derivative failed {}: {}", err_ctx, st0));
            }
            let yp_ptr = N_VGetArrayPointer(yp);
            if yp_ptr.is_null() {
                return Err(format!(
                    "IDA returned null derivative vector pointer on force advance {}",
                    err_ctx
                ));
            }
            ptr::copy(derivs.as_ptr(), yp_ptr, derivs.len());
            IDAReInit(mem, t_force as sunrealtype, y, yp)
        }
    };
    if force_code != CV_SUCCESS as i32 && force_code != IDA_SUCCESS as i32 {
        return Err(format!(
            "SUNDIALS force advance reinit failed {}: {}",
            err_ctx, force_code
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
unsafe fn refine_event_step(
    kind: SundialsKind,
    mem: *mut c_void,
    y: N_Vector,
    yp: N_Vector,
    tret: &mut f64,
    t_prev: f64,
    event_time: f64,
    calc_derivs: CalcDerivsFunc,
    states: &mut Vec<f64>,
    discrete_vals: &mut Vec<f64>,
    derivs: &mut [f64],
    params: &[f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    pre_states: &mut [f64],
    pre_discrete_vals: &mut [f64],
    t_end: f64,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    homotopy_lambda_ptr: *const f64,
) -> Result<(), String> {
    let reinit_code = match kind {
        SundialsKind::Cvode => CVodeReInit(mem, t_prev as sunrealtype, y),
        SundialsKind::Ida => {
            native::suppress_assert_begin();
            let st0 = (calc_derivs)(
                t_prev,
                states.as_mut_ptr(),
                discrete_vals.as_mut_ptr(),
                derivs.as_mut_ptr(),
                params.as_ptr(),
                outputs.as_mut_ptr(),
                when_states.as_mut_ptr(),
                crossings.as_mut_ptr(),
                pre_states.as_ptr(),
                pre_discrete_vals.as_ptr(),
                t_end,
                diag_res_ptr,
                diag_x_ptr,
                homotopy_lambda_ptr,
            );
            native::suppress_assert_end();
            if st0 != 0 {
                return Err(format!("IDA reinit pre-derivative failed: {}", st0));
            }
            let yp_ptr = N_VGetArrayPointer(yp);
            if yp_ptr.is_null() {
                return Err("IDA returned null derivative vector pointer before reinit".to_string());
            }
            ptr::copy(derivs.as_ptr(), yp_ptr, derivs.len());
            IDAReInit(mem, t_prev as sunrealtype, y, yp)
        }
    };
    if reinit_code != CV_SUCCESS as i32 && reinit_code != IDA_SUCCESS as i32 {
        return Err(format!("SUNDIALS reinit failed: {}", reinit_code));
    }
    native::suppress_assert_begin();
    let refine_code = match kind {
        SundialsKind::Cvode => CVode(mem, event_time as sunrealtype, y, tret, CV_NORMAL as i32),
        SundialsKind::Ida => IDASolve(
            mem,
            event_time as sunrealtype,
            tret,
            y,
            yp,
            IDA_NORMAL as i32,
        ),
    };
    native::suppress_assert_end();
    let refine_ok = match kind {
        SundialsKind::Cvode => refine_code == CV_SUCCESS as i32 || refine_code == CV_ROOT_RETURN as i32,
        SundialsKind::Ida => refine_code == IDA_SUCCESS as i32 || refine_code == IDA_ROOT_RETURN as i32,
    };
    if !refine_ok {
        return Err(format!("SUNDIALS event refine failed: {}", refine_code));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(super) unsafe fn drive_print_loop(
    kind: SundialsKind,
    mem: *mut c_void,
    y: N_Vector,
    yp: N_Vector,
    tret: &mut f64,
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    states: &mut Vec<f64>,
    discrete_vals: &mut Vec<f64>,
    derivs: &mut [f64],
    params: &[f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    pre_states: &mut [f64],
    pre_discrete_vals: &mut [f64],
    homotopy_lambda: &mut f64,
    homotopy_lambda_ptr: *const f64,
    newton_tearing_var_names: &[String],
    output_start_vals: &[f64],
    output_vars: &[String],
    _discrete_vars: &[String],
    _state_vars: &[String],
    state_var_index: &std::collections::HashMap<String, usize>,
    t_end: f64,
    dt: f64,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    diag_residual: &mut f64,
    diag_x: &mut f64,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    diag_call_index: &mut u32,
    diag_time: &mut f64,
    _diag_state: &mut [f64],
    eval_call_index_ptr: *mut u32,
    last_eval_time_ptr: *mut f64,
    last_eval_state_ptr: *mut f64,
    last_eval_state_len: usize,
    prev_outputs: &mut [f64],
    w: &mut dyn Write,
    csv_row: &mut String,
    rows_since_flush: &mut u32,
    print_interval: f64,
    next_print: &mut f64,
    epsilon: f64,
    result_collector: &mut Option<&mut ResultCollector>,
) -> Result<(), String> {
    *tret = 0.0_f64;
    let mut time = *tret;
    let min_step = 1e-10_f64;
    let debounce_cfg = EventDebounceConfig::adaptive_from_dt(dt);
    let event_deadband = debounce_cfg.base_deadband;
    let count_deadband = debounce_cfg.count_deadband;
    let max_same_event_hits = debounce_cfg.max_same_event_hits;
    let tail_crossing_deadband = env_f64("RUSTMODLICA_TAIL_CROSSING_DEADBAND").unwrap_or(5e-3);
    let tail_height_deadband = env_f64("RUSTMODLICA_TAIL_HEIGHT_DEADBAND").unwrap_or(2e-4);
    let tail_velocity_deadband = env_f64("RUSTMODLICA_TAIL_VELOCITY_DEADBAND").unwrap_or(3e-2);
    let mut stagnant_steps = 0_u32;
    let mut stagnant_event_refines = 0_u32;
    let mut last_event_time: Option<f64> = None;
    let mut same_event_hits = 0_u32;
    let mut last_counted_event_time: Option<f64> = None;
    let mut save_states = vec![0.0_f64; states.len()];
    let mut save_discrete = vec![0.0_f64; discrete_vals.len()];
    let mut save_crossings = vec![0.0_f64; crossings.len()];
    let mut roots_found = vec![0_i32; crossings.len()];
    let mut event_queue = EventQueue::default();
    while time <= t_end + epsilon {
        native::reset_assert_counter();
        pre_states.copy_from_slice(states);
        pre_discrete_vals.copy_from_slice(discrete_vals);

        match run_event_iteration_at_time(
            time,
            t_end,
            calc_derivs,
            when_count,
            states,
            discrete_vals,
            derivs,
            params,
            outputs,
            when_states,
            crossings,
            pre_states,
            pre_discrete_vals,
            homotopy_lambda,
            homotopy_lambda_ptr,
            newton_tearing_var_names,
            output_start_vals,
            output_vars,
            diag_residual,
            diag_x,
            diag_res_ptr,
            diag_x_ptr,
            diag_call_index,
            diag_time,
            prev_outputs,
            &[],
            w,
            Some(&mut event_queue),
        )? {
            EventIterationOutcome::TerminatedOk => return Ok(()),
            EventIterationOutcome::Completed => {}
        }
        let _dispatched_events = event_queue.drain_sorted();

        maybe_print_numeric_jacobian(
            numeric_ode_jacobian,
            time,
            epsilon,
            states,
            calc_derivs,
            params,
            discrete_vals,
            outputs,
            when_states,
            crossings,
            pre_states,
            pre_discrete_vals,
            t_end,
            symbolic_ode_jacobian,
            state_var_index,
            homotopy_lambda_ptr,
        );

        if time >= *next_print - epsilon {
            write_output_row(
                time,
                states,
                discrete_vals,
                outputs,
                crossings,
                tail_crossing_deadband,
                tail_height_deadband,
                tail_velocity_deadband,
                csv_row,
                w,
                rows_since_flush,
                result_collector,
            )?;
            *next_print += print_interval;
        }

        if time >= t_end - epsilon {
            break;
        }

        let dt_step = dt.abs().max(min_step);
        let mut tout = (time + dt_step).min(t_end);
        tout = tout.min(*next_print);
        if tout <= time + 1e-14 && time < t_end - epsilon {
            tout = (time + min_step).min(t_end);
        }
        let tout = tout.max(time + 1e-14);
        let tout = tout as sunrealtype;
        let t_prev = time;
        save_states.copy_from_slice(states);
        save_discrete.copy_from_slice(discrete_vals);
        save_crossings.copy_from_slice(crossings);

        native::suppress_assert_begin();
        let step_code = match kind {
            SundialsKind::Cvode => CVode(mem, tout, y, tret, CV_NORMAL as i32),
            SundialsKind::Ida => IDASolve(mem, tout, tret, y, yp, IDA_NORMAL as i32),
        };
        native::suppress_assert_end();

        let ok = match kind {
            SundialsKind::Cvode => {
                step_code == CV_SUCCESS as i32 || step_code == CV_ROOT_RETURN as i32
            }
            SundialsKind::Ida => {
                step_code == IDA_SUCCESS as i32 || step_code == IDA_ROOT_RETURN as i32
            }
        };
        if !ok {
            eprintln!(
                "{}",
                i18n::msg(
                    "simulation_failed_at",
                    &[&format!("{:.4}", time) as &dyn std::fmt::Display, &step_code]
                )
            );
            if step_code == 2 {
                print_newton_diag("sundials-step", *diag_call_index, *diag_time, *diag_residual, *diag_x);
            }
            let _ = flush_writer(w);
            return Err(format!("SUNDIALS step failed: {}", step_code));
        }

        time = *tret;
        let y_ptr = N_VGetArrayPointer(y);
        if y_ptr.is_null() {
            return Err("SUNDIALS returned null state vector pointer".to_string());
        }
        ptr::copy(y_ptr, states.as_mut_ptr(), states.len());
        if time <= t_prev + min_step * 0.1 {
            stagnant_steps = stagnant_steps.saturating_add(1);
            if stagnant_steps > 8 {
                return Err("SUNDIALS stepping stalled near an event (time not advancing)".to_string());
            }
        } else {
            stagnant_steps = 0;
        }

        fail_if_assert_storm("sundials-step", time)?;

        let detection = post_step_evaluate_and_detect_events(
            kind,
            mem,
            step_code,
            time,
            t_end,
            calc_derivs,
            states,
            discrete_vals,
            derivs,
            params,
            outputs,
            when_states,
            crossings,
            pre_states,
            pre_discrete_vals,
            diag_res_ptr,
            diag_x_ptr,
            homotopy_lambda_ptr,
            eval_call_index_ptr,
            last_eval_time_ptr,
            last_eval_state_ptr,
            last_eval_state_len,
            &save_crossings,
            &mut roots_found,
            &mut event_queue,
            w,
        )?;
        let root_triggered = detection.root_triggered;
        let event_found = detection.event_found;
        let min_alpha = detection.min_alpha;

        if event_found {
            let same_event_time = last_event_time
                .map(|t_last| (time - t_last).abs() <= event_deadband)
                .unwrap_or(false);
            let count_duplicate = last_counted_event_time
                .map(|t_last| (time - t_last).abs() <= count_deadband)
                .unwrap_or(false);
            if root_triggered {
                if handle_root_triggered_event(
                    kind,
                    mem,
                    y,
                    yp,
                    tret,
                    &mut time,
                    dt,
                    min_step,
                    t_end,
                    min_alpha,
                    calc_derivs,
                    states,
                    discrete_vals,
                    derivs,
                    params,
                    outputs,
                    when_states,
                    crossings,
                    pre_states,
                    pre_discrete_vals,
                    diag_res_ptr,
                    diag_x_ptr,
                    homotopy_lambda_ptr,
                    max_same_event_hits,
                    same_event_time,
                    count_duplicate,
                    &mut same_event_hits,
                    &mut last_event_time,
                    &mut stagnant_event_refines,
                    &mut roots_found,
                    &mut last_counted_event_time,
                )? {
                    continue;
                }
            } else if handle_crossing_refine_event(
                kind,
                mem,
                y,
                yp,
                tret,
                &mut time,
                t_prev,
                dt,
                min_step,
                t_end,
                min_alpha,
                root_triggered,
                calc_derivs,
                states,
                discrete_vals,
                derivs,
                params,
                outputs,
                when_states,
                crossings,
                pre_states,
                pre_discrete_vals,
                diag_res_ptr,
                diag_x_ptr,
                homotopy_lambda_ptr,
                max_same_event_hits,
                same_event_time,
                count_duplicate,
                &mut same_event_hits,
                &mut last_event_time,
                &mut stagnant_event_refines,
                &mut roots_found,
                &mut last_counted_event_time,
                &save_states,
                &save_discrete,
                &save_crossings,
            )? {
                continue;
            }
        }
    }
    Ok(())
}
