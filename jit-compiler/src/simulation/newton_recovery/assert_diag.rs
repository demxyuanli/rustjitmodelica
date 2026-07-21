use crate::jit::native;

fn assert_storm_limit() -> u64 {
    std::env::var("RUSTMODLICA_ASSERT_STORM_LIMIT")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(256)
}

pub fn fail_if_assert_storm(stage: &str, time: f64) -> Result<(), String> {
    let hits = native::assert_hit_count();
    if hits > assert_storm_limit() {
        return Err(format!(
            "Aborting due to assertion storm at stage={} time={:.6} assert_hits={}",
            stage, time, hits
        ));
    }
    Ok(())
}

pub fn print_newton_diag(
    phase: &str,
    eval_calls: u32,
    last_eval_time: f64,
    diag_residual: f64,
    diag_x: f64,
) {
    let assert_hits = native::assert_hit_count();
    eprintln!(
        "[newton-diag] phase={} eval_calls={} last_eval_time={:.6} diag_residual={:.6e} diag_x={:.6e} assert_hits={}",
        phase,
        eval_calls,
        last_eval_time,
        diag_residual,
        diag_x,
        assert_hits
    );
}
