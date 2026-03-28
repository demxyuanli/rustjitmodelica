//! Fixed-point event iteration at the current simulation time (when clauses, algebraic refinement).

use crate::i18n;
use crate::compiler::{ClockPartitionScheduleEntry, ClockPartitionTrigger};
use crate::diag::fallback_counter;
use crate::jit::native;
use crate::jit::CalcDerivsFunc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use super::newton_recovery::{
    allow_algebraic_newton_fallback, allow_zero_residual_newton, fail_if_assert_storm,
    print_newton_diag, recover_newton_at_t0,
};
use super::sim_io::flush_writer;
use super::types::{EventQueue, QueuedEvent, QueuedEventKind};

pub(crate) enum EventIterationOutcome {
    Completed,
    TerminatedOk,
}

static PERF_EVENT_ITER_TOTAL: AtomicU64 = AtomicU64::new(0);
static PERF_CLOCK_DISPATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static PERF_ENABLED: OnceLock<bool> = OnceLock::new();
static EVENT_TRACE_ENABLED: OnceLock<bool> = OnceLock::new();

pub(crate) fn perf_enabled() -> bool {
    *PERF_ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_PERF_TRACE")
            .ok()
            .map(|v| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

fn event_trace_enabled() -> bool {
    *EVENT_TRACE_ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_EVENT_TRACE")
            .ok()
            .map(|v| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

pub(crate) fn perf_reset_counters() {
    PERF_EVENT_ITER_TOTAL.store(0, Ordering::Relaxed);
    PERF_CLOCK_DISPATCH_TOTAL.store(0, Ordering::Relaxed);
}

pub(crate) fn perf_inc_event_iter() {
    PERF_EVENT_ITER_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn perf_inc_clock_dispatch() {
    PERF_CLOCK_DISPATCH_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn perf_snapshot() -> (u64, u64) {
    (
        PERF_EVENT_ITER_TOTAL.load(Ordering::Relaxed),
        PERF_CLOCK_DISPATCH_TOTAL.load(Ordering::Relaxed),
    )
}

#[derive(Debug, Clone, Copy)]
struct SyncEvent {
    priority: u32,
}

#[derive(Debug, Default)]
struct SyncEventQueue {
    items: Vec<SyncEvent>,
}

impl SyncEventQueue {
    fn push(&mut self, event: SyncEvent) {
        self.items.push(event);
    }

    fn sort_by_priority(&mut self) {
        self.items.sort_by_key(|e| e.priority);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_event_iteration_at_time(
    time: f64,
    t_end: f64,
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    states: &mut [f64],
    discrete_vals: &mut [f64],
    derivs: &mut [f64],
    params: &[f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    pre_states: &[f64],
    pre_discrete_vals: &[f64],
    homotopy_lambda: &mut f64,
    homotopy_lambda_ptr: *const f64,
    newton_tearing_var_names: &[String],
    output_start_vals: &[f64],
    output_vars: &[String],
    diag_residual: &mut f64,
    diag_x: &mut f64,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    diag_call_index: &mut u32,
    diag_time: &mut f64,
    prev_outputs: &mut [f64],
    clock_partition_schedule: &[ClockPartitionScheduleEntry],
    w: &mut dyn std::io::Write,
    mut event_queue: Option<&mut EventQueue>,
) -> Result<EventIterationOutcome, String> {
    fn sample_active(time: f64, start: f64, interval: f64) -> bool {
        const SAMPLE_EPS: f64 = 1e-9;
        if interval <= 0.0 {
            return false;
        }
        let phase = (time - start) / interval;
        let k = phase.floor();
        let frac = phase - k;
        frac < SAMPLE_EPS || (1.0 - frac) < SAMPLE_EPS || (time - start).abs() < SAMPLE_EPS
    }

    let active_partition_ids: Vec<&str> = clock_partition_schedule
        .iter()
        .filter_map(|p| match p.trigger {
            ClockPartitionTrigger::Always => Some(p.id.as_str()),
            ClockPartitionTrigger::Sample { start, interval } => {
                if sample_active(time, start, interval) {
                    Some(p.id.as_str())
                } else {
                    None
                }
            }
        })
        .collect();
    if let Some(queue) = event_queue.as_mut() {
        for partition_id in &active_partition_ids {
            queue.push_unique(QueuedEvent {
                time,
                kind: QueuedEventKind::ClockPartition((*partition_id).to_string()),
            });
        }
    }

    let trace_events = event_trace_enabled();
    let mut event_iter_count = 0;
    const ALG_FIXED_POINT_MAX: u32 = 15;
    let do_alg_iter =
        states.is_empty()
            && !output_vars.is_empty()
            && !newton_tearing_var_names.is_empty()
            && (clock_partition_schedule.is_empty() || !active_partition_ids.is_empty());
    let mut alg_iter = 0u32;
    prev_outputs.fill(0.0);
    let mut sync_queue = SyncEventQueue::default();
    if !active_partition_ids.is_empty() {
        sync_queue.push(SyncEvent { priority: 0 });
        sync_queue.sort_by_priority();
    }

    loop {
        let prev_state_snapshot = if trace_events {
            Some(states.to_vec())
        } else {
            None
        };
        let prev_discrete_snapshot = if trace_events {
            Some(discrete_vals.to_vec())
        } else {
            None
        };
        // Reset "new" when-condition slots before each derivative/event evaluation.
        // Generated evaluators may only write active conditions; keeping stale values
        // here can suppress later rising edges on periodic clock partitions.
        if when_count > 0 {
            for i in 0..when_count {
                when_states[i * 2 + 1] = 0.0;
            }
        }
        unsafe {
            native::suppress_assert_begin();
            let status = (calc_derivs)(
                time,
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
            if status != 0 {
                if allow_zero_residual_newton(status, *diag_residual) {
                    break;
                }
                if time == 0.0 && status == 2 && event_iter_count == 0 {
                    let recovered = recover_newton_at_t0(
                        calc_derivs,
                        time,
                        t_end,
                        params,
                        pre_states,
                        pre_discrete_vals,
                        output_start_vals,
                        output_vars,
                        states,
                        discrete_vals,
                        derivs,
                        outputs,
                        when_states,
                        crossings,
                        diag_res_ptr,
                        diag_x_ptr,
                        diag_residual,
                        homotopy_lambda,
                        homotopy_lambda_ptr,
                    );
                    if recovered {
                        break;
                    }
                    // Do not continue ODE integration after failed t=0 recovery unless Newton reports
                    // a numerically negligible residual (avoids accepting huge |r| for large DAEs).
                    if !states.is_empty() && allow_zero_residual_newton(status, *diag_residual) {
                        fallback_counter::inc_newton_init_accept();
                        eprintln!(
                            "[fallback:newton-init] accepting t=0 Newton non-convergence (residual={:.6e}), continuing with current values",
                            *diag_residual
                        );
                        break;
                    }
                }
                if allow_algebraic_newton_fallback(status, states.len()) {
                    fallback_counter::inc_newton_event_accept();
                    eprintln!(
                        "[fallback:newton-event] phase=event-iteration-fallback eval_calls={} last_eval_time={:.6} diag_residual={:.6e} diag_x={:.6e} (algebraic Newton fallback accepted)",
                        *diag_call_index, *diag_time, *diag_residual, *diag_x,
                    );
                    break;
                }
                let t_fmt = format!("{:.4}", time);
                eprintln!(
                    "{}",
                    i18n::msg(
                        "simulation_failed_at",
                        &[&t_fmt as &dyn std::fmt::Display, &status]
                    )
                );
                if status == 2 {
                    eprintln!("{}", i18n::msg0("newton_failure"));
                    print_newton_diag(
                        "event-iteration",
                        *diag_call_index,
                        *diag_time,
                        *diag_residual,
                        *diag_x,
                    );
                    if !newton_tearing_var_names.is_empty() {
                        let names = newton_tearing_var_names.join(", ");
                        let res_fmt = format!("{:.6e}", *diag_residual);
                        let val_fmt = format!("{:.6e}", *diag_x);
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
                return Err(format!(
                    "Simulation failed at t={:.4} with status {}",
                    time, status
                ));
            }
        }
        fail_if_assert_storm("event-iteration", time)?;

        if do_alg_iter && alg_iter < ALG_FIXED_POINT_MAX {
            let max_diff = if alg_iter == 0 {
                1.0
            } else {
                let mut m = 0.0_f64;
                for i in 0..outputs.len() {
                    let d = (prev_outputs[i] - outputs[i]).abs();
                    if d > m {
                        m = d;
                    }
                }
                m
            };
            if alg_iter > 0 && max_diff < 1e-10 {
                break;
            }
            prev_outputs.copy_from_slice(outputs);
            alg_iter += 1;
            if alg_iter < ALG_FIXED_POINT_MAX {
                continue;
            }
        }

        if native::terminate_requested() {
            println!(
                "{}",
                i18n::msg(
                    "simulation_terminated",
                    &[&format!("{:.4}", time) as &dyn std::fmt::Display]
                )
            );
            flush_writer(w)?;
            return Ok(EventIterationOutcome::TerminatedOk);
        }

        let mut converged = true;
        let mut state_changed = false;
        let mut discrete_changed = false;
        if let Some(prev) = &prev_state_snapshot {
            state_changed = prev
                .iter()
                .zip(states.iter())
                .any(|(a, b)| (a - b).abs() > 1e-12);
        }
        if let Some(prev) = &prev_discrete_snapshot {
            discrete_changed = prev
                .iter()
                .zip(discrete_vals.iter())
                .any(|(a, b)| (a - b).abs() > 1e-12);
        }
        if when_count > 0 {
            for i in 0..when_count {
                let idx_pre = i * 2;
                let idx_new = i * 2 + 1;
                let pre_val = when_states[idx_pre];
                let new_val = when_states[idx_new];

                if pre_val != new_val {
                    when_states[idx_pre] = new_val;
                    converged = false;
                    if let Some(queue) = event_queue.as_mut() {
                        queue.push_unique(QueuedEvent {
                            time,
                            kind: QueuedEventKind::WhenEdge(i),
                        });
                    }
                }
                if trace_events {
                    eprintln!(
                        "[event-trace] t={:.6} iter={} when[{}] pre={:.0} new={:.0}",
                        time, event_iter_count, i, pre_val, new_val
                    );
                }
            }
        }
        if state_changed || discrete_changed {
            converged = false;
        }
        if trace_events {
            if !clock_partition_schedule.is_empty() {
                let ids = if active_partition_ids.is_empty() {
                    "-".to_string()
                } else {
                    active_partition_ids.join("|")
                };
                eprintln!(
                    "[event-trace] t={:.6} active_clock_partitions={}",
                    time, ids
                );
            }
            eprintln!(
                "[event-trace] t={:.6} iter={} state_changed={} discrete_changed={} converged={}",
                time, event_iter_count, state_changed, discrete_changed, converged
            );
        }

        if converged {
            break;
        }

        event_iter_count += 1;
        if perf_enabled() {
            perf_inc_event_iter();
        }
        if event_iter_count > 100 {
            eprintln!("{}", i18n::msg("event_loop_no_converge", &[&time]));
            break;
        }
    }

    Ok(EventIterationOutcome::Completed)
}
