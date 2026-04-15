# 核心仿真回归 / Core Simulation Regression

## 范围 / Scope

覆盖基础仿真主路径能力。  
Covers baseline simulation capabilities.

- 初始化方程与参数初始化 / initialization equations and parameter initialization
- ODE 求解路径与代数环 Newton 路径 / ODE solve path and algebraic loop/Newton path
- 方程与算法中的核心语义执行 / core language execution in equations/algorithms
- 数学内建函数与数组基础行为 / builtin math and array baseline behavior

## 关键能力映射 / Key Capability Mapping

- Init 与初始方程处理 / init and initial equation handling
- BLT 排序与 solvable block / BLT ordering and solvable blocks
- 求解器基础路径（`rk4`，含 `rk45` 样例） / solver baseline (`rk4`, optional `rk45` path sample)
- 代数诊断稳定性 / algebraic diagnostics stability

## 代表用例 / Representative Cases

| Case | Expect | Risk Focus |
|---|---|---|
| `TestLib/InitDummy` | pass | Initial equation ordering |
| `TestLib/InitWithParam` | pass | Parameter propagation |
| `TestLib/InitAlg` | pass | Algorithm in init stage |
| `TestLib/InitWhen` | pass | Init + event interaction |
| `TestLib/JacobianTest` | pass | Jacobian generation path |
| `TestLib/AlgebraicLoop2Eq` | pass | Small algebraic loop solve |
| `TestLib/SolvableBlock4Res` | pass | Newton residual block |
| `TestLib/SolvableBlockMultiRes` | pass | Multi residual solve |
| `TestLib/MathBuiltins` | pass | Builtin math runtime behavior |
| `TestLib/ArrayTest` | pass | Array expression execution |
| `TestLib/ArrayLoopTest` | pass | Array + loop integration |
| `TestLib/Pendulum` | pass | Index reduction path |

## 执行命令 / Execution Command

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File d:/source/repos/rustmodlica/run_regression.ps1
```

The script defaults to full-optimization mode and does not override `RUSTMODLICA_JIT_CODEGEN_CACHE` / `RUSTMODLICA_AOT_NATIVE_LOAD`. Use `-DisableNativeAccelForStability` to force both to `0` when bisecting native reuse crashes (for example, stale disk codegen blobs or a mismatched default `aot_archive.bin`). Prefer `-File` (as above) so the process exit code is 0 or 1 for CI.

单用例快速检查 / Single-case quick check:

```powershell
cargo run -p rustmodlica --bin rustmodlica --release -- TestLib/InitDummy
```

Pendulum 指标约简检查 / Pendulum with index reduction:

```powershell
cargo run -p rustmodlica --bin rustmodlica --release -- --index-reduction-method=dummyDerivative TestLib/Pendulum
```

## 判定标准 / Verdict Criteria

- 期望 `pass` 的用例退出码必须为 `0` / expected pass cases must exit with code `0`
- 不应出现用例期望反转 / no case should flip expected pass/fail label
- 求解器敏感场景应正常生成 CSV 且无致命运行时错误 / solver-sensitive baselines should generate CSV without fatal runtime errors

## 常见失败模式 / Common Failure Modes

- 初始化不匹配或方程过定/欠定 / initialization mismatch or under/over-determined setup
- Newton 在 solvable block 中不收敛 / Newton non-convergence in solvable block
- Jacobian 在符号与数值路径切换时报错 / Jacobian path error in symbolic/numeric transitions
- 约束系统的指标约简路径失败 / index reduction route failure for constrained systems
