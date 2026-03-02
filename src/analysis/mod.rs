mod variable_collection;
mod derivative;
mod expression_utils;
mod blt;
mod initial;

#[allow(unused_imports)]
pub use derivative::{normalize_der, collect_states_from_eq, find_unsupported_der_in_eq};
#[allow(unused_imports)]
pub use blt::{sort_algebraic_equations, SortAlgebraicResult};
#[allow(unused_imports)]
pub use initial::{
    analyze_initial_equations, order_initial_equations_for_application, InitialSystemInfo,
};
#[allow(unused_imports)]
pub use expression_utils::{
    make_num, make_mul, make_div, make_add, make_binary, expression_is_zero, partial_derivative,
    time_derivative,
};
#[allow(unused_imports)]
pub use variable_collection::contains_var;

#[derive(Clone, Default)]
pub struct AnalysisOptions {
    #[allow(dead_code)]
    pub index_reduction_method: String,
    pub tearing_method: String,
}
