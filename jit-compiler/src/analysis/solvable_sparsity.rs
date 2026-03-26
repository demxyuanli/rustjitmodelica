use crate::analysis::contains_var;
use crate::ast::Expression;

#[derive(Debug, Clone, Default)]
pub struct SolvableBlockSparseStats {
    pub n: usize,
    pub nnz: usize,
    pub density: f64,
    pub row_ptr_len: usize,
    pub col_idx_len: usize,
}

#[derive(Debug, Clone)]
pub struct SolvableBlockSparsePattern {
    pub row_ptr: Vec<i32>,
    pub col_idx: Vec<i32>,
    pub entries: Vec<(usize, usize)>,
}

impl SolvableBlockSparsePattern {
    pub fn stats(&self, n: usize) -> SolvableBlockSparseStats {
        let total = n.saturating_mul(n);
        let nnz = self.entries.len();
        SolvableBlockSparseStats {
            n,
            nnz,
            density: if total == 0 {
                0.0
            } else {
                nnz as f64 / total as f64
            },
            row_ptr_len: self.row_ptr.len(),
            col_idx_len: self.col_idx.len(),
        }
    }
}

pub fn build_solvable_block_sparse_pattern(
    unknowns: &[String],
    residuals: &[Expression],
) -> Option<SolvableBlockSparsePattern> {
    let n = residuals.len();
    if n == 0 || unknowns.len() < n {
        return None;
    }
    let mut row_ptr = Vec::with_capacity(n + 1);
    let mut col_idx = Vec::new();
    let mut entries = Vec::new();
    row_ptr.push(0);
    for (row, residual) in residuals.iter().enumerate() {
        let row_start = col_idx.len();
        let mut seen = vec![false; n];
        for (col, unknown) in unknowns.iter().take(n).enumerate() {
            if !seen[col] && contains_var(residual, unknown) {
                seen[col] = true;
                col_idx.push(col as i32);
                entries.push((row, col));
            }
        }
        if col_idx.len() == row_start {
            return None;
        }
        row_ptr.push(col_idx.len() as i32);
    }
    Some(SolvableBlockSparsePattern {
        row_ptr,
        col_idx,
        entries,
    })
}
