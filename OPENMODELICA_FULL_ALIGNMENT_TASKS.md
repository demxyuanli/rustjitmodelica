# OpenModelica Compiler Full Alignment – Detailed Task List

**Goal:** Align rustmodlica compiler and simulation with OpenModelica (OMC) functionality.  
Tasks are ordered by dependency and impact; each should be completable in one focused session and pass `cargo build --release`.

Reference: `OPENMODELICA_VS_RUSTMODLICA.md` (gap analysis), OpenModelica User's Guide (compiler/backend).

---

## Phase 1: Frontend & Language

### 1.1 Parsing & AST

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| F1-1 | **record** semantic support | Parse `record`; build AST; support as structured type in declarations and equations (flatten to scalar/array as today). Add `TestLib/SimpleRecord.mo`. | P1 |
| F1-2 | **block** semantic support | Treat `block` like model for flatten/JIT (inputs/outputs, no dynamics). Add `TestLib/SimpleBlock.mo`. | P2 |
| F1-3 | **package** as namespace only | Resolve `Package.Class` and load from package structure; no package instantiation. Add test with nested package. | P2 |
| F1-4 | **operator** / **type** (optional) | Grammar and AST for `operator record` / `type ... = ...` if needed for MSL compatibility. | P3 |
| F1-5 | **annotation** (parse-only) | Parse `annotation(...)` on class/component/equation; store in AST; ignore in backend. | P2 |
| F1-6 | **modification** in **extends** | Full modifier merging for `extends` (e.g. redeclare, each); align with OMC semantics. | P2 |

### 1.2 Built-in operators & functions

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| F2-1 | **Nested der()** | Allow `der(expr)` where `expr` is not only Variable (e.g. linear combination); flatten to state derivatives or reject with clear error. | P1 |
| F2-2 | **pre() / edge() / change()** | Ensure correct semantics in when/initial; add tests (e.g. `TestLib/PreEdgeChange.mo`). | P1 |
| F2-3 | **sample() / interval()** | Parse and implement clock/sample if targeting synchronous semantics; or document as out-of-scope. | P3 |
| F2-4 | **Built-in math/type functions** | Expand: `abs`, `sign`, `sqrt`, `min`, `max`, `mod`, `div`, `rem`, `ceil`, `floor`, `integer`, etc. Map to JIT or inline. | P1 |
| F2-5 | **String / Boolean built-ins** | `String()`, `Boolean()`, and string comparison if needed for MSL/scripts. | P3 |

### 1.3 Functions (full pipeline)

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| F3-1 | **Function as callable unit** | Compile `function` to callable JIT or C stub (single output, multiple inputs); no simulation of standalone function yet. | P1 |
| F3-2 | **Function in simulation** | Allow top-level or called function to be executed during simulation (e.g. from equation/algorithm). | P1 |
| F3-3 | **Multiple outputs / record return** | Function returning record or tuple; map to flattened outputs. | P2 |
| F3-4 | **External function** (optional) | Declare and link external C function; document ABI. | P3 |

### 1.4 Equations & algorithms

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| F4-1 | **connect() inside when** | Support connect in when (e.g. conditional connections); flatten to equations with when conditions. | P2 |
| F4-2 | **SolvableBlock inside when/algorithm** | Allow small algebraic blocks in when/algorithm or reject with clear message. | P2 |
| F4-3 | **if-equation** | Full if-equation in equation section; branch during flatten/analysis. | P2 |
| F4-4 | **assert / terminate** | Parse and implement assert(cond, msg) and terminate(msg); integrate with simulation (log/stop). | P2 |
| F4-5 | **Array equations** | Treat array equation `A = B` as element-wise or whole-array; consistent with indexing. | P1 |
| F4-6 | **Record equations** | Flatten record equation to scalar equations. | P2 |

---

## Phase 2: IR & Backend (DAE-style)

