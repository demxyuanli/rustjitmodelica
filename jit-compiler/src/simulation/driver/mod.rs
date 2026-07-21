//! Main fixed/adaptive simulation driver (`run_simulation`).

mod crossing;
mod run;
mod sched;
mod sundials_dispatch;

pub use run::run_simulation;

use crate::jit::deopt::DeoptSimPerfSummary;

pub(super) fn fill_deopt_sim_perf_out(
    deopt_manager: &Option<crate::jit::deopt::DeoptManager>,
    deopt_sim_perf: Option<&mut DeoptSimPerfSummary>,
) {
    if let Some(out) = deopt_sim_perf {
        *out = match deopt_manager {
            Some(dm) => DeoptSimPerfSummary::from_manager(dm),
            None => DeoptSimPerfSummary::default(),
        };
    }
}
