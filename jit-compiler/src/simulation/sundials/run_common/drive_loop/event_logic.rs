use super::*;

pub(super) struct EventDetection {
    pub(super) root_triggered: bool,
    pub(super) event_found: bool,
    pub(super) min_alpha: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn handle_root_triggered_event(
    kind: SundialsKind,
    mem: *mut c_void,
    y: N_Vector,
    yp: N_Vector,
    tret: &mut f64,
    time: &mut f64,
    dt: f64,
    min_step: f64,
    t_end: f64,
    min_alpha: f64,
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
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    homotopy_lambda_ptr: *const f64,
    max_same_event_hits: u32,
    same_event_time: bool,
    count_duplicate: bool,
    same_event_hits: &mut u32,
    last_event_time: &mut Option<f64>,
    stagnant_event_refines: &mut u32,
    roots_found: &mut [i32],
    last_counted_event_time: &mut Option<f64>,
) -> Result<bool, String> {
    if same_event_time {
        *same_event_hits = same_event_hits.saturating_add(1);
    } else {
        *same_event_hits = 0;
    }
    *last_event_time = Some(*time);
    *stagnant_event_refines = 0;
    roots_found.fill(0);
    if !count_duplicate {
        if sundials_event_log_enabled() {
            eprintln!("[sundials-event] t={:.6} root=true alpha={:.6}", *time, min_alpha);
        }
        *last_counted_event_time = Some(*time);
    }
    if *same_event_hits > max_same_event_hits {
        let mut t_force = (*time + dt.abs() * 0.1).min(t_end);
        if t_force <= *time + min_step {
            t_force = (*time + min_step).min(t_end);
        }
        super::force_advance_reinit(
            kind,
            mem,
            y,
            yp,
            t_force,
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
            t_end,
            diag_res_ptr,
            diag_x_ptr,
            homotopy_lambda_ptr,
            "near Zeno",
        )?;
        *tret = t_force;
        *time = t_force;
        *same_event_hits = 0;
        *last_event_time = Some(*time);
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn handle_crossing_refine_event(
    kind: SundialsKind,
    mem: *mut c_void,
    y: N_Vector,
    yp: N_Vector,
    tret: &mut f64,
    time: &mut f64,
    t_prev: f64,
    dt: f64,
    min_step: f64,
    t_end: f64,
    min_alpha: f64,
    root_triggered: bool,
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
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    homotopy_lambda_ptr: *const f64,
    max_same_event_hits: u32,
    same_event_time: bool,
    count_duplicate: bool,
    same_event_hits: &mut u32,
    last_event_time: &mut Option<f64>,
    stagnant_event_refines: &mut u32,
    roots_found: &mut [i32],
    last_counted_event_time: &mut Option<f64>,
    save_states: &[f64],
    save_discrete: &[f64],
    save_crossings: &[f64],
) -> Result<bool, String> {
    if same_event_time && min_alpha <= 1e-9 {
        *same_event_hits = same_event_hits.saturating_add(1);
        if *same_event_hits > max_same_event_hits {
            roots_found.fill(0);
        }
        return Ok(true);
    } else if !same_event_time {
        *same_event_hits = 0;
    }
    if min_alpha <= 1e-9 {
        *stagnant_event_refines = stagnant_event_refines.saturating_add(1);
        let mut t_force = (*time + dt.abs() * 0.1).min(t_end);
        if t_force <= *time + min_step {
            t_force = (*time + min_step).min(t_end);
        }
        super::force_advance_reinit(
            kind,
            mem,
            y,
            yp,
            t_force,
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
            t_end,
            diag_res_ptr,
            diag_x_ptr,
            homotopy_lambda_ptr,
            "on alpha~=0",
        )?;
        *tret = t_force;
        *time = t_force;
        *last_event_time = Some(*time);
        *same_event_hits = 0;
        return Ok(true);
    }
    *stagnant_event_refines = 0;
    let mut event_time = t_prev + (*time - t_prev) * min_alpha;
    if event_time <= t_prev + min_step * 0.1 {
        event_time = (t_prev + min_step).min(t_end);
    }
    states.copy_from_slice(save_states);
    discrete_vals.copy_from_slice(save_discrete);
    crossings.copy_from_slice(save_crossings);
    let y_ptr = N_VGetArrayPointer(y);
    if y_ptr.is_null() {
        return Err("SUNDIALS returned null state vector pointer before refine".to_string());
    }
    ptr::copy(states.as_ptr(), y_ptr, states.len());
    super::refine_event_step(
        kind,
        mem,
        y,
        yp,
        tret,
        t_prev,
        event_time,
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
        t_end,
        diag_res_ptr,
        diag_x_ptr,
        homotopy_lambda_ptr,
    )?;
    roots_found.fill(0);
    *time = *tret;
    *last_event_time = Some(*time);
    let y_ptr = N_VGetArrayPointer(y);
    if y_ptr.is_null() {
        return Err("SUNDIALS returned null state vector pointer after refine".to_string());
    }
    ptr::copy(y_ptr, states.as_mut_ptr(), states.len());
    if !count_duplicate {
        if sundials_event_log_enabled() {
            eprintln!(
                "[sundials-event] t={:.6} root={} alpha={:.6}",
                *time, root_triggered, min_alpha
            );
        }
        *last_counted_event_time = Some(*time);
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn post_step_evaluate_and_detect_events(
    kind: SundialsKind,
    mem: *mut c_void,
    step_code: i32,
    time: f64,
    t_end: f64,
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
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    homotopy_lambda_ptr: *const f64,
    eval_call_index_ptr: *mut u32,
    last_eval_time_ptr: *mut f64,
    last_eval_state_ptr: *mut f64,
    last_eval_state_len: usize,
    save_crossings: &[f64],
    roots_found: &mut [i32],
    event_queue: &mut EventQueue,
    w: &mut dyn Write,
) -> Result<EventDetection, String> {
    let t_trial = time;
    let mut eval_discrete = vec![0.0_f64; discrete_vals.len()];
    let mut eval_when_states = vec![0.0_f64; when_states.len()];
    let mut eval_crossings = vec![0.0_f64; crossings.len()];
    eval_discrete.copy_from_slice(discrete_vals);
    eval_when_states.fill(0.0);
    eval_crossings.fill(0.0);
    native::suppress_assert_begin();
    let eval_status = (calc_derivs)(
        t_trial,
        states.as_mut_ptr(),
        eval_discrete.as_mut_ptr(),
        derivs.as_mut_ptr(),
        params.as_ptr(),
        outputs.as_mut_ptr(),
        eval_when_states.as_mut_ptr(),
        eval_crossings.as_mut_ptr(),
        pre_states.as_ptr(),
        pre_discrete_vals.as_ptr(),
        t_end,
        diag_res_ptr,
        diag_x_ptr,
        homotopy_lambda_ptr,
    );
    native::suppress_assert_end();
    if eval_status != 0 {
        eprintln!(
            "{}",
            i18n::msg(
                "simulation_failed_trial",
                &[&format!("{:.4}", t_trial) as &dyn std::fmt::Display, &eval_status]
            )
        );
        let _ = flush_writer(w);
        return Err(format!("post-step evaluate failed: {}", eval_status));
    }
    if !eval_call_index_ptr.is_null() {
        *eval_call_index_ptr = (*eval_call_index_ptr).saturating_add(1);
    }
    if !last_eval_time_ptr.is_null() {
        *last_eval_time_ptr = t_trial;
    }
    if !last_eval_state_ptr.is_null() && last_eval_state_len == states.len() {
        ptr::copy_nonoverlapping(states.as_ptr(), last_eval_state_ptr, states.len());
    }
    crossings.copy_from_slice(&eval_crossings);

    let mut root_triggered = false;
    if !crossings.is_empty() {
        match kind {
            SundialsKind::Cvode => {
                if step_code == CV_ROOT_RETURN as i32 {
                    let _ = CVodeGetRootInfo(mem, roots_found.as_mut_ptr());
                    root_triggered = roots_found.iter().any(|&v| v != 0);
                    for (idx, root) in roots_found.iter().enumerate() {
                        if *root != 0 {
                            event_queue.push_unique(QueuedEvent {
                                time,
                                kind: QueuedEventKind::ZeroCrossing(idx),
                            });
                        }
                    }
                }
            }
            SundialsKind::Ida => {
                if step_code == IDA_ROOT_RETURN as i32 {
                    let _ = IDAGetRootInfo(mem, roots_found.as_mut_ptr());
                    root_triggered = roots_found.iter().any(|&v| v != 0);
                    for (idx, root) in roots_found.iter().enumerate() {
                        if *root != 0 {
                            event_queue.push_unique(QueuedEvent {
                                time,
                                kind: QueuedEventKind::ZeroCrossing(idx),
                            });
                        }
                    }
                }
            }
        }
    }

    let mut event_found = root_triggered;
    let mut min_alpha = 1.0_f64;
    for i in 0..crossings.len() {
        let c_prev = save_crossings[i];
        let c_curr = crossings[i];
        if c_prev * c_curr < 0.0 {
            event_found = true;
            event_queue.push_unique(QueuedEvent {
                time,
                kind: QueuedEventKind::ZeroCrossing(i),
            });
            let d = c_curr - c_prev;
            if d.abs() > 1e-12 {
                let alpha = (-c_prev / d).clamp(0.0, 1.0);
                if alpha > 0.0 && alpha < min_alpha {
                    min_alpha = alpha;
                }
            }
        }
    }
    Ok(EventDetection {
        root_triggered,
        event_found,
        min_alpha,
    })
}
