# Audit Gap Closure — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close three audit gaps: harden DAE index reduction (default-on), fix stream semantics warnings, implement expandable connector.

**Architecture:** Sequential execution (smallest→largest). Each gap independently testable. Gap 1 changes compiler default + extends linear solver. Gap 2 removes misleading warnings on already-correct JIT stream code. Gap 3 adds AST field → parser capture → flatten dynamic members.

**Tech Stack:** Rust 2021 edition, Cranelift JIT, Pest parser

---

## Gap 1: DAE Index Reduction Hardening

### Task 1.1: Change default index_reduction_method

**Files:**
- Modify: `jit-compiler/src/compiler/mod.rs:498`

- [ ] **Step 1: Change default from `"none"` to `"pantelides"`**

In `jit-compiler/src/compiler/mod.rs`, line 498:
```rust
index_reduction_method: "none".to_string(),
```
Change to:
```rust
index_reduction_method: "pantelides".to_string(),
```

- [ ] **Step 2: Verify existing tests pass**

Run: `rtk cargo test -p rustmodlica -- --nocapture`
Expected: 88/88 tests pass (no regressions from default change)

- [ ] **Step 3: Commit**

```bash
rtk git add jit-compiler/src/compiler/mod.rs
rtk git commit -m "feat: enable index reduction by default (pantelides)

Change default --index-reduction-method from none to pantelides.
Users can still opt out with --index-reduction-method=none.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 1.2: Extend solve_residual_linear

**Files:**
- Modify: `jit-compiler/src/analysis/blt/blt_expr.rs:441-499`

- [ ] **Step 1: Add `coeff*var - rest` and `rest - coeff*var` patterns, and bare `var - rest`**

In `jit-compiler/src/analysis/blt/blt_expr.rs`, replace the `solve_residual_linear` function with:

```rust
pub(super) fn solve_residual_linear(expr: &Expression, var: &str) -> Option<Expression> {
    if !contains_var(expr, var) {
        return None;
    }
    if let Some((coeff, rest)) = split_linear(expr, var) {
        if expression_is_zero(&coeff) {
            return None;
        }
        return Some(make_binary(
            make_binary(make_num(0.0), Operator::Sub, rest),
            Operator::Div,
            coeff,
        ));
    }
    if let Expression::BinaryOp(lhs, op, rhs) = expr {
        let (rest, coeff) = match (op, lhs.as_ref(), rhs.as_ref()) {
            // rest - coeff*var  →  var = rest / coeff
            (Operator::Sub, rest, Expression::BinaryOp(mul_l, Operator::Mul, mul_r)) => {
                let coeff = extract_var_coeff(mul_l, mul_r, var, rest)?;
                (rest.clone(), coeff)
            }
            // coeff*var - rest  →  var = rest / coeff
            (Operator::Sub, Expression::BinaryOp(mul_l, Operator::Mul, mul_r), rest) => {
                let coeff = extract_var_coeff(mul_l, mul_r, var, rest)?;
                (rest.clone(), coeff)
            }
            // var + rest = 0  →  var = -rest
            (Operator::Add, lhs_inner, rest) if is_var(lhs_inner, var) && !contains_var(rest, var) => {
                (make_binary(make_num(0.0), Operator::Sub, rest.clone()), make_num(1.0))
            }
            // rest + var = 0  →  var = -rest
            (Operator::Add, rest, rhs_inner) if is_var(rhs_inner, var) && !contains_var(rest, var) => {
                (make_binary(make_num(0.0), Operator::Sub, rest.clone()), make_num(1.0))
            }
            // var - rest = 0  →  var = rest
            (Operator::Sub, lhs_inner, rest) if is_var(lhs_inner, var) && !contains_var(rest, var) => {
                (rest.clone(), make_num(1.0))
            }
            // rest - var = 0  →  var = rest
            (Operator::Sub, rest, rhs_inner) if is_var(rhs_inner, var) && !contains_var(rest, var) => {
                (rest.clone(), make_num(1.0))
            }
            _ => return None,
        };
        if expression_is_zero(&coeff) {
            return None;
        }
        return Some(make_binary(rest, Operator::Div, coeff));
    }
    None
}

