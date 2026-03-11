// IR4-4: Sparse linear solve. CSR format; solve A*x = b via dense fallback for small n.
// For large n a proper sparse LU or iterative solver can be substituted.
#![allow(dead_code)]

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
    pub fn solve_in_place(&self, b: &[f64], x: &mut [f64]) -> Result<(), ()> {
        let n = self.n;
        if b.len() < n || x.len() < n {
            return Err(());
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
        if max_val < 1e-14 {
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
    let mut row_ptr = vec![0usize; n + 1];
    for (i, _, _) in triples {
        if *i < n {
            row_ptr[*i + 1] += 1;
        }
    }
    for i in 1..=n {
        row_ptr[i] += row_ptr[i - 1];
    }
    let nnz = row_ptr[n];
    let mut col_idx = vec![0usize; nnz];
    let mut values = vec![0.0; nnz];
    let mut pos = row_ptr.clone();
    for (i, j, v) in triples {
        if *i < n && *j < n {
            let p = pos[*i];
            if p < row_ptr[*i + 1] {
                col_idx[p] = *j;
                values[p] = *v;
                pos[*i] += 1;
            }
        }
    }
    CsrMatrix {
        n,
        row_ptr,
        col_idx,
        values,
    }
}
