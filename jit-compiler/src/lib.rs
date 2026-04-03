// Library target for rustmodlica: used by CLI binary and by ModAI IDE (Tauri).

pub mod analysis;
pub mod annotation;
pub mod api;
pub mod ast;
pub mod cache;
pub mod backend_dae;
pub mod compiler;
pub mod diag;
pub mod equation_graph;
pub mod error;
pub mod expr_eval;
pub mod flatten;
pub mod fmi;
pub use fmi::FmiExportOptions;
pub mod i18n;
pub mod instantiate;
pub mod jit;
mod loader_compat;
mod math_fft;
mod modelica_random;
pub mod newton_policy;
pub mod loader;
pub mod parser;
pub mod script;
pub mod simulation;
pub mod solver;
pub mod solvable_limits;
pub mod sparse_solve;
pub mod string_intern;
pub mod query_db;
pub mod unparse;

pub use compiler::{
    Artifacts, CompileOutput, CompileStopPhase, Compiler, CompilerOptions, ValidationAnalyzedSummary,
};
pub use diag::{ParseErrorInfo, SourceLocation, WarningInfo};
pub use string_intern::{StringInterner, VarId};
pub use equation_graph::{EquationGraph, EquationGraphEdge, EquationGraphNode};
pub use loader::{LoadError, ModelLoader};
pub use simulation::{run_simulation, run_simulation_collect, runtime_perf_counters, SimulationResult};
#[cfg(feature = "sundials")]
pub use simulation::{
    kinsol_solve_square_spgmr, parse_linsol_env, KinResidualFn, KinsolCallbackPack, SundialsLinSolKind,
};
