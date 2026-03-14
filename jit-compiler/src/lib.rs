// Library target for rustmodlica: used by CLI binary and by ModAI IDE (Tauri).

pub mod annotation;
pub mod ast;
pub mod backend_dae;
pub mod unparse;
pub mod expr_eval;
pub mod i18n;
pub mod diag;
pub mod parser;
pub mod loader;
pub mod flatten;
pub mod analysis;
pub mod jit;
pub mod simulation;
pub mod compiler;
pub mod fmi;
pub mod script;
pub mod solver;
pub mod sparse_solve;
pub mod equation_graph;
pub mod api;

pub use compiler::{Artifacts, CompileOutput, Compiler, CompilerOptions};
pub use simulation::{run_simulation, run_simulation_collect, SimulationResult};
pub use loader::{LoadError, ModelLoader};
pub use diag::{ParseErrorInfo, SourceLocation, WarningInfo};
pub use equation_graph::{EquationGraph, EquationGraphEdge, EquationGraphNode};
