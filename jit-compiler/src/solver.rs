use crate::jit::CalcDerivsFunc;

/// Wrapper for the system dynamics function
pub struct System<'a> {
    pub calc_derivs: CalcDerivsFunc,
    pub params: &'a [f64],
    pub discrete: &'a mut [f64],
    pub outputs: &'a mut [f64],
    pub when_states: &'a mut [f64],
    pub crossings: &'a mut [f64],
    pub pre_states: &'a [f64],
    pub pre_discrete: &'a [f64],
    pub t_end: f64,
    pub diag_residual: *mut f64,
    pub diag_x: *mut f64,
    /// Step-internal diag: write current eval call index before each evaluate (solver increments).
    pub eval_call_index: *mut u32,
    /// Time of the evaluate call (solver sets before each call).
    pub last_eval_time: *mut f64,
    /// State vector at the evaluate call (solver copies before each call).
    pub last_eval_state: *mut f64,
    pub last_eval_state_len: usize,
    /// When set, evaluate_scratch uses this as outputs buffer (initial guess from caller; JIT overwrites with solution).
    /// Enables using previous step/stage algebraic values as Newton initial guess across a step.
    pub scratch_outputs: Option<&'a mut [f64]>,
    /// Pointer to homotopy lambda parameter; passed through to calc_derivs.
    pub homotopy_lambda_ptr: *const f64,
    /// Pre-allocated scratch buffers reused across evaluate_scratch calls.
    pub buf_discrete: Vec<f64>,
    pub buf_when: Vec<f64>,
    pub buf_crossings: Vec<f64>,
    pub buf_outputs: Vec<f64>,
}

impl<'a> System<'a> {
    /// Call before each evaluate inside a step so that on status 2 we know which call, time, and state.
    pub fn record_eval(&mut self, time: f64, state: &[f64]) {
        if !self.eval_call_index.is_null() {
            unsafe {
                *self.eval_call_index += 1;
            }
        }
        if !self.last_eval_time.is_null() {
            unsafe {
                *self.last_eval_time = time;
            }
        }
        if !self.last_eval_state.is_null() && self.last_eval_state_len >= state.len() {
            unsafe {
                std::ptr::copy_nonoverlapping(state.as_ptr(), self.last_eval_state, state.len());
            }
        }
    }
}

