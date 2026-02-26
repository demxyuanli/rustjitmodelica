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
}

impl<'a> System<'a> {
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
            );
            if status != 0 {
                return Err(status);
            }
        }
        Ok(())
    }

    /// Evaluate with temporary buffers to avoid side effects on event indicators
    pub fn evaluate_scratch(
        &mut self,
        time: f64,
        states: &mut [f64],
        derivs: &mut [f64],
    ) -> Result<(), i32> {
        // Create scratch buffers for event outputs
        let mut scratch_outputs = vec![0.0; self.outputs.len()];
        let mut scratch_when = vec![0.0; self.when_states.len()];
        let mut scratch_crossings = vec![0.0; self.crossings.len()];

        unsafe {
            let status = (self.calc_derivs)(
                time,
                states.as_mut_ptr(),
                self.discrete.as_mut_ptr(), // Discrete vars are input (constant during step)
                derivs.as_mut_ptr(),
                self.params.as_ptr(),
                scratch_outputs.as_mut_ptr(),
                scratch_when.as_mut_ptr(),
                scratch_crossings.as_mut_ptr(),
                self.pre_states.as_ptr(),
                self.pre_discrete.as_ptr(),
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
        // 1. Evaluate f(t, y)
        // For Euler, we use the main evaluate because we want the outputs/events at t
        // But usually integration step is separate from event detection.
        // The simulation loop handles event detection at `time`.
        // The integration step moves from `time` to `time + dt`.
        // So we evaluate at `time`.
        system.evaluate(time, states, &mut self.derivs)?;

        // 2. y_{n+1} = y_n + h * f(t, y_n)
        for i in 0..states.len() {
            states[i] += self.derivs[i] * dt;
        }

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

        // k1 = f(t, y)
        system.evaluate_scratch(time, states, &mut self.k1)?;

        // k2 = f(t + h/2, y + h*k1/2)
        for i in 0..n {
            self.tmp_states[i] = states[i] + 0.5 * dt * self.k1[i];
        }
        system.evaluate_scratch(time + 0.5 * dt, &mut self.tmp_states, &mut self.k2)?;

        // k3 = f(t + h/2, y + h*k2/2)
        for i in 0..n {
            self.tmp_states[i] = states[i] + 0.5 * dt * self.k2[i];
        }
        system.evaluate_scratch(time + 0.5 * dt, &mut self.tmp_states, &mut self.k3)?;

        // k4 = f(t + h, y + h*k3)
        for i in 0..n {
            self.tmp_states[i] = states[i] + dt * self.k3[i];
        }
        system.evaluate_scratch(time + dt, &mut self.tmp_states, &mut self.k4)?;

        // y_{n+1} = y_n + h/6 * (k1 + 2k2 + 2k3 + k4)
        for i in 0..n {
            states[i] += (dt / 6.0) * (self.k1[i] + 2.0 * self.k2[i] + 2.0 * self.k3[i] + self.k4[i]);
        }

        Ok(())
    }
}
