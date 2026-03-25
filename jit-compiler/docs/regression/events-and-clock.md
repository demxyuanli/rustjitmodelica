# 事件与时钟回归 / Events And Clock Regression

## 范围 / Scope

覆盖事件语义与时钟语义主路径。  
Covers event semantics and clocked behavior.

- `when`/`elsewhen` 执行 / `when`/`elsewhen` execution
- `pre`/`edge`/`change` 语义 / `pre`/`edge`/`change` semantics
- `reinit` 行为 / `reinit` behavior
- 时钟分区与多速率稳定性 / clock partition and multi-rate stability

## 关键能力映射 / Key Capability Mapping

- 事件触发正确性与过零处理 / event trigger correctness and crossing handling
- 时钟分区执行可观测性 / clocked partition execution visibility
- 时钟用例重复执行确定性 / deterministic repeated outputs for clocked cases
- 子采样/超采样/移相/反向采样稳定性 / sub/super/shift/back sample stability

## 代表用例 / Representative Cases

| Case | Expect | Risk Focus |
|---|---|---|
| `TestLib/WhenTest` | pass | Event branch trigger |
| `TestLib/BouncingBall` | pass | Zero crossing + reinit |
| `TestLib/PreEdgeChange` | pass | Discrete history semantics |
| `TestLib/ReinitTest` | pass | State reset during events |
| `TestLib/ClockedPartitionTest` | pass | Clock partition path |
| `TestLib/ClockedTwoRates` | pass | Multi-rate scheduling |
| `TestLib/HoldPreviousTest` | pass | Hold/previous semantics |
| `TestLib/IntervalClockTest` | pass | Interval-driven clock |
| `TestLib/SubSuperShiftSampleTest` | pass | Derived clock operators |
| `TestLib/ClockedStartAndShiftTest` | pass | start+shift derived clock |
| `TestLib/ClockedNestedSubSuperTest` | pass | nested sub/super derived clock |
| `TestLib/ClockedStartAndSubSampleTest` | pass | start+subSample derived clock |
| `TestLib/ClockedStartAndBackSampleTest` | pass | start+backSample derived clock |
| `TestLib/ClockedStartShiftThenBackSampleTest` | pass | shift then backSample derived clock |
| `TestLib/ClockedStartShiftThenSuperSampleTest` | pass | shift then superSample derived clock |
| `TestLib/ClockedStartAndSuperSampleTest` | pass | start+superSample derived clock |
| `TestLib/ClockedStartShiftThenSubSampleTest` | pass | shift then subSample derived clock |
| `TestLib/BackSampleClockTest` | pass | `backSample(sample(T), n)` JIT path |
| `TestLib/ClockedInvalidFactorClampTest` | pass | invalid factor clamp (n<=0 => 1) |
| `ModelicaTest.JitStress.SyncOmCompare` | pass | Sync compare stress |

## 执行命令 / Execution Command

执行全量回归脚本 / Run full script regression:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File d:/source/repos/rustmodlica/run_regression.ps1
```

Clock backend 信息检查 / Clock backend info check:

```powershell
cargo run -p rustmodlica --bin rustmodlica --release -- --backend-dae-info TestLib/ClockedPartitionTest
```

确定性检查遵循脚本逻辑：同一模型执行两次并比较输出文件哈希。  
Determinism check follows script logic: run same model twice and compare output file hashes.

## 判定标准 / Verdict Criteria

- `pass` 用例退出码必须为 `0` / pass cases exit code must be `0`
- `ClockedPartitionTest` backend 输出需包含 clocked 相关信息 / backend output should contain clocked-related information
- 确定性集合重复运行 CSV 哈希应一致 / determinism set must produce identical repeated CSV hashes
- event-scan matrix 不应出现 nondeterministic 状态 / no nondeterministic status in event-scan matrix checks

## 常见失败模式 / Common Failure Modes

- 事件抖动导致触发计数不稳定 / event jitter causing unstable trigger counts
- 时钟分区调度不一致 / clock partition scheduling inconsistency
- 时钟模型重复运行非确定性 / non-deterministic repeated runs in clocked models
- sub/super/shift/back sample 行为缺失或退化 / missing or degraded behavior in sub/super/shift/back sample handling
