//! Cooperative simulation control: cancel, pause/step, and progress callbacks.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

const LIVE_SERIES_CAP: usize = 12;
const LIVE_POINT_CAP: usize = 8000;

/// Incremental Scope samples since the last [`LiveSeriesHandle::take_delta`].
#[derive(Debug, Clone)]
pub struct LiveSeriesDelta {
    pub time: Vec<f64>,
    pub series: HashMap<String, Vec<f64>>,
}

struct LiveCol {
    name: String,
    kind: u8,
    index: usize,
    values: Vec<f64>,
}

struct LiveSeriesState {
    bound: bool,
    cols: Vec<LiveCol>,
    time: Vec<f64>,
    cursor: usize,
}

impl LiveSeriesState {
    fn bind(&mut self, state: &[String], disc: &[String], out: &[String]) {
        if self.bound {
            return;
        }
        self.bound = true;
        let mut cols = Vec::new();
        for (kind, names) in [(2_u8, out), (0, state), (1, disc)] {
            for (i, n) in names.iter().enumerate() {
                if cols.len() >= LIVE_SERIES_CAP {
                    break;
                }
                if cols.iter().any(|c: &LiveCol| c.name == *n) {
                    continue;
                }
                cols.push(LiveCol {
                    name: n.clone(),
                    kind,
                    index: i,
                    values: Vec::new(),
                });
            }
        }
        self.cols = cols;
    }

    fn push(&mut self, t: f64, st: &[f64], disc: &[f64], out: &[f64]) {
        if !self.bound {
            return;
        }
        self.time.push(t);
        for c in &mut self.cols {
            let v = match c.kind {
                0 => st.get(c.index).copied().unwrap_or(0.0),
                1 => disc.get(c.index).copied().unwrap_or(0.0),
                _ => out.get(c.index).copied().unwrap_or(0.0),
            };
            c.values.push(v);
        }
        if self.time.len() > LIVE_POINT_CAP {
            let drop_n = self.time.len() - LIVE_POINT_CAP;
            self.time.drain(..drop_n);
            for c in &mut self.cols {
                if c.values.len() >= drop_n {
                    c.values.drain(..drop_n);
                }
            }
            self.cursor = self.cursor.saturating_sub(drop_n);
        }
    }

    fn take_delta(&mut self) -> Option<LiveSeriesDelta> {
        if self.cursor >= self.time.len() {
            return None;
        }
        let time = self.time[self.cursor..].to_vec();
        let mut series = HashMap::new();
        for c in &self.cols {
            series.insert(c.name.clone(), c.values[self.cursor..].to_vec());
        }
        self.cursor = self.time.len();
        Some(LiveSeriesDelta { time, series })
    }
}

/// Shareable live Scope buffer (UI thread polls; integrator thread appends).
#[derive(Clone)]
pub struct LiveSeriesHandle {
    inner: Arc<Mutex<LiveSeriesState>>,
}

impl LiveSeriesHandle {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LiveSeriesState {
                bound: false,
                cols: Vec::new(),
                time: Vec::new(),
                cursor: 0,
            })),
        }
    }

    pub fn take_delta(&self) -> Option<LiveSeriesDelta> {
        self.inner.lock().ok().and_then(|mut g| g.take_delta())
    }

    fn bind(&self, state: &[String], disc: &[String], out: &[String]) {
        if let Ok(mut g) = self.inner.lock() {
            g.bind(state, disc, out);
        }
    }

    fn push(&self, t: f64, st: &[f64], disc: &[f64], out: &[f64]) {
        if let Ok(mut g) = self.inner.lock() {
            g.push(t, st, disc, out);
        }
    }
}

/// Optional cooperative control handle for [`crate::simulation::run_simulation`].
///
/// Pause holds the integrator at step boundaries (between `poll_step` calls).
/// Step clears pause for exactly one boundary then re-pauses.
pub struct SimControl {
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    step_once: Arc<AtomicBool>,
    on_progress: Option<ProgressCb>,
    /// Minimum simulation-time delta between progress callbacks (0 = every poll).
    progress_min_dt: f64,
    last_progress_time: f64,
    last_progress_step: u64,
    live: Option<LiveSeriesHandle>,
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
            pause: Arc::new(AtomicBool::new(false)),
            step_once: Arc::new(AtomicBool::new(false)),
            on_progress: None,
            progress_min_dt: 0.0,
            last_progress_time: f64::NEG_INFINITY,
            last_progress_step: u64::MAX,
            live: None,
        }
    }

    /// Enable incremental Scope samples. Clone the handle for the UI poller.
    pub fn enable_live_series(&mut self) -> LiveSeriesHandle {
        let handle = self.live.clone().unwrap_or_else(LiveSeriesHandle::new);
        self.live = Some(handle.clone());
        handle
    }

    /// Bind flattened variable names so live samples can be keyed.
    pub fn bind_live_names(&mut self, state: &[String], discrete: &[String], output: &[String]) {
        if let Some(live) = &self.live {
            live.bind(state, discrete, output);
        }
    }

    pub(crate) fn emit_sample(&mut self, time: f64, states: &[f64], discrete: &[f64], outputs: &[f64]) {
        if let Some(live) = &self.live {
            live.push(time, states, discrete, outputs);
        }
    }

    /// Shareable cancel flag (set from another thread / UI Stop button).
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    pub fn pause_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.pause)
    }

    pub fn step_once_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.step_once)
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        // Wake a paused loop.
        self.pause.store(false, Ordering::SeqCst);
        self.step_once.store(true, Ordering::SeqCst);
    }

    pub fn request_pause(&self) {
        self.pause.store(true, Ordering::SeqCst);
    }

    pub fn request_resume(&self) {
        self.step_once.store(false, Ordering::SeqCst);
        self.pause.store(false, Ordering::SeqCst);
    }

    /// While paused, allow exactly one more `poll_step` boundary then re-pause.
    pub fn request_step(&self) {
        self.step_once.store(true, Ordering::SeqCst);
        self.pause.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn is_paused(&self) -> bool {
        self.pause.load(Ordering::SeqCst)
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

    /// Poll cancel/pause + emit progress. Returns Err(SIM_CANCELLED_MSG) when cancelled.
    pub(crate) fn poll_step(&mut self, time: f64, t_end: f64, step: u64) -> Result<(), String> {
        loop {
            if self.is_cancelled() {
                return Err(SIM_CANCELLED_MSG.to_string());
            }
            if !self.pause.load(Ordering::SeqCst) {
                break;
            }
            // Paused: wait for resume, step-once, or cancel.
            if self.step_once.swap(false, Ordering::SeqCst) {
                // One boundary allowed; keep pause=true so the next poll waits again.
                break;
            }
            thread::sleep(Duration::from_millis(15));
        }
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
