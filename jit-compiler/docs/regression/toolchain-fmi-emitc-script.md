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

## FMI 元数据与校验要点 / FMI metadata and validation notes

- **`modelName`（XML 根）**：保留完整限定 Modelica 名（如 `TestLib/SimpleTest`），便于工具展示与追溯。  
  **`modelName` (root attribute)**: keeps the qualified Modelica name (e.g. `TestLib/SimpleTest`) for display and traceability.

- **`modelIdentifier`（`CoSimulation` / `ModelExchange`）**：须为可移植 C 标识符；由包名最后一段经净化得到，或由 `--fmi-model-id=` / `RUSTMODLICA_FMI_MODEL_ID` 覆盖（CLI 优先于环境变量），可选前缀 `RUSTMODLICA_FMI_MODEL_ID_PREFIX`。  
  **`modelIdentifier`**: portable C-style id; derived from the last path/segment after sanitization, or overridden by `--fmi-model-id=` / `RUSTMODLICA_FMI_MODEL_ID` (CLI wins over env), optional `RUSTMODLICA_FMI_MODEL_ID_PREFIX`.

- **`guid`**：默认每次导出随机 UUID；固定值用 `--fmi-guid=` 或 `RUSTMODLICA_FMI_GUID`（标准 UUID 或 ASCII `alnum`/`_`/`-`）。非法值会导致导出失败。  
  **`guid`**: random UUID by default; pin with `--fmi-guid=` or `RUSTMODLICA_FMI_GUID` (standard UUID or ASCII `alnum`/`_`/`-`). Invalid values fail the export.

- **`generationTool`**：`RUSTMODLICA_FMI_GENERATION_TOOL`，默认 `rustmodlica`。  
  **`generationTool`**: `RUSTMODLICA_FMI_GENERATION_TOOL`, default `rustmodlica`.

- **连续标量变量**：`ScalarVariable` 内含 `<Real/>`，以满足常见 FMI 2.0 XML 校验器对实数类型的期望。  
  **Continuous reals**: each relevant `ScalarVariable` includes `<Real/>` for stricter FMI 2.0 XML validators.

- **Co-Simulation 能力标志**：`canHandleVariableCommunicationStepSize`、`canInterpolateInputs`；以及 `canBeInstantiatedOnlyOncePerProcess`、`canNotUseMemoryManagementFunctions`（显式声明为 `false`）。  
  **CS flags**: variable step CS, input interpolation, and explicit instantiation / memory-management capability attributes.

- **库 API**：`FmiExportOptions`、`emit_fmu_artifacts_with_options` / `emit_fmu_me_artifacts_with_options`；`emit_fmu_artifacts` 仍为默认选项的薄封装。  
  **Library API**: `FmiExportOptions`, `*_with_options` entry points; `emit_fmu_*` without options forwards to `Default`.

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
- `modelDescription.xml` 内容：`fmiVersion="2.0"`、带 `guid` 的根元素、`<CoSimulation>`、`modelIdentifier="SimpleTest"`（对 `TestLib/SimpleTest` 用例，且该段运行前会临时清除进程内 `RUSTMODLICA_FMI_MODEL_ID` / `_PREFIX` / `_GUID` 以保证默认推导）、至少一处 `<Real/>`（与 `run_regression.ps1` 断言一致）  
  **XML content checks** (aligned with `run_regression.ps1`): `fmiVersion="2.0"`, root `guid`, `<CoSimulation>`, `modelIdentifier="SimpleTest"` for SimpleTest (that block clears process `RUSTMODLICA_FMI_MODEL_ID`, `_PREFIX`, and `_GUID` first so defaults apply), and at least one `<Real/>`

## 常见失败模式 / Common Failure Modes

- Script 命令解析或执行回归 / script command parser/executor regression
- 命令成功但产物缺失 / missing output artifact after successful command
- 生成 C 内容缺少 ABI 关键符号 / generated C content missing expected ABI symbols
- FMI 产物不完整 / FMI emission incomplete artifacts
- PowerShell 正则中误用双引号导致 `\s` 被当作字面反斜杠（应使用单引号模式） / regex in double-quoted PowerShell strings treating `\s` as a literal backslash (use single-quoted patterns)
- `--fmi-guid` / `RUSTMODLICA_FMI_GUID` 格式不合法导致导出报错 / invalid guid override causes export failure