### 2.1 Intermediate representation

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| IR1-1 | **Explicit DAE form** | Represent sorted equations as `0 = F(x, x', u, t)` with clear state/derivative/algebraic/input sets. | P1 |
| IR1-2 | **Initial vs simulation DAE** | Separate initial equation system and simulation equation system; same backend pipeline for both. | P1 |
| IR1-3 | **Partitioning / blocks** | Output strongly connected components (blocks) with block type: explicit, linear, nonlinear (algebraic), mixed. | P1 |
| IR1-4 | **backend-dae-info style output** | Emit equation/variable counts, states, discrete, blocks (single/torn/mixed) as in OMC `backenddaeinfo`. | P1 |

### 2.2 Matching & causalization

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| IR2-1 | **Bipartite matching** | Variable–equation matching (e.g. BFS/Dulmage–Mendelsohn); produce assignment and blocks. | P1 |
| IR2-2 | **Sorting / BLT** | Block Lower Triangular ordering; handle algebraic loops as nonlinear blocks. | P1 |
| IR2-3 | **Alias removal** | Detect `a = b` and substitute; reduce variables and equations. | P1 |
| IR2-4 | **State selection** | Prefer states that avoid high index; optional state selection hints. | P2 |

### 2.3 Index reduction

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| IR3-1 | **Differentiation index** | Compute differentiation index; if > 1, apply index reduction (e.g. dummy derivative or substitution). | P1 |
| IR3-2 | **Constraint equations** | Identify constraint equations (e.g. from connect/loops); differentiate to get index-1. | P1 |
| IR3-3 | **Consistent initialization** | Initial system: ensure no over/under-determination; solve for initial state and derivatives. | P1 |
| IR3-4 | **time_derivative() integration** | Use existing `time_derivative()` in real index-reduction path (not only debugPrint). | P1 |

### 2.4 Tearing & Jacobian

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| IR4-1 | **General tearing** | Support SolvableBlock with N residuals (N > 2); numerical Newton with symbolic or numeric Jacobian. | P1 |
| IR4-2 | **Tearing variable selection** | Heuristics for choosing tearing variables (e.g. least occurrence, structural). | P2 |
| IR4-3 | **Jacobian for DAEs** | Full Jacobian for algebraic and DAE residuals; use in Newton/solver. | P1 |
| IR4-4 | **Sparse structure** | Preserve sparsity in large systems; optional sparse linear solve. | P3 |

---

## Phase 3: Code Generation & Simulation

### 3.1 Code generation

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| CG1-1 | **C code generation (optional)** | Emit C file(s) for residual, Jacobian, outputs; compile and link with small runtime. | P2 |
| CG1-2 | **Standalone executable** | Build a single executable that runs simulation (like OMC default); CLI args for model, t_end, etc. | P2 |
| CG1-3 | **JIT fallback** | Keep current JIT path as default; C path for portability or debugging. | P2 |
| CG1-4 | **Array preservation** | Avoid full scalarization of arrays in generated code where possible (pseudo array causalization). | P3 |

### 3.2 Solvers & runtimes

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| RT1-1 | **DAE/ODE solver with events** | Use adaptive RK (or other) with event detection and reinit; support when + zero-crossing in same run. | P1 |
| RT1-2 | **Implicit / stiff solver (optional)** | Add simple implicit method (e.g. backward Euler or BDF-like) for stiff models. | P2 |
| RT1-3 | **Solver selection flag** | CLI or option to choose solver (e.g. rk4, rk45, implicit). | P2 |
| RT1-4 | **Step size / tolerance options** | Expose dt, atol, rtol for user. | P1 |
| RT1-5 | **Output interval / result file** | Configurable print interval; optional CSV/Mat result file. | P2 |

### 3.3 FMI (optional)

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| FMI-1 | **FMI 2.0 CS (co-simulation)** | Export FMU with C code; implement required FMI API. | P3 |
| FMI-2 | **FMI 2.0 ME (model exchange)** | Export FMU for model exchange; document solver requirements. | P3 |

