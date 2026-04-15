//! Profile data structures for training runs (Phase 2 of Leyden-inspired compilation).
//!
//! Training runs observe model simulation behavior and record profiling data that
//! guides subsequent speculative AOT compilation. Analogous to Leyden's
//! `-XX:AOTMode=record` training runs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub const PROFILE_VERSION: u32 = 1;

/// Aggregated profile data collected during a training simulation run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelProfile {
    pub model_name: String,
    pub profile_version: u32,
    pub solver_branches: Vec<SolverBranchProfile>,
    pub zero_crossing_stats: Vec<ZeroCrossingProfile>,
    pub state_value_ranges: HashMap<String, ValueRange>,
    pub clock_activation_pattern: Vec<ClockActivationProfile>,
    /// Equation indices sorted by evaluation frequency (hottest first).
    pub hot_equations: Vec<usize>,
    pub newton_iteration_stats: NewtonStats,
    /// Total simulation steps observed.
    pub total_steps: u64,
    /// Total wall-clock time of the training run in microseconds.
    pub training_wall_us: u64,
}

/// Per-solver-block branch profile: which solver path was taken and how often.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverBranchProfile {
    pub block_index: usize,
    pub dense_count: u64,
    pub sparse_count: u64,
    pub scalar_count: u64,
    pub dominant_path: SolverPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolverPath {
    Dense,
    Sparse,
    Scalar,
    Mixed,
}

/// Zero-crossing event profile for a single crossing function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroCrossingProfile {
    pub crossing_index: usize,
    pub trigger_count: u64,
    pub last_trigger_time: f64,
    pub avg_interval: f64,
}

/// Observed value range for a state variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueRange {
    pub min: f64,
    pub max: f64,
    pub is_integer_like: bool,
}

/// Clock partition activation profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockActivationProfile {
    pub partition_index: usize,
    pub activation_count: u64,
    pub total_eval_us: u64,
}

/// Newton solver iteration statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewtonStats {
    pub total_calls: u64,
    pub total_iterations: u64,
    pub max_iterations_single: u32,
    pub convergence_failures: u64,
    pub avg_iterations: f64,
}

impl ModelProfile {
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            profile_version: PROFILE_VERSION,
            ..Default::default()
        }
    }

    /// Serialize profile to bincode bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("profile serialize: {}", e))
    }

    /// Deserialize profile from bincode bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let profile: Self =
            bincode::deserialize(bytes).map_err(|e| format!("profile deserialize: {}", e))?;
        if profile.profile_version != PROFILE_VERSION {
            return Err(format!(
                "profile version mismatch: got {} expected {}",
                profile.profile_version, PROFILE_VERSION
            ));
        }
        Ok(profile)
    }

    /// Write profile to a file.
    pub fn write_to_file(&self, path: &Path) -> Result<(), String> {
        let bytes = self.to_bytes()?;
        std::fs::write(path, bytes).map_err(|e| format!("write profile: {}", e))
    }

    /// Read profile from a file.
    pub fn read_from_file(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read profile: {}", e))?;
        Self::from_bytes(&bytes)
    }

    pub fn has_useful_data(&self) -> bool {
        self.total_steps > 0
    }

    /// Return equation indices considered "hot" (top 20% by evaluation frequency).
    pub fn hot_equation_indices(&self, total_equations: usize) -> Vec<usize> {
        let threshold = (total_equations as f64 * 0.2).ceil() as usize;
        self.hot_equations
            .iter()
            .take(threshold.max(1))
            .copied()
            .collect()
    }
}

/// Runtime profile collector injected into the simulation loop.
#[derive(Debug)]
pub struct ProfileCollector {
    profile: ModelProfile,
    equation_eval_counts: Vec<u64>,
    step_count: u64,
}

impl ProfileCollector {
    pub fn new(model_name: &str, equation_count: usize) -> Self {
        Self {
            profile: ModelProfile::new(model_name),
            equation_eval_counts: vec![0; equation_count],
            step_count: 0,
        }
    }

    pub fn record_step(&mut self) {
        self.step_count += 1;
    }

    pub fn record_equation_eval(&mut self, eq_index: usize) {
        if eq_index < self.equation_eval_counts.len() {
            self.equation_eval_counts[eq_index] += 1;
        }
    }

