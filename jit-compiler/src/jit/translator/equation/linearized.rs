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
    let Ok(raw) = std::env::var("RUSTMODLICA_NEWTON_PATH") else {
        return NewtonPathPreference::Auto;
    };
    let s = raw.trim().to_ascii_lowercase();
    match s.as_str() {
        "dense" | "dense_only" => NewtonPathPreference::DenseOnly,
        "sparse" | "csr" | "sparse_only" => NewtonPathPreference::SparseOnly,
        _ => NewtonPathPreference::Auto,
    }
}
