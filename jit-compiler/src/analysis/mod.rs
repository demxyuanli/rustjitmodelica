pub(crate) mod blt;
pub mod derivative;
mod expression_utils;
mod initial;
pub mod provenance;
mod solvable_sparsity;
mod variable_collection;

#[allow(unused_imports)]
pub use blt::{sort_algebraic_equations, BlockCausalityInfo, SortAlgebraicResult};
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
pub use variable_collection::{contains_var, extract_unknowns, extract_unknowns_from_algorithm};
pub use solvable_sparsity::{
    build_solvable_block_sparse_pattern, SolvableBlockSparsePattern, SolvableBlockSparseStats,
};
pub use provenance::{
    provenance_index_from_flat_model, ChangeImpact, ImpactAnalysisResult, ProvenanceBuilder,
    ProvenanceIndex, ProvenanceStats,
};

#[derive(Clone, Default)]
pub struct AnalysisOptions {
    #[allow(dead_code)]
    pub index_reduction_method: String,
    pub tearing_method: String,
    pub quiet: bool,
}
