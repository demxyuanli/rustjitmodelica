// Library target for rustmodlica: used by CLI binary and by ModAI IDE (Tauri).

pub mod ast;
pub mod backend_dae;
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

pub use compiler::{Compiler, CompilerOptions, Artifacts, CompileOutput};
pub use simulation::{run_simulation, run_simulation_collect, SimulationResult};
pub use loader::{ModelLoader, LoadError};
pub use diag::{WarningInfo, SourceLocation, ParseErrorInfo};
