//! Tiered compilation scheduler (Phase 5 of Leyden-inspired compilation).
//!
//! Implements a multi-tier compilation strategy analogous to HotSpot's C1/C2:
//!
//!   Tier 0: Tree-walking interpreter (optional, for small models / fast startup)
//!   Tier 1: Fast Cranelift JIT (opt_level=none, minimal optimization)
//!   Tier 2: Optimized Cranelift JIT (opt_level=speed, current default path)
//!   Tier 3: Profile-guided speculative JIT (based on training run data)
//!
//! The scheduler decides the initial tier based on model complexity and upgrades
//! tiers in background threads. Tier transitions happen at simulation step boundaries
//! (no OSR required -- a natural fit for Modelica's time-stepping).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::sync::OnceLock;

use super::types::CalcDerivsFunc;
use crate::condenser::profile_data::ModelProfile;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TieredCompilationEvent {
    pub from_tier: CompileTier,
    pub to_tier: CompileTier,
    pub step_number: u64,
}

fn tiered_events_store() -> &'static Mutex<Vec<TieredCompilationEvent>> {
    static EVENTS: OnceLock<Mutex<Vec<TieredCompilationEvent>>> = OnceLock::new();
    EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn clear_tiered_events() {
    if let Ok(mut events) = tiered_events_store().lock() {
        events.clear();
    }
}

pub fn tiered_events_snapshot() -> Vec<TieredCompilationEvent> {
    tiered_events_store()
        .lock()
        .map(|events| events.clone())
        .unwrap_or_default()
}

/// Compilation tier levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum CompileTier {
    /// Tree-walking interpreter (fastest startup, slowest execution).
    Interpreter = 0,
    /// Fast JIT with no optimization (fast compile, moderate execution).
    FastJit = 1,
    /// Optimized JIT (moderate compile, fast execution).
    OptimizedJit = 2,
    /// Profile-guided speculative JIT (slow compile, fastest execution).
    ProfileGuided = 3,
}

impl CompileTier {
    pub fn cranelift_opt_level(&self) -> &'static str {
        match self {
            CompileTier::Interpreter => "none",
            CompileTier::FastJit => "none",
            CompileTier::OptimizedJit => "speed",
            CompileTier::ProfileGuided => "speed",
        }
    }

    pub fn skip_const_fold(&self) -> bool {
        matches!(self, CompileTier::Interpreter | CompileTier::FastJit)
    }

    pub fn skip_eq_dce(&self) -> bool {
        matches!(self, CompileTier::Interpreter | CompileTier::FastJit)
    }

    pub fn enable_speculation(&self) -> bool {
        matches!(self, CompileTier::ProfileGuided)
    }
}

/// Policy for selecting the initial compilation tier.
#[derive(Debug, Clone)]
pub struct TieringPolicy {
    /// Equation count threshold: below this, use Tier 0 (interpreter).
    pub interpreter_threshold: usize,
    /// Equation count threshold: below this, use Tier 1 (fast JIT).
    pub fast_jit_threshold: usize,
    /// Whether background tier-up is enabled.
    pub background_tierup: bool,
    /// Minimum simulation steps before considering tier-up.
    pub tierup_step_threshold: u64,
    /// Whether profile-guided tier is available.
    pub profile_available: bool,
    /// When set, background tier-up recompiles with CONST_FOLD/EQ_DCE off (adaptive policy).
    pub force_adaptive_skip_const_fold: bool,
    pub force_adaptive_skip_eq_dce: bool,
}

impl Default for TieringPolicy {
    fn default() -> Self {
        Self {
            interpreter_threshold: 5,
            fast_jit_threshold: 50,
            background_tierup: true,
            tierup_step_threshold: 100,
            profile_available: false,
            force_adaptive_skip_const_fold: false,
            force_adaptive_skip_eq_dce: false,
        }
    }
}

impl TieringPolicy {
    /// Select initial tier based on model complexity.
    pub fn select_initial_tier(&self, equation_count: usize) -> CompileTier {
        if equation_count <= self.interpreter_threshold {
            CompileTier::Interpreter
        } else if equation_count <= self.fast_jit_threshold {
            CompileTier::FastJit
        } else {
            CompileTier::OptimizedJit
        }
    }

    /// Decide if a tier-up should be triggered at the current step.
    pub fn should_tierup(
        &self,
        current_tier: CompileTier,
        step_count: u64,
    ) -> Option<CompileTier> {
        if !self.background_tierup {
            return None;
        }
        if step_count < self.tierup_step_threshold {
            return None;
        }
        match current_tier {
            CompileTier::Interpreter => Some(CompileTier::FastJit),
            CompileTier::FastJit => Some(CompileTier::OptimizedJit),
            CompileTier::OptimizedJit if self.profile_available => {
                Some(CompileTier::ProfileGuided)
            }
            _ => None,
        }
    }
}

/// Handle for a tiered function: tracks the current tier and function pointer.
pub struct TieredFunction {
    current_tier: AtomicU32,
    func: Mutex<CalcDerivsFunc>,
    /// Background compilation result: set when a tier-up completes.
    pending_upgrade: Mutex<Option<(CompileTier, CalcDerivsFunc)>>,
    /// Counters.
    tier_transitions: AtomicU32,
}

impl TieredFunction {
    pub fn new(initial_tier: CompileTier, func: CalcDerivsFunc) -> Self {
        Self {
            current_tier: AtomicU32::new(initial_tier as u32),
            func: Mutex::new(func),
            pending_upgrade: Mutex::new(None),
            tier_transitions: AtomicU32::new(0),
        }
    }