impl<'a> System<'a> {
    /// Compute ODE Jacobian J by finite differences: J[i][j] = d(deriv_i)/d(state_j).
    /// jacobian must be row-major, length n*n. Uses one-sided difference with eps.
    #[allow(dead_code)]
    pub fn compute_ode_jacobian_numeric(
        &mut self,
        time: f64,
        states: &[f64],
        jacobian: &mut [f64],
        eps: f64,
    ) -> Result<(), i32> {
        let n = states.len();
        if jacobian.len() < n * n {
            return Err(-1);
        }
        let mut derivs_base = vec![0.0_f64; n];
        let mut derivs_pert = vec![0.0_f64; n];
        let mut states_scratch = states.to_vec();
        self.evaluate(time, &mut states_scratch, &mut derivs_base)?;
        for j in 0..n {
            states_scratch.copy_from_slice(states);
            states_scratch[j] += eps;
            self.evaluate(time, &mut states_scratch, &mut derivs_pert)?;
            for i in 0..n {
                jacobian[i * n + j] = (derivs_pert[i] - derivs_base[i]) / eps;
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn evaluate(
        &mut self,
        time: f64,
        states: &mut [f64],
        derivs: &mut [f64],
    ) -> Result<(), i32> {
        unsafe {
            let status = (self.calc_derivs)(
                time,
                states.as_mut_ptr(),
                self.discrete.as_mut_ptr(),
                derivs.as_mut_ptr(),
                self.params.as_ptr(),
                self.outputs.as_mut_ptr(),
                self.when_states.as_mut_ptr(),
                self.crossings.as_mut_ptr(),
                self.pre_states.as_ptr(),
                self.pre_discrete.as_ptr(),
                self.t_end,
                self.diag_residual,
                self.diag_x,
                self.homotopy_lambda_ptr,
            );
            if status != 0 {
                return Err(status);
            }
        }
        Ok(())
    }

    /// Evaluate with temporary buffers to avoid side effects on event indicators.
    /// When scratch_outputs is Some, uses it as outputs buffer (caller must fill with prev step/stage values for Newton init).
    pub fn evaluate_scratch(
        &mut self,
        time: f64,
        states: &mut [f64],
        derivs: &mut [f64],
    ) -> Result<(), i32> {
        self.buf_discrete.resize(self.discrete.len(), 0.0);
        self.buf_discrete.copy_from_slice(self.discrete);
        self.buf_when.resize(self.when_states.len(), 0.0);
        self.buf_when.fill(0.0);
        self.buf_crossings.resize(self.crossings.len(), 0.0);
        self.buf_crossings.fill(0.0);
        self.buf_outputs.resize(self.outputs.len(), 0.0);
        self.buf_outputs.fill(0.0);
        let mut last_status = 0_i32;

        if let Some(scratch) = self.scratch_outputs.as_mut() {
            let base_guess = scratch.to_vec();
            // Newton init fallback chain at t=0/stiff algebraic loops:
            // keep guess -> damped guess -> zero guess.
            let scales = [1.0_f64, 0.5_f64, 0.0_f64];
            for scale in scales {
                if !self.eval_call_index.is_null() {
                    unsafe {
                        *self.eval_call_index += 1;
                    }
                }
                for (dst, src) in scratch.iter_mut().zip(base_guess.iter()) {
                    *dst = *src * scale;
                }
                self.buf_discrete.copy_from_slice(self.discrete);
                self.buf_when.fill(0.0);
                self.buf_crossings.fill(0.0);
                unsafe {
                    let status = (self.calc_derivs)(
                        time,
                        states.as_mut_ptr(),
                        self.buf_discrete.as_mut_ptr(),
                        derivs.as_mut_ptr(),
                        self.params.as_ptr(),
                        scratch.as_mut_ptr(),
                        self.buf_when.as_mut_ptr(),
                        self.buf_crossings.as_mut_ptr(),
                        self.pre_states.as_ptr(),
                        self.pre_discrete.as_ptr(),
                        self.t_end,
                        self.diag_residual,
                        self.diag_x,
                        self.homotopy_lambda_ptr,
                    );
                    if status == 0 {
                        return Ok(());
                    }
                    last_status = status;
                    if status != 2 {
                        return Err(status);
                    }
                }
            }
            return Err(last_status);
        }

        unsafe {
            let status = (self.calc_derivs)(
                time,
                states.as_mut_ptr(),
                self.buf_discrete.as_mut_ptr(),
                derivs.as_mut_ptr(),
                self.params.as_ptr(),
                self.buf_outputs.as_mut_ptr(),
                self.buf_when.as_mut_ptr(),
                self.buf_crossings.as_mut_ptr(),
                self.pre_states.as_ptr(),
                self.pre_discrete.as_ptr(),
                self.t_end,
                self.diag_residual,
                self.diag_x,
                self.homotopy_lambda_ptr,
            );
            if status != 0 {
                return Err(status);
            }
        }
        Ok(())
    }
}

pub trait Solver {
    fn step(
        &mut self,
        system: &mut System,
        time: f64,
        dt: f64,
        states: &mut [f64],
    ) -> Result<(), i32>;

    #[allow(dead_code)]
    fn name(&self) -> &str;
}

#[allow(dead_code)]
pub struct EulerSolver {
    derivs: Vec<f64>,
}

impl EulerSolver {
    #[allow(dead_code)]
    pub fn new(state_len: usize) -> Self {
        Self {
            derivs: vec![0.0; state_len],
        }
    }
}

impl Solver for EulerSolver {
    fn name(&self) -> &str {
        "Euler"
    }

    fn step(
        &mut self,
        system: &mut System,
        time: f64,
        dt: f64,
        states: &mut [f64],
    ) -> Result<(), i32> {
        if states.is_empty() {
            return Ok(());
        }
        if !system.eval_call_index.is_null() {
            unsafe {
                *system.eval_call_index = 0;
            }
        }
        system.record_eval(time, states);
        system.evaluate(time, states, &mut self.derivs)?;

        // 2. y_{n+1} = y_n + h * f(t, y_n)
        for i in 0..states.len() {
            states[i] += self.derivs[i] * dt;
        }

        Ok(())
    }
}

/// RT1-2: Backward Euler (implicit) for stiff systems. Fixed-point iteration: y^{k+1} = y_n + dt*f(t+dt, y^k).
pub struct BackwardEulerSolver {
    derivs: Vec<f64>,
    tmp: Vec<f64>,
    max_iter: usize,
    tol: f64,
}

impl BackwardEulerSolver {
    pub fn new(state_len: usize) -> Self {
        Self {
            derivs: vec![0.0; state_len],
            tmp: vec![0.0; state_len],
            max_iter: 20,
            tol: 1e-10,
        }
    }
}

impl Solver for BackwardEulerSolver {
    fn name(&self) -> &str {
        "BackwardEuler"
    }

    fn step(
        &mut self,
        system: &mut System,
        time: f64,
        dt: f64,
        states: &mut [f64],
    ) -> Result<(), i32> {
        let n = states.len();
        if n == 0 {
            return Ok(());
        }
        if !system.eval_call_index.is_null() {
            unsafe {
                *system.eval_call_index = 0;
            }
        }
        let y_n: Vec<f64> = states.to_vec();
        self.tmp.copy_from_slice(&y_n);
        for _ in 0..self.max_iter {
            system.record_eval(time + dt, &self.tmp);
            system.evaluate(time + dt, &mut self.tmp, &mut self.derivs)?;
            let mut max_change = 0.0_f64;
            for i in 0..n {
                let y_new = y_n[i] + dt * self.derivs[i];
                let change = (y_new - self.tmp[i]).abs();
                if change > max_change {
                    max_change = change;
                }
                self.tmp[i] = y_new;
            }
            if max_change <= self.tol {
                states.copy_from_slice(&self.tmp);
                return Ok(());
            }
        }
        states.copy_from_slice(&self.tmp);
        Ok(())
    }
}

pub struct RungeKutta4Solver {
    k1: Vec<f64>,
    k2: Vec<f64>,
    k3: Vec<f64>,
    k4: Vec<f64>,
    tmp_states: Vec<f64>,
}

impl RungeKutta4Solver {
    pub fn new(state_len: usize) -> Self {
        Self {
            k1: vec![0.0; state_len],
            k2: vec![0.0; state_len],
            k3: vec![0.0; state_len],
            k4: vec![0.0; state_len],
            tmp_states: vec![0.0; state_len],
        }
    }
}

impl Solver for RungeKutta4Solver {
    fn name(&self) -> &str {
        "RK4"
    }

    fn step(
        &mut self,
        system: &mut System,
        time: f64,
        dt: f64,
        states: &mut [f64],
    ) -> Result<(), i32> {
        let n = states.len();
        if n == 0 {
            return Ok(());
        }
        if !system.eval_call_index.is_null() {
            unsafe {
                *system.eval_call_index = 0;
            }
        }

        system.record_eval(time, states);
        system.evaluate_scratch(time, states, &mut self.k1)?;

        for i in 0..n {
            self.tmp_states[i] = states[i] + 0.5 * dt * self.k1[i];
        }
        system.record_eval(time + 0.5 * dt, &self.tmp_states);
        system.evaluate_scratch(time + 0.5 * dt, &mut self.tmp_states, &mut self.k2)?;

        for i in 0..n {
            self.tmp_states[i] = states[i] + 0.5 * dt * self.k2[i];
        }
        system.record_eval(time + 0.5 * dt, &self.tmp_states);
        system.evaluate_scratch(time + 0.5 * dt, &mut self.tmp_states, &mut self.k3)?;

        for i in 0..n {
            self.tmp_states[i] = states[i] + dt * self.k3[i];
        }
        system.record_eval(time + dt, &self.tmp_states);
        system.evaluate_scratch(time + dt, &mut self.tmp_states, &mut self.k4)?;

        // y_{n+1} = y_n + h/6 * (k1 + 2k2 + 2k3 + k4)
        for i in 0..n {
            states[i] +=
                (dt / 6.0) * (self.k1[i] + 2.0 * self.k2[i] + 2.0 * self.k3[i] + self.k4[i]);
        }

        Ok(())
    }
}

pub struct AdaptiveRK45Solver {
    k1: Vec<f64>,
    k2: Vec<f64>,
    k3: Vec<f64>,
    k4: Vec<f64>,
    k5: Vec<f64>,
    k6: Vec<f64>,
    tmp: Vec<f64>,
    y4: Vec<f64>,
    y5: Vec<f64>,
    abs_tol: f64,
    rel_tol: f64,
}

impl AdaptiveRK45Solver {
    pub fn new(state_len: usize, abs_tol: f64, rel_tol: f64) -> Self {
        Self {
            k1: vec![0.0; state_len],
            k2: vec![0.0; state_len],
            k3: vec![0.0; state_len],
            k4: vec![0.0; state_len],
            k5: vec![0.0; state_len],
            k6: vec![0.0; state_len],
            tmp: vec![0.0; state_len],
            y4: vec![0.0; state_len],
            y5: vec![0.0; state_len],
            abs_tol,
            rel_tol,
        }
    }
}

impl Solver for AdaptiveRK45Solver {
    fn name(&self) -> &str {
        "AdaptiveRK45"
    }

    fn step(
        &mut self,
        system: &mut System,
        time: f64,
        mut dt: f64,
        states: &mut [f64],
    ) -> Result<(), i32> {
        // Single adaptive step attempt: adjust dt internally but caller passes dt as suggestion.
        let n = states.len();
        if n == 0 {
            return Ok(());
        }

        let t = time;
        loop {
            if !system.eval_call_index.is_null() {
                unsafe {
                    *system.eval_call_index = 0;
                }
            }
            let y = states.to_vec();

            self.tmp.copy_from_slice(&y);
            system.record_eval(t, &self.tmp);
            system.evaluate_scratch(t, &mut self.tmp, &mut self.k1)?;

            for i in 0..n {
                self.tmp[i] = y[i] + dt * (1.0 / 5.0) * self.k1[i];
            }
            system.record_eval(t + dt * (1.0 / 5.0), &self.tmp);
            system.evaluate_scratch(t + dt * (1.0 / 5.0), &mut self.tmp, &mut self.k2)?;

            for i in 0..n {
                self.tmp[i] = y[i] + dt * (3.0 / 40.0 * self.k1[i] + 9.0 / 40.0 * self.k2[i]);
            }
            system.record_eval(t + dt * (3.0 / 10.0), &self.tmp);
            system.evaluate_scratch(t + dt * (3.0 / 10.0), &mut self.tmp, &mut self.k3)?;

            for i in 0..n {
                self.tmp[i] = y[i]
                    + dt * (44.0 / 45.0 * self.k1[i] - 56.0 / 15.0 * self.k2[i]
                        + 32.0 / 9.0 * self.k3[i]);
            }
            system.record_eval(t + dt * (4.0 / 5.0), &self.tmp);
            system.evaluate_scratch(t + dt * (4.0 / 5.0), &mut self.tmp, &mut self.k4)?;

            for i in 0..n {
                self.tmp[i] = y[i]
                    + dt * (19372.0 / 6561.0 * self.k1[i] - 25360.0 / 2187.0 * self.k2[i]
                        + 64448.0 / 6561.0 * self.k3[i]
                        - 212.0 / 729.0 * self.k4[i]);
            }
            system.record_eval(t + dt * (8.0 / 9.0), &self.tmp);
            system.evaluate_scratch(t + dt * (8.0 / 9.0), &mut self.tmp, &mut self.k5)?;

            for i in 0..n {
                self.tmp[i] = y[i]
                    + dt * (9017.0 / 3168.0 * self.k1[i] - 355.0 / 33.0 * self.k2[i]
                        + 46732.0 / 5247.0 * self.k3[i]
                        + 49.0 / 176.0 * self.k4[i]
                        - 5103.0 / 18656.0 * self.k5[i]);
            }
            system.record_eval(t + dt, &self.tmp);
            system.evaluate_scratch(t + dt, &mut self.tmp, &mut self.k6)?;

            for i in 0..n {
                self.y5[i] = y[i]
                    + dt * (35.0 / 384.0 * self.k1[i]
                        + 500.0 / 1113.0 * self.k3[i]
                        + 125.0 / 192.0 * self.k4[i]
                        - 2187.0 / 6784.0 * self.k5[i]
                        + 11.0 / 84.0 * self.k6[i]);

                self.y4[i] = y[i]
                    + dt * (5179.0 / 57600.0 * self.k1[i]
                        + 7571.0 / 16695.0 * self.k3[i]
                        + 393.0 / 640.0 * self.k4[i]
                        - 92097.0 / 339200.0 * self.k5[i]
                        + 187.0 / 2100.0 * self.k6[i]
                        + 1.0 / 40.0 * self.k2[i]);
            }

            let mut err = 0.0;
            for i in 0..n {
                let sk = self.abs_tol + self.rel_tol * self.y5[i].abs();
                let e = ((self.y5[i] - self.y4[i]) / sk).abs();
                if e > err {
                    err = e;
                }
            }

            if err <= 1.0 {
                states.copy_from_slice(&self.y5);
                break;
            } else {
                // Reject and reduce dt
                let factor = (1.0 / (2.0 * err)).powf(0.25).min(0.5);
                dt *= factor.max(0.1);
            }
        }

        Ok(())
    }
}