fn is_var(expr: &Expression, var: &str) -> bool {
    matches!(expr, Expression::Variable(id) if resolve_id(*id) == var)
}

fn extract_var_coeff(
    mul_l: &Box<Expression>,
    mul_r: &Box<Expression>,
    var: &str,
    rest: &Expression,
) -> Option<Box<Expression>> {
    if let Expression::Variable(id) = mul_r.as_ref() {
        if resolve_id(*id) == var && !contains_var(rest, var) && !contains_var(mul_l, var) {
            return Some(mul_l.clone());
        }
    }
    if let Expression::Variable(id) = mul_l.as_ref() {
        if resolve_id(*id) == var && !contains_var(rest, var) && !contains_var(mul_r, var) {
            return Some(mul_r.clone());
        }
    }
    None
}
```

- [ ] **Step 2: Build check**

Run: `rtk cargo build -p rustmodlica`
Expected: 0 errors

- [ ] **Step 3: Run existing tests to verify no regressions**

Run: `rtk cargo test -p rustmodlica -- --nocapture`
Expected: 88/88 pass

- [ ] **Step 4: Commit**

```bash
rtk git add jit-compiler/src/analysis/blt/blt_expr.rs
rtk git commit -m "feat: extend solve_residual_linear with Add/Sub patterns for index reduction

Adds var+rest, rest+var, var-rest, rest-var forms to improve
constraint solving coverage for Pantelides index reduction.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 1.3: Add diagnostics when nonlinear constraints cannot be reduced

**Files:**
- Modify: `jit-compiler/src/analysis/blt/helpers.rs:90-241`

- [ ] **Step 1: Add warning for unhandled nonlinear constraints**

In `jit-compiler/src/analysis/blt/helpers.rs`, in `try_index_reduction`, after line 146 (`if !is_constraint { continue; }`), after all Phase 1 and Phase 2 attempts fail (after line 239, before `None`), add a diagnostic:

Find the end of the `for eq_idx in unassigned` loop (line 239 is `}` closing Phase 2). After that closing brace and before the `None` on line 241, insert:

```rust
        // Fallback: constraint could not be reduced by any strategy
        eprintln!(
            "[index-reduction] constraint equation {} could not be reduced (nonlinear or unsupported form)",
            eq_idx
        );
```

- [ ] **Step 2: Build check**

Run: `rtk cargo build -p rustmodlica`
Expected: 0 errors

- [ ] **Step 3: Commit**

