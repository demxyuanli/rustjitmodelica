//! Leyden-inspired condenser architecture for Modelica hybrid compilation.
//!
//! Condensers shift computation from runtime to earlier phases (build-time, install-time,
//! first-run, warmup, hot-reload). Each condenser is a composable, meaning-preserving
//! transformer that produces cached artifacts for downstream stages.

mod flatten_condenser;
mod analysis_condenser;
mod codegen_condenser;
mod symbolic_condenser;
mod param_condenser;
pub mod profile_data;
pub mod training_run;
pub mod stats;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Instant;

pub use flatten_condenser::FlattenCondenser;
pub use analysis_condenser::AnalysisCondenser;
pub use codegen_condenser::CodegenCondenser;
pub use symbolic_condenser::SymbolicCondenser;
pub use param_condenser::ParamCondenser;

/// Phase in which a condenser operates (maps to Leyden's "time-shifting" concept).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CondenserPhase {
    /// Ahead-of-time at library build / cargo build.
    BuildTime,
    /// At MSL or library install time.
    InstallTime,
    /// First compilation of a specific model.
    FirstRun,
    /// Background warmup after a successful compile.
    Warmup,
    /// Parameter-only change (structural cache reuse).
    HotReload,
}

impl fmt::Display for CondenserPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CondenserPhase::BuildTime => write!(f, "build_time"),
            CondenserPhase::InstallTime => write!(f, "install_time"),
            CondenserPhase::FirstRun => write!(f, "first_run"),
            CondenserPhase::Warmup => write!(f, "warmup"),
            CondenserPhase::HotReload => write!(f, "hot_reload"),
        }
    }
}

/// Output produced by a condenser application.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CondenserOutput {
    pub condenser_name: String,
    pub phase: CondenserPhase,
    pub artifacts_written: u32,
    pub cache_hits: u32,
    pub elapsed_us: u64,
    pub detail: Option<String>,
}

/// Errors that can occur during condenser application.
#[derive(Debug)]
pub enum CondenserError {
    CacheUnavailable(String),
    CompilationFailed(String),
    IoError(std::io::Error),
    Internal(String),
}

impl fmt::Display for CondenserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CondenserError::CacheUnavailable(msg) => write!(f, "cache unavailable: {}", msg),
            CondenserError::CompilationFailed(msg) => write!(f, "compilation failed: {}", msg),
            CondenserError::IoError(e) => write!(f, "I/O error: {}", e),
            CondenserError::Internal(msg) => write!(f, "internal: {}", msg),
        }
    }
}

impl std::error::Error for CondenserError {}

impl From<std::io::Error> for CondenserError {
    fn from(e: std::io::Error) -> Self {
        CondenserError::IoError(e)
    }
}

/// Context passed to condensers during application.
pub struct CondenserContext {
    pub model_name: String,
    pub lib_paths: Vec<PathBuf>,
    pub cache_root: Option<PathBuf>,
    pub phase: CondenserPhase,
    pub quiet: bool,
    /// Profile data from a previous training run (Phase 2).
    pub profile: Option<profile_data::ModelProfile>,
    /// Key-value store for inter-condenser data passing.
    pub artifacts: HashMap<String, Vec<u8>>,
}

impl CondenserContext {
    pub fn new(model_name: &str, phase: CondenserPhase) -> Self {
        let cache_root = crate::flatten::flatten_cache_dir();
        Self {
            model_name: model_name.to_string(),
            lib_paths: Vec::new(),
            cache_root,
            phase,
            quiet: false,
            profile: None,
            artifacts: HashMap::new(),
        }
    }

    pub fn with_lib_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.lib_paths = paths;
        self
    }

    pub fn with_profile(mut self, profile: profile_data::ModelProfile) -> Self {
        self.profile = Some(profile);
        self
    }

    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }
}

/// The core Condenser trait (analogous to Leyden's condenser concept).
///
/// Each condenser shifts work from a later phase to an earlier one, producing
/// cached artifacts that downstream stages can consume without recomputation.
pub trait Condenser: Send + Sync {
    fn name(&self) -> &str;

    fn phase(&self) -> CondenserPhase;

    /// Check if this condenser can be applied given the current context.
    fn can_apply(&self, ctx: &CondenserContext) -> bool;

    /// Apply the condenser, producing cached artifacts.
    fn apply(&self, ctx: &mut CondenserContext) -> Result<CondenserOutput, CondenserError>;
}

/// Registry of all available condensers, keyed by name.
pub struct CondenserRegistry {
    condensers: Vec<Box<dyn Condenser>>,
}

impl CondenserRegistry {
    pub fn new() -> Self {
        Self {
            condensers: Vec::new(),
        }
    }

    /// Create a registry pre-populated with all built-in condensers.
    pub fn with_builtins() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(FlattenCondenser));
        reg.register(Box::new(AnalysisCondenser));
        reg.register(Box::new(CodegenCondenser));
        reg.register(Box::new(SymbolicCondenser));
        reg.register(Box::new(ParamCondenser));
        reg
    }

    pub fn register(&mut self, condenser: Box<dyn Condenser>) {
        self.condensers.push(condenser);
    }

    /// Get all condensers applicable to the given phase, in registration order.
    pub fn for_phase(&self, phase: CondenserPhase) -> Vec<&dyn Condenser> {
        self.condensers
            .iter()
            .filter(|c| c.phase() == phase)
            .map(|c| c.as_ref())
            .collect()
    }

    /// Run all applicable condensers for the given context, collecting outputs.
    pub fn run_applicable(
        &self,
        ctx: &mut CondenserContext,
    ) -> Vec<Result<CondenserOutput, CondenserError>> {
        let phase = ctx.phase;
        let mut results = Vec::new();
        let applicable: Vec<usize> = self
            .condensers
            .iter()
            .enumerate()
            .filter(|(_, c)| c.phase() == phase && c.can_apply(ctx))
            .map(|(i, _)| i)
            .collect();
        for idx in applicable {
            let t0 = Instant::now();
            let res = self.condensers[idx].apply(ctx);
            let elapsed = t0.elapsed().as_micros() as u64;
            results.push(res.map(|mut out| {
                out.elapsed_us = elapsed;
                out
            }));
        }
        results
    }

    pub fn condenser_names(&self) -> Vec<&str> {
        self.condensers.iter().map(|c| c.name()).collect()
    }
}

impl Default for CondenserRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
