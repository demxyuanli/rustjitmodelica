mod assign;
mod compile_equation_impl;
mod helpers;
mod linearized;
mod solvable_assert;
mod solvable_general_dense;
mod solvable_general_sparse;
mod solvable;
mod solvable_tearing;

pub(crate) use solvable_general_sparse::solvable_block_uses_sparse_jacobian_path;
pub use compile_equation_impl::compile_equation;
