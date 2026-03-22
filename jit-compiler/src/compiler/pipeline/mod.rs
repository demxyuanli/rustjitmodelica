mod algorithms;
mod analyze;
mod classify;
mod frontend;
mod geometric_default;
mod normalize_eq;
mod trace;
mod types;

pub use geometric_default::geometric_default_for_name;
pub(crate) use algorithms::{build_runtime_algorithms, collect_newton_tearing_var_names};
pub(crate) use analyze::analyze_equations;
pub(crate) use classify::classify_variables;
pub(crate) use frontend::flatten_and_inline;
pub(crate) use trace::stage_trace_enabled;
