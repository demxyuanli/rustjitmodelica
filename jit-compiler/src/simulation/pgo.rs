//! Lightweight runtime profile-guided optimization for simulation.
//!
//! Tracks per-equation evaluation frequency during simulation. When an equation
//! exceeds the hot threshold, triggers JIT recompilation at a higher tier.
//! Profiles are persisted to disk for reuse on subsequent runs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// PGO configuration.
#[derive(Clone, Debug)]
pub struct PgoConfig {
    /// Number of simulation steps before first hotness check.
    pub warmup_steps: u64,
    /// Re-check interval (steps).
    pub check_interval: u64,
    /// Evaluations per step above which an equation is considered "hot".
    pub hot_threshold: u64,
    /// Path to save/load PGO profiles. None disables persistence.
    pub profile_path: Option<std::path::PathBuf>,
    /// Whether PGO is enabled.
    pub enabled: bool,
}

impl Default for PgoConfig {
    fn default() -> Self {
        Self {
            warmup_steps: 100,
            check_interval: 500,
            hot_threshold: 10,
            profile_path: None,
            enabled: std::env::var("RUSTMODLICA_PGO_ENABLE")
                .ok()
                .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
                .unwrap_or(false),
        }
    }
}

/// Persistent profile of hot equations for a specific model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgoProfile {
    /// Model artifact hash for identity validation.
    pub model_hash: String,
    /// Equation indices (0-based) that were identified as hot.
    pub hot_equation_indices: Vec<usize>,
    /// Total simulation steps used to produce this profile.
    pub total_steps: u64,
    /// Timestamp of profile creation (Unix seconds).
    pub created_at: u64,
}

/// Runtime PGO tracker that counts equation evaluations during simulation.
pub struct PgoTracker {
    config: PgoConfig,
    /// Counts per equation index.
    eval_counts: Vec<u64>,
    /// Total step count since last reset.
    steps: u64,
    /// Previously identified hot equations (loaded from disk).
    known_hot: Vec<usize>,
}

impl PgoTracker {
    pub fn new(config: PgoConfig, equation_count: usize) -> Self {
        // Try to load existing profile
        let known_hot = config
            .profile_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<PgoProfile>(&s).ok())
            .map(|p| p.hot_equation_indices)
            .unwrap_or_default();

        Self {
            eval_counts: vec![0; equation_count],
            steps: 0,
            known_hot,
            config,
        }
    }

    /// Record one evaluation of equation `idx`.
    pub fn record_eval(&mut self, idx: usize) {
        if idx < self.eval_counts.len() {
            self.eval_counts[idx] = self.eval_counts[idx].saturating_add(1);
        }
    }

    /// Record a completed simulation step.
    pub fn record_step(&mut self) {
        self.steps += 1;
    }

    /// Check if it's time to analyze hotness and return newly hot equation indices.
    pub fn check_hotness(&mut self) -> Option<Vec<usize>> {
        if !self.config.enabled {
            return None;
        }
        if self.steps < self.config.warmup_steps {
            return None;
        }
        if self.steps % self.config.check_interval != 0 {
            return None;
        }
        let mut hot = Vec::new();
        for (idx, &count) in self.eval_counts.iter().enumerate() {
            let avg_per_step = count / self.steps.max(1);
            if avg_per_step >= self.config.hot_threshold && !self.known_hot.contains(&idx) {
                hot.push(idx);
            }
        }
        if hot.is_empty() {
            None
        } else {
            self.known_hot.extend(&hot);
            Some(hot)
        }
    }

    /// Reset counters (call after tier promotion to avoid re-triggering).
    pub fn reset_counts(&mut self) {
        self.eval_counts.fill(0);
        self.steps = 0;
    }

    /// Get indices of known-hot equations (from disk + runtime detection).
    pub fn known_hot_indices(&self) -> &[usize] {
        &self.known_hot
    }

    /// Save current profile to disk.
    pub fn save_profile(&self, model_hash: &str) -> Result<(), String> {
        let path = match &self.config.profile_path {
            Some(p) => p.clone(),
            None => return Ok(()),
        };
        let profile = PgoProfile {
            model_hash: model_hash.to_string(),
            hot_equation_indices: self.known_hot.clone(),
            total_steps: self.steps,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        let json = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, &json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Load profile from disk.
    pub fn load_profile(path: &std::path::Path) -> Result<PgoProfile, String> {
        let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }
}
