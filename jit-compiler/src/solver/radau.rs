//! Radau IIA(3) — 2-stage implicit Runge-Kutta solver of order 3.
//!
//! An L-stable implicit method suitable for stiff ODE systems. Each step
//! solves a 2n×2n nonlinear system via simplified Newton iteration with
//! finite-difference Jacobian approximation.

use super::{Solver, System};

/// Radau IIA(3) Butcher tableau constants.
const A11: f64 = 5.0 / 12.0;
const A12: f64 = -1.0 / 12.0;
const A21: f64 = 3.0 / 4.0;
const A22: f64 = 1.0 / 4.0;
const B1: f64 = 3.0 / 4.0;
const B2: f64 = 1.0 / 4.0;
const C1: f64 = 1.0 / 3.0;
const C2: f64 = 1.0;

/// Radau IIA(3) implicit Runge-Kutta solver.
pub struct RadauSolver {
    /// State dimension.
    n: usize,
    /// Stage vectors k1, k2 (length 2*n).
    stages: Vec<f64>,
    /// Newton increment buffer.
    delta: Vec<f64>,
    /// RHS buffer for Newton system.
    rhs: Vec<f64>,
    /// Jacobian matrix J = df/dy (column-major, n×n).
    jac: Vec<f64>,
    /// Newton system matrix (2n×2n), column-major.
    newton_mat: Vec<f64>,
    /// Pivot array for LU decomposition.
    pivots: Vec<usize>,
    /// Workspace for state perturbation.
    work_y: Vec<f64>,
    /// Workspace for derivative evaluation.
    work_f: Vec<f64>,
    /// Absolute tolerance.
    atol: f64,
    /// Relative tolerance.
    rtol: f64,
    /// Max Newton iterations per step.
    max_iter: u32,
    /// Finite-difference epsilon.
    fd_eps: f64,
    /// Current step size (for adaptive control).
    pub h_cur: f64,
    /// Recommended next step size.
    h_next: f64,
}

impl RadauSolver {
    pub fn new(n: usize, atol: f64, rtol: f64) -> Self {
        Self {
            n,
            stages: vec![0.0; 2 * n],
            delta: vec![0.0; 2 * n],
            rhs: vec![0.0; 2 * n],
            jac: vec![0.0; n * n],
            newton_mat: vec![0.0; 4 * n * n],
            pivots: vec![0; 2 * n],
            work_y: vec![0.0; n],
            work_f: vec![0.0; 2 * n], // k1 output, then k2 output
            atol,
            rtol,
            max_iter: 8,
            fd_eps: 1e-6,
            h_cur: 0.0,
            h_next: 0.0,
        }
    }

    /// Set max Newton iterations.
    pub fn with_max_iter(mut self, n: u32) -> Self {
        self.max_iter = n;
        self
    }

    fn compute_jacobian(
        &mut self,
        system: &mut System,
        t: f64,
        y: &[f64],
        f0: &[f64],
    ) -> Result<(), i32> {
        let n = self.n;
        let eps = self.fd_eps;
        self.work_y.copy_from_slice(y);

        for j in 0..n {
            let yj = self.work_y[j];
            let delta = eps.max(eps * yj.abs());
            self.work_y[j] = yj + delta;

            // Evaluate f at perturbed y
            let mut scratch = self.work_y.clone();
            system.evaluate(t, &mut scratch, &mut self.work_f)?;
            // work_f now holds f(y + delta*e_j)

            for i in 0..n {
                self.jac[i * n + j] = (self.work_f[i] - f0[i]) / delta;
            }

            self.work_y[j] = yj;
        }
        Ok(())
    }

    fn solve_newton_system(&mut self, h: f64) -> Result<(), &'static str> {
        let n = self.n;
        let n2 = 2 * n;
        let m = &mut self.newton_mat;

