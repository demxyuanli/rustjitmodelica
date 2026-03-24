// IR4-4: Sparse linear solve (CSR). Large n + moderate nnz: faer sparse LU first; then in-tree sparse elimination; finally dense fallback.
use crate::solvable_limits::{csr_should_fallback_to_dense, csr_use_faer_sparse_lu};
use faer::linalg::solvers::Solve;
use faer::sparse::linalg::solvers::Lu;
use faer::sparse::{SparseRowMat, SymbolicSparseRowMat};
use faer::Mat;

const PIVOT_EPS: f64 = 1e-14;

fn sparse_debug_enabled() -> bool {
    std::env::var("RUSTMODLICA_NEWTON_SPARSE_DEBUG")
        .ok()
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            t == "1" || t == "true" || t == "on" || t == "yes"
        })
        .unwrap_or(false)
}

fn try_solve_csr_faer_lu(
    n: usize,
    row_ptr: &[usize],
    col_idx: &[usize],
    values: &[f64],
    b: &[f64],
    x: &mut [f64],
) -> Result<(), ()> {
    if row_ptr.len() != n + 1 || col_idx.len() != values.len() || b.len() < n || x.len() < n {
        return Err(());
    }
    let sym = SymbolicSparseRowMat::<usize, usize, usize>::new_unsorted_checked(
        n,
        n,
        row_ptr.to_vec(),
        None,
        col_idx.to_vec(),
    );
    let a = SparseRowMat::new(sym, values.to_vec());
    let lu: Lu<usize, f64> = a.sp_lu().map_err(|_| ())?;
    let mut rhs = Mat::<f64>::from_fn(n, 1, |i, _| b[i]);
    lu.solve_in_place(&mut rhs);
    for i in 0..n {
        x[i] = *rhs.get(i, 0);
    }
    Ok(())
}

/// Compressed sparse row: row_ptr.len() == n+1, col_idx and values have nnz entries.
#[derive(Debug, Clone)]
pub struct CsrMatrix {
    pub n: usize,
    pub row_ptr: Vec<usize>,
    pub col_idx: Vec<usize>,
    pub values: Vec<f64>,
}

impl CsrMatrix {
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Solve A * x = b; x overwritten. Returns Ok(()) or Err on singular.
    /// Fallback order: faer sparse LU -> in-tree sparse elimination -> dense.
    pub fn solve_in_place(&self, b: &[f64], x: &mut [f64]) -> Result<(), ()> {
        let n = self.n;
        if b.len() < n || x.len() < n {
            return Err(());
        }
        let nnz = self.nnz();
        let debug = sparse_debug_enabled();
        if csr_use_faer_sparse_lu(n, nnz) {
            if try_solve_csr_faer_lu(
                n,
                &self.row_ptr,
                &self.col_idx,
                &self.values,
                b,
                x,
            )
            .is_ok()
            {
                if debug {
                    eprintln!("[newton-sparse] csr solve path=faer_lu n={} nnz={}", n, nnz);
                }
                return Ok(());
            }
            if debug {
                eprintln!("[newton-sparse] csr solve fallback from=faer_lu reason=fail n={} nnz={}", n, nnz);
            }
        }

        // Sparse Gaussian elimination with partial pivoting using linked-list rows.
        // Each row is stored as a Vec<(col, val)> sorted by column.
        let mut rows: Vec<Vec<(usize, f64)>> = Vec::with_capacity(n);
        for i in 0..n {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];
            let mut row = Vec::with_capacity(end - start);
            for p in start..end {
                let j = self.col_idx[p];
                if j < n {
                    row.push((j, self.values[p]));
                }
            }
            rows.push(row);
        }
        let mut rhs: Vec<f64> = b[..n].to_vec();
        let mut perm: Vec<usize> = (0..n).collect();

        for k in 0..n {
            let mut pivot_row = k;
            let mut pivot_val = 0.0_f64;
            for i in k..n {
                let ri = perm[i];
                for &(col, val) in &rows[ri] {
                    if col == k && val.abs() > pivot_val.abs() {
                        pivot_val = val;
                        pivot_row = i;
                    }
                }
            }
            if pivot_val.abs() < PIVOT_EPS {
                if debug {
                    eprintln!("[newton-sparse] csr solve fallback from=sparse_elim reason=small_pivot k={}", k);
                }
                return self.solve_dense_fallback(n, b, x, debug);
            }
            perm.swap(k, pivot_row);

            let pr = perm[k];
            let inv = 1.0 / pivot_val;

            for i in (k + 1)..n {
                let ri = perm[i];
                let mut factor = 0.0_f64;
                let mut found = false;
                for &(col, val) in &rows[ri] {
                    if col == k {
                        factor = val * inv;
                        found = true;
                        break;
                    }
                }
                if !found || factor == 0.0 {
                    continue;
                }

                let pivot_entries: Vec<(usize, f64)> = rows[pr].clone();

                rows[ri].retain(|&(col, _)| col != k);

                for &(pcol, pval) in &pivot_entries {
                    if pcol == k {
                        continue;
                    }
                    let delta = factor * pval;
                    let mut updated = false;
                    for entry in rows[ri].iter_mut() {
                        if entry.0 == pcol {
                            entry.1 -= delta;
                            updated = true;
                            break;
                        }
                    }
                    if !updated {
                        rows[ri].push((pcol, -delta));
                    }
                }

                rhs[ri] -= factor * rhs[pr];
            }
        }