    pub fn current_tier(&self) -> CompileTier {
        match self.current_tier.load(Ordering::Relaxed) {
            0 => CompileTier::Interpreter,
            1 => CompileTier::FastJit,
            2 => CompileTier::OptimizedJit,
            3 => CompileTier::ProfileGuided,
            _ => CompileTier::OptimizedJit,
        }
    }

    pub fn get_func(&self) -> CalcDerivsFunc {
        *self.func.lock().unwrap()
    }

    /// Offer a tier-up: atomically replace the active function if upgrade is pending.
    /// Should be called at step boundaries.
    pub fn try_apply_upgrade(&self) -> bool {
        let mut pending = self.pending_upgrade.lock().unwrap();
        if let Some((new_tier, new_func)) = pending.take() {
            *self.func.lock().unwrap() = new_func;
            self.current_tier
                .store(new_tier as u32, Ordering::Release);
            self.tier_transitions.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Set a pending upgrade (called from a background compilation thread).
    pub fn set_pending_upgrade(&self, tier: CompileTier, func: CalcDerivsFunc) {
        let mut pending = self.pending_upgrade.lock().unwrap();
        *pending = Some((tier, func));
    }

    pub fn tier_transition_count(&self) -> u32 {
        self.tier_transitions.load(Ordering::Relaxed)
    }
}

/// Scheduler that manages tiered compilation for a model.
pub struct TieredScheduler {
    policy: TieringPolicy,
    tiered_func: Arc<TieredFunction>,
    model_name: String,
    step_count: u64,
    profile: Option<ModelProfile>,
    tierup_in_flight: bool,
}

impl TieredScheduler {
    pub fn new(
        model_name: &str,
        initial_tier: CompileTier,
        func: CalcDerivsFunc,
        policy: TieringPolicy,
    ) -> Self {
        Self {
            policy,
            tiered_func: Arc::new(TieredFunction::new(initial_tier, func)),
            model_name: model_name.to_string(),
            step_count: 0,
            profile: None,
            tierup_in_flight: false,
        }
    }

    pub fn with_profile(mut self, profile: ModelProfile) -> Self {
        self.profile = Some(profile);
        self.policy.profile_available = true;
        self
    }

    pub fn tiered_func(&self) -> &Arc<TieredFunction> {
        &self.tiered_func
    }

    /// Called at each simulation step boundary.
    pub fn on_step(&mut self) -> CalcDerivsFunc {
        self.step_count += 1;

        let prev_tier = self.tiered_func.current_tier();
        if self.tiered_func.try_apply_upgrade() {
            let current_tier = self.tiered_func.current_tier();
            if current_tier != prev_tier {
                if let Ok(mut events) = tiered_events_store().lock() {
                    events.push(TieredCompilationEvent {
                        from_tier: prev_tier,
                        to_tier: current_tier,
                        step_number: self.step_count,
                    });
                }
            }
            self.tierup_in_flight = false;
        }

        if !self.tierup_in_flight {
            let current = self.tiered_func.current_tier();
            if let Some(target_tier) = self.policy.should_tierup(current, self.step_count) {
                self.request_background_tierup(target_tier);
            }
        }

        self.tiered_func.get_func()
    }

    fn request_background_tierup(&mut self, target_tier: CompileTier) {
        self.tierup_in_flight = true;
        let tf = Arc::clone(&self.tiered_func);
        let model = self.model_name.clone();
        let profile = self.profile.clone();
        let force_skip_cf = self.policy.force_adaptive_skip_const_fold;
        let force_skip_dce = self.policy.force_adaptive_skip_eq_dce;

        if let Err(e) = std::thread::Builder::new()
            .name(format!("tierup-{}", target_tier as u32))
            .stack_size(4 * 1024 * 1024)
            .spawn(move || {
                eprintln!(
                    "[tiered] background tier-up to {:?} for {}",
                    target_tier, model
                );
                let t0 = Instant::now();

                if target_tier.enable_speculation() {
                    if let Some(ref prof) = profile {
                        crate::jit::speculation::init_global_registry(prof);
                    }
                }

                std::env::set_var(
                    "RUSTMODLICA_CRANELIFT_OPT_LEVEL",
                    target_tier.cranelift_opt_level(),
                );
                if target_tier.skip_const_fold() || force_skip_cf {
                    std::env::set_var("RUSTMODLICA_CONST_FOLD", "0");
                }
                if target_tier.skip_eq_dce() || force_skip_dce {
                    std::env::set_var("RUSTMODLICA_EQ_DCE", "0");
                }

                let mut compiler = crate::Compiler::new();
                compiler.options_mut().quiet = true;
                if target_tier.enable_speculation() {
                    compiler.options_mut().dual_compile = true;
                }
                match compiler.compile(&model) {
                    Ok(crate::CompileOutput::Simulation(artifacts)) => {
                        tf.set_pending_upgrade(target_tier, artifacts.calc_derivs);
                        eprintln!(
                            "[tiered] tier-up to {:?} ready in {:.1}ms",
                            target_tier,
                            t0.elapsed().as_millis()
                        );
                    }
                    Ok(_) => {
                        eprintln!("[tiered] tier-up: non-simulation output, skipping");
                    }
                    Err(e) => {
                        eprintln!("[tiered] tier-up failed: {}", e);
                    }
                }
            })
        {
            eprintln!("[tiered] thread spawn failed: {}", e);
            self.tierup_in_flight = false;
        }
    }
}
