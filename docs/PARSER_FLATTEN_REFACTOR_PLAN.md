# Parser and Flatten Refactoring Plan

Refactoring plan for splitting `parser.rs` (~1018 lines) and `flatten/mod.rs` (~883 lines) into smaller modules. Dependencies are drawn first; splits are done in order to avoid moving large blocks at once.

---

## 1. Parser dependency graph

### 1.1 Call graph (who calls whom)

```
parse() ──────────────────────────────────────────────────────────► parse_model()
                                                                           │
parse_model() ◄── (recursive)                                            │
     │                                                                   │
     ├── parse_expression() ◄───────────────────────────────────────────┼─── (many places)
     │         │                                                        │
     │         ├── parse_if_expression() ──► parse_expression()        │
     │         ├── parse_logical_or() ──► parse_logical_and()           │
     │         │         └── parse_relation() ──► parse_arithmetic()   │
     │         │                   └── parse_term() ──► parse_factor()   │
     │         │                             └── parse_expression()      │
     │         │                             └── parse_component_ref()  │
     │         └── parse_factor() ──► parse_expression(), parse_component_ref()
     │                                                                  │
     ├── parse_component_ref() ──► parse_expression() ◄────────────────┤
     │                                                                  │
     ├── parse_for_loop() ──► parse_expression(), parse_equation_stmt_inner()
     ├── parse_when_equation() ──► parse_expression(), parse_equation_stmt_inner()
     ├── parse_if_equation() ──► parse_expression(), parse_equation_stmt_inner()
     ├── parse_algorithm_stmt() ──► parse_component_ref(), parse_expression()
     │                                                                  │
     └── parse_equation_stmt_inner() ──► parse_expression(), parse_component_ref(),
              │                          parse_for_loop(), parse_when_equation(),
              │                          parse_if_equation(), parse_multi_assign_equation()
              └── parse_multi_assign_equation() ──► parse_component_ref(), parse_expression()

parse_const_expression() ──► parse_expression(), eval_const_expr()
eval_const_expr() (local to parser, only used by parse_const_expression)
expr_to_string(Expression) (used only inside parse_model for modification names)
parse_annotation_to_string() (used only inside parse_model)
```

### 1.2 Dependency layers (bottom-up)

| Layer | Functions | Dependencies | Approx. lines |
|-------|-----------|--------------|---------------|
| **L0** | `parse_expression` → `parse_if_expression`, `parse_logical_or` → … → `parse_term`, `parse_factor` | `Rule`, `Expression`, `Operator` | ~220 |
| **L1** | `parse_component_ref` | `parse_expression` (via factor branch), `Rule` | ~25 |
| **L2** | `expr_to_string`, `eval_const_expr`, `parse_const_expression` | `Expression`, `parse_expression` | ~55 |
| **L3** | `parse_equation_stmt_inner`, `parse_for_loop`, `parse_when_equation`, `parse_if_equation`, `parse_multi_assign_equation` | L0, L1, each other (equation recursion) | ~160 |
| **L4** | `parse_algorithm_stmt` | L0, L1 | ~150 |
| **L5** | `parse_annotation_to_string`, `parse_model` | All above | ~450 |

### 1.3 Parser split proposal (by module)

- **parser/mod.rs**  
  - `ModelicaParser`, `parse()`, `parse_model()`, `parse_annotation_to_string()`.  
  - Re-exports from submodules so that `crate::parser::parse` and `Rule` stay the single entry.  
  - Keep `parse_model` here so that declaration_section / equation_section / algorithm_section loops stay in one place; they only call into submodules.

- **parser/expression.rs**  
  - `parse_expression`, `parse_if_expression`, `parse_logical_or`, `parse_logical_and`, `parse_relation`, `parse_arithmetic`, `parse_term`, `parse_factor`.  
  - No dependency on equation/algorithm; only on `Rule`, `Expression`, `Operator`.  
  - ~220 lines.

- **parser/component_ref.rs**  
  - `parse_component_ref` (calls `parse_expression` for subscript).  
  - `pub(super) fn parse_component_ref` and `parser/expression.rs` exposed so mod.rs or equation/algorithm can call it.  
  - ~25 lines.

