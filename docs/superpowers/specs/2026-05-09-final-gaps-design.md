# Final Audit Gap Closure — Design Spec

> Date: 2026-05-09
> Gaps: Jacobian coloring (P3) + KINSOL init (P3) + KINSOL algebraic refine (P3)

---

## 1. Jacobian Structural Coloring

### Problem

Finite-difference Jacobian in SUNDIALS callbacks evaluates `f(y + delta*e_j)` per column. For n-state sparse system, this is n evaluations per Jacobian. With coloring, we group structurally independent columns and evaluate them simultaneously — reducing evaluations from n to c (color count, typically 5-20 for sparse systems).

### Algorithm

**Distance-1 graph coloring:**

```
Input:  SparseJacobianPattern { row_ptr[n+1], col_idx[nnz], n }
Output: Vec<Vec<usize>>  // groups of column indices, each group = one color

1. Build conflict graph adjacency:
   For each column j, its neighbors are columns that share a non-zero row.
   For each row i with non-zeros at columns [a, b, c]:
     Add edges between all pairs in [a, b, c].

2. Greedy coloring:
   colors[0..n-1] = -1 (unassigned)
   For each column j (sorted by degree descending):
     Find the smallest non-negative integer not used by any neighbor.
     Assign that color to column j.

3. Group by color:
   groups[color].push(j)
```

### Integration

Modify `cv_jac` and `ida_jac` callbacks in `simulation/sundials/mod.rs`:
- Before the Jacobian loop, compute coloring groups
- Instead of `for j in 0..n: perturb y[j], eval f(), fill column j`
- Use `for each color group: perturb y[group], eval f(), fill columns[group]`

For dense Jacobians (nnz > 80% of n²), skip coloring and use per-column evaluation.

### Files

| File | Change |
|------|--------|
| `analysis/jacobian_coloring.rs` | New — graph coloring algorithm |
| `analysis/mod.rs` | Register module |
| `analysis/blt/types.rs` or new | Store coloring groups in sparse pattern |
| `simulation/sundials/mod.rs` | Colored Jacobian evaluation in cv_jac/ida_jac |
| `simulation/sundials/run_common.rs` | Pass coloring data in user_data |

### Tests

- 3x3 tridiagonal → 2 colors
- 4x4 dense → 4 colors (no benefit, skip coloring)
- Identical sparsity pattern as existing `build_sparse_jacobian_pattern` test

### Effort: 1-2 days

---

## 2. KINSOL Algebraic Initialization (t=0)

### Problem

`recover_newton_at_t0()` has a 5-phase fallback (homotopy → perturbation → random → geometric → projection) but all phases use the same `calc_derivs` Newton-style evaluation. For stiff algebraic initialization, KINSOL's Newton+linesearch+SPGMR converges faster and more reliably.

### Integration

In `simulation/newton_recovery.rs:recover_newton_at_t0()`, add Phase 0 before existing phases:

```rust
#[cfg(feature = "sundials")]
if try_kinsol_init(states, n, calc_derivs, time, params, ...) {
    return true; // KINSOL converged, skip remaining phases
}
// Fall through to existing 5-phase recovery
```

### KINSOL residual function

Build a residual `F(u) = 0` where `u` is the state vector. The residual evaluates `calc_derivs` at the initial time with the candidate state values, and returns `derivs` as the residual (at t=0 with steady-state assumption, we want `derivs ≈ 0`).

### Files

| File | Change |
|------|--------|
| `simulation/newton_recovery.rs` | Add `try_kinsol_init()` + Phase 0 call |

### Tests

- Model with algebraic initial equations that fails current Newton but converges with KINSOL
- Model that works with existing Newton (KINSOL skipped or also succeeds)
- `#[cfg(not(feature = "sundials"))]` — no change in behavior

### Effort: 0.5-1 day

---

## 3. KINSOL Algebraic Refinement (Event Iteration)

### Problem

In `run_event_iteration_at_time()`, when the system is purely algebraic (`states.is_empty()`), a fixed-point loop (max 15 iterations) refines the algebraic outputs. For stiff algebraic systems, this may not converge. KINSOL can replace this.

### Integration

In `simulation/events.rs:run_event_iteration_at_time()`, inside the algebraic refinement path (where `do_alg_iter` is true), replace the fixed-point loop with a single KINSOL call:

```rust
#[cfg(feature = "sundials")]
if do_alg_iter && states.is_empty() {
    if try_kinsol_algebraic_refine(outputs, n_outputs, ...) {
        break; // KINSOL converged
    }
}
// Fall back to existing fixed-point loop
```

### KINSOL residual

The algebraic system is defined by the tearing equations. The residual `F(outputs) = 0` is evaluated via `calc_derivs` with the current output values. KINSOL solves for outputs that satisfy the tearing constraints.

### Files

| File | Change |
|------|--------|
| `simulation/events.rs` | KINSOL algebraic refinement in event loop |

### Tests

- Pure algebraic model (no states) with tearing — verify KINSOL converges
- Same model with `sundials` disabled — existing fixed-point path unchanged

### Effort: 0.5 day

---

## Implementation Order

1. Jacobian coloring (new algorithm, most impactful)
2. KINSOL init (simple integration, wide coverage)
3. KINSOL algebraic refine (narrower scope, quick add)

Total: ~2-3 days. Each independently testable.
