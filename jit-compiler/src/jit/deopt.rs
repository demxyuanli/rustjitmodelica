//! Deoptimization framework for speculative AOT (Phase 3 of Leyden-inspired compilation).
//!
//! When a speculative assumption is invalidated at runtime, the deoptimizer transitions
//! from the optimized fast path to a generic fallback. Since Cranelift does not support
//! on-stack replacement (OSR), deoptimization happens at simulation step boundaries --
//! a natural fit for Modelica's time-stepping structure.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

static EMPTY_ENUMS: LazyLock<HashMap<String, Vec<String>>> = LazyLock::new(HashMap::new);

use super::types::CalcDerivsFunc;
use crate::ast::{AlgorithmStatement, Equation};
use crate::compiler::ClockPartitionScheduleEntry;
use crate::jit::types::ArrayInfo;

pub(crate) static DEOPT_PENDING: AtomicBool = AtomicBool::new(false);

use std::sync::Mutex;
use std::sync::OnceLock;

static PRECOMPILED_GENERIC: OnceLock<Mutex<Option<CalcDerivsFunc>>> = OnceLock::new();

pub fn set_precompiled_generic(func: CalcDerivsFunc) {
    let lock = PRECOMPILED_GENERIC.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(func);
    }
}

pub fn take_precompiled_generic() -> Option<CalcDerivsFunc> {
    let lock = PRECOMPILED_GENERIC.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        guard.take()
    } else {
        None
    }
}

/// Manager for handling deoptimization transitions between simulation steps.
pub struct DeoptManager {
    /// Generic (unoptimized) compiled function to fall back to.
    generic_func: Option<CalcDerivsFunc>,
    /// Currently active (possibly speculative) function.
    active_func: CalcDerivsFunc,
    /// Whether a deoptimization is pending (will take effect at next step boundary).
    deopt_pending: bool,
    /// Log of deoptimization events for diagnostics.
    deopt_events: Vec<DeoptEvent>,
    /// Statistics.
    total_deopts: u64,
    total_recompilations: u64,
}

#[derive(Debug, Clone)]
pub struct DeoptEvent {
    pub guard_id: u32,
    pub simulation_time: f64,
    pub step_number: u64,
    pub reason: String,
}

impl DeoptManager {
    /// Create a new deopt manager with the optimized function as active.
    pub fn new(optimized_func: CalcDerivsFunc) -> Self {
        Self {
            generic_func: None,
            active_func: optimized_func,
            deopt_pending: false,
            deopt_events: Vec::new(),
            total_deopts: 0,
            total_recompilations: 0,
        }
    }

    /// Create a manager with both optimized and generic fallback functions.
    pub fn with_fallback(
        optimized_func: CalcDerivsFunc,
        generic_func: CalcDerivsFunc,
    ) -> Self {
        Self {
            generic_func: Some(generic_func),
            active_func: optimized_func,
            deopt_pending: false,
            deopt_events: Vec::new(),
            total_deopts: 0,
            total_recompilations: 0,
        }
    }

    /// Get the currently active function pointer.
    pub fn active_func(&self) -> CalcDerivsFunc {
        self.active_func
    }

    /// Signal that a deoptimization should happen at the next step boundary.
    pub fn request_deopt(&mut self, guard_id: u32, time: f64, step: u64, reason: &str) {
        self.deopt_pending = true;
        DEOPT_PENDING.store(true, Ordering::Release);
        self.deopt_events.push(DeoptEvent {
            guard_id,
            simulation_time: time,
            step_number: step,
            reason: reason.to_string(),
        });
    }

    /// Called at each simulation step boundary. If deopt is pending (from either
    /// local request_deopt or the global DEOPT_PENDING flag set by guard checks),
    /// switches to the generic fallback.
    pub fn check_and_apply(&mut self) -> bool {
        let global_pending = DEOPT_PENDING.load(Ordering::Acquire);
        if !self.deopt_pending && !global_pending {
            return false;
        }
        self.deopt_pending = false;
        DEOPT_PENDING.store(false, Ordering::Release);

        if let Some(generic) = self.generic_func {
            self.active_func = generic;
            self.total_deopts += 1;
            true
        } else {
            false
        }
    }

    /// Replace the active function with a newly recompiled version (after deopt + reprofile).
    #[allow(dead_code)]
    pub fn hot_replace(&mut self, new_func: CalcDerivsFunc) {
        self.active_func = new_func;
        self.total_recompilations += 1;
    }

    pub fn is_deopt_pending(&self) -> bool {
        self.deopt_pending
    }

    pub fn deopt_event_count(&self) -> usize {
        self.deopt_events.len()
    }

    pub fn total_deopts(&self) -> u64 {
        self.total_deopts
    }

    pub fn total_recompilations(&self) -> u64 {
        self.total_recompilations
    }

    pub fn deopt_events(&self) -> &[DeoptEvent] {
        &self.deopt_events
    }
}

