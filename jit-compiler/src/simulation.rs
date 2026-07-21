// RT1-1: DAE/ODE solver with events. Adaptive RK45 when no when/zero-crossing; fixed-step with
// event detection and reinit when when/zero-crossing present. Event iteration at each time step.

pub mod checkpoint;
pub mod control;
mod driver;
mod events;
pub mod pgo;
mod jacobian;
mod newton_recovery;
mod switch_drive;
mod sim_io;
mod step;
mod types;
#[cfg(feature = "sundials")]
mod sundials;

#[cfg(feature = "sundials")]
pub use sundials::{
    kinsol_solve_square_spgmr, parse_linsol_env, KinResidualFn, KinsolCallbackPack, SundialsLinSolKind,
};

pub type SimulationResult = types::SimulationResult;
pub use self::control::{is_sim_cancelled_error, SimControl, SimProgress, SIM_CANCELLED_MSG};
pub use self::types::run_simulation_collect;
pub use self::driver::run_simulation;

pub fn runtime_perf_counters() -> (u64, u64) {
    events::perf_snapshot()
}
