# Rust Modelica Compiler - Optimization Plan

## 1. System Overview

The compiler pipeline: **Parse (Pest)** -> **Load/Flatten** -> **Variable classification** -> **Normalize der()** -> **BLT (Block Lower Triangular)** -> **JIT (Cranelift)** -> **Simulation (RK4 + events)**.

| Component   | File(s)           | Lines | Role |
|------------|-------------------|-------|------|
| Entry      | main.rs           | 62    | CLI, compile(), run_simulation() |
| Compiler   | compiler.rs       | 371   | Pipeline orchestration, var classification |
| Parser     | parser.rs         | 693   | Pest grammar, AST construction |
| AST        | ast.rs            | 94    | Model, Equation, Expression, Declaration |
| Loader     | loader.rs         | 65    | Library paths, file resolution, parse cache |
| Flatten    | flatten/*         | ~856  | Inheritance, instantiation, connections, array/For/When expansion |
| Analysis   | analysis.rs       | 626   | normalize_der, BLT, alias elimination, matching, Tarjan SCC |
| JIT        | jit/*             | ~1273 | Cranelift JIT, equation/algorithm/expr -> IR, native symbols |
| Solver     | solver.rs         | 194   | RK4 integration |
| Simulation | simulation.rs    | 196   | Event loop, JIT calls, CSV output |

Unused in module tree: `codegen.rs` (AOT), `main_helpers.rs`.

---

## 2. Optimization Recommendations

### 2.1 Code Structure (Refactor by Size)

- **jit/translator.rs (867 lines)**  
  Over 800 lines; approaching 1000.  
  **Action:** Split into submodules, e.g.:
  - `translator/expr.rs`: `compile_expression` and expression cases
  - `translator/equation.rs`: `compile_equation` and equation cases
  - `translator/algorithm.rs`: `compile_algorithm_stmt` and algorithm cases  
  Keep `mod.rs` as a thin facade that re-exports and delegates.

- **parser.rs (693 lines)**  
  Close to 800-line guideline.  
  **Action:** Extract expression/declaration parsing into `parser/expr.rs` and `parser/decl.rs` (or similar) to keep single-file size under 800.

- **analysis.rs (626 lines)**  
  Monitor; consider splitting BLT + alias elimination into `analysis/blt.rs` and `analysis/alias.rs` if it grows.

### 2.2 Compiler: Variable Lookup (Performance)

**Issue:** Repeated O(n) scans over declarations and variable lists.

- `compiler.rs` lines 93, 99: For each state/discrete variable, `flat_model.declarations.iter().find(|d| d.name == *var)`.
- Lines 107–108: `discrete_vars_sorted.contains()`, `state_vars_sorted.contains()` on `Vec` (or sorted vec used as set) — use `HashSet` for O(1).
- Lines 134–140, 198, 216: Multiple `state_vars_sorted.iter().position()`, `output_vars.iter().position()`, etc.

**Action:**

1. Build once: `HashMap<String, &Declaration>` or `HashMap<String, Declaration>` from `flat_model.declarations` (by `name`) for initial value lookup.
2. Build once: `HashMap<String, usize>` for `state_vars`, `discrete_vars`, `output_vars`, `param_vars` (name -> index) and use it in compiler and in JIT context.
3. Use `HashSet<String>` for membership tests (e.g. algebraic_vars, known_vars) instead of `.contains()` on Vec.

This reduces variable/declaration resolution from O(n) per lookup to O(1).

### 2.3 JIT: Variable Index Lookup (Performance)

**Issue:** In `jit/translator.rs`, `ctx.state_vars.iter().position(|x| x == name)` (and similar for `output_vars`, `discrete_vars`) is used many times per compilation.

**Action:** Extend `TranslationContext` (or the JIT layer that builds it) to hold precomputed `HashMap<String, usize>` for:

- `state_vars`: name -> index  
- `discrete_vars`: name -> index  
- `output_vars`: name -> index  

Build these maps once when creating the context; in translator use `ctx.state_var_index(name)` instead of `iter().position()`. Same pattern for discrete and output indices.

### 2.4 Loader and Error Handling

**Issue:**  
- `loader.load_model()` returns `Option<Model>`; failure is reported via `expect()` or `eprintln` + `None`.  
- `flatten/mod.rs` uses `process::exit(1)` on load failure and unknown type.

**Action:**

1. Change loader to return `Result<Model, LoadError>` (e.g. with `thiserror`), and propagate in compiler.
2. In flattener, replace `process::exit(1)` with returning `Result` (or propagate compiler’s `Result`) so that the main pipeline can report errors consistently and optionally run multiple compilations in one process.

### 2.5 Flatten: Allocations and Cloning

**Issue:**  
- In `expand_equation_list` (For loop), each iteration does `new_context = context.clone()` (line 242).  
- When handling When/If, temporary `FlattenedModel` structs are created with `array_sizes: flat.array_sizes.clone()`.

**Action:**

1. For the For-loop context: consider a small stack of contexts (e.g. `Vec<HashMap<...>>`) and push/pop instead of cloning the whole map when the loop variable is the only change.
2. For temporary flattens used only to collect equations/algorithms: pass `&flat.array_sizes` or use a shared structure (e.g. `Arc<HashMap<...>>`) to avoid cloning `array_sizes` repeatedly.

### 2.6 Analysis: Alias Elimination and BLT

**Issue:**  
- `eliminate_aliases` does multiple passes over a cloned equation list and rebuilds collections; allocation can be significant for large systems.  
- Comment in code suggests Hopcroft-Karp for matching; current implementation uses greedy + DFS augmenting path.

**Action:**

1. Profile; if alias elimination is hot, consider a single pass where possible or reuse buffers instead of allocating new Vecs every iteration.
2. For large models, consider replacing the current matching with a proper bipartite matching algorithm (e.g. Hopcroft-Karp) to improve BLT quality and possibly reduce iteration count in dependency resolution.

### 2.7 Loader: Cache and Clone

**Issue:** `loaded_models.get(name)` returns `Some(model.clone())`, so every use of a cached model pays a full clone.

**Action:**  
- If the cache is only read after insert, store `Model` and return `Option<&Model>` (or a clone only when the caller needs ownership).  
- If the compiler needs ownership for flattening, consider `Arc<Model>` in the cache and clone only the Arc when returning, to reduce deep copies. This depends on how much the flattener mutates the model; if it can work on a reference, avoid returning owned copies.

### 2.8 Unused Code and Cargo

**Issue:**  
- `codegen.rs` (AOT) and `main_helpers.rs` are not in the module tree; dead code or future use is unclear.  
- `Cargo.toml`: `edition = "2024"` — Rust 2024 may not be stable in all environments.

**Action:**  
- Either integrate `codegen` (e.g. behind a feature or subcommand) or remove it to avoid confusion.  
- Same for `main_helpers`: integrate or remove.  
- Confirm Rust version policy; if 2024 is not required, use `edition = "2021"` for broader compatibility.

### 2.9 Parser and Grammar

- Parser is Pest-based; no separate lexer. No change suggested unless profiling shows parse time as a bottleneck.
- If new language features are added, keep grammar and AST in sync and consider fuzz or regression tests for the parser.

### 2.10 Simulation and Solver

- Solver and simulation are relatively small. Optimize only if profiling shows them as hot (e.g. JIT call overhead or RK4 inner loop).

---

## 3. Priority Summary

| Priority | Item | Impact | Effort |
|----------|------|--------|--------|
| High     | Compiler + JIT: build declaration and var-index maps once, use O(1) lookup | Less compile-time for large models | Low |
| High     | Split jit/translator.rs into submodules (e.g. expr, equation, algorithm) | Maintainability, future growth | Medium |
| Medium   | Loader + Flatten: Result-based errors, remove process::exit(1) | Robustness, testability | Medium |
| Medium   | Flatten: reduce context and array_sizes cloning in expand_* | Lower memory and CPU during flatten | Medium |
| Low      | analysis: optional Hopcroft-Karp, alias pass tuning | Better BLT and possibly faster analysis | Medium |
| Low      | Loader cache: Arc<Model> or return reference to avoid full clone | Lower memory and clone cost | Low–Medium |
| Low      | Resolve unused code (codegen, main_helpers) and edition | Clarity and compatibility | Low |

---

## 4. Next Steps

1. Implement declaration and variable-index maps in `compiler.rs` and JIT context; replace all `iter().find()` and `iter().position()` on declarations and variable lists with map lookups.  
2. Split `jit/translator.rs` into `translator/expr.rs`, `translator/equation.rs`, `translator/algorithm.rs` and a thin `translator/mod.rs`.  
3. Change loader to `Result<Model, E>` and flattener to propagate errors instead of `process::exit(1)`.  
4. Re-run build and any existing runs (e.g. `cargo run <model>`) to verify behavior unchanged.

After code changes, run: `cargo build --release` (and optionally run a representative model) to confirm compilation and correctness.
