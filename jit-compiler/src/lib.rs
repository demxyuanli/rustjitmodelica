// Library target for rustmodlica: used by CLI binary and by ModAI IDE (Tauri).

pub mod analysis;
pub mod annotation;
pub mod api;
pub mod ast;
pub mod cache;
pub mod backend_dae;
pub mod compiler;
pub mod condenser;
pub mod diag;
pub mod equation_graph;
pub mod equation_graph_inc;
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
    Artifacts, CompileOutput, CompileStopPhase, CompiledModel, Compiler, CompilerOptions,
    ValidationAnalyzedSummary,
};
pub use diag::{ParseErrorInfo, SourceLocation, WarningInfo};
pub use string_intern::{StringInterner, VarId};
pub use equation_graph::{EquationGraph, EquationGraphEdge, EquationGraphNode};
pub use equation_graph_inc::{build_or_update_equation_graph, DirtySet, NodeKey};
pub use loader::{LoadError, ModelLoader};
pub use simulation::{run_simulation, run_simulation_collect, runtime_perf_counters, SimulationResult};
pub use api::{
    affected_models_for_changed_files, analyze_change_impact, analyze_instance_change_impact,
    incremental_codegen_worthwhile_hint, provenance_index_for_flat_model,
};
pub use query_db::salsa_session::salsa_process_db_stats;
pub use cache::msl_pack::manifest::read_manifest as read_msl_pack_manifest;
pub use cache::msl_pack::tree_digest::compute_msl_tree_digest;
pub use cache::msl_pack::version::read_msl_version_label;
pub use flatten::cache_sqlite::sqlite_connection_pool_clear;
pub use analysis::{ImpactAnalysisResult, ProvenanceIndex};
#[cfg(feature = "sundials")]
pub use simulation::{
    kinsol_solve_square_spgmr, parse_linsol_env, KinResidualFn, KinsolCallbackPack, SundialsLinSolKind,
};
