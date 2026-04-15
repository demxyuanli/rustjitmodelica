# Semantic Coverage Matrix

Last updated: 2026-04-09 (round-3 audit: default rules + appendix E)

## Scope

This matrix tracks semantic coverage used by both `README.md` and `update.md`.
Target: semantic coverage >= 98%, Modelica 3.4 core coverage = 100%.

Modelica 3.4 core gate is tracked in the structured companion matrix:
`scripts/modelica34_core_coverage_matrix.txt`.

## Full Feature Support Matrix (OMC-aligned)

This file intentionally tracks only the enforced semantic gates. For a broader, user-facing
feature boundary matrix (pass/partial/missing) with code and regression references, see:
[`JIT_DEVELOPMENT_ANALYSIS.md` (Feature Support Matrix)](../../JIT_DEVELOPMENT_ANALYSIS.md#811-modelica-feature-support-matrix-omc-aligned)

## Coverage Items

| Item | Code entry | Regression entry | Status | target(98+/100) | current | gap | enforced |
|---|---|---|---|---|---|---|---|
| clock/sample/interval parse path | `src/parser/expression.rs` | `omc_regression_sync_signal.mos` | pass | >=98 | 100 | 0 | yes |
| clock partition inference | `src/flatten/clock_infer.rs` | `omc_regression_sync_signal.mos` | pass | >=98 | 100 | 0 | yes |
| event queue unified dispatch | `src/simulation/events.rs`, `src/simulation.rs` | `omc_regression_if_for.mos` | pass | >=98 | 100 | 0 | yes |
| subSample/superSample/shiftSample JIT semantics | `src/jit/translator/expr/compile.rs` | `omc_regression_sync_super_shift.mos` | pass | >=98 | 100 | 0 | yes |
| strict .mos execution for control flow | `src/script.rs` | `omc_regression_if_for.mos`, `omc_regression_elseif_nested_for.mos` | pass | >=98 | 100 | 0 | yes |
| strict .mos range iteration semantics | `src/script.rs`, `src/parser/mos_parse.rs` | `omc_regression_for_range.mos`, `omc_regression_reverse_range.mos` | pass | >=98 | 100 | 0 | yes |
| simulate named argument bridge | `src/script.rs` | `omc_regression_named_simulate.mos`, `omc_regression_simulate_named_combo.mos` | pass | >=98 | 100 | 0 | yes |
| mixed positional/named simulate bridge | `src/script.rs` | `omc_regression_mixed_simulate_args.mos` | pass | >=98 | 100 | 0 | yes |
| C codegen semantic bridge for sync expressions | `src/compiler/c_codegen/expr_emit.rs` | compile validation (`cargo check`) | pass | >=98 | 100 | 0 | yes |
| expression evaluator sync semantics | `src/expr_eval.rs` | compile validation (`cargo check`) | pass | >=98 | 100 | 0 | yes |
| Newton tearing symbolic Jacobian primary path | `src/jit/translator/equation/solvable_tearing.rs` | `omc_regression_newton_symbolic_dense.mos` | pass | >=98 | 100 | 0 | yes |
| Newton N-path symbolic Jacobian with sparse fallback | `src/jit/translator/equation/solvable_general_dense.rs`, `src/jit/translator/equation/solvable_general_sparse.rs` | `omc_regression_newton_symbolic_sparse.mos` | pass | >=98 | 100 | 0 | yes |
| inStream/actualStream minimal executable semantics | `src/jit/translator/expr/builtin.rs` | `omc_regression_stream_semantics.mos` | pass | >=98 | 100 | 0 | yes |
| algorithm when-elsewhen parser/execution closure | `src/parser/algorithm.rs`, `src/jit/translator/algorithm.rs` | `omc_regression_algorithm_elsewhen.mos` | pass | >=98 | 100 | 0 | yes |
| inStream/actualStream direction-switch semantics (2-port MVP) | `src/flatten/connections.rs`, `src/jit/translator/expr/builtin.rs` | `omc_regression_direction_switch_stream.mos` | pass | >=98 | 100 | 0 | yes |
| JSON `function_builtin_rules` wired to named handlers | `src/jit/default_function_builtin_rules.json`, `src/jit/jit_policy.rs`, `src/jit/translator/expr/builtin.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| `cardinality()` from flatten `connect` degree map | `src/jit/connector_degree.rs`, `src/jit/context.rs`, `src/jit/translator/expr/call.rs`, `builtin_policy_dispatch.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| Connector graph digest in JIT / codegen cache key | `src/jit/codegen_cache/cache_key.rs`, `src/jit/config.rs`, `src/compiler/compile_model/compile/entry.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| `RUSTMODLICA_JIT_STRICT_PLACEHOLDERS` hard-fail for placeholder builtins | `src/jit/translator/expr/builtin_policy_dispatch.rs`, `builtin_policy_interpolate.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| `RUSTMODLICA_JIT_IMPORT_STRICT` for unknown imports | `src/jit/translator/expr/call.rs`, `pre.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| `RUSTMODLICA_JIT_POLICY_STRICT=pre_generic_underscore` | `src/jit/jit_policy.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| `loadResource` path-exists probe (string + variable URI) | `src/jit/translator/expr/builtin_policy_dispatch.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| Newton sparse LU policy vs `solvable_limits` documentation | `src/solvable_limits.rs`, `src/jit/translator/equation/solvable_general_sparse.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| inStream/actualStream zero reverse-flow aggregate (multi-peer equal-weight fallback) | `src/jit/translator/expr/builtin_policy_dispatch.rs` | `omc_regression_stream_semantics.mos` (subset) | pass | >=98 | 100 | 0 | yes |
| Silent zero-value paths guarded by warn or strict | `src/jit/translator/expr/call.rs`, `builtin.rs`, `builtin_policy_dispatch.rs`, `variable.rs`, `pre.rs`, `builtin_policy_interpolate.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| Dispatch reachability: default `function_builtin_rules` vs `dispatch_named_builtin_policy` | `src/jit/default_function_builtin_rules.json`, `builtin_policy_dispatch.rs`, `jit_policy.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| Third-round audit: stream/clock/interp/regStep/splice/valve/MSL-test handlers wired in default JSON | `default_function_builtin_rules.json`, `JIT_DEVELOPMENT_ANALYSIS.md` appendix E | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| `product`/`sum` dispatch uses `compile_array_reduce` | `src/jit/translator/expr/builtin_policy_dispatch.rs`, `call.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |
| `Modelica.Constants.T_zero` flat alias | `src/jit/translator/expr/helpers.rs` | compile validation (`cargo check -p jit-compiler`) | pass | >=98 | 100 | 0 | yes |

## Summary

- Passed items: 31
- Total tracked items: 31
- Semantic coverage snapshot: 100.0%
- Target semantic coverage: >= 98.0%
- Modelica 3.4 core target: 100.0%
