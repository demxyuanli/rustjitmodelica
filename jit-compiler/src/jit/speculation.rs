//! Speculative AOT framework for Modelica JIT (Phase 3 of Leyden-inspired compilation).
//!
//! Manages speculation assumptions derived from training-run profile data. Each
//! speculation is a runtime-verifiable assertion about model behavior (e.g. "Newton
//! solver always converges in dense mode"). Speculations are compiled into guard
//! checks in the generated native code.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use crate::condenser::profile_data::{ModelProfile, SolverPath};

static GUARD_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Types of speculative assumptions that can be derived from profile data.
/// Float values are stored as bit patterns (u64) to enable Eq + Hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpeculationKind {
    /// Newton solver always uses dense path for the given block.
    NewtonDense { block_index: usize },
    /// Newton solver always uses sparse path for the given block.
    NewtonSparse { block_index: usize },
    /// A zero-crossing never triggers (dead code in training).
    ZeroCrossingNeverTriggers { crossing_index: usize },
    /// A state variable stays within a known range (min/max as f64 bits).
    StateValueRange {
        var_name: String,
        min_bits: u64,
        max_bits: u64,
    },
    /// A clock partition is never activated (can skip).
    ClockPartitionInactive { partition_index: usize },
    /// Parameter value is effectively constant (value as f64 bits).
    ParamConstant { param_name: String, value_bits: u64 },
}

impl SpeculationKind {
    pub fn state_value_range(var_name: String, min: f64, max: f64) -> Self {
        Self::StateValueRange {
            var_name,
            min_bits: min.to_bits(),
            max_bits: max.to_bits(),
        }
    }

    pub fn param_constant(param_name: String, value: f64) -> Self {
        Self::ParamConstant {
            param_name,
            value_bits: value.to_bits(),
        }
    }
}

/// A guard is a runtime check that validates a speculation assumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guard {
    pub id: u32,
    pub speculation: SpeculationKind,
    pub invalidation_count: u64,
    pub active: bool,
}

/// Deoptimization frame: captures the state needed to fall back from a speculative
/// fast path to a generic implementation.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DeoptFrame {
    pub guard_id: u32,
    pub speculation: SpeculationKind,
    /// Maps optimized slot indices to generic slot indices.
    pub state_map: Vec<(u32, u32)>,
    /// Name of the fallback function to invoke after deoptimization.
    pub fallback_label: String,
}

/// Registry of all active speculations and their guards for a compilation unit.
pub struct SpeculationRegistry {
    guards: HashMap<u32, Guard>,
    deopt_frames: HashMap<u32, DeoptFrame>,
    invalidation_log: Vec<InvalidationEvent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvalidationEvent {
    pub guard_id: u32,
    pub speculation: SpeculationKind,
    pub timestamp_us: u64,
    pub reason: String,
}

impl SpeculationRegistry {
    pub fn new() -> Self {
        Self {
            guards: HashMap::new(),
            deopt_frames: HashMap::new(),
            invalidation_log: Vec::new(),
        }
    }

    /// Create speculations from training profile data.
    pub fn from_profile(profile: &ModelProfile) -> Self {
        let mut reg = Self::new();

        for sb in &profile.solver_branches {
            let total = sb.dense_count + sb.sparse_count + sb.scalar_count;
            if total == 0 {
                continue;
            }
            let dominance_ratio = match sb.dominant_path {
                SolverPath::Dense => sb.dense_count as f64 / total as f64,
                SolverPath::Sparse => sb.sparse_count as f64 / total as f64,
                SolverPath::Scalar => sb.scalar_count as f64 / total as f64,
                SolverPath::Mixed => 0.0,
            };
            if dominance_ratio >= 0.95 {
                let kind = match sb.dominant_path {
                    SolverPath::Dense => SpeculationKind::NewtonDense {
                        block_index: sb.block_index,
                    },
                    SolverPath::Sparse => SpeculationKind::NewtonSparse {
                        block_index: sb.block_index,
                    },
                    _ => continue,
                };
                reg.add_speculation(kind);
            }
        }

        for zc in &profile.zero_crossing_stats {
            if zc.trigger_count == 0 {
                reg.add_speculation(SpeculationKind::ZeroCrossingNeverTriggers {
                    crossing_index: zc.crossing_index,
                });
            }
        }

        for (var_name, range) in &profile.state_value_ranges {
            let margin = (range.max - range.min).abs() * 0.1;
            reg.add_speculation(SpeculationKind::state_value_range(
                var_name.clone(),
                range.min - margin,
                range.max + margin,
            ));
        }

        for ca in &profile.clock_activation_pattern {
            if ca.activation_count == 0 {
                reg.add_speculation(SpeculationKind::ClockPartitionInactive {
                    partition_index: ca.partition_index,
                });
            }
        }

        reg
    }

    pub fn add_speculation(&mut self, kind: SpeculationKind) -> u32 {
        let id = GUARD_COUNTER.fetch_add(1, Ordering::Relaxed) as u32;
        self.guards.insert(id, Guard { id, speculation: kind, invalidation_count: 0, active: true });
        id
    }

