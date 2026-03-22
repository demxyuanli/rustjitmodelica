// Library target for rustmodlica: used by CLI binary and by ModAI IDE (Tauri).

pub mod analysis;
pub mod annotation;
pub mod api;
pub mod ast;
pub mod backend_dae;
pub mod compiler;
pub mod diag;
pub mod equation_graph;
pub mod error;
pub mod expr_eval;
pub mod flatten;
pub mod fmi;
pub mod i18n;
pub mod jit;
mod loader_compat;
pub mod loader;
pub mod parser;
pub mod script;
pub mod simulation;
pub mod solver;
pub mod sparse_solve;
pub mod string_intern;
pub mod unparse;

pub use compiler::{Artifacts, CompileOutput, Compiler, CompilerOptions};
pub use diag::{ParseErrorInfo, SourceLocation, WarningInfo};
pub use string_intern::{StringInterner, VarId};
pub use equation_graph::{EquationGraph, EquationGraphEdge, EquationGraphNode};
pub use loader::{LoadError, ModelLoader};
pub use simulation::{run_simulation, run_simulation_collect, SimulationResult};
