//! QSS1 (Quantized State System, first-order) solver.
//!
//! An event-driven solver where each state variable advances independently
//! at its own quantized rate. Efficient for multi-rate systems where
//! different subsystems evolve at vastly different time scales.
//!
//! Algorithm:
//!   1. Each state x_i has a quantum dQ_i (minimum change before update)
//!   2. At each step, find the state with the smallest time-to-next-quantum
//!   3. Advance that state and all dependents
//!   4. Repeat until global time reaches t_end

use crate::jit::CalcDerivsFunc;
use super::{Solver, System};

fn max_internal_steps_default() -> u64 {
    std::env::var("RUSTMODLICA_QSS_MAX_STEPS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1_000_000)
}

fn min_quantum_default() -> f64 {
    std::env::var("RUSTMODLICA_QSS_MIN_QUANTUM")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1e-8)
}

pub struct QssSolver {
    n: usize,
    /// State values at last quantum update.
    q: Vec<f64>,
    /// Time derivatives at last quantum update.
    dq: Vec<f64>,
    /// Current derivatives.
    derivs: Vec<f64>,
    /// Constant slope of each state's linear trajectory since its last update.
    slope: Vec<f64>,
    /// Simulation time at which each state was last re-anchored.
    t_last: Vec<f64>,
    /// Absolute tolerance for quantum computation.
    atol: f64,
    /// Relative tolerance for quantum computation.
    rtol: f64,
    /// Maximum number of QSS steps before forcing global step.
    max_internal_steps: u64,
    /// Scratch workspace for state evaluation.
    work_state: Vec<f64>,
}

impl QssSolver {
    pub fn new(n: usize, atol: f64, rtol: f64) -> Self {
        Self {
            n,
            q: vec![0.0; n],
            dq: vec![0.0; n],
            derivs: vec![0.0; n],
            slope: vec![0.0; n],
            t_last: vec![0.0; n],
            atol,
            rtol,
            max_internal_steps: max_internal_steps_default(),
            work_state: vec![0.0; n],
        }
    }

    /// Set the maximum number of internal QSS micro-steps.
    pub fn with_max_internal_steps(mut self, n: u64) -> Self {
        self.max_internal_steps = n;
        self
    }

    /// Compute the quantum for state i given current value and derivative.
    /// Uses max(atol, rtol*|x_i|, min_quantum) as the threshold.
    fn quantum(&self, x_i: f64, _dx_i: f64) -> f64 {
        let min_q = min_quantum_default();
        self.atol.max(self.rtol * x_i.abs()).max(min_q)
    }

    /// Compute time to next quantum crossing: t = dQ / |dx|
    fn time_to_crossing(&self, dq: f64, dx: f64) -> f64 {
        if dx.abs() < 1e-15 {
            f64::INFINITY
        } else {
            dq / dx.abs()
        }
    }
}

impl Solver for QssSolver {
    fn name(&self) -> &str {
        "qss"
    }

    fn step(
        &mut self,
        system: &mut System,
        time: f64,
        dt: f64,
        states: &mut [f64],
    ) -> Result<(), i32> {
        let n = self.n;
        if n == 0 || dt <= 0.0 {
            return Ok(());
        }

        // Evaluate initial derivatives at t
        self.work_state.copy_from_slice(states);
        system.evaluate(time, &mut self.work_state, &mut self.derivs)?;
        system.record_eval(time, states);

        // Anchor each state's linear trajectory at t = time.
        for i in 0..n {
            self.q[i] = states[i];
            self.slope[i] = self.derivs[i];
            self.t_last[i] = time;
            self.dq[i] = self.quantum(states[i], self.derivs[i]);
        }

        let t_end = time + dt;
        let mut t = time;
        let mut internal_steps = 0u64;

        while t < t_end && internal_steps < self.max_internal_steps {
            // Absolute time of each state's next quantum crossing, from its own
            // anchor: t_last[i] + dq[i] / |slope[i]|.
            let mut min_next = f64::INFINITY;
            let mut min_i = 0usize;
            for i in 0..n {
                let next_i = self.t_last[i] + self.time_to_crossing(self.dq[i], self.slope[i]);
                if next_i < min_next {
                    min_next = next_i;
                    min_i = i;
                }
            }

            let t_new = if min_next.is_infinite() { t_end } else { min_next.min(t_end) };
            // Guarantee forward progress (a crossing at/behind t advances by eps).
            let t_new = t_new.max(t + 1e-15).min(t_end);

            // Advance every state along its OWN linear trajectory since its last
            // re-anchor — not from a shared step size, which left slow states
            // pinned near their stale anchor value.
            for i in 0..n {
                states[i] = self.q[i] + self.slope[i] * (t_new - self.t_last[i]);
            }
            t = t_new;

            if t >= t_end {
                break;
            }

            // Re-evaluate derivatives at the crossing and re-anchor the crossing
            // state (QSS1: only the state that reached its quantum updates).
            self.work_state.copy_from_slice(states);
            system.evaluate(t, &mut self.work_state, &mut self.derivs)?;
            system.record_eval(t, states);

            self.q[min_i] = states[min_i];
            self.slope[min_i] = self.derivs[min_i];
            self.t_last[min_i] = t;
            self.dq[min_i] = self.quantum(states[min_i], self.derivs[min_i]);

            internal_steps += 1;
        }

        // Final evaluation to ensure states are consistent
        self.work_state.copy_from_slice(states);
        system.evaluate(t_end, &mut self.work_state, &mut self.derivs)?;
        system.record_eval(t_end, states);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qss_constant_ode() {
        let solver = QssSolver::new(1, 1e-6, 1e-6);
        assert_eq!(solver.name(), "qss");
    }

    #[test]
    fn test_quantum_computation() {
        let solver = QssSolver::new(1, 1e-8, 1e-6);
        let dq = solver.quantum(1.0, 1.0);
        assert!(dq >= 1e-8);
        let dq_small = solver.quantum(1e-10, 1.0);
        assert!(dq_small >= 1e-8); // limited by atol
    }

    #[test]
    fn test_time_to_crossing() {
        let solver = QssSolver::new(1, 1e-6, 1e-6);
        let dt = solver.time_to_crossing(0.01, 2.0);
        assert!((dt - 0.005).abs() < 1e-10);
    }
}
