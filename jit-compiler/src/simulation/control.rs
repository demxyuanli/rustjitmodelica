//! Cooperative simulation control: cancel token + time-progress callbacks.
//!
//! This unlocks IDE/Studio P1 Stop and simulation-time progress. True pause/step
//! (hold the integrator mid-solve) is intentionally out of scope here (P3).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Snapshot of simulation time progress (not wall-clock elapsed).
#[derive(Debug, Clone, Copy)]
pub struct SimProgress {
    pub time: f64,
    pub t_end: f64,
    pub step: u64,
}

impl SimProgress {
    pub fn fraction(&self) -> f64 {
        if !self.t_end.is_finite() || self.t_end <= 0.0 {
            return 0.0;
        }
        (self.time / self.t_end).clamp(0.0, 1.0)
    }
}

/// Stable error string returned when a cooperative cancel is honored.
pub const SIM_CANCELLED_MSG: &str = "simulation cancelled";

pub fn is_sim_cancelled_error(msg: &str) -> bool {
    msg == SIM_CANCELLED_MSG || msg.contains(SIM_CANCELLED_MSG)
}

type ProgressCb = Box<dyn FnMut(&SimProgress) + Send>;

/// Optional cooperative control handle for [`crate::simulation::run_simulation`].
pub struct SimControl {
    cancel: Arc<AtomicBool>,
    on_progress: Option<ProgressCb>,
    /// Minimum simulation-time delta between progress callbacks (0 = every poll).
    progress_min_dt: f64,
    last_progress_time: f64,
    last_progress_step: u64,
}

impl Default for SimControl {
    fn default() -> Self {
        Self::new()
    }
}

impl SimControl {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            on_progress: None,
            progress_min_dt: 0.0,
            last_progress_time: f64::NEG_INFINITY,
            last_progress_step: u64::MAX,
        }
    }

    /// Shareable cancel flag (set from another thread / UI Stop button).
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Throttle progress callbacks by simulation time (seconds).
    pub fn with_progress_min_dt(mut self, dt: f64) -> Self {
        self.set_progress_min_dt(dt);
        self
    }

    pub fn set_progress_min_dt(&mut self, dt: f64) {
        self.progress_min_dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
    }

    pub fn on_progress<F>(mut self, f: F) -> Self
    where
        F: FnMut(&SimProgress) + Send + 'static,
    {
        self.on_progress = Some(Box::new(f));
        self
    }

    /// Poll cancel + emit progress. Returns Err(SIM_CANCELLED_MSG) when cancelled.
    pub(crate) fn poll_step(&mut self, time: f64, t_end: f64, step: u64) -> Result<(), String> {
        if self.is_cancelled() {
            return Err(SIM_CANCELLED_MSG.to_string());
        }
        if let Some(cb) = self.on_progress.as_mut() {
            let due_time = time + 1e-15 >= self.last_progress_time + self.progress_min_dt
                || self.last_progress_time.is_infinite();
            let due_step = step != self.last_progress_step;
            if due_time && due_step {
                let p = SimProgress { time, t_end, step };
                cb(&p);
                self.last_progress_time = time;
                self.last_progress_step = step;
            }
        }
        Ok(())
    }
}
