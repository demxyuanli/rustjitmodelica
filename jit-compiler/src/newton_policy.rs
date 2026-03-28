//! Shared Newton acceptance policy (solver + simulation) without solver/simulation crate cycles.

use std::sync::OnceLock;

/// When RUSTMODLICA_STRICT_NEWTON=1, do not accept Newton failures via zero-residual or algebraic fallbacks.
pub(crate) fn strict_newton_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("RUSTMODLICA_STRICT_NEWTON")
            .ok()
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
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
