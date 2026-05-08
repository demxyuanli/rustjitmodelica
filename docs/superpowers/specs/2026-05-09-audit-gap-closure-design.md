# Audit Gap Closure — Design Spec

> Date: 2026-05-09
> Based on: `docs/JIT_COMPILER_AUDIT_VS_OMC_DYMOLA.md`
> Strategy: Sequential (A) — smallest to largest

Three gaps from the audit report, implemented in order.

---

## Gap 1: DAE Index Reduction Hardening

### Current state

Code exists in `analysis/blt/helpers.rs`:
- Phase 1: symbolic linear solving (`solve_residual_linear`, handles `coeff*var` form)
- Phase 1b: Pantelides-style repeated differentiation (configurable max order, default 3)
- Phase 2: dummy derivative substitution (replaces higher-order der with algebraic dummy variable)
- BLT sort loop (`analysis/blt/sort.rs`) iteratively applies index reduction (max 20 rounds, configurable)
- **Default is `"none"`** — users must pass `--index-reduction-method=pantelides` to enable

### Changes

1. **Default-on** — `compiler/mod.rs`: change `index_reduction_method` default from `"none"` to `"pantelides"`. Keep `--index-reduction-method=none` for opt-out.

2. **Tests** — `analysis/blt/helpers.rs`: add `#[cfg(test)]` module with cases:
   - Single constraint equation (index-2 → index-1)
   - Already index-1 (no change)
   - Multiple constraints in one model

3. **Linear solver extension** — `analysis/blt/blt_expr.rs` `solve_residual_linear`: add `coeff*var - rest` and `rest - coeff*var` patterns, and `var + rest` simple additive form.

4. **Diagnostics** — when index > 1 and method is explicitly `none`, warn the user. When nonlinear constraint cannot be handled, emit warning instead of silent skip.

### Files

| File | Change |
|------|--------|
| `compiler/mod.rs` | Default `index_reduction_method` → `"pantelides"` |
| `analysis/blt/helpers.rs` | Add tests, enhanced diagnostics |
| `analysis/blt/blt_expr.rs` | Extend `solve_residual_linear` |

### Validation

- Existing unit tests pass (88/88)
- TestLib validation pass (171/171)
- Manual: run a known high-index model without `--index-reduction-method` flag → index reduction activates automatically

---

## Gap 2: Stream Semantics — Remove Misleading Warnings

### Discovery

The JIT translator (`jit/translator/expr/builtin_policy_dispatch.rs`) already implements the full MSL 3.1 stream mixing formula:

```
inStream(h)    = Σ max(-m_j, 0) * h_j / Σ max(-m_j, 0)
actualStream(h) = if m > 0 then h else inStream(h)
```

With zero-flow fallback to equal average and proper peer iteration from `stream_connection_set`. The flatten layer (`flatten/connections.rs`) correctly builds both `stream_peer_map` (1:1 peer mapping) and `stream_connection_set` (connected components). Passthrough only triggers when peers genuinely don't exist — which is correct behavior.

The audit report's "minimal semantics" assessment was inaccurate — it was based on the warning messages, not the actual code.

### Changes

1. **Remove misleading warnings** — `builtin_policy_stream.rs`: delete or correct the `warn_stream_semantics_once` messages that claim "minimal semantics in JIT" when the implementation is actually complete.

2. **Tests** — add unit tests for the stream mixing formula:
   - 2-peer mixing (standard case)
   - Single peer
   - Zero-flow fallback to equal average
   - actualStream with positive flow (returns self)
   - actualStream with negative flow (returns inStream)

3. **Interpreter path** — `jit/compile.rs`: verify/fix interpreter handling of `inStream`/`actualStream` if currently skipped.

### Files

| File | Change |
|------|--------|
| `jit/translator/expr/builtin_policy_stream.rs` | Fix/remove misleading warnings |
| `jit/translator/expr/builtin_policy_dispatch.rs` | Possibly no change (formula is correct) |
| `jit/compile.rs` | Check interpreter path for stream functions |

### Validation

- Existing tests pass
- New stream formula tests pass
- Manual: run a thermofluid model with stream variables → no misleading "minimal semantics" warnings

---

## Gap 3: Expandable Connector

### Current state

The `expandable` keyword is parsed by `modelica.pest` (`class_prefixes` rule) but discarded. No AST storage, no flatten logic.

### Modelica semantics

```modelica
expandable connector C end C;

model A
  C c;
end A;

model B
  Real x;
end B;

model Top
  A a;
  B b;
equation
  connect(a.c, b);  // ← a.c gains member x from b
end Top;
```

After connection, `a.c.x` is accessible and equated to `b.x`.

### Changes

**Phase 1: AST + Parser**

- `ast.rs` `Model`: add `is_expandable: bool` (default `false`)
- `parser/model_parse.rs`: in `parse_model`, detect `"expandable"` in class_prefixes, set `is_expandable = true`
- `parser/entry.rs`: same for short class definitions

**Phase 2: Flatten dynamic members**

- During instantiation of an expandable connector instance, initialize with empty member set (no fixed declarations from the connector class)
- In `flatten/connections.rs` `equations_for_connections`: when processing `connect(a.c, b)` where `a.c` is an expandable connector instance and `b` is a non-expandable connector/component, add `b`'s public members to `a.c` and generate equality equations
- Member name mapping: `a.c.x` ↔ `b.x`
- Handle bidirectional case: two expandable connectors connected together

**Phase 3: Tests**

- Simple expandable connector with single-member injection
- Bidirectional expandable connector connection
- Error case: connecting two non-expandable connectors that have incompatible members (no-op, already handled)

### Files

| File | Change |
|------|--------|
| `ast.rs` | Add `is_expandable` field to `Model` |
| `parser/model_parse.rs` | Capture `expandable` keyword |
| `parser/entry.rs` | Same for short class defs |
| `flatten/connections.rs` | Dynamic member injection for expandable instances |
| `flatten/` (instancing path) | Identify expandable instances, defer member resolution |

### Validation

- Unit tests for expandable connector member injection
- TestLib validation pass (no regression)
- Manual: create a minimal model similar to MSL MultiBody Frame pattern → verify members propagate correctly

---

## Implementation Order

1. **DAE index reduction** (smallest, existing code)
2. **Stream semantics** (formula correct, just warnings + tests)
3. **Expandable connector** (largest, new functionality)

Each gap is independently testable and mergable.