- **parser/equation.rs**  
  - `parse_equation_stmt_inner`, `parse_for_loop`, `parse_when_equation`, `parse_if_equation`, `parse_multi_assign_equation`.  
  - Depends on `parser::expression` and `parser::component_ref` (or super).  
  - ~160 lines.

- **parser/algorithm.rs**  
  - `parse_algorithm_stmt`.  
  - Depends on `parser::expression`, `parser::component_ref`.  
  - ~150 lines.

- **parser/helpers.rs** (optional)  
  - `expr_to_string`, `eval_const_expr`, `parse_const_expression`.  
  - Used by `parse_model` (modification name) and optionally by tests.  
  - ~55 lines.

Dependency between parser modules:

```
parser/mod.rs
    ├── parser/expression.rs     (no internal parser deps)
    ├── parser/component_ref.rs   ──► expression
    ├── parser/equation.rs        ──► expression, component_ref
    ├── parser/algorithm.rs       ──► expression, component_ref
    └── parser/helpers.rs         ──► expression (eval_const_expr uses Expression)
```

### 1.4 Parser split steps (order to avoid big moves)

1. **Step P1: Add `parser/expression.rs`**  
   - Create `parser/` directory and `parser/expression.rs`.  
   - Move `parse_expression`, `parse_if_expression`, `parse_logical_or`, `parse_logical_and`, `parse_relation`, `parse_arithmetic`, `parse_term`, `parse_factor` into it.  
   - In `parser.rs` (or future `parser/mod.rs`): keep `use crate::ast::*` and `use pest::*`; add `mod expression; use expression::*;` (or qualified calls).  
   - Build and fix: all call sites in current `parser.rs` that call these functions must use `expression::parse_expression` etc. or re-export and use unqualified.

2. **Step P2: Add `parser/component_ref.rs`**  
   - Move `parse_component_ref` to `parser/component_ref.rs`.  
   - It calls `parse_expression` → use `super::expression::parse_expression` (or re-export in mod).  
   - Adjust `parser.rs` to use `component_ref::parse_component_ref`.

3. **Step P3: Add `parser/helpers.rs`**  
   - Move `expr_to_string`, `eval_const_expr`, `parse_const_expression`.  
   - `eval_const_expr` and `parse_const_expression` depend only on `Expression`/`Operator` and `parse_expression` (for const parsing). So `helpers` depends on `expression`.  
   - `parse_model` currently uses `expr_to_string` and `parse_expression`; after P1/P2 it can use `helpers::expr_to_string` and `expression::parse_expression`.

4. **Step P4: Add `parser/equation.rs`**  
   - Move `parse_equation_stmt_inner`, `parse_for_loop`, `parse_when_equation`, `parse_if_equation`, `parse_multi_assign_equation`.  
   - They need `parse_expression`, `parse_component_ref` → use `super::expression` and `super::component_ref`.  
   - In `parser.rs` / `parse_model`, replace direct calls with `equation::parse_for_loop` etc.

5. **Step P5: Add `parser/algorithm.rs`**  
   - Move `parse_algorithm_stmt`.  
   - Depends on `parse_expression`, `parse_component_ref` → same as equation.  
   - In `parse_model`, use `algorithm::parse_algorithm_stmt`.

6. **Step P6: Rename `parser.rs` → `parser/mod.rs`**  
   - Replace `src/parser.rs` with `src/parser/mod.rs` and keep only: `ModelicaParser`, `parse()`, `parse_model()`, `parse_annotation_to_string()`, plus `mod` and `pub use` for submodules.  
   - Ensure `main.rs` / rest of crate still use `parser::parse` and `parser::Rule` unchanged.

---

## 2. Flatten dependency graph

### 2.1 Flattener methods and helpers

