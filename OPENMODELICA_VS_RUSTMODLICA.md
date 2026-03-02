# OpenModelica vs rustmodlica Compiler Comparison

## Short Answer

**No. rustmodlica is not fully aligned with the OpenModelica compiler.**  
It is aligned with the **subset** defined in `OPENMODELICA_FULL_ALIGNMENT_TASKS.md` (incremental “core” compiler and simulation features). That subset is largely implemented; the **full** OpenModelica compiler has many additional features that rustmodlica does not yet provide.

---

## 1. What the Alignment Document Covers

`OPENMODELICA_FULL_ALIGNMENT_TASKS.md` states:

- **Goal**: *Incrementally* align rustmodlica with OpenModelica’s **core** compiler and simulation features.
- It is a **curated subset** of features, not a full spec of OMC.

For that subset (Phase 1–5, P1/P2 tasks), rustmodlica is **largely aligned**: parsing, flattening, BLT, alias removal, index reduction, tearing (1–32 residuals), JIT, C emit, solvers (RK4/RK45/implicit), REPL/script, and regression are in place as described in `OPENMODELICA_ALIGNMENT_COVERAGE.md`.

---

## 2. OpenModelica Compiler (Full) vs rustmodlica

### 2.1 Architecture (high level)

| Aspect | OpenModelica | rustmodlica |
|--------|--------------|-------------|
| IR | SCode → DAE (full backend) | AST → FlattenedModel → sorted equations (BLT, blocks) |
| Code generation | C (or C++) executable | JIT (Cranelift) default; optional `--emit-c` for C source |
| Solver | Multiple runtimes (C/C++), configurable | RK4 (with events), Adaptive RK45 (no when), implicit (BackwardEuler) |

### 2.2 Language and frontend

| Feature | OpenModelica | rustmodlica |
|---------|--------------|-------------|
| model / connector / block / record / package / function (syntax) | Full | Grammar + semantics: record (flatten), block (like model), package (namespace), function (inline in sim) |
| function | Parse, type-check, compile, run | Parse + AST + inline in equations; no standalone function simulation |
| MSL (Standard Library) | MSL 3.2.3, 4.0.0 | Pinned subset (MSL 3.2.3); Blocks/Math/SIunits used in regression |
| Index reduction | Full (DAE, differentiation index) | `--index-reduction-method=dummyDerivative` (or none/debugPrint); constraint diff via time_derivative |
| Array / record equations | Full | Arrays via flattening; record equations flattened to scalar (F4-6) |

### 2.3 Backend and numerical

| Feature | OpenModelica | rustmodlica |
|---------|--------------|-------------|
| Partitioning / causalization / matching / sorting | Full backend | Flatten + BLT, alias removal, blocks (Single/Torn/Mixed) |
| Tearing | General | SolvableBlock with 1–32 residuals; tearing method options (first, leastOccurrence, etc.) |
| Jacobian | Symbolic + numeric, full use | Symbolic + numeric for ODE and tearing; consistency check in sim |
| DAE / high index | Supported (index reduction) | Index-2 reduced via dummyDerivative when enabled; consistent init supported |

### 2.4 Simulation and deployment

| Feature | OpenModelica | rustmodlica |
|---------|--------------|-------------|
| Output | C/C++ executable (or FMU) | JIT default; `--result-file` CSV; optional `--emit-c` C source |
| FMI (FMU export/import) | Yes | Stub only (FMI_README.md); not implemented |
| Interactive (e.g. evaluate expressions) | Yes | `--repl` (vars, simulate, quit); `--script=<path>` (load, setParameter, simulate) |
| Solver options | Many (e.g. dassl, ida, rungekutta) | `--solver=rk4|rk45|implicit`; `--dt`, `--atol`, `--rtol`, `--output-interval` |

### 2.5 Known limitations in rustmodlica (from codebase)

- **Functions**: “User functions are inlined in equations (no standalone function simulation).”
- **SolvableBlock**: 1–32 residuals supported; beyond that “not supported (1 to 32 allowed)”.
- **Nested der()**: “Linear forms (e.g. der(a+b), der(c*x)) expanded; arbitrary nested der(expr) may still error.”
- **Connect** inside when: Supported (F4-1). **SolvableBlock** inside when/algorithm: Rejected with clear message (F4-2).
- **Index-2 DAE**: Reduction via `--index-reduction-method=dummyDerivative`; Pendulum may still fail if model needs more.
- **pre() / edge() / change()**: Implemented (TestLib/PreEdgeChange); semantics in when/initial.
- **Dot / ArrayLiteral** at JIT: Must be flattened before JIT (no late dot/array literal in JIT).
- **Record/package/block**: Record flattened; block as model; package as namespace (full semantic in flatten).

---

## 3. Conclusion

- **Relative to the alignment task list (core subset)**  
  rustmodlica is **largely aligned**: the tasks in `OPENMODELICA_FULL_ALIGNMENT_TASKS.md` are implemented or partially implemented as documented in `OPENMODELICA_ALIGNMENT_COVERAGE.md`.

- **Relative to the full OpenModelica compiler**  
  rustmodlica is **not fully aligned**. Remaining gaps include (among others):

  - Full Modelica language and MSL support (subset only)  
  - FMI/FMU (stub only)  
  - Function as standalone runnable (inline only)  
  - Rich solver and runtime configuration (fixed set: rk4, rk45, implicit)  

Already aligned for the core subset: index reduction (dummyDerivative), tearing (1–32 residuals), C emit (`--emit-c`), standalone binary, REPL/script, backend-dae-info, regression and OMC comparison script (`compare_omc.ps1`).

So: **alignment with the “core” checklist is largely done; alignment with the full OpenModelica compiler is not.**
