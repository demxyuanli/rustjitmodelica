# MSL validate failure classification and minimal fix playbook

Use this when a model fails `--validate`. Classify by symptom, then apply the minimal change in the listed location.

## 1. Name resolution (unknown class / type / import)

**Symptom:** Error about unknown class, type, or unresolved import; missing prefix for Blocks/Math, Electrical, Utilities, etc.

**Fix location:** `jit-compiler/src/flatten/mod.rs` — `resolve_import_prefix()`.

**Minimal fix:**
- Add or extend fallback rules by library context (Blocks subpackages, Electrical.Analog pins, Utilities.Internal, Rotational.Interfaces, Thermal, Fluid).
- Handle leading dot: ensure `trim_start_matches('.')` and context-specific prefixes so names like `.Math` or `Internal.xxx` resolve.

---

## 2. Connector compatibility (connect equation / type mismatch)

**Symptom:** Connect equation fails; types not compatible; e.g. Pin / PositivePin / NegativePin, or Rotational Support / Flange_a / Flange_b.

**Fix location:** `jit-compiler/src/flatten/utils.rs` — `are_types_compatible()`.

**Minimal fix:**
- Extend compatibility matrix: same physical domain (Electrical pins, Rotational flanges/support, HeatTransfer HeatPort, Fluid ports) treat as compatible when semantically same domain.
- Prefer one rule per domain instead of one-off type pairs.

---

## 3. Flatten residual: Dot (a.b in equations)

**Symptom:** Flattened equations still contain `Dot` (component access like `a.b`); JIT or backend cannot resolve.

**Short-term fix location:** JIT — `jit-compiler/src/jit/translator/expr.rs`: use `expr_to_connector_path` (or equivalent) to map residual `Dot` to a known connector/variable path so codegen does not emit unresolved symbol.

**Medium-term fix location:** Flatten — reduce or eliminate common `a.b` in flatten so fewer Dots reach the backend.

---

## 4. Flatten: for-loop with non-const bound

**Symptom:** Panic or error in flatten or backend when a `for` loop has non-constant upper bound.

**Fix location:** Keep `Equation::For` through the pipeline; ensure JIT/analysis in `jit-compiler` correctly allocates loop variable and any variables referenced inside the loop.

**Minimal fix:** Do not expand such for-loops into a fixed number of equations in flatten; pass through as `Equation::For` and handle in backend (loop var allocation and bounds evaluation at runtime if needed).

---

## 5. Flatten: arrays

**Symptom:** Errors about array indexing, size, or dimension in flattened model.

**Fix location:** Flatten array handling (same area as equation flattening). Ensure array subscripts and sizes are consistent; if a minimal placeholder is used (e.g. fixed size), document and centralize.

---

## 6. JIT external symbol (Cranelift "can't resolve symbol ...")

**Symptom:** JIT link step fails with "can't resolve symbol" for a function or global (e.g. `fill`, `product`, `firstTrueIndex`, `isEmpty`, `interpolate`, `ExternalCombiTimeTable`).

**Fix location:** `jit-compiler/src/jit/translator/expr.rs` — central builtin/fallback dispatch (function name -> implementation or placeholder).

**Minimal fix:** Add the missing name to the central dispatcher; provide a small placeholder (e.g. constant or stub) so link succeeds. Prefer one table or match per function name.

---

## 7. Constants / enumerations

**Symptom:** After flatten, constants or enums appear as variable names (e.g. `*_Types_*`, `Modelica.Constants.T_zero`, `Machine_inf`); JIT or backend does not recognize them.

**Fix location:**
- Flatten: preserve or emit constant/enum values instead of opaque names where possible.
- JIT: central map from known constant/enum names to values or symbols; use in expr translation so codegen sees a constant, not an external symbol.

---

## 8. Tables / ExternalObject / Utilities (CombiTimeTable, getTimeTableValue, Strings.*)

**Unified policy:** Treat as builtin/placeholder so validate passes without external symbol link. No `load_model()`; JIT returns constant (e.g. 0.0) so Cranelift does not need to resolve external symbols.

**Implement in:**
- **Compiler:** `jit-compiler/src/compiler/inline.rs` — `is_builtin_function()`: CombiTimeTable, getTimeTableValue, ExternalCombiTimeTable, ExternalObject, Modelica.Utilities.* / .isEmpty.
- **JIT:** `jit-compiler/src/jit/translator/expr.rs` — central builtin dispatch and `try_compile_builtin_placeholder_constant()`: same names; return 0.0 or 1.0 (isEmpty) to avoid link panic.

**Symptom:** Failure or panic involving CombiTimeTable, ExternalCombiTimeTable, ExternalObject, getTimeTableValue, or Strings.*.

**Minimal fix:** Add any missing name to both the inline whitelist and the JIT placeholder list; use a small placeholder until proper implementation in a later phase.

---

## Quick reference: main files

| Failure type           | Primary file(s)                                      |
|------------------------|------------------------------------------------------|
| Name resolution        | `flatten/mod.rs` (`resolve_import_prefix`)           |
| Connector compatibility| `flatten/utils.rs` (`are_types_compatible`)           |
| Dot residual           | `jit/translator/expr.rs` (short-term); flatten (mid) |
| For non-const bound    | Keep `Equation::For`; JIT/analysis loop var handling|
| Arrays                 | Flatten array handling                              |
| JIT unresolved symbol  | `jit/translator/expr.rs` (central builtin dispatch)  |
| Constants/enums         | Flatten + JIT constant/enum map                      |
| Tables/ExternalObject  | `compiler/inline.rs` + `jit/translator/expr.rs`       |

After any change: `cargo build --release -j 8`, then re-run the failing model with `rustmodlica.exe --validate --lib-path=<MSL_parent> <ModelQualifiedName>`.