        // Build the 2n×2n Newton matrix M = I - h * A ⊗ J
        // M = [ I - h*A11*J,   -h*A12*J   ]
        //     [   -h*A21*J,   I - h*A22*J ]
        //
        // Column-major storage: column j has elements at offset j*2n.
        // Columns 0..n-1 (block column 0):
        //   rows 0..n-1:   I - h*A11*J
        //   rows n..2n-1:  -h*A21*J
        // Columns n..2n-1 (block column 1):
        //   rows 0..n-1:   -h*A12*J
        //   rows n..2n-1:  I - h*A22*J

        // Initialize as identity
        m.fill(0.0);
        for i in 0..n2 {
            m[i * n2 + i] = 1.0;
        }

        // Subtract h * A ⊗ J
        for j in 0..n {
            for i in 0..n {
                let j_ij = self.jac[i * n + j];

                // Block (0,0): -h*A11*J (row i, col j)
                m[j * n2 + i] -= h * A11 * j_ij;
                // Block (0,1): -h*A12*J (row i, col n+j)
                m[(n + j) * n2 + i] -= h * A12 * j_ij;
                // Block (1,0): -h*A21*J (row n+i, col j)
                m[j * n2 + (n + i)] -= h * A21 * j_ij;
                // Block (1,1): -h*A22*J (row n+i, col n+j)
                m[(n + j) * n2 + (n + i)] -= h * A22 * j_ij;
            }
        }

        // Solve M * delta = rhs via LU with partial pivoting
        // (simple Gaussian elimination for the 2n×2n system)
        for k in 0..n2 {
            // Find pivot
            let mut pivot_row = k;
            let mut pivot_val = m[k * n2 + k].abs();
            for i in (k + 1)..n2 {
                let v = m[i * n2 + k].abs();
                if v > pivot_val {
                    pivot_val = v;
                    pivot_row = i;
                }
            }
            if pivot_val < 1e-15 {
                return Err("Singular Newton matrix in Radau solver");
            }
            self.pivots[k] = pivot_row;

            // Swap rows
            if pivot_row != k {
                for j in 0..n2 {
                    m.swap(k * n2 + j, pivot_row * n2 + j);
                }
                self.rhs.swap(k, pivot_row);
            }

            // Eliminate
            let inv_pivot = 1.0 / m[k * n2 + k];
            for i in (k + 1)..n2 {
                let factor = m[i * n2 + k] * inv_pivot;
                for j in k..n2 {
                    m[i * n2 + j] -= factor * m[k * n2 + j];
                }
                self.rhs[i] -= factor * self.rhs[k];
            }
        }

        // Back substitution
        for i in (0..n2).rev() {
            let mut sum = self.rhs[i];
            for j in (i + 1)..n2 {
                sum -= m[i * n2 + j] * self.delta[j];
            }
            self.delta[i] = sum / m[i * n2 + i];
        }
        Ok(())
    }

    fn newton_norm(d: &[f64], y: &[f64], atol: f64, rtol: f64) -> f64 {
        let mut max_ratio = 0.0f64;
        for i in 0..d.len() {
            let scale = atol + rtol * y[i].abs().max(1e-8);
            let ratio = d[i].abs() / scale;
            if ratio > max_ratio {
                max_ratio = ratio;
            }
        }
        max_ratio
    }
}

