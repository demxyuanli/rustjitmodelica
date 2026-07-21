//! Ideal-switch solver-step and trial-eval recovery (micro-steps, bisect, nudge).

mod step;
mod trial;

pub(crate) use step::recover_failed_solver_step;
pub(crate) use trial::recover_failed_trial_eval;

pub(crate) const SWITCH_DISCONTINUITY_NUDGE_MAX: u32 = 256;

pub(crate) enum SwitchStepRecoverResult {
    Recovered {
        dt_step: f64,
        dt_active: f64,
        switch_recover_streak: u32,
        prefer_implicit_near_switch: bool,
    },
    Nudge {
        time: f64,
        switch_discontinuity_nudges: u32,
    },
    TerminatedOk,
    Fatal {
        status: i32,
    },
}

pub(crate) enum SwitchTrialRecoverResult {
    Recovered {
        prefer_implicit_near_switch: bool,
        switch_recover_streak: u32,
        dt_active: f64,
    },
    Nudge {
        time: f64,
        switch_discontinuity_nudges: u32,
    },
    NotRecovered,
}