---

## Phase 4: Standard Library & Compatibility

### 4.1 Modelica Standard Library (MSL)

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| MSL-1 | **MSL version pin** | Document and test against one MSL version (e.g. 3.2.3); add subset to repo or loader. | P1 |
| MSL-2 | **Modelica.Blocks (core)** | Ensure Sources (Constant, Step, Sine, etc.), Interfaces (RealInput/Output), Continuous (Integrator, TransferFunction) work. | P1 |
| MSL-3 | **Modelica.Math** | Support used built-in functions (sin, cos, exp, log, etc.); add wrappers if needed. | P1 |
| MSL-4 | **Modelica.SIunits** | Resolve SIunits as Real with unit string (or ignore units but parse). | P2 |
| MSL-5 | **Common MSL patterns** | Conditional components, replaceable, redeclare (minimal set for Blocks). | P2 |

### 4.2 Regression & testing

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| REG-1 | **Expand REGRESSION_CASES.txt** | Add MSL and OMC test models; mark pass/fail and reason. | P1 |
| REG-2 | **OMC comparison tests** | For a small set of models, compare final state or trajectory with OMC (same solver/tolerance). | P2 |
| REG-3 | **CI regression** | Run regression in CI (e.g. GitHub Actions); fail on new failures. | P2 |

---

## Phase 5: Tooling & Debug

### 5.1 Debug and info

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| DBG-1 | **--backend-dae-info** | Implement full backend DAE info (equation/variable counts, states, discrete, block stats) as in OMC. | P1 |
| DBG-2 | **--index-reduction-method** | Support `none` / `dummyDerivative` (or similar); wire to real index reduction. | P1 |
| DBG-3 | **Warnings as configurable** | Overdetermined/underdetermined, unused variables; optional suppress. | P2 |
| DBG-4 | **Source location in errors** | Attach file/line to flatten, analysis, and JIT errors. | P2 |

### 5.2 Interactive (optional)

| ID | Task | Detail | Priority |
|----|------|--------|----------|
| INT-1 | **REPL / evaluate expression** | Load model, then evaluate expressions (e.g. parameters, initial values). | P3 |
| INT-2 | **Script mode** | Parse and run a small script (load, setParameter, simulate). | P3 |

---

## Summary Table (by priority)

| Priority | Meaning | Example tasks |
|----------|--------|----------------|
| P1 | Core alignment; blocks many tests | F1-1, F2-1, F2-4, F3-1, F3-2, F4-5, IR1-*–IR4-*, RT1-1, RT1-4, MSL-1–MSL-3, REG-1, DBG-1, DBG-2 |
| P2 | Important for MSL/real models | F1-2, F1-3, F1-5, F1-6, F4-1–F4-4, F4-6, IR2-4, IR4-2, CG1-1–CG1-3, RT1-2–RT1-5, MSL-4–MSL-5, REG-2–REG-3, DBG-3–DBG-4 |
| P3 | Nice-to-have / later | F1-4, F2-3, F2-5, F3-4, IR4-4, CG1-4, FMI-*, INT-* |

---

## Suggested implementation order (high level)

1. **Backend DAE & index reduction** (IR1, IR2, IR3) – foundation for correct DAE and MSL.
2. **General tearing & Jacobian** (IR4) – remove 1-or-2-residual limit.
3. **Functions & built-ins** (F2, F3) – needed for MSL and user models.
4. **Record/block/array/record equations** (F1, F4) – language completeness.
5. **Solver with events** (RT1-1) – adaptive + when/zero-crossing.
6. **MSL subset & regression** (MSL, REG).
7. **C code gen / standalone / FMI** (CG, FMI) – deployment.
8. **Tooling** (DBG, INT) – usability.

Each task should close a concrete gap listed in `OPENMODELICA_VS_RUSTMODLICA.md` where applicable.