/// One deoptimization event for `sim_perf` JSON export.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DeoptSimPerfEvent {
    pub guard_id: u32,
    pub simulation_time: f64,
    pub step_number: u64,
    pub reason: String,
}

/// Deoptimization summary at end of simulation (last N events only).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DeoptSimPerfSummary {
    pub deopt_total: u64,
    pub deopt_recompilations: u64,
    pub deopt_event_count: usize,
    pub deopt_events: Vec<DeoptSimPerfEvent>,
}

const DEOPT_SIM_PERF_EVENT_CAP: usize = 32;

impl DeoptSimPerfSummary {
    pub fn from_manager(dm: &DeoptManager) -> Self {
        let ev = dm.deopt_events();
        let start = ev.len().saturating_sub(DEOPT_SIM_PERF_EVENT_CAP);
        let deopt_events: Vec<DeoptSimPerfEvent> = ev[start..]
            .iter()
            .map(|e| DeoptSimPerfEvent {
                guard_id: e.guard_id,
                simulation_time: e.simulation_time,
                step_number: e.step_number,
                reason: e.reason.clone(),
            })
            .collect();
        Self {
            deopt_total: dm.total_deopts(),
            deopt_recompilations: dm.total_recompilations(),
            deopt_event_count: ev.len(),
            deopt_events,
        }
    }
}

/// Global flag: any JIT code can query this atomically to see if it should deopt.
pub fn is_global_deopt_pending() -> bool {
    DEOPT_PENDING.load(Ordering::Acquire)
}

/// Result of dual-path compilation: both speculative (fast) and generic (safe)
/// functions, enabling deoptimization fallback without recompilation.
pub struct DualCompileResult {
    pub speculative: CalcDerivsFunc,
    pub generic: CalcDerivsFunc,
    pub speculation_count: usize,
    pub speculative_jit_keepalive: Option<Box<super::compile::Jit>>,
    pub generic_jit_keepalive: Option<Box<super::compile::Jit>>,
}

#[derive(Debug, Clone, Copy)]
pub enum RegistryErrorKind {
    Poisoned,
    Other,
}

#[derive(Debug, Clone)]
pub enum DualCompileError {
    SpeculationRegistryRead {
        kind: RegistryErrorKind,
    },
    SpeculationRegistryWrite {
        kind: RegistryErrorKind,
    },
    SpeculativeCompile {
        cranelift_phase: &'static str,
        symbol_name: Option<String>,
        raw_message: String,
    },
    GenericCompile {
        cranelift_phase: &'static str,
        symbol_name: Option<String>,
        raw_message: String,
    },
}

impl DualCompileError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SpeculationRegistryRead { .. } => "registry_read",
            Self::SpeculationRegistryWrite { .. } => "registry_write",
            Self::SpeculativeCompile { .. } => "compile_spec",
            Self::GenericCompile { .. } => "compile_generic",
        }
    }

    pub fn registry_poisoned(&self) -> bool {
        match self {
            Self::SpeculationRegistryRead { kind } | Self::SpeculationRegistryWrite { kind } => {
                matches!(kind, RegistryErrorKind::Poisoned)
            }
            _ => false,
        }
    }

    pub fn cranelift_phase(&self) -> Option<&'static str> {
        match self {
            Self::SpeculativeCompile { cranelift_phase, .. } => Some(*cranelift_phase),
            Self::GenericCompile { cranelift_phase, .. } => Some(*cranelift_phase),
            _ => None,
        }
    }

    pub fn symbol_name(&self) -> Option<&str> {
        match self {
            Self::SpeculativeCompile {
                symbol_name: Some(s),
                ..
            } => Some(s.as_str()),
            Self::GenericCompile {
                symbol_name: Some(s),
                ..
            } => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn stable_detail(&self) -> String {
        let reg = if self.registry_poisoned() { "1" } else { "0" };
        let phase = self.cranelift_phase().unwrap_or("none");
        let sym = self.symbol_name().unwrap_or("none");
        format!(
            "code={};registry_poisoned={};cranelift_phase={};symbol_name={}",
            self.code(),
            reg,
            phase,
            sym
        )
    }
}

impl std::fmt::Display for DualCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpeculationRegistryRead { kind } => {
                write!(f, "{}: kind={:?}", self.code(), kind)
            }
            Self::SpeculationRegistryWrite { kind } => {
                write!(f, "{}: kind={:?}", self.code(), kind)
            }
            Self::SpeculativeCompile {
                cranelift_phase,
                symbol_name,
                raw_message,
            } => write!(
                f,
                "{}: phase={} symbol={:?} msg={}",
                self.code(),
                cranelift_phase,
                symbol_name,
                raw_message
            ),
            Self::GenericCompile {
                cranelift_phase,
                symbol_name,
                raw_message,
            } => write!(
                f,
                "{}: phase={} symbol={:?} msg={}",
                self.code(),
                cranelift_phase,
                symbol_name,
                raw_message
            ),
        }
    }
}

