mod blt;
pub mod derivative;
mod expression_utils;
mod initial;
mod variable_collection;

#[allow(unused_imports)]
pub use blt::{sort_algebraic_equations, SortAlgebraicResult};
#[allow(unused_imports)]
pub use derivative::{collect_states_from_eq, find_unsupported_der_in_eq, normalize_der};
#[allow(unused_imports)]
pub use expression_utils::{
    expression_is_zero, make_add, make_binary, make_div, make_mul, make_num, partial_derivative,
    time_derivative,
};
#[allow(unused_imports)]
pub use initial::{
    analyze_initial_equations, order_initial_equations_for_application, InitialSystemInfo,
};
#[allow(unused_imports)]
pub(crate) use variable_collection::collect_vars_expr;
pub use variable_collection::{contains_var, extract_unknowns};

#[derive(Clone, Default)]
pub struct AnalysisOptions {
    #[allow(dead_code)]
    pub index_reduction_method: String,
    pub tearing_method: String,
    pub quiet: bool,
}
