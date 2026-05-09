//! Simulation checkpoint/restart: serialize and restore full simulation state.
//!
//! Format: JSON with model identity hash for cross-model validation.

use serde::{Deserialize, Serialize};

/// Serializable simulation snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Model artifact hash for identity validation on restore.
    pub model_hash: String,
    /// Simulation time.
    pub time: f64,
    /// State variable names (for validation).
    pub state_names: Vec<String>,
    /// State variable values.
    pub state_values: Vec<f64>,
    /// Discrete variable names.
    pub discrete_names: Vec<String>,
    /// Discrete variable values.
    pub discrete_values: Vec<f64>,
    /// Parameter names.
    pub param_names: Vec<String>,
    /// Parameter values.
    pub param_values: Vec<f64>,
    /// Solver type that created this checkpoint.
    pub solver: String,
    /// Solver-specific metadata (step size, order, etc.).
    pub solver_meta: Option<SolverMeta>,
    /// Event iteration counter.
    pub event_counter: u64,
    /// Total step count.
    pub step_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverMeta {
    /// Current step size (for adaptive solvers).
    pub step_size: Option<f64>,
    /// Solver order (for multi-step methods).
    pub order: Option<u32>,
    /// Internal solver state (opaque bytes for SUNDIALS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_state: Option<Vec<u8>>,
}

impl Checkpoint {
    /// Write checkpoint to a file as JSON.
    pub fn write_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, &json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Read checkpoint from a JSON file.
    pub fn read_from_file(path: &std::path::Path) -> Result<Self, String> {
        let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }

    /// Validate that this checkpoint matches the given model hash.
    pub fn validate_model(&self, expected_hash: &str) -> Result<(), String> {
        if self.model_hash != expected_hash {
            return Err(format!(
                "Checkpoint model hash mismatch: checkpoint has '{}', model has '{}'",
                self.model_hash, expected_hash
            ));
        }
        Ok(())
    }
}

/// Configuration for checkpoint scheduling.
#[derive(Clone, Debug)]
pub struct CheckpointConfig {
    /// Interval in simulation seconds between checkpoints. None disables.
    pub interval_seconds: Option<f64>,
    /// Maximum number of checkpoint files to keep (oldest deleted first).
    pub max_keep: usize,
    /// Directory for checkpoint files.
    pub output_dir: std::path::PathBuf,
    /// Model hash for identity validation.
    pub model_hash: String,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            interval_seconds: None,
            max_keep: 10,
            output_dir: std::path::PathBuf::from("."),
            model_hash: String::new(),
        }
    }
}

/// Tracks the last checkpoint time for interval-based scheduling.
pub struct CheckpointScheduler {
    config: CheckpointConfig,
    last_checkpoint_time: f64,
    sequence: u64,
}

impl CheckpointScheduler {
    pub fn new(config: CheckpointConfig) -> Self {
        Self {
            config,
            last_checkpoint_time: f64::NEG_INFINITY,
            sequence: 0,
        }
    }

    /// Check if a checkpoint should be taken at the given simulation time.
    pub fn should_checkpoint(&self, current_time: f64) -> bool {
        match self.config.interval_seconds {
            Some(interval) if interval > 0.0 => {
                current_time - self.last_checkpoint_time >= interval
            }
            _ => false,
        }
    }

    /// Record that a checkpoint was taken.
    pub fn record_checkpoint(&mut self) {
        self.sequence += 1;
    }

    /// Generate the checkpoint file path for the current sequence.
    pub fn checkpoint_path(&self) -> std::path::PathBuf {
        self.config.output_dir.join(format!(
            "checkpoint_{:04}.json",
            self.sequence
        ))
    }

    /// Build a checkpoint from current simulation state.
    pub fn build_checkpoint(
        &mut self,
        current_time: f64,
        state_names: &[String],
        state_values: &[f64],
        discrete_names: &[String],
        discrete_values: &[f64],
        param_names: &[String],
        param_values: &[f64],
        solver: &str,
        solver_meta: Option<SolverMeta>,
        event_counter: u64,
        step_count: u64,
    ) -> Checkpoint {
        self.last_checkpoint_time = current_time;
        Checkpoint {
            model_hash: self.config.model_hash.clone(),
            time: current_time,
            state_names: state_names.to_vec(),
            state_values: state_values.to_vec(),
            discrete_names: discrete_names.to_vec(),
            discrete_values: discrete_values.to_vec(),
            param_names: param_names.to_vec(),
            param_values: param_values.to_vec(),
            solver: solver.to_string(),
            solver_meta,
            event_counter,
            step_count,
        }
    }
}
