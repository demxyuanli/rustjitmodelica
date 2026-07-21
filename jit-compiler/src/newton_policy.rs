//! Shared Newton acceptance policy (solver + simulation) without solver/simulation crate cycles.

use std::sync::OnceLock;

fn env_flag_true(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

/// When RUSTMODLICA_STRICT_NEWTON=1, do not accept Newton failures via zero-residual or algebraic fallbacks.
pub(crate) fn strict_newton_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_true("RUSTMODLICA_STRICT_NEWTON"))
}

/// Opt-in: allow ideal-switch force-s0 paths to continue when |r| exceeds accept tol.
/// Default OFF. Production should leave unset; Chopper/PWM short-sim may set
/// `RUSTMODLICA_SWITCH_FAIL_OPEN=1` and must treat stderr `[degraded:switch-fail-open]` as a quality signal.
pub fn switch_fail_open_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        if strict_newton_enabled() {
            return false;
        }
        env_flag_true("RUSTMODLICA_SWITCH_FAIL_OPEN")
    })
}

/// Residual bound for accepting force-s0 / switch recovery without fail-open.
/// Override with `RUSTMODLICA_SWITCH_ACCEPT_TOL` (default 1e-5).
pub fn switch_accept_residual_tol() -> f64 {
    static TOL: OnceLock<f64> = OnceLock::new();
    *TOL.get_or_init(|| {
        std::env::var("RUSTMODLICA_SWITCH_ACCEPT_TOL")
            .ok()
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|t| t.is_finite() && *t > 0.0)
            .unwrap_or(1e-5)
    })
}

/// Decision after ideal-switch force-s0 re-evaluation (event-iteration / trial-eval).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchForceS0Decision {
    /// status==0 or |r| <= accept tol.
    AcceptConverged,
    /// Fail-open enabled: continue with explicit degradation report.
    AcceptDegraded,
    /// Abort: residual too large and fail-open disabled (or non-finite).
    Reject,
}

/// Classify force-s0 outcome. Production default accepts only |r|<=tol; degraded
/// continuation requires `RUSTMODLICA_SWITCH_FAIL_OPEN=1`.
pub fn decide_switch_force_s0(status: i32, diag_residual: f64) -> SwitchForceS0Decision {
    if status == 0 {
        return SwitchForceS0Decision::AcceptConverged;
    }
    if strict_newton_enabled() {
        return SwitchForceS0Decision::Reject;
    }
    if diag_residual.is_finite() && diag_residual.abs() <= switch_accept_residual_tol() {
        return SwitchForceS0Decision::AcceptConverged;
    }
    if switch_fail_open_enabled() && diag_residual.is_finite() {
        return SwitchForceS0Decision::AcceptDegraded;
    }
    SwitchForceS0Decision::Reject
}

/// Explicit degradation report required whenever AcceptDegraded is taken.
pub fn report_switch_fail_open_degraded(stage: &str, time: f64, residual: f64, detail: &str) {
    eprintln!(
        "[degraded:switch-fail-open] stage={} time={:.6} residual={:.6e} {} (quality not guaranteed; disable RUSTMODLICA_SWITCH_FAIL_OPEN for strict |r|<tol)",
        stage, time, residual, detail
    );
}

/// Reject hint when force-s0 cannot continue under production policy.
pub fn report_switch_force_s0_rejected(stage: &str, time: f64, residual: f64, status: i32) {
    eprintln!(
        "[newton-switch-recover] {} force-s0 rejected at t={:.6} residual={:.6e} status={} tol={:.3e} (set RUSTMODLICA_SWITCH_FAIL_OPEN=1 to continue degraded)",
        stage,
        time,
        residual,
        status,
        switch_accept_residual_tol()
    );
}

pub fn allow_zero_residual_newton(status: i32, diag_residual: f64) -> bool {
    if strict_newton_enabled() {
        return false;
    }
    status == 2 && diag_residual.is_finite() && diag_residual.abs() <= 1e-5
}

pub fn allow_algebraic_newton_fallback(status: i32, state_len: usize) -> bool {
    if strict_newton_enabled() {
        return false;
    }
    status == 2 && state_len == 0
}

/// Ideal semiconductor / switch complementarity variable `s` (MSL IdealDiode etc.).
pub fn is_ideal_switch_complementarity_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let is_s = n.ends_with("_s") || n.ends_with(".s") || n == "s";
    if !is_s {
        return false;
    }
    n.contains("diode")
        || n.contains("transistor")
        || n.contains("thyristor")
        || n.contains("gto")
        || n.contains("idealopening")
        || n.contains("idealclosing")
        || n.contains("idealswitch")
        || n.contains("idealcommutingswitch")
        || n.contains("_switch_")
        || n.contains("idealintermediate")
}
