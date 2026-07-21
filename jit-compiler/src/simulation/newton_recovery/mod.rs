pub use crate::newton_policy::{
    allow_algebraic_newton_fallback, allow_zero_residual_newton, decide_switch_force_s0,
    is_ideal_switch_complementarity_name, report_switch_fail_open_degraded,
    report_switch_force_s0_rejected, SwitchForceS0Decision,
};

mod algebraic;
mod assert_diag;
mod eval_helpers;
mod ideal_switch;
mod t0;

pub use algebraic::{
    commit_solver_algebraic_guess, prepare_event_step_algebraic_guess,
    sanitize_extreme_algebraic_outputs, warm_start_ideal_switch_outputs,
};
pub use assert_diag::{fail_if_assert_storm, print_newton_diag};
pub use ideal_switch::{recover_newton_ideal_switch, try_switch_mode_seed_newton};
pub use t0::recover_newton_at_t0;