```bash
rtk git add jit-compiler/src/analysis/blt/helpers.rs
rtk git commit -m "feat: add diagnostic for unreduced nonlinear constraints in index reduction

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 1.4: Add unit tests for index reduction

**Files:**
- Modify: `jit-compiler/src/analysis/blt/helpers.rs` (append test module)

- [ ] **Step 1: Add test module**

Append to `jit-compiler/src/analysis/blt/helpers.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Equation, Expression, Operator};
    use std::collections::HashSet;

    fn make_var(name: &str) -> Expression {
        Expression::Variable(crate::string_intern::intern(name))
    }

    fn make_der(name: &str) -> Expression {
        Expression::Der(Box::new(Expression::Variable(crate::string_intern::intern(name))))
    }

    fn make_der_var(name: &str) -> Expression {
        Expression::Variable(crate::string_intern::intern(&format!("der_{}", name)))
    }

    fn default_options() -> AnalysisOptions {
        AnalysisOptions {
            index_reduction_method: "pantelides".to_string(),
            tearing_method: "first".to_string(),
            quiet: true,
        }
    }

    #[test]
    fn test_index_reduction_simple_constraint() {
        // der_x = y   (differential equation)
        // x = 1       (constraint — no der, creates index-2 system)
        // After reduction: constraint should be replaced with x expressed in terms of y
        let equations = vec![
            Equation::Simple(make_der_var("x"), make_var("y")),
            Equation::Simple(make_var("x"), Expression::Number(1.0)),
        ];
        let assigned_var = vec![Some(0), None]; // eq1 gets der_x, eq2 unassigned
        let assigned_eq = vec![Some(0), None];  // der_x assigned, others unassigned
        let unknown_list = vec!["der_x".to_string(), "x".to_string(), "y".to_string()];
        let state_vars = vec!["x".to_string()];

        let result = try_index_reduction(
            &equations,
            &assigned_var,
            &assigned_eq,
            &unknown_list,
            &state_vars,
            &default_options(),
        );
        // Should produce modified equations with index reduction applied
        assert!(result.is_some(), "Expected index reduction to apply");
    }

    #[test]
    fn test_index_reduction_already_index_one() {
        // der_x = -x  (already proper ODE, index-1)
        let equations = vec![
            Equation::Simple(make_der_var("x"), make_binary(
                make_num(0.0), Operator::Sub, make_var("x"),
            )),
        ];
        let assigned_var = vec![Some(0)];
        let assigned_eq = vec![Some(0)];
        let unknown_list = vec!["der_x".to_string(), "x".to_string()];
        let state_vars = vec!["x".to_string()];

        let result = try_index_reduction(
            &equations,
            &assigned_var,
            &assigned_eq,
            &unknown_list,
            &state_vars,
            &default_options(),
        );
        // No constraint equations → no reduction needed
        assert!(result.is_none(), "Expected no index reduction for index-1 system");
    }

    #[test]
    fn test_solve_residual_linear_bare_var() {
        // var - 3 = 0  →  var = 3
        let expr = make_binary(make_var("z"), Operator::Sub, Expression::Number(3.0));
        let result = solve_residual_linear(&expr, "z");
        assert!(result.is_some());
    }

    #[test]
    fn test_solve_residual_linear_var_plus_rest() {
        // z + 5 = 0  →  z = -5
        let expr = make_binary(make_var("z"), Operator::Add, Expression::Number(5.0));
        let result = solve_residual_linear(&expr, "z");
        assert!(result.is_some());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `rtk cargo test -p rustmodlica -- --nocapture`
Expected: new tests + all existing tests pass

- [ ] **Step 3: Commit**

```bash
rtk git add jit-compiler/src/analysis/blt/helpers.rs
rtk git commit -m "test: add unit tests for index reduction and linear solver

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Gap 2: Stream Semantics

### Task 2.1: Fix misleading warnings in builtin_policy_stream.rs

**Files:**
- Modify: `jit-compiler/src/jit/translator/expr/builtin_policy_stream.rs:1-26`

- [ ] **Step 1: Rewrite warnings to reflect actual implementation**

Replace the `warn_stream_semantics_once` function body (lines 4-26) with:

```rust
pub(super) fn warn_stream_semantics_once(kind: &'static str) {
    static INSTREAM_WARNED: OnceLock<()> = OnceLock::new();
    static ACTUAL_WARNED: OnceLock<()> = OnceLock::new();
    static PEER_WARNED: OnceLock<()> = OnceLock::new();
    match kind {
        "inStream" => {
            let _ = INSTREAM_WARNED.get_or_init(|| {
                eprintln!("[stream] inStream(): using MSL 3.1 flow-weighted mixing formula")
            });
        }
        "actualStream" => {
            let _ = ACTUAL_WARNED.get_or_init(|| {
                eprintln!("[stream] actualStream(): using MSL 3.1 semantics (positive flow → self, negative → inStream)")
            });
        }
        "peerMissing" => {
            let _ = PEER_WARNED.get_or_init(|| {
                eprintln!("[stream] stream peer/flow mapping not found, fallback to passthrough for this model path")
            });
        }
        _ => {}
    }
}
```

- [ ] **Step 2: Build check**

Run: `rtk cargo build -p rustmodlica`
Expected: 0 errors

- [ ] **Step 3: Verify tests pass**

Run: `rtk cargo test -p rustmodlica -- --nocapture`
Expected: 88/88 pass

- [ ] **Step 4: Commit**

```bash
rtk git add jit-compiler/src/jit/translator/expr/builtin_policy_stream.rs
rtk git commit -m "fix: correct misleading stream semantics warnings

The JIT already implements full MSL 3.1 stream mixing formula.
Warnings incorrectly claimed 'minimal semantics' / 'passthrough'.
Updated to reflect actual implementation.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2.2: Check interpreter path for stream functions

**Files:**
- Investigate: `jit-compiler/src/jit/compile.rs`

- [ ] **Step 1: Search for interpreter stream handling**

Run: `grep -n "inStream\|actualStream\|instream\|actualstream" jit-compiler/src/jit/compile.rs`
If no matches found: interpreter path does not handle stream functions (acceptable gap — interpreter is for <10 equations, <5 states and stream models exceed those limits).

- [ ] **Step 2: If found, verify correctness; if not found, document as known gap**

No code change required. The interpreter is a limited fallback for trivial models. Stream models have flow variables and peer iteration which inherently require JIT tier.

- [ ] **Step 3: Commit (no-op, just documentation)**

No commit needed unless grep finds issues.

---

## Gap 3: Expandable Connector

### Task 3.1: Add `is_expandable` to AST Model

**Files:**
- Modify: `jit-compiler/src/ast.rs:61-90` (Model struct)
- Modify: `jit-compiler/src/ast.rs:33-57` (From<Function> for Model)

- [ ] **Step 1: Add field to Model struct**

In `jit-compiler/src/ast.rs`, add after line 67 (`pub is_block: bool,`):
```rust
    pub is_expandable: bool,
```

- [ ] **Step 2: Add default in From<Function> impl**

In `jit-compiler/src/ast.rs`, inside the `impl From<Function> for Model` block (line 33), add after `is_block: false,`:
```rust
            is_expandable: false,
```

- [ ] **Step 3: Check all Model construction sites compile**

Run: `rtk cargo build -p rustmodlica 2>&1 | head -80`
Expected: compilation errors listing every place that constructs Model without `is_expandable`. Fix each by adding `is_expandable: false,` at the construction site.

This will affect:
- `jit-compiler/src/parser/model_parse.rs` (parse_model, ~line 100 area)
- `jit-compiler/src/parser/decl_parse.rs` (short_class_definition, ~line 504 area)
- `jit-compiler/src/parser/entry.rs` (make_alias_model output)
- `jit-compiler/src/flatten/` (multiple places that construct synthetic Models)
- `jit-compiler/src/compiler/` (test helpers)

- [ ] **Step 4: Iterate — fix compilation errors one file at a time**

Run build after each fix until 0 errors.

- [ ] **Step 5: Final build and test check**

Run: `rtk cargo test -p rustmodlica -- --nocapture`
Expected: 88/88 pass

- [ ] **Step 6: Commit**

```bash
rtk git add jit-compiler/src/ast.rs jit-compiler/src/parser/model_parse.rs jit-compiler/src/parser/decl_parse.rs jit-compiler/src/parser/entry.rs jit-compiler/src/flatten/
rtk git commit -m "feat: add is_expandable field to Model AST struct

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3.2: Capture `expandable` keyword in parser

**Files:**
- Modify: `jit-compiler/src/parser/model_parse.rs:59-84`
- Modify: `jit-compiler/src/parser/entry.rs:25-83`
- Modify: `jit-compiler/src/parser/decl_parse.rs:490-530`

- [ ] **Step 1: Set is_expandable in parse_model**

In `jit-compiler/src/parser/model_parse.rs`, the `parse_model` function at line 63 has a `for p in prefix_pair.into_inner()` loop that checks `p.as_str().trim()`. After the `} else if p.as_str().trim() == "block" {` block (line 81), add:

```rust
        } else if p.as_str().trim() == "expandable" {
            // captured but used downstream when connector is also set
```

Then after line 84 (closing brace of the for loop), where `is_connector` is already determined, add:

```rust
    let is_expandable = is_connector && prefix_text_contains_expandable;

```

Actually, simpler approach: check `p.as_str().trim() == "expandable"` inside the loop and set a local:

In the prefix loop (lines 69-84), add before the loop:
```rust
    let mut is_expandable = false;
```

In the loop, after line 83 (the `else if` chain for block):
```rust
        } else if p.as_str().trim() == "expandable" {
            is_expandable = true;
```

Then in the Model construction (around line 100+), add `is_expandable,` alongside the other boolean fields.

- [ ] **Step 2: Same for short_class_definition in entry.rs**

In `jit-compiler/src/parser/entry.rs`, the `class_prefixes` handling at lines 33-44. Add expandable detection:

In the `Rule::class_prefixes` match arm (line 33), after existing prefix checks, add:
```rust
                            if ptext.contains("expandable") {
                                is_expandable = true;
                            }
```

And in the short class definition handling in `decl_parse.rs` (around line 500), add `is_expandable: prefixes.contains("expandable")` to the Model construction.

- [ ] **Step 3: Build and test**

Run: `rtk cargo test -p rustmodlica -- --nocapture`
Expected: 0 errors, 88/88 pass

- [ ] **Step 4: Commit**

```bash
rtk git add jit-compiler/src/parser/model_parse.rs jit-compiler/src/parser/entry.rs jit-compiler/src/parser/decl_parse.rs
rtk git commit -m "feat: capture expandable keyword in parser

Sets is_expandable=true on Model when class_prefixes contains 'expandable'.
Previously parsed by pest grammar but discarded.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3.3: Flatten — dynamic member injection for expandable connectors

**Files:**
- Modify: `jit-compiler/src/flatten/connections.rs` (equations_for_connections)
- Modify: `jit-compiler/src/flatten/flattener_impl.rs` or `jit-compiler/src/flatten/inheritance.rs` (mark expandable instances)

- [ ] **Step 1: Track expandable connector instances during flatten**

In the flatten pipeline, when instantiating a connector whose class has `is_expandable = true`, record the instance path in a new HashSet on FlattenedModel. Add to `jit-compiler/src/flatten/structures.rs`:

```rust
    /// Instance paths for expandable connector instances (populated during flatten).
    pub expandable_instances: HashSet<String>,
```

Initialize to `HashSet::new()` in all construction sites (similar to `stream_peer_map`).

- [ ] **Step 2: During flatten instantiation, populate expandable_instances**

In the flatten instancing code (where components are instantiated), after resolving the class type and checking `is_expandable`, insert the instance path:

```rust
if model.is_expandable {
    flat.expandable_instances.insert(instance_path.clone());
}
```

This requires tracing the exact instancing path. The key location is likely in `flatten/inheritance.rs` or `flatten/decl_expand/flattener_impl.rs` where declarations are expanded into the flattened model.

- [ ] **Step 3: In equations_for_connections, inject members for expandable instances**

In `jit-compiler/src/flatten/connections.rs`, in `equations_for_connections` (around line 232), add logic after the existing connector member iteration. When processing a connection between `(a_path, b_path)`:

- If `a_path` refers to an expandable connector instance (`flat.expandable_instances.contains(a_path)`) and `b_path` refers to a regular connector/component with members, add `b_path`'s members to `a_path`'s namespace and generate equality equations.
- Similarly for the reverse direction.
- For bidirectional expandable connections, merge members from both sides.

Pseudo-code placement (after line ~280, inside the `for (a_path, b_path) in connections` loop):
```rust
// Expandable connector: inject members from non-expandable side
let a_is_expandable = flat.expandable_instances.contains(a_path);
let b_is_expandable = flat.expandable_instances.contains(b_path);
if a_is_expandable || b_is_expandable {
    if a_is_expandable && !b_is_expandable {
        // Inject b's members into a's namespace
        // For each declaration under b_path prefix, create equivalent under a_path prefix
        inject_expandable_members(flat, a_path, b_path, &mut potential_eqs);
    } else if b_is_expandable && !a_is_expandable {
        inject_expandable_members(flat, b_path, a_path, &mut potential_eqs);
    } else {
        // Both expandable: cross-inject
        inject_expandable_members(flat, a_path, b_path, &mut potential_eqs);
        inject_expandable_members(flat, b_path, a_path, &mut potential_eqs);
    }
}
```

Helper function in the same file:
```rust
fn inject_expandable_members(
    flat: &FlattenedModel,
    expandable_path: &str,
    source_path: &str,
    potential_eqs: &mut Vec<Equation>,
) {
    let source_prefix = format!("{}_", source_path);
    let target_prefix = format!("{}_", expandable_path);
    for decl in &flat.declarations {
        if let Some(suffix) = decl.name.strip_prefix(&source_prefix) {
            let target_name = format!("{}{}", target_prefix, suffix);
            if !flat.declarations.iter().any(|d| d.name == target_name) {
                potential_eqs.push(Equation::Simple(
                    Expression::var(&target_name),
                    Expression::var(&decl.name),
                ));
            }
        }
    }
}
```

- [ ] **Step 4: Build and fix compilation**

Run: `rtk cargo build -p rustmodlica`
Fix any compilation errors from the new field/function.

- [ ] **Step 5: Run full test suite**

Run: `rtk cargo test -p rustmodlica -- --nocapture`
Expected: 88/88 pass (no regression)

- [ ] **Step 6: Commit**

```bash
rtk git add jit-compiler/src/flatten/structures.rs jit-compiler/src/flatten/connections.rs jit-compiler/src/flatten/flattener_impl.rs
rtk git commit -m "feat: implement expandable connector dynamic member injection in flatten

When connecting an expandable connector instance to a non-expandable
component, inject the source component's members into the expandable
instance namespace. Supports bidirectional expandable connections.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3.4: Add expandable connector test

**Files:**
- Create: `jit-compiler/TestLib/expandable_basic.mo` (integration test)

- [ ] **Step 1: Create test Modelica model**

Create `jit-compiler/TestLib/expandable_basic.mo`:
```modelica
expandable connector C
end C;

model Container
  C c;
end Container;

model Source
  Real x = 1.0;
  Real y = 2.0;
end Source;

model ExpandableTest
  Container cont;
  Source src;
equation
  connect(cont.c, src);
end ExpandableTest;
```

- [ ] **Step 2: Validate the test model**

Run: `cargo run -p rustmodlica -- --validate jit-compiler/TestLib/expandable_basic.mo`
Expected: validation passes

- [ ] **Step 3: Verify no regression in TestLib batch**

Run: `pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1`
Expected: all positive cases pass

- [ ] **Step 4: Commit**

```bash
rtk git add jit-compiler/TestLib/expandable_basic.mo
rtk git commit -m "test: add expandable connector basic integration test

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Final Verification

After all tasks complete:

- [ ] Run full test suite: `rtk cargo test -p rustmodlica -- --nocapture` → 88+/88 pass
- [ ] Run TestLib validation: `pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1` → 171+/171 pass
- [ ] Run quick regression: `pwsh -File ./run_jit_rules_full_regress.ps1` → all pass
- [ ] Build with sundials: `rtk cargo build -p rustmodlica --features sundials` → 0 errors
