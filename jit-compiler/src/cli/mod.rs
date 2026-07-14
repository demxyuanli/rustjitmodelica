use rustmodlica::error;

pub(crate) type RunError = error::AppError;

mod args;
mod cache_invalidate;
mod cache_ops;
mod cache_stats;
mod event_scan;
mod perf_json;
mod precompile;
mod repl;
mod run;
mod validate_json;
mod validate_stdio;

pub use run::run;
