#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewtonLinearizationKind {
    Dense,
    Csr,
}

#[derive(Clone, Debug, Default)]
pub struct NewtonLinearizationStats {
    pub residual_count: usize,
    pub nnz: usize,
}

#[derive(Clone, Debug)]
pub enum NewtonLinearizedSystem {
    Dense(NewtonLinearizationStats),
    Csr(NewtonLinearizationStats),
}

impl NewtonLinearizedSystem {
    pub fn kind(&self) -> NewtonLinearizationKind {
        match self {
            Self::Dense(_) => NewtonLinearizationKind::Dense,
            Self::Csr(_) => NewtonLinearizationKind::Csr,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewtonPathPreference {
    Auto,
    DenseOnly,
    SparseOnly,
}

pub fn parse_newton_path_preference() -> NewtonPathPreference {
    // Prefer explicit path override; fall back to production sparse policy env.
    let raw = std::env::var("RUSTMODLICA_NEWTON_PATH")
        .ok()
        .or_else(|| std::env::var("RUSTMODLICA_NEWTON_SPARSE_POLICY").ok())
        .unwrap_or_else(|| "auto".to_string());
    let s = raw.trim().to_ascii_lowercase();
    match s.as_str() {
        "dense" | "dense_only" => NewtonPathPreference::DenseOnly,
        "sparse" | "csr" | "sparse_only" => NewtonPathPreference::SparseOnly,
        _ => NewtonPathPreference::Auto,
    }
}

pub fn preference_to_sparse_policy(
    preference: NewtonPathPreference,
) -> crate::solvable_limits::NewtonSparsePolicy {
    match preference {
        NewtonPathPreference::DenseOnly => crate::solvable_limits::NewtonSparsePolicy::Dense,
        NewtonPathPreference::SparseOnly => crate::solvable_limits::NewtonSparsePolicy::Sparse,
        NewtonPathPreference::Auto => crate::solvable_limits::NewtonSparsePolicy::Auto,
    }
}
