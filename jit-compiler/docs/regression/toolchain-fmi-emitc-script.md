# 工具链FMI EmitC Script回归 / Toolchain FMI EmitC Script Regression

## 范围 / Scope

覆盖非核心仿真输出与工具链相关能力。  
Covers non-core runtime outputs and toolchain-level functionality.

- Script 模式命令流 / script mode command flow
- C 代码生成（`emit-c`） / C code emission (`emit-c`)
- FMU 生成（`emit-fmu`） / FMU emission (`emit-fmu`)
- 产物存在性与命令级 pass/fail / artifact existence checks and command-level pass/fail

## 关键能力映射 / Key Capability Mapping

- Script 命令覆盖（`load`、`setParameter`、`simulate`、`plot` 等） / script command coverage
- 常规与字符串 external function 场景下的 C 后端输出 / C backend generation for normal and string external function scenarios
- FMI 2.0 产物完整性 / FMI 2.0 artifact generation integrity

## 代表用例 / Representative Cases

### Script模式 / Script Mode

`run_regression.ps1` includes:

- `ScriptMode/init_dummy`
- `ScriptMode/init_with_param_setparam`
- `ScriptMode/multi_model_use`
- `ScriptMode/setStartValue`
- `ScriptMode/getParameter`
- `ScriptMode/setStopTime`
- `ScriptMode/setTolerance`
- `ScriptMode/saveResult`
- `ScriptMode/plot`
- `ScriptMode/eval`
- `ScriptMode/loadClass`
- `ScriptMode/switchModel`

以上期望均为 `pass`。  
All expected: `pass`.

### Emit-C与FMI / Emit-C / FMI

| Case | Expect | Risk Focus |
|---|---|---|
| `EmitC/RecursiveFunc` | pass | C artifact generation |
| `FUNC-7/EmitC/StringArgExtFunc` | pass (script-level rule) | String ABI path in generated C |
| `FMI/emit-fmu` | pass | FMU artifacts generated |

## 执行命令 / Execution Command

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File d:/source/repos/rustmodlica/run_regression.ps1
```

独立 emit 检查 / Standalone emit checks:

```powershell
cargo run -p rustmodlica --bin rustmodlica --release -- --emit-c=build_regress_emit TestLib/RecursiveFunc
cargo run -p rustmodlica --bin rustmodlica --release -- --emit-fmu=build_regress_fmu TestLib/SimpleTest
```

## 判定标准 / Verdict Criteria

- Script 用例退出行为需符合预期 / script mode cases must match declared expected exit behavior
- Emit-C 需生成目标 C 文件 / Emit-C must generate expected C artifact
- `StringArgExtFunc` 特殊规则 / special rule:
  - 命令可走 JIT 失败路径 / command may fail JIT route
  - 但必须产出 C 文件且包含字符串 ABI 关键符号 / but generated C file must exist and contain string ABI indicators
- FMI 必须同时产出 `modelDescription.xml` 与 `fmi2_cs.c` / FMI artifacts must both exist

## 常见失败模式 / Common Failure Modes

- Script 命令解析或执行回归 / script command parser/executor regression
- 命令成功但产物缺失 / missing output artifact after successful command
- 生成 C 内容缺少 ABI 关键符号 / generated C content missing expected ABI symbols
- FMI 产物不完整 / FMI emission incomplete artifacts