    pub fn restore_speculation(&mut self, id: u32, kind: SpeculationKind) {
        self.guards.insert(
            id,
            Guard {
                id,
                speculation: kind,
                invalidation_count: 0,
                active: true,
            },
        );
        let next_counter = id as u64 + 1;
        let _ = GUARD_COUNTER.fetch_max(next_counter, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn register_deopt_frame(&mut self, frame: DeoptFrame) {
        self.deopt_frames.insert(frame.guard_id, frame);
    }

    /// Check if a guard is still valid. Returns false if the speculation has been
    /// invalidated (runtime detected assumption violation).
    pub fn check_guard(&self, guard_id: u32) -> bool {
        self.guards
            .get(&guard_id)
            .map(|g| g.active)
            .unwrap_or(false)
    }

    /// Invalidate a guard due to runtime observation contradicting the speculation.
    pub fn invalidate(&mut self, guard_id: u32, reason: &str) {
        if let Some(guard) = self.guards.get_mut(&guard_id) {
            guard.active = false;
            guard.invalidation_count += 1;
            self.invalidation_log.push(InvalidationEvent {
                guard_id,
                speculation: guard.speculation.clone(),
                timestamp_us: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as u64,
                reason: reason.to_string(),
            });
        }
    }

    pub fn active_guard_count(&self) -> usize {
        self.guards.values().filter(|g| g.active).count()
    }

    pub fn total_guard_count(&self) -> usize {
        self.guards.len()
    }

    pub fn invalidation_count(&self) -> usize {
        self.invalidation_log.len()
    }

    pub fn deopt_frame_for(&self, guard_id: u32) -> Option<&DeoptFrame> {
        self.deopt_frames.get(&guard_id)
    }

    pub fn active_speculations(&self) -> Vec<&SpeculationKind> {
        self.guards
            .values()
            .filter(|g| g.active)
            .map(|g| &g.speculation)
            .collect()
    }

    pub fn active_speculations_with_ids(&self) -> Vec<(u32, &SpeculationKind)> {
        self.guards
            .values()
            .filter(|g| g.active)
            .map(|g| (g.id, &g.speculation))
            .collect()
    }

    pub fn invalidation_log(&self) -> &[InvalidationEvent] {
        &self.invalidation_log
    }

    /// Speculations relevant to a Newton solvable block.
    pub fn newton_speculation_for_block(&self, block_index: usize) -> Option<&Guard> {
        self.guards.values().find(|g| {
            g.active
                && matches!(
                    &g.speculation,
                    SpeculationKind::NewtonDense { block_index: bi }
                    | SpeculationKind::NewtonSparse { block_index: bi }
                    if *bi == block_index
                )
        })
    }
}

/// Global speculation registry for the current compilation (thread-safe singleton).
static GLOBAL_REGISTRY: std::sync::OnceLock<RwLock<SpeculationRegistry>> =
    std::sync::OnceLock::new();

pub fn global_registry() -> &'static RwLock<SpeculationRegistry> {
    GLOBAL_REGISTRY.get_or_init(|| RwLock::new(SpeculationRegistry::new()))
}

pub fn init_global_registry(profile: &ModelProfile) {
    let reg = SpeculationRegistry::from_profile(profile);
    if let Some(existing) = GLOBAL_REGISTRY.get() {
        if let Ok(mut guard) = existing.write() {
            *guard = reg;
        }
        return;
    }
    let _ = GLOBAL_REGISTRY.set(RwLock::new(reg));
}

pub fn validate_runtime_assumptions(
    state_vars: &[String],
    states: &[f64],
    clock_activations: &[usize],
) {
    let Ok(mut reg) = global_registry().write() else {
        return;
    };
    let active: Vec<(u32, SpeculationKind)> = reg
        .active_speculations_with_ids()
        .into_iter()
        .map(|(id, kind)| (id, kind.clone()))
        .collect();
    for (guard_id, spec) in active {
        match spec {
            SpeculationKind::StateValueRange {
                var_name,
                min_bits,
                max_bits,
            } => {
                if let Some(var_idx) = state_vars.iter().position(|n| n == &var_name) {
                    if let Some(v) = states.get(var_idx).copied() {
                        let min = f64::from_bits(min_bits);
                        let max = f64::from_bits(max_bits);
                        if v < min || v > max {
                            reg.invalidate(guard_id, "state_value_range_violation");
                        }
                    }
                }
            }
            SpeculationKind::ClockPartitionInactive { partition_index } => {
                if clock_activations.contains(&partition_index) {
                    reg.invalidate(guard_id, "clock_partition_activated");
                }
            }
            SpeculationKind::ParamConstant { .. } => {
                reg.invalidate(guard_id, "param_constant_runtime_not_verifiable");
            }
            _ => {}
        }
    }
}

/// Runtime guard check function exposed to JIT-generated code via C ABI.
/// Returns 1 if speculation still holds, 0 if invalidated.
/// When a guard fails, automatically signals deoptimization via the global flag.
pub extern "C" fn speculation_holds(guard_id: u32) -> i32 {
    match global_registry().read() {
        Ok(reg) => {
            if reg.check_guard(guard_id) {
                1
            } else {
                super::deopt::DEOPT_PENDING.store(true, std::sync::atomic::Ordering::Release);
                0
            }
        }
        Err(_) => {
            super::deopt::DEOPT_PENDING.store(true, std::sync::atomic::Ordering::Release);
            0
        }
    }
}

/// Runtime deoptimization trigger exposed to JIT-generated code via C ABI.
/// Marks a guard as invalidated and logs the event.
#[allow(dead_code)]
pub extern "C" fn deopt_trigger(guard_id: u32) {
    if let Ok(mut reg) = global_registry().write() {
        reg.invalidate(guard_id, "runtime_deopt_trigger");
    }
}