```
Flattener::flatten()
    ├── flatten_inheritance()
    ├── expand_declarations()     (uses loader, utils, expressions)
    ├── expand_equations()       ──► expand_equation_list()
    ├── expand_initial_equations() ──► expand_equation_list()
    ├── expand_algorithms()      ──► expand_algorithm_list()
    ├── expand_initial_algorithms() ──► expand_algorithm_list()
    └── resolve_connections()   (in connections.rs)

expand_equation_list()  (~260 lines)
    ├── substitute_stack()
    ├── prefix_expression()      (expressions.rs)
    ├── index_expression()       (expressions.rs)
    ├── eval_const_expr()        (expressions.rs)
    ├── get_record_components()  ──► loader
    ├── get_function_outputs()   (utils), apply_modification (utils)
    ├── convert_eq_to_alg()      (utils)
    ├── expr_to_path()           (expressions.rs)
    └── expand_equation_list()   (recursive)

expand_algorithm_list() (~170 lines)
    ├── substitute_stack()
    ├── prefix_expression()
    └── expand_algorithm_list()  (recursive)

substitute_stack()  (~65 lines)
    ├── lookup_context_stack()
    ├── expr_to_path()
    └── resolve_global_constant()

substitute()  (~70 lines)
    ├── expr_to_path()
    └── resolve_global_constant()

resolve_global_constant()  (~20 lines) ──► loader
get_record_components()  (~10 lines) ──► loader
lookup_context_stack()   (~8 lines)   (pure)
```

### 2.2 External deps (flatten/mod.rs)

- **flatten/structures.rs**: `FlattenedModel`
- **flatten/expressions.rs**: `prefix_expression`, `index_expression`, `eval_const_expr`, `expr_to_path`
- **flatten/utils.rs**: `is_primitive`, `resolve_type_alias`, `apply_modification`, `merge_models`, `convert_eq_to_alg`, `get_function_outputs`
- **flatten/connections.rs**: `resolve_connections`
- **crate::loader**: `ModelLoader`, `load_model`, `load_model_silent`
- **crate::ast**: `Expression`, `Equation`, `Declaration`, `Model`, `AlgorithmStatement`
- **crate::diag**: `SourceLocation`

### 2.3 Flatten split proposal (by module)

- **flatten/mod.rs** (slim)  
  - `FlattenError`, `Flattener`, `flatten()`, `flatten_inheritance()`, `expand_declarations()`, `expand_equations()`, `expand_initial_equations()`, `expand_algorithms()`, `expand_initial_algorithms()`.  
  - Defines `ExpandTarget` and delegates equation/algorithm expansion to `expand` submodule.  
  - Target: &lt; 250 lines.

- **flatten/expand.rs** (new)  
  - `expand_equation_list`, `expand_algorithm_list`.  
  - Takes `&mut Flattener` (or an explicit context struct) so it can call `substitute_stack`, `get_record_components`, `resolve_global_constant`.  
  - Either: implement as `Flattener` methods in expand.rs (e.g. `impl Flattener { fn expand_equation_list(...) }`) and call from mod.rs, or extract a context that holds references to loader + substitution helpers.  
  - ~430 lines (equation_list + algorithm_list).

- **flatten/substitute.rs** (new)  
  - `lookup_context_stack`, `substitute_stack`, `substitute`, `resolve_global_constant`.  
  - These need `Flattener`’s `loader` and (for substitute_stack) no mutable state beyond context. So either:  
    - Implement as `Flattener` methods in substitute.rs (e.g. `impl Flattener { fn substitute_stack(...) }`), and have `expand.rs` call `self.substitute_stack` (if expand is also impl Flattener in the same impl block or in expand.rs as `impl Flattener`), or  
    - Introduce a `SubstituteContext<'a>` that holds `&'a ModelLoader` and optional context stacks, and move pure logic into free functions where possible.  
  - ~165 lines.

- **flatten/mod.rs** after split:  
  - Keeps `ExpandTarget`, `Flattener`, and the five `expand_*` entry methods.  
  - `expand_equations` / `expand_initial_equations` call `self.expand_equation_list(...)` (method moved to expand.rs in same `impl Flattener`).  
  - `expand_algorithms` / `expand_initial_algorithms` call `self.expand_algorithm_list(...)`.

Dependency between flatten modules:

