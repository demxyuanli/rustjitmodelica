//! Distance-1 structural Jacobian coloring for finite-difference acceleration.
//!
//! Groups structurally independent columns (columns that don't share any non-zero
//! row) into color groups. Columns of the same color can be perturbed simultaneously
//! in finite-difference Jacobian computation, reducing function evaluations from n
//! to c (color count).

/// Sparse Jacobian pattern in CSR-like format.
#[derive(Debug, Clone)]
pub struct ColoringPattern {
    /// Number of columns (and rows — square Jacobian).
    pub n: usize,
    /// CSR row pointers: row_ptr[i]..row_ptr[i+1] are column indices in row i.
    pub row_ptr: Vec<usize>,
    /// CSR column indices.
    pub col_idx: Vec<usize>,
}

/// Result of distance-1 graph coloring. Each inner Vec contains column indices
/// that can be perturbed simultaneously.
pub type ColorGroups = Vec<Vec<usize>>;

impl ColoringPattern {
    /// Create from sparse Jacobian pattern (already validated as square).
    pub fn new(n: usize, row_ptr: Vec<usize>, col_idx: Vec<usize>) -> Self {
        Self { n, row_ptr, col_idx }
    }

    /// Check if Jacobian is too dense for coloring to help. Threshold: >60% fill.
    pub fn is_too_dense(&self) -> bool {
        let max_nnz = self.n.saturating_mul(self.n);
        if max_nnz == 0 {
            return true;
        }
        let fill_ratio = self.col_idx.len() as f64 / max_nnz as f64;
        fill_ratio > 0.6
    }

    /// Compute distance-1 coloring groups.
    /// Returns None if coloring is not beneficial (dense Jacobian).
    pub fn compute_coloring(&self) -> Option<ColorGroups> {
        if self.n <= 1 || self.is_too_dense() {
            return None;
        }

        // Build conflict graph adjacency list
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); self.n];

        for i in 0..self.n {
            if i + 1 >= self.row_ptr.len() {
                continue;
            }
            let row_start = self.row_ptr[i];
            let row_end = self.row_ptr[i + 1];
            // For each row, every pair of columns in that row are neighbors
            for a_idx in row_start..row_end {
                let a = self.col_idx[a_idx];
                if a >= self.n {
                    continue;
                }
                for b_idx in (a_idx + 1)..row_end {
                    let b = self.col_idx[b_idx];
                    if b >= self.n || a == b {
                        continue;
                    }
                    // Add edge in both directions (undirected)
                    if !adj[a].contains(&b) {
                        adj[a].push(b);
                    }
                    if !adj[b].contains(&a) {
                        adj[b].push(a);
                    }
                }
            }
        }

        // Sort columns by degree descending (Welsh-Powell heuristic)
        let mut columns: Vec<usize> = (0..self.n).collect();
        columns.sort_by_key(|&j| std::cmp::Reverse(adj[j].len()));

        // Greedy coloring
        let mut colors: Vec<i32> = vec![-1; self.n];
        let mut max_color: i32 = -1;

        for &col in &columns {
            // Find colors used by neighbors
            let mut used_colors: Vec<bool> = vec![false; (max_color + 2) as usize];
            for &neighbor in &adj[col] {
                let nc = colors[neighbor];
                if nc >= 0 {
                    let nc_u = nc as usize;
                    if nc_u < used_colors.len() {
                        used_colors[nc_u] = true;
                    }
                }
            }
            // Find first available color
            let mut assigned = -1i32;
            for (c_idx, &used) in used_colors.iter().enumerate() {
                if !used {
                    assigned = c_idx as i32;
                    break;
                }
            }
            if assigned < 0 {
                assigned = used_colors.len() as i32;
            }
            colors[col] = assigned;
            if assigned > max_color {
                max_color = assigned;
            }
        }

        // Group by color
        let num_colors = (max_color + 1) as usize;
        if num_colors >= self.n {
            // No compression achieved
            return None;
        }
        let mut groups: ColorGroups = vec![Vec::new(); num_colors];
        for (col, &color) in colors.iter().enumerate() {
            if color >= 0 {
                groups[color as usize].push(col);
            }
        }

        Some(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tridiagonal_5x5() {
        // Tridiagonal 5x5: fill ratio 13/25 = 52% (below 60% threshold)
        // x x 0 0 0
        // x x x 0 0
        // 0 x x x 0
        // 0 0 x x x
        // 0 0 0 x x
        let n = 5;
        let row_ptr = vec![0, 2, 5, 8, 11, 13];
        let col_idx = vec![0,1, 0,1,2, 1,2,3, 2,3,4, 3,4];
        let pattern = ColoringPattern::new(n, row_ptr, col_idx);
        let groups = pattern.compute_coloring().unwrap();
        // Tricolor should work: columns {0,2,4} and {1,3}
        assert!(groups.len() <= 3);
        // Verify no two columns in same group share a row
        for group in &groups {
            for (i, &a) in group.iter().enumerate() {
                for &b in &group[i + 1..] {
                    for r in 0..n {
                        let start = pattern.row_ptr[r];
                        let end = pattern.row_ptr[r + 1];
                        let cols_in_row: Vec<usize> = pattern.col_idx[start..end].to_vec();
                        assert!(
                            !(cols_in_row.contains(&a) && cols_in_row.contains(&b)),
                            "Columns {} and {} in same color group share row {}",
                            a, b, r
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_dense_4x4_skips_coloring() {
        let n = 4;
        let row_ptr = vec![0, 4, 8, 12, 16];
        let col_idx = (0..16).map(|i| i % 4).collect();
        let pattern = ColoringPattern::new(n, row_ptr, col_idx);
        assert!(pattern.compute_coloring().is_none());
    }

    #[test]
    fn test_diagonal_only() {
        // Diagonal only: no column conflicts → 1 color
        let n = 3;
        let row_ptr = vec![0, 1, 2, 3];
        let col_idx = vec![0, 1, 2];
        let pattern = ColoringPattern::new(n, row_ptr, col_idx);
        let groups = pattern.compute_coloring().unwrap();
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn test_single_column() {
        let n = 1;
        let row_ptr = vec![0, 1];
        let col_idx = vec![0];
        let pattern = ColoringPattern::new(n, row_ptr, col_idx);
        assert!(pattern.compute_coloring().is_none()); // n <= 1
    }
}