        // Back-substitution
        for k in (0..n).rev() {
            let pr = perm[k];
            let mut diag = 0.0_f64;
            let mut sum = rhs[pr];
            for &(col, val) in &rows[pr] {
                if col == k {
                    diag = val;
                } else if col > k {
                    sum -= val * x[col];
                }
            }
            if diag.abs() < PIVOT_EPS {
                if debug {
                    eprintln!("[newton-sparse] csr solve fallback from=back_sub reason=small_diag k={}", k);
                }
                return self.solve_dense_fallback(n, b, x, debug);
            }
            x[k] = sum / diag;
        }
        if debug {
            eprintln!("[newton-sparse] csr solve path=sparse_elim n={} nnz={}", n, nnz);
        }
        Ok(())
    }

    fn solve_dense_fallback(
        &self,
        n: usize,
        b: &[f64],
        x: &mut [f64],
        debug: bool,
    ) -> Result<(), ()> {
        let nnz = self.nnz();
        if debug {
            eprintln!(
                "[newton-sparse] csr solve path=dense_fallback n={} nnz={} trigger={}",
                n,
                nnz,
                if csr_should_fallback_to_dense(n, nnz) {
                    "policy_or_pivot"
                } else {
                    "pivot"
                }
            );
        }
        let mut dense = vec![0.0; n * n];
        for i in 0..n {
            for p in self.row_ptr[i]..self.row_ptr[i + 1] {
                let j = self.col_idx[p];
                if j < n {
                    dense[i * n + j] = self.values[p];
                }
            }
        }
        solve_dense_in_place(n, &mut dense, b, x)
    }
}

/// Dense solve A*x = b (row-major A), x overwritten. Returns Err(()) if singular.
pub fn solve_dense_in_place(n: usize, a: &mut [f64], b: &[f64], x: &mut [f64]) -> Result<(), ()> {
    if a.len() < n * n || b.len() < n || x.len() < n {
        return Err(());
    }
    x[..n].copy_from_slice(&b[..n]);
    for k in 0..n {
        let mut max_row = k;
        let mut max_val = a[k * n + k].abs();
        for i in (k + 1)..n {
            let v = a[i * n + k].abs();
            if v > max_val {
                max_val = v;
                max_row = i;
            }
        }
        if max_val < PIVOT_EPS {
            return Err(());
        }
        if max_row != k {
            for j in 0..n {
                a.swap(k * n + j, max_row * n + j);
            }
            x.swap(k, max_row);
        }
        let inv = 1.0 / a[k * n + k];
        a[k * n + k] = 1.0;
        for j in (k + 1)..n {
            a[k * n + j] *= inv;
        }
        x[k] *= inv;
        for i in 0..n {
            if i == k {
                continue;
            }
            let f = a[i * n + k];
            a[i * n + k] = 0.0;
            for j in (k + 1)..n {
                a[i * n + j] -= f * a[k * n + j];
            }
            x[i] -= f * x[k];
        }
    }
    Ok(())
}

/// Build CSR from (row, col, value) triples; assumes 0-based indices.
pub fn csr_from_triples(n: usize, triples: &[(usize, usize, f64)]) -> CsrMatrix {
    let mut compact: Vec<(usize, usize, f64)> = triples
        .iter()
        .copied()
        .filter(|(i, j, _)| *i < n && *j < n)
        .collect();
    compact.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let mut merged: Vec<(usize, usize, f64)> = Vec::with_capacity(compact.len());
    for (i, j, v) in compact {
        if let Some((li, lj, lv)) = merged.last_mut() {
            if *li == i && *lj == j {
                *lv += v;
                continue;
            }
        }
        merged.push((i, j, v));
    }
    let mut row_ptr = vec![0usize; n + 1];
    for (i, _, _) in &merged {
        row_ptr[*i + 1] += 1;
    }
    for i in 1..=n {
        row_ptr[i] += row_ptr[i - 1];
    }
    let nnz = row_ptr[n];
    let mut col_idx = vec![0usize; nnz];
    let mut values = vec![0.0; nnz];
    let mut pos = row_ptr.clone();
    for (i, j, v) in merged {
        let p = pos[i];
        if p < row_ptr[i + 1] {
            col_idx[p] = j;
            values[p] = v;
            pos[i] += 1;
        }
    }
    CsrMatrix {
        n,
        row_ptr,
        col_idx,
        values,
    }
}