```
flatten/mod.rs
    ├── flatten/structures.rs (existing)
    ├── flatten/expressions.rs (existing)
    ├── flatten/utils.rs (existing)
    ├── flatten/connections.rs (existing)
    ├── flatten/substitute.rs  ──► loader, expressions (expr_to_path)
    └── flatten/expand.rs     ──► substitute (substitute_stack), expressions, utils, ExpandTarget
```

Note: `expand_equation_list` and `expand_algorithm_list` use `self` (Flattener). So they must stay as `impl Flattener` methods. The only way to reduce mod.rs size without changing the impl structure is to put those impl blocks in separate files:

- **Rust pattern**: `impl Flattener` in `mod.rs` for `flatten`, `expand_equations`, `expand_initial_equations`, `expand_algorithms`, `expand_initial_algorithms`, `expand_declarations`, `get_record_components`.
- In **flatten/expand.rs**: `impl Flattener { fn expand_equation_list(...) { ... } fn expand_algorithm_list(...) { ... } }`.
- In **flatten/substitute.rs**: `impl Flattener { fn lookup_context_stack(...), substitute_stack(...), substitute(...), resolve_global_constant(...) }`.

So we need multiple `impl Flattener` blocks across files. Rust allows this. Then mod.rs only contains the small methods that build context and call `expand_equation_list` / `expand_algorithm_list`.

### 2.4 Flatten split steps (order to avoid big moves)

1. **Step F1: Add `flatten/substitute.rs`**  
   - Create `flatten/substitute.rs` with `impl Flattener { fn lookup_context_stack, substitute_stack, substitute, resolve_global_constant }`.  
   - Move the bodies of these four methods from `flatten/mod.rs` into this new impl block.  
   - Remove those methods from `flatten/mod.rs`.  
   - Build: no other code calls these directly except inside Flattener (expand_declarations, expand_equation_list, expand_algorithm_list), so once moved, all calls remain `self.substitute_stack(...)` etc.  
   - Result: mod.rs loses ~165 lines.

2. **Step F2: Add `flatten/expand.rs`**  
   - Create `flatten/expand.rs` with `impl Flattener { fn expand_equation_list(...), fn expand_algorithm_list(...) }`.  
   - Move the two method bodies from `flatten/mod.rs` into this file.  
   - Keep `expand_equations`, `expand_initial_equations`, `expand_algorithms`, `expand_initial_algorithms` in mod.rs; they only build target/context and call `self.expand_equation_list` / `self.expand_algorithm_list`.  
   - Add `use super::*` or necessary imports in expand.rs (ExpandTarget, expressions, utils, convert_eq_to_alg, etc.).  
   - Build and fix imports.  
   - Result: mod.rs loses ~430 lines; expand.rs holds the large match blocks.

3. **Step F3: Optional – extract `get_record_components`**  
   - Move `get_record_components` to `flatten/utils.rs` or a small `flatten/decl_utils.rs` if it’s used only for expansion. Currently only `expand_equation_list` uses it; it could stay in mod.rs or move with expand.  
   - Low priority; do only if mod.rs is still above target line count.

---

## 3. Summary

| File / phase | Action | Approx. line change |
|--------------|--------|----------------------|
| **Parser** | P1: expression.rs | parser.rs −220, new file +220 |
| | P2: component_ref.rs | parser.rs −25, new +25 |
| | P3: helpers.rs | parser.rs −55, new +55 |
| | P4: equation.rs | parser.rs −160, new +160 |
| | P5: algorithm.rs | parser.rs −150, new +150 |
| | P6: parser.rs → parser/mod.rs | Rename; mod.rs ~450 lines |
| **Flatten** | F1: substitute.rs | mod.rs −165, new +165 |
| | F2: expand.rs | mod.rs −430, new +430 |

After refactor:

- **parser**: single file ~1018 → `parser/mod.rs` ~450 + 5 submodules (each &lt; 220 lines).
- **flatten**: single file ~883 → `flatten/mod.rs` ~290 + `expand.rs` ~430 + `substitute.rs` ~165 (all under 500 lines).

Recommended implementation order: **P1 → P2 → P3 → P4 → P5 → P6** for parser; **F1 → F2** for flatten. Run `cargo build --release` after each step.