impl std::error::Error for DualCompileError {}

fn classify_registry_error(e: &str) -> RegistryErrorKind {
    if e.to_lowercase().contains("poison") {
        RegistryErrorKind::Poisoned
    } else {
        RegistryErrorKind::Other
    }
}

fn extract_symbol_name(msg: &str) -> Option<String> {
    // Example expected pattern: External function 'foo' is not linked...
    let marker = "External function '";
    let start = msg.find(marker)?;
    let rest = &msg[start + marker.len()..];
    let end = rest.find('\'')?;
    let name = rest[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Compile a model with two code paths:
///  - **speculative**: uses profile-guided guards and optimized fast paths
///  - **generic**: no speculation, always-correct fallback
///
/// The speculative function is used as long as all speculation guards hold.
/// If a guard trips, `DeoptManager` switches to the generic path at the next
/// simulation step boundary.
#[allow(clippy::too_many_arguments)]
pub fn dual_compile(
    state_vars: &[String],
    discrete_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    array_info: &HashMap<String, ArrayInfo>,
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
    clock_partition_schedule: &[ClockPartitionScheduleEntry],
    t_end: f64,
    param_values: &[f64],
    newton_tearing_var_names: &[String],
    external_modelica_names: &HashSet<String>,
    const_fold_params: &[(String, f64)],
    stream_connection_set: &HashMap<String, Vec<String>>,
    stream_flow_map: &HashMap<String, String>,
    connector_connection_degree: &HashMap<String, usize>,
    extra_symbols: Option<&HashMap<String, *const u8>>,
) -> Result<DualCompileResult, DualCompileError> {
    let speculation_count = crate::jit::speculation::global_registry()
        .read()
        .map(|r| r.active_guard_count())
        .unwrap_or(0);

    let mut spec_jit = super::compile::Jit::new_with_extra_symbols(extra_symbols);
    let (spec_func, _spec_when, _spec_crossings) = spec_jit
        .compile(
        state_vars,
        discrete_vars,
        param_vars,
        output_vars,
        array_info,
        alg_equations,
        diff_equations,
        algorithms,
        clock_partition_schedule,
        t_end,
        param_values,
        newton_tearing_var_names,
        external_modelica_names,
        const_fold_params,
        stream_connection_set,
        stream_flow_map,
        connector_connection_degree,
        &*EMPTY_ENUMS,
    )
    .map_err(|e| DualCompileError::SpeculativeCompile {
        cranelift_phase: "compile_speculative",
        symbol_name: extract_symbol_name(&e),
        raw_message: e,
    })?;

    let saved_guards: Vec<(u32, super::speculation::SpeculationKind)> = {
        let reg = crate::jit::speculation::global_registry()
            .read()
            .map_err(|e| DualCompileError::SpeculationRegistryRead {
                kind: classify_registry_error(&e.to_string()),
            })?;
        reg.active_speculations_with_ids()
            .into_iter()
            .map(|(id, s)| (id, s.clone()))
            .collect()
    };

    {
        let mut reg = crate::jit::speculation::global_registry()
            .write()
            .map_err(|e| DualCompileError::SpeculationRegistryWrite {
                kind: classify_registry_error(&e.to_string()),
            })?;
        for (id, _) in &saved_guards {
            reg.invalidate(*id, "dual_compile_generic_path");
        }
    }

    let mut gen_jit = super::compile::Jit::new_with_extra_symbols(extra_symbols);
    let (gen_func, _gen_when, _gen_crossings) = gen_jit
        .compile(
        state_vars,
        discrete_vars,
        param_vars,
        output_vars,
        array_info,
        alg_equations,
        diff_equations,
        algorithms,
        clock_partition_schedule,
        t_end,
        param_values,
        newton_tearing_var_names,
        external_modelica_names,
        const_fold_params,
        stream_connection_set,
        stream_flow_map,
        connector_connection_degree,
        &*EMPTY_ENUMS,
    )
    .map_err(|e| DualCompileError::GenericCompile {
        cranelift_phase: "compile_generic",
        symbol_name: extract_symbol_name(&e),
        raw_message: e,
    })?;

    {
        let mut reg = crate::jit::speculation::global_registry()
            .write()
            .map_err(|e| DualCompileError::SpeculationRegistryWrite {
                kind: classify_registry_error(&e.to_string()),
            })?;
        for (id, kind) in saved_guards {
            reg.restore_speculation(id, kind);
        }
    }

    Ok(DualCompileResult {
        speculative: spec_func,
        generic: gen_func,
        speculation_count,
        speculative_jit_keepalive: Some(Box::new(spec_jit)),
        generic_jit_keepalive: Some(Box::new(gen_jit)),
    })
}

/// Create a `DeoptManager` from a dual-compile result, with the speculative
/// path as active and the generic path as the fallback.
pub fn deopt_manager_from_dual(result: &DualCompileResult) -> DeoptManager {
    DeoptManager::with_fallback(result.speculative, result.generic)
}
