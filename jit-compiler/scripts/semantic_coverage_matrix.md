# Semantic Coverage Matrix

Last updated: 2026-03-24

## Scope

This matrix tracks semantic coverage used by both `README.md` and `update.md`.
Target: semantic coverage >= 98%, Modelica 3.4 core coverage = 100%.

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

## Summary

- Passed items: 15
- Total tracked items: 15
- Semantic coverage snapshot: 100.0%
- Target semantic coverage: >= 98.0%
- Modelica 3.4 core target: 100.0%
