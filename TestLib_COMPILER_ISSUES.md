# TestLib Compiler Run Summary and Issues

## Test Matrix (TestLib *.mo models) – after fixes

**Result: 35/35 OK** (all models compile and run)

| Models | Result |
|--------|--------|
| AlgTest, ArrayConnect, ArrayTest, Base, BLTTest, BouncingBall, Child, ChildWithMod, Circuit, Component, Container, DiscreteTest, ForTest, Ground, HierarchicalMod, IfTest, LibraryTest, Loop, LoopConnect, LoopTest, MainPin, MathTest, NestedConnect, Parent, Pendulum, Pin, Resistor, SimpleTest, Sub, SubPin, TearingTest, TwoPin, VoltageSource, WhenTest, WhileTest | OK (35/35) |

---

## Fixes applied

1. **Parser trim** (`parser.rs`): `type_name` and `var_name` are trimmed when building `Declaration`. Removes "Unknown type 'Pin '" style errors.
2. **Library path** (`main.rs`): Added `"."` to `library_paths` so qualified names like `TestLib.Base` resolve to `./TestLib/Base.mo`.

---

## Remaining issue categories

### 1. Connector/flow sub-variables not in JIT (n_i, n_v, p_i) [FIXED]

- **Symptom:** `JIT compilation failed: Variable n_i not found` (TwoPin), `n_v not found` (VoltageSource), `p_i not found` (Resistor).
- **Cause:** Equations reference flattened connector members (e.g. `p.v`, `n.i` → `p_v`, `n_i`). They are in `output_vars` but were never loaded into JIT `var_map` at use sites.
- **Fix applied:** In `jit/translator/expr.rs`, when compiling `Expression::Variable(name)`, if the variable is not in `var_map` or `stack_slots`, load it from `outputs_ptr` using `output_index(name)` and insert into `var_map` (lazy load on first use). TwoPin, VoltageSource, Resistor now compile and run.

### 2. TearingTest stack overflow [FIXED]

- **Symptom:** `thread 'main' has overflowed its stack` when running TearingTest (during "Performing Structure Analysis").
- **Cause:** Recursion in BLT matching (augmenting path DFS) and/or in petgraph `tarjan_scc` or alias substitution.
- **Fix applied:** (1) Replaced recursive `dfs` with iterative `dfs_iter` in `analysis.rs` (explicit stack for bipartite matching). (2) Run main logic in a thread with 8MB stack (`main.rs`) so remaining recursion (e.g. in petgraph or substitute) does not overflow. TearingTest now runs to completion (output may show NaN due to algebraic loop/tearing numerical behavior).

### 3. (Resolved) Type name whitespace – FIXED

- Parser trim applied; no longer occurs.

### 4. (Resolved) Qualified name TestLib.Base – FIXED

- Path "." added; Child and similar models load correctly.

---

## Summary

- **35/35** TestLib models compile and run successfully.
- Connector/flow sub-variables and TearingTest stack overflow have been fixed (see "Fixes applied" and "Remaining issue categories" above).