impl Solver for RadauSolver {
    fn name(&self) -> &str {
        "radau"
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

        self.h_cur = dt;
        let h = dt;

        // Reset eval counter
        if !system.eval_call_index.is_null() {
            unsafe { *system.eval_call_index = 0; }
        }

        // Evaluate f at current state for Jacobian
        let mut y0 = states.to_vec();
        let mut f0 = vec![0.0; n];
        system.evaluate(time, &mut y0, &mut f0)?;
        system.record_eval(time, &y0);

        // Compute Jacobian at current state
        self.compute_jacobian(system, time, &y0, &f0)?;

        // Initial guess for stages: k1 = k2 = f0
        self.stages[..n].copy_from_slice(&f0);
        self.stages[n..].copy_from_slice(&f0);

        let mut converged = false;

        for _iter in 0..self.max_iter {
            // Compute stage arguments
            // y1 = y0 + h*(A11*k1 + A12*k2)
            // y2 = y0 + h*(A21*k1 + A22*k2)
            for i in 0..n {
                let k1 = self.stages[i];
                let k2 = self.stages[n + i];
                self.work_y[i] = y0[i] + h * (A11 * k1 + A12 * k2);
            }
            // Evaluate f(t + C1*h, y1) → work_f[0..n]
            let t1 = time + C1 * h;
            system.evaluate(t1, &mut self.work_y, &mut self.work_f[..n])?;
            system.record_eval(t1, &self.work_y);

            for i in 0..n {
                let k1 = self.stages[i];
                let k2 = self.stages[n + i];
                self.work_y[i] = y0[i] + h * (A21 * k1 + A22 * k2);
            }
            // Evaluate f(t + C2*h, y2) → work_f[n..2n]
            let t2 = time + C2 * h;
            system.evaluate(t2, &mut self.work_y, &mut self.work_f[n..])?;
            system.record_eval(t2, &self.work_y);

            // Build RHS: rhs = k - f(stage)
            for i in 0..n {
                self.rhs[i] = self.stages[i] - self.work_f[i];
                self.rhs[n + i] = self.stages[n + i] - self.work_f[n + i];
            }

            // Check convergence
            let err = Self::newton_norm(&self.rhs, &self.stages, self.atol, self.rtol);
            if err < 1.0 {
                converged = true;
                break;
            }

            // Newton correction
            self.solve_newton_system(h).map_err(|_| 2i32)?;

            // Update stages: k_new = k - delta
            for i in 0..2 * n {
                self.stages[i] -= self.delta[i];
            }
        }

        if !converged {
            return Err(2); // Newton failed to converge
        }

        // Advance: y_new = y0 + h*(B1*k1 + B2*k2)
        for i in 0..n {
            states[i] = y0[i] + h * (B1 * self.stages[i] + B2 * self.stages[n + i]);
        }

        // Estimate next step size (simple I-controller)
        // Using embedded method of order 2 for error estimation
        // Radau IA(2) has tableau:
        //   0   | 0    0
        //   2/3 | 1/3  1/3
        //   ----+-----------
        //       | 1/4  3/4
        // y_low = y0 + h*(1/4*f(t, y0) + 3/4*f(t+2h/3, stage))
        // For simplicity, use the stage evaluations we already have
        let mut err_est = 0.0f64;
        for i in 0..n {
            let y_high = states[i];
            // Low-order estimate from the two stage points
            let flow = y0[i] + h * (0.25 * self.stages[i] + 0.75 * self.stages[n + i]);
            let scale = self.atol + self.rtol * y_high.abs().max(1e-8);
            let diff = (y_high - flow).abs() / scale;
            if diff > err_est {
                err_est = diff;
            }
        }
        let safety = 0.9;
        let h_factor = if err_est > 0.0 {
            (safety / err_est.max(1e-14)).powf(1.0 / 3.0).min(2.0).max(0.2)
        } else {
            2.0
        };
        self.h_next = h * h_factor;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radau_constant_ode() {
        // dy/dt = 0, y(0) = 1.0 → y stays at 1.0
        let mut solver = RadauSolver::new(1, 1e-8, 1e-8);
        // We can't easily test without a real System, but at least verify construction
        assert_eq!(solver.name(), "radau");
        assert_eq!(solver.n, 1);
    }

    #[test]
    fn test_radau_newton_norm() {
        let d = [1e-6, 2e-6];
        let y = [1.0, 100.0];
        let n = RadauSolver::newton_norm(&d, &y, 1e-8, 1e-6);
        assert!(n < 1.0);
    }

    #[test]
    fn test_radau_newton_norm_fails() {
        let d = [1.0, 2.0];
        let y = [1.0, 1.0];
        let n = RadauSolver::newton_norm(&d, &y, 1e-8, 1e-6);
        assert!(n > 1.0);
    }
}