    pub fn record_zero_crossing(&mut self, crossing_index: usize, time: f64) {
        if let Some(zc) = self
            .profile
            .zero_crossing_stats
            .iter_mut()
            .find(|z| z.crossing_index == crossing_index)
        {
            zc.trigger_count += 1;
            if zc.trigger_count > 1 {
                let total_interval = zc.avg_interval * (zc.trigger_count - 1) as f64;
                zc.avg_interval =
                    (total_interval + (time - zc.last_trigger_time)) / zc.trigger_count as f64;
            }
            zc.last_trigger_time = time;
        } else {
            self.profile.zero_crossing_stats.push(ZeroCrossingProfile {
                crossing_index,
                trigger_count: 1,
                last_trigger_time: time,
                avg_interval: 0.0,
            });
        }
    }

    pub fn record_newton_iteration(&mut self, iterations: u32, converged: bool) {
        let s = &mut self.profile.newton_iteration_stats;
        s.total_calls += 1;
        s.total_iterations += iterations as u64;
        if iterations > s.max_iterations_single {
            s.max_iterations_single = iterations;
        }
        if !converged {
            s.convergence_failures += 1;
        }
        s.avg_iterations = s.total_iterations as f64 / s.total_calls as f64;
    }

    pub fn record_state_value(&mut self, var_name: &str, value: f64) {
        let entry = self
            .profile
            .state_value_ranges
            .entry(var_name.to_string())
            .or_insert(ValueRange {
                min: f64::MAX,
                max: f64::MIN,
                is_integer_like: true,
            });
        if value < entry.min {
            entry.min = value;
        }
        if value > entry.max {
            entry.max = value;
        }
        if entry.is_integer_like && value.fract().abs() > 1e-12 {
            entry.is_integer_like = false;
        }
    }

    pub fn record_solver_branch(
        &mut self,
        block_index: usize,
        path: SolverPath,
    ) {
        if let Some(sb) = self
            .profile
            .solver_branches
            .iter_mut()
            .find(|s| s.block_index == block_index)
        {
            match path {
                SolverPath::Dense => sb.dense_count += 1,
                SolverPath::Sparse => sb.sparse_count += 1,
                SolverPath::Scalar => sb.scalar_count += 1,
                SolverPath::Mixed => {}
            }
            sb.dominant_path = if sb.dense_count >= sb.sparse_count && sb.dense_count >= sb.scalar_count {
                SolverPath::Dense
            } else if sb.sparse_count >= sb.dense_count && sb.sparse_count >= sb.scalar_count {
                SolverPath::Sparse
            } else {
                SolverPath::Scalar
            };
        } else {
            let mut entry = SolverBranchProfile {
                block_index,
                dense_count: 0,
                sparse_count: 0,
                scalar_count: 0,
                dominant_path: path,
            };
            match path {
                SolverPath::Dense => entry.dense_count = 1,
                SolverPath::Sparse => entry.sparse_count = 1,
                SolverPath::Scalar => entry.scalar_count = 1,
                SolverPath::Mixed => {}
            }
            self.profile.solver_branches.push(entry);
        }
    }

    pub fn record_clock_activation(&mut self, partition_index: usize, eval_us: u64) {
        if let Some(ca) = self
            .profile
            .clock_activation_pattern
            .iter_mut()
            .find(|c| c.partition_index == partition_index)
        {
            ca.activation_count += 1;
            ca.total_eval_us += eval_us;
        } else {
            self.profile
                .clock_activation_pattern
                .push(ClockActivationProfile {
                    partition_index,
                    activation_count: 1,
                    total_eval_us: eval_us,
                });
        }
    }

    /// Finalize collection: sort hot equations and set totals.
    pub fn finalize(mut self, wall_us: u64) -> ModelProfile {
        self.profile.total_steps = self.step_count;
        self.profile.training_wall_us = wall_us;

        let mut indexed: Vec<(usize, u64)> = self
            .equation_eval_counts
            .iter()
            .enumerate()
            .filter(|(_, &c)| c > 0)
            .map(|(i, &c)| (i, c))
            .collect();
        indexed.sort_by(|a, b| b.1.cmp(&a.1));
        self.profile.hot_equations = indexed.into_iter().map(|(i, _)| i).collect();

        self.profile
    }
}
