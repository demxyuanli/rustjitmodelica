# MSL与ModelicaTest目录回归 / MSL And ModelicaTest Directory Regression

## 范围 / Scope

覆盖大规模目录级回归执行。  
Covers large-scale directory regression runs.

- MSL 与 ModelicaTest 批量执行 / MSL and ModelicaTest batch execution
- 自一致性约束检查 / self-consistency invariant checks
- event-scan matrix 一致性检查 / event-scan matrix consistency checks

## 关键能力映射 / Key Capability Mapping

- 库级大样本兼容性 / library-wide compatibility over many models
- 大批量回归稳定性与返回码门禁 / large-batch stability and return-code gate
- event-scan 非确定性与配置错误控制 / event-scan nondeterminism/configuration error control

## 代表检查项 / Representative Checks

| Check | Expect | Risk Focus |
|---|---|---|
| `DIR-MSL+ModelicaTest` | pass | Directory-wide run health |
| `EVENT-SCAN-MATRIX` | pass | nondeterministic/config_error must be zero |

来自分析基线的参考项 / Reference from analysis baseline:

- TestLib + ScriptMode + EmitC + FMI regression aggregate
- MSL aggregate coverage
- ModelicaTest aggregate coverage

## 执行命令 / Execution Command

目录级回归 / Directory regression:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File d:/source/repos/rustmodlica/run_modelica_dir_regression.ps1 -Root d:/source/repos/rustmodlica -MaxCases 0 -AllLibraryMo -NewtonCountsAsFailed
```

主脚本入口（脚本可用时包含 DIR 与 EVENT-SCAN 检查）  
Main script entry (includes DIR and EVENT-SCAN checks when scripts are available):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File d:/source/repos/rustmodlica/run_regression.ps1
```

## 判定标准 / Verdict Criteria

- 目录回归命令退出码为 `0` / directory regression command exits with code `0`
- event-scan matrix 报告满足 / report satisfies:
  - `nondeterministic=0`
  - `config_error=0`
- matrix 检查依赖的报告或 csv 缺失视为失败 / missing report/csv required by matrix check is treated as failure

## 常见失败模式 / Common Failure Modes

- 批量模型运行中断导致聚合退出码非 `0` / batch model run interruptions causing non-zero aggregate exit
- event-scan matrix 输出缺失或格式异常 / event-scan matrix output missing or malformed
- 扫描模型出现非确定性事件行为 / non-deterministic event behavior in scanned models
- event-scan 脚本执行出现配置级错误 / configuration-level errors in event-scan matrix script execution
