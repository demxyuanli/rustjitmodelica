//! Condenser statistics for observability (Phase 7 of Leyden-inspired compilation).
//!
//! Collects and reports condenser execution metrics: hit rates, latencies,
//! artifacts produced, and cache savings.

use super::CondenserOutput;
use serde::Serialize;
use std::collections::HashMap;

/// Aggregated statistics for all condenser executions in a session.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CondenserStats {
    pub per_condenser: HashMap<String, SingleCondenserStats>,
    pub total_elapsed_us: u64,
    pub total_artifacts_written: u32,
    pub total_cache_hits: u32,
    pub total_errors: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SingleCondenserStats {
    pub name: String,
    pub run_count: u32,
    pub success_count: u32,
    pub error_count: u32,
    pub total_elapsed_us: u64,
    pub total_artifacts_written: u32,
    pub total_cache_hits: u32,
    pub last_detail: Option<String>,
}

impl CondenserStats {
    pub fn record_success(&mut self, output: &CondenserOutput) {
        let entry = self
            .per_condenser
            .entry(output.condenser_name.clone())
            .or_insert_with(|| SingleCondenserStats {
                name: output.condenser_name.clone(),
                ..Default::default()
            });
        entry.run_count += 1;
        entry.success_count += 1;
        entry.total_elapsed_us += output.elapsed_us;
        entry.total_artifacts_written += output.artifacts_written;
        entry.total_cache_hits += output.cache_hits;
        entry.last_detail = output.detail.clone();

        self.total_elapsed_us += output.elapsed_us;
        self.total_artifacts_written += output.artifacts_written;
        self.total_cache_hits += output.cache_hits;
    }

    pub fn record_error(&mut self, condenser_name: &str) {
        let entry = self
            .per_condenser
            .entry(condenser_name.to_string())
            .or_insert_with(|| SingleCondenserStats {
                name: condenser_name.to_string(),
                ..Default::default()
            });
        entry.run_count += 1;
        entry.error_count += 1;
        self.total_errors += 1;
    }

    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.total_cache_hits + self.total_artifacts_written;
        if total == 0 {
            0.0
        } else {
            self.total_cache_hits as f64 / total as f64
        }
    }

    pub fn format_summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Condenser stats: {:.1}ms total, {} artifacts, {} cache hits ({:.0}% hit rate), {} errors",
            self.total_elapsed_us as f64 / 1000.0,
            self.total_artifacts_written,
            self.total_cache_hits,
            self.cache_hit_rate() * 100.0,
            self.total_errors,
        ));
        let mut names: Vec<&String> = self.per_condenser.keys().collect();
        names.sort();
        for name in names {
            let s = &self.per_condenser[name];
            lines.push(format!(
                "  {}: {}x run, {}x ok, {}x err, {:.1}ms, {} writes, {} hits{}",
                s.name,
                s.run_count,
                s.success_count,
                s.error_count,
                s.total_elapsed_us as f64 / 1000.0,
                s.total_artifacts_written,
                s.total_cache_hits,
                s.last_detail
                    .as_ref()
                    .map(|d| format!(" [{}]", d))
                    .unwrap_or_default(),
            ));
        }
        lines.join("\n")
    }
}
