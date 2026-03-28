# RustModlica

RustModlica 是一个**一体运行**的 Modelica 系统：ModAI IDE 与 JIT 编译器（rustmodlica）在运行时协同工作、**相互促进**。ModAI 负责编辑与仿真体验，JIT 负责编译与执行；JIT 的能力边界通过 ModAI 内的自迭代持续扩展，扩展后的 JIT 又反哺 ModAI 的建模与仿真能力。二者不是两个独立产品，而是同一系统中相互依赖、相互增强的两翼。支持 JIT/AOT 编译、FMI 2.0 导出以及 AI 辅助开发（含编译器自迭代）。

---

## 系统目标

系统目标是提供 Modelica 开发与仿真能力，且 **ModAI 与 JIT 在运行时协同、相互促进**：JIT 支撑 ModAI 的验证与仿真，ModAI 在 JIT 遇限时触发自迭代以增强 JIT，形成闭环。

### 编译器（jit-compiler）

实现 Modelica 语言与信号语义的广覆盖编译能力：解析、展平、BLT（块下三角）排序、JIT（默认）与可选 AOT 编译及仿真。路线以与 OpenModelica 行为对齐为目标（详见 `OPENMODELICA_VS_RUSTMODLICA.md`），并持续向全量信号覆盖推进。支持 FMI 2.0 CS/ME 导出（详见 `FMI_README.md`）。该引擎被 ModAI 在运行时调用，其能力边界通过 ModAI 内自迭代持续扩展。

### IDE（modai-ide）

提供以 AI Coding 为核心的 Modelica 开发环境：编辑、JIT 验证、仿真与结果可视化。依赖 JIT 完成验证与仿真；当 JIT 遇限时，在 ModAI 内触发自迭代（沙箱构建/测试、采纳或提交补丁），使 JIT 升级后反哺本 IDE。

---

## 设计思想

### ModAI 与 JIT 自迭代的运行时关系

- 运行时 **ModAI 调用 JIT** 做验证与仿真；用户在 ModAI 中编辑与运行模型，能力由 JIT 提供。
- **JIT 遇限**（如语法不支持）时，由 **ModAI 触发自迭代**：目标描述 → AI 生成补丁 → 沙箱构建/测试 → 用户采纳；升级的是 jit-compiler 代码库。
- 升级后的 JIT 被同一 ModAI 环境使用（或重启后加载），ModAI 无需改版即可获得更强的验证/仿真能力。
- 因此：**ModAI 驱动 JIT 进化，JIT 进化反哺 ModAI**，二者相互促进、非独立存在。自迭代不是在 ModAI 之外单独运行的工具，而是 ModAI 内针对 JIT 的闭环能力扩展。

### 其他设计原则

- **技术选型**：纯 Rust 编译器；JIT 基于 Cranelift，无需外部 C 编译器。IDE 采用 Tauri 2 + React/TypeScript，实现轻量级跨平台桌面应用。
- **AI 优先**：复杂任务（代码生成、编译器补丁）通过 DeepSeek API 完成；自迭代在 ModAI 内触发，在沙箱中应用 diff、执行构建与回归测试，由用户决定采纳或提交。
- **JIT 编译器自迭代优势**：通过上述运行时关系，编译器能够随使用场景自主扩展（新语法、优化、求解器），无需开发者手动编写每行代码，形成“编译器自我进化”的闭环。
- **Rust 开发的编译器核心优势**（与传统 C/C++ 编译器对比）：
  - **内存安全与零崩溃**：Rust 的所有权系统在编译期消除空指针、数据竞争与内存泄漏风险。
  - **高性能与零开销抽象**：性能接近或超越传统 C++ 编译器，编译与仿真速度更快、资源占用更低。
  - **纯 Rust JIT（Cranelift）**：无需外部 C 编译器或运行时依赖，单二进制跨平台部署。
  - **与 AI 自迭代深度融合**：强类型与 Cargo 生态使 AI 生成的补丁可自动编译、测试与合并，并在 ModAI 内完成采纳与反哺。
  - **可维护性与安全性**：现代工具链支持热重载、并行编译，长期维护成本低于传统大型 C++ 代码库。
- **范围界定**：以“尽量实现全量信号覆盖”为目标，按阶段推进与 OMC 的行为一致性；当前版本仍存在未覆盖项并在持续收敛。MSL 目前以固定子集为主（详见 `MSL_SUBSET.md`），并逐步扩展。
- **安全与可维护性**：AI 生成补丁在沙箱中执行；API Key 加密存储；大文件拆分规范（800 行阈值）详见 `OPTIMIZATION_PLAN_CN.md`。

### 整体数据流

1. 用户在 **ModAI IDE** 中编辑 Modelica 模型。
2. **ModAI 调用 JIT**（Rust 后端）进行验证与仿真。
3. **验证通过**：JIT 完成仿真，结果在 ModAI 中展示。
4. **若 JIT 遇限**：ModAI 提示并触发自迭代（DeepSeek 生成补丁 → 沙箱构建/测试 → 用户采纳）→ JIT 升级 → 回到步骤 2，ModAI 继续用升级后的 JIT 验证/仿真。

ModAI 依赖 JIT 提供能力，JIT 的不足通过 ModAI 触发的自迭代补全，升级后的 JIT 再服务 ModAI，形成**相互促进的闭环**。

---

## 与其他 Modelica 编译器的比较

RustModlica 定位为轻量级、AI 增强型、纯 Rust 实现的 Modelica 工具链，以 **ModAI + JIT 一体、自迭代相互促进** 为差异化特点，与主流开源及商业工具在以下关键维度存在显著差异：

| 维度                  | RustModlica                          | OpenModelica (开源)                  | Dymola (商业)                        | SimulationX / Wolfram SystemModeler 等 |
|-----------------------|--------------------------------------|--------------------------------------|--------------------------------------|----------------------------------------|
| **实现语言**          | 纯 Rust + Cranelift JIT              | C/C++                                | C/C++                                | C/C++ 或混合                           |
| **内存与运行时安全**  | 原生内存安全，零崩溃风险             | 存在内存泄漏/崩溃风险                | 存在内存泄漏/崩溃风险                | 存在内存泄漏/崩溃风险                  |
| **部署方式**          | 单二进制，无外部依赖，极简跨平台     | 需要完整安装环境，依赖较多           | 需要完整安装环境                     | 需要完整安装环境                       |
| **JIT 支持**          | 原生进程内 Cranelift JIT（默认）     | 解释执行 + C 代码生成                | 优化 C 代码生成                      | 优化代码生成                           |
| **AOT 支持**          | 可选系统链接器 AOT                   | 支持 C 代码生成与外部编译            | 强优化 C 代码生成                    | 强优化代码生成                         |
| **AI 自迭代能力**     | 原生支持（DeepSeek + 沙箱闭环）      | 无                                   | 无                                   | 无                                     |
| **IDE 集成**          | 内置 ModAI IDE（Tauri + AI 面板）    | OMEdit（功能完备但传统）             | 完整图形化 IDE                       | 完整图形化 IDE                         |
| **FMI 2.0 支持**      | CS/ME 完整导出                       | 完整支持                             | 完整支持（含 FMI 3.0）               | 完整支持                               |
| **语言覆盖**          | 全量信号覆盖导向（分阶段向完整规范收敛） | 接近完整 Modelica 规范               | 完整 Modelica 规范                   | 完整或接近完整                         |
| **主要适用场景**      | AI 辅助快速原型、研究、教育、轻量部署 | 学术研究、开源项目、工业验证         | 汽车/航空高端工业应用                | 多领域工业仿真、特定行业优化           |

RustModlica 在**安全性**、**部署简易性**与 **ModAI 与 JIT 相互促进的持续进化** 上具备明显差异化优势；在全语言覆盖与工业级优化深度上，仍处于追赶主流工具的阶段。

---

## JIT vs OpenModelica 覆盖对比（rustmodlica）

本文档以表格“打勾”形式，对当前 rustmodlica JIT 与 OpenModelica（OMC）在 Modelica 功能覆盖上的差异进行对比。

> 说明：  
> - `[x]` 表示在该维度上功能已实现或与 OMC 对齐。  
> - `[ ]` 表示该维度尚未覆盖、仅有局部实现，或明显弱于 OMC。  
> - 范围主要基于 `OPENMODELICA_FULL_ALIGNMENT_TASKS` 与 `FULL_MODELICA_SPEC_TASKS`。
> - 覆盖率与通过率数字属于阶段性快照，最终门禁以最新回归/对标日志与汇总报告为准。

---

## 1. 语言 / Flatten / IR / 求解器（对齐任务范围内）

| 类别 | 功能点 | rustmodlica JIT | OpenModelica |
|------|--------|-----------------|--------------|
| 语言前端 | `record` / `block` / `package` / `operator record` / `annotation` / `extends` 修改 | [x] 对齐（F1-* 全覆盖） | [x] |
| 表达式 | `noEvent` / `initial` / `terminal` / `der` / 内建数学函数 | [x] 对齐 | [x] |
| 函数（基础） | 用户函数定义、调用、inline、多返回值（record/tuple） | [x] 对齐（F3-*） | [x] |
| Flatten | for 展开、connect 检查、if-equation、array/record equation | [x] 对齐（T2-*、F4-*） | [x] |
| 结构分析 | 匹配、BLT、alias 消除、block 类型（single/torn/mixed） | [x] 对齐（IR1-*、IR2-*） | [x] |
| Index reduction | 微分索引、约束方程、`time_derivative`、index-1 化 | [x] 对齐（IR3-*） | [x] |
| 初值方程 | 初始方程分析、过/欠定检测、有序应用 | [x] 对齐 | [x] |
| 求解器（基础） | RK4、事件 + zero-crossing | [x] 对齐 | [x] |
| 自适应显式 | 自适应 RK45（无事件时启用） | [x] 对齐（T4-1/2） | [x] |
| 隐式求解器 | 简单隐式（Backward Euler/BDF-like） | [x] 有实现，功能子集 | [x] 更成熟 |
| SUNDIALS ODE/DAE | CVODE（ODE 子集）/ IDA（index-1 子集） | [x] 新增（feature gate） | [x] |
| 事件处理 | when、零点检测、`reinit` | [x] 对齐（RT1-1） | [x] |
| CLI 选项 | 步长、容差、输出间隔、结果文件 | [x] 对齐（RT1-2/3/5） | [x] |
| 回归体系 | `REGRESSION_CASES` + CI + OMC 对比脚本 | [x] REG-*、`OMC_COMPARISON` 已实现 | [x] |

**小结：**在对齐任务列表（P1/P2/P3）范围内，rustmodlica JIT 与 OMC 在“是否支持”的层面基本一致（以最新回归与对标日志为准）。

---

## 2. 标准库 / FMI / 调试与工具链

| 类别 | 功能点 | rustmodlica JIT | OpenModelica |
|------|--------|-----------------|--------------|
| MSL 子集 | Blocks / Math / SIunits 核心子集（固定版本 3.2.3） | [x] MSL-* 对齐，子集已实现 | [x] 完整 MSL |
| 结果对比 | OMC 数值结果对比脚本（支持多模型、JSON 摘要、严格模式鲁棒过滤） | [x] `compare_omc.ps1`（REG-2，已增强对象化返回与空值安全过滤） | [x] |
| FMI 2.0 | FMI 2.0 CS / ME 导出 | [x] `--emit-fmu` / `--emit-fmu-me` | [x] 且生态更成熟 |
| 调试工具 | backend-dae-info / index reduction 选项 / 警告级别 / 源位置 | [x] DBG-* 全覆盖 | [x] |

本轮 OMC CSV 实值对比样例（末行 max abs diff）：

- `TestLib/AlgorithmElseWhen` vs `jit-compiler/omc_algorithm_elsewhen.csv`：`maxDiff=0`
- `TestLib/DirectionSwitchStream` vs `jit-compiler/omc_direction_switch_stream.csv`：`maxDiff=0`
- `TestLib/BouncingBall` vs `jit-compiler/omc_sync_signal.csv`：`maxDiff=0`

---

## 3. 与 OMC 的主要差异点（超出对齐任务）

下表聚焦于 **OpenModelica 通常支持，而 rustmodlica 当前仅部分或显式 out-of-scope 的领域**。

| 领域 | 功能点 | rustmodlica JIT | OpenModelica |
|------|--------|-----------------|--------------|
| 同步语义 | clock / `sample` / `interval` / clocked partition | [x] 覆盖率 98%+（主路径已接通，持续收敛到完整语义） | [x] 完整同步语义 |
| clocked 变量 | clocked state、clock partition、connect 推断 | [x] 主路径已接通（时钟识别与分区键推断已增强）；复杂多速率边界持续收敛 | [x] |
| 复杂函数 | 深递归、大型函数、不纯函数（side-effect） | [ ] 全覆盖；有 JIT stub + 深度限制，side-effect 多为阻止/报错 | [x] 更宽松、成熟 |
| 外部函数 ABI | 数组/字符串、跨平台 ABI | [x] 数组参数链路已增强（含 `ArrayLiteral` 签名识别）；[ ] 字符串和复杂类型完整支持 | [x] 成熟且广泛使用 |
| 算法多赋值 | `(a,b,...):=f(x)` | [x] 已支持数组字面量/固定数组变量分解写回；[x] 函数多输出调用式主路径已补齐（`TwoOutputs`/`MultiOutputFunc` 已验证）；[x] 异常 arity/非法 LHS 路径已收紧为显式报错；[x] 复杂输出形态专项样例已纳入回归（`MultiOutputNestedExpr`/`MultiOutputMixedArrayScalar`/`MultiOutputShapeMismatch`）；[x] 新增深层分级策略：`1D array -> scalar` 走 warning，`record|multidim|comprehension -> scalar` 走 hard error；[x] 深层扩展场景已验证（`MultiOutputRecordNestedArrayMismatch`/`MultiOutputCrossLayerComprehensionMismatch`/`MultiOutputComplexLhsFieldStore`/`DeepRecordNestedMismatch`/`MixedNestedLhsFieldStoreMismatch`/`CrossModuleComprehensionMismatch`）；[x] 新增跨模块 record 复合推导与跨包类型别名链边界回归（`CrossModuleRecordCompositeMismatch`/`AliasChainTypeMismatch`）；[x] 新增更复杂 mixed LHS 多目标对称样例（`MixedMultiTargetSafePass`/`MixedMultiTargetFieldStoreFail`）并输出可定位错误索引（`#n=<lhs>`） | [x] |
| 解析器文件组织 | 单 `.mo` 多顶层定义 | [x] 已支持同文件多顶层定义（`function + model`）并在加载器中建立同文件索引回退（样例 `TestLib/MultiTopCombined.mo`） | [x] |
| 脚本语言 | 完整 Modelica 脚本（`.mos`） | [x] 覆盖率 98%+（AST+strict 主路径 + 14 例回归） | [x] 完整 `.mos` |
| 稀疏求解 | 稀疏 Jacobian + 稀疏线性求解接入 JIT | [x] 稀疏链路持续增强（CSR triples 去重合并、SUNDIALS 线性求解自动策略可调） | [x] 在大规模模型上成熟使用 |
| 大规模/刚性系统 | 高阶 DAE 求解、复杂 tearing 策略 | [ ] 工业级覆盖；定位在中小规模 | [x] 工业级后端 |

注：`connect/stream` 已从单向最小子集推进到方向切换语义 MVP（2-port），并接入 `omc_regression_direction_switch_stream.mos` 回归；多端口混合网络仍在持续扩展。

---

## 4. 针对差异的后续工作建议

| 优先级 | 方向 | 建议任务 |
|--------|------|----------|
| 高 | 同步语义二阶段增强 | 在已接通主路径基础上，完善 `superSample` / `shiftSample` / `backSample` 的分区调度一致性，并增加与 OMC 的对比用例。 |
| 中 | 稀疏与数值 | 持续扩展符号雅可比覆盖面并完善大模型性能基准；当前 Newton/tearing 主路径已接入 symbolic Jacobian 优先策略。 |
| 中 | 外部函数与脚本 | 完善 external 函数的字符串/复杂类型 ABI 实现，扩展脚本命令集以覆盖常见 `.mos` 场景；当前已建立 14 例 OMC 风格 `.mos` 回归基线（`jit-compiler/scripts/run_mos_regression.ps1`）。TestLib 全量 JIT `--validate` 门禁：在仓库根执行 `pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1`（可选用 `-CargoTargetSubdir target_regression` 指向 `jit-compiler` 下隔离构建目录）。`TestLib/*.mo` 须通过校验；故意失败用例放在 `TestLib/negative/*.mo`，脚本要求其对 `--validate` 返回 `success:false`。`run_regression.ps1` 在主用例矩阵之后包含同一步骤并记入 `build_regression_logs`。快速三件套（单测 + TestLib 批量 + 14 个 `.mos`）：仓库根执行 `powershell -File ./run_jit_rules_full_regress.ps1`。GitHub Actions `jit-ci` 已接入 TestLib 批量步骤。 |
| 低 | 算法多赋值边界增强 | 已形成并验证更深形状分级闭环（含更深 record 层级、mixed nested LHS、跨模块推导式），并补齐跨模块 record 复合推导、跨包类型别名链与 mixed 多目标对称样例；后续继续清理 Windows `release` 文件占用对主回归 pass 统计的扰动。 |

覆盖门槛说明：默认目标为语义覆盖率 `>=98%` 且 `Modelica 3.4核心=100%`。若当前统计未达标，JIT 会输出明确编译告警；设置 `RUSTMODLICA_COVERAGE_STRICT=1` 时会直接阻断编译。实际判定以当前回归/对标流水线最新日志为准。


---
## 主要功能

### 运行时的一体协同

在用户视角下，ModAI 与 JIT 作为**一个系统、两翼协同**运作：编辑在 ModAI、验证与仿真由 JIT 执行；遇限时在 ModAI 内发起自迭代并采纳以升级 JIT，升级后继续在 ModAI 中使用 JIT。二者相互促进：ModAI 为 JIT 提供真实使用场景与触发条件（遇限即补全），JIT 能力增强后 ModAI 能验证与仿真的模型更多、体验更好。

### JIT 编译器（rustmodlica / jit-compiler）

作为**引擎**提供的能力。该引擎被 ModAI 在运行时调用，其能力边界通过 ModAI 触发的自迭代扩展。

- **前端**：基于 Pest 的语法解析器、AST、加载器及展平模块（继承、实例化、connect、数组、for/when）。流水线详见 `OPTIMIZATION_PLAN_CN.md`。
- **后端与求解器**：变量分类、导数规范化、BLT 排序、别名消除、tearing（1–32 个残差）、可选 index reduction（dummyDerivative）。默认 Cranelift JIT；可选 AOT（.o + 系统链接器）。求解器：RK4（含事件）、RK45、隐式（BackwardEuler）、CVODE（`solver=cvode`，支持 `when + zero-crossing + reinit` 的 ODE 子集）、IDA（`solver=ida`，支持 `when + zero-crossing + reinit` 的 index-1 子集）。
- **运行与导出**：`--result-file` CSV、`--repl`、`--script`、`--emit-c`、`--emit-fmu`、`--emit-fmu-me`（FMI 2.0 CS/ME）。详见 `FMI_README.md`。
- **外部函数（JIT）**：`String` 字面量实参按 `const char*` 传入；**常量** `{1,2,3}` 形式 `Real` 数组字面量按 `double*` + `size`（`f64`）传入，与变量数组 ABI 一致。`--external-lib` 仍为主路径。无动态库时，进程内提供 `extLog` 桩，并对已知 C 名（如 TestLib 的 `rustmodlica_print_string`、`rustmodlica_sum_array`）按 **Modelica 调用名** 注册桩（见 `jit_stub_for_external_c_name`）。包名限定的 `external` 调用不再误走“命名空间 helper”占位降级。`collect_external_calls` 不再仅因 `TestLib.*` 这类大写包前缀被标成“builtin”而跳过收集；加载器在缺少独立父包 `.mo` 时仍会扫描库目录/已加载源以解析 `Pkg.shortName`（例如 `TestLib` 目录下的 `ExtFuncArrayArg.mo` 内 `function sumArrayExternal`）；`external "C" y = foo(...)` 的 C 入口名取 **右侧** 调用的 `foo`，而非左侧 `y`。
- **MSL 支持**：固定子集（Constants、SIunits、Blocks 子集、内置 Math）。详见 `MSL_SUBSET.md`。
- **数组维**：若展平时无法把 `array_size` 求成常量，默认（`--array-size-policy=legacy`）会按标量近似并可选告警；`strict` 则报错，除非用 `--array-sizes-json=<path>` 提供 `{"array_sizes":{"<flat_base_name>": N}}` 覆盖。`--warnings=error` 时 legacy 模式下也会将此类情况视为错误。展平阶段可选磁盘缓存：环境变量 `RUSTMODLICA_FLATTEN_CACHE_DIR` 指向目录时，会按模型源与策略键读写 `*.array-sizes.json`，在仍需求不出维时作为补充提示（用户 JSON 优先）。
- **JIT named rules（统一策略）**：默认 `jit-compiler/src/jit/default_jit_policy.json`（变量/pre 占位、Dot 路径零、hysteresis 表、MSL random 名匹配等）；内置函数名路由表在构建时由 `jit-compiler/build.rs` 生成至 `OUT_DIR` 并 `include_str!` 进 crate。环境变量：`RUSTMODLICA_JIT_POLICY_JSON` 指向叠加 JSON；`RUSTMODLICA_JIT_POLICY_STRICT` 为逗号分隔域（`variable,pre,dot,function_builtin,algorithm`）关闭对应回退。兼容：`RUSTMODLICA_JIT_VAR_POLICY_JSON` 仅追加 `variable_fallbacks`；`RUSTMODLICA_JIT_VAR_STRICT=1` 仍关闭变量类回退（并等价于 strict 域 `variable`）。API：`CompilerOptions.jit_policy_json` 在进程内未设置 `RUSTMODLICA_JIT_POLICY_JSON` 时写入同名环境变量（仅影响该进程内**首次** JIT 策略装载）。

**CLI 功能概览**：解析器（Modelica 子集）、解释器（Rust 内求值）、JIT 编译器（默认，纯 Rust）、AOT 编译器（可选，.o + cl.exe/gcc）、内置数学函数（sin、cos、tan、sqrt、exp、log 等）。

### ModAI IDE（modai-ide）

作为**工作台**提供的能力。依赖 JIT 完成验证与仿真；JIT 遇限时通过本 IDE 完成自迭代，使 JIT 升级后反哺本 IDE。

- **编辑与导航**：Monaco 编辑器、文件树、大纲、代码索引/符号、可选 Git 集成。
- **验证与仿真**：JIT 验证（CompilerOptions）、仿真运行、结果可视化（Plotly 曲线与表格、时间线）。
- **AI 功能**：自然语言生成 Modelica 代码、AI 面板；JIT 遇限时提示“使用 AI 补全”并触发自迭代：描述目标 → 生成/粘贴 diff → 沙箱（check、build/test、mo 回归）→ 采纳或提交。详见 `modai-ide/JIT_SELF_ITERATE_FULL_TEST_CASE.md`。
- **双工作区**：Modelica 项目开发与 JIT 编译器自迭代（JitIdeWorkspace）可切换。

---

## 预期目标与路线图

- **当前状态**：核心编译/仿真、JIT/AOT、FMI 2.0 CS/ME 导出、IDE 基础功能及自迭代流程已实现，已建立“全量信号覆盖”推进主线，并持续扩展 `OPENMODELICA_FULL_ALIGNMENT_TASKS` 覆盖面。
- **MVP / 近期**（详见 `iderequirest.md`）：P0 编辑 + AI 生成、JIT 验证 + 仿真、可视化；P1 编译器功能补全、2–3 轮自迭代与性能提升（在 ModAI 内完成，与 JIT 能力扩展形成闭环）；P2 跨平台稳定与 token 预算控制。
- **后续方向**：扩展 MSL/语法覆盖、FMI 导入、更多求解器与运行时选项（见 `FULL_MODELICA_SPEC_TASKS`、`ALIGNMENT_TASK_GAP`）。

---

## 项目结构

- **Workspace**（根目录 `Cargo.toml`）：成员 `jit-compiler`（rustmodlica）、`modai-ide/src-tauri`（Tauri 后端，依赖 jit-compiler）。运行时 ModAI 调用 JIT，自迭代则修改 jit-compiler 源码并重新构建，二者在运行时相互促进。
- **jit-compiler/**：编译器库与 CLI（parser、ast、flatten、analysis、jit、solver、simulation、fmi、script、api 等）。可单独构建与测试，亦供 ModAI IDE 在运行时调用。
- **modai-ide/**：Tauri + React 应用。前端 `modai-ide/src`，Tauri 后端 `modai-ide/src-tauri`。

---

## 构建与运行

**环境要求**：Rust 2021 edition。modai-ide 需 Node.js 与 npm。仅 AOT 或 FMU 打包时需要系统 C 编译器。

### 仅编译器（jit-compiler）

从仓库根目录：

```bash
cargo build --release -p rustmodlica
cargo run -p rustmodlica -- complex.mo
```

从 `jit-compiler/` 目录：

```bash
cargo build --release
cargo test --release
cargo run -- complex.mo
```

- **JIT（默认）**：进程内运行，无需外部工具。
- **AOT**：`cargo run -- complex.mo obj`，生成 .o 并链接（需 cl.exe 或 gcc）。
- **FMI 导出**：`--emit-fmu=<dir>`（CS）、`--emit-fmu-me=<dir>`（ME）。详见 `FMI_README.md`。

#### `--validate`（仅编译校验 / JSON）

- stdout 输出一行 JSON：`success`、`errors`、`warnings`、`state_vars`、`output_vars`（函数入口时后两项常为空数组）。
- **模型**为根：与完整编译路径一致，成功即 `success:true`。
- **函数**为根：若能用当前标量 `expr_eval` 跑通入口求值则直接成功；若因数组/点号/range 等不在该求值器范围内而失败，在 **`--validate` 模式下仍判 `success:true`**，并追加一条 **warning**（文案含 `validate: function root accepted without scalar entry eval`）。直接以函数名运行仿真（非 `--validate`）时行为不变，仍会报错。
- **TestLib 批量门禁**：`jit-compiler/scripts/run_testlib_validate.ps1` 扫描 `jit-compiler/TestLib/*.mo`（须全部 `success:true`），并扫描 `jit-compiler/TestLib/negative/*.mo`（须全部 `success:false`，用于语法/类型/connect 等负面样例）。`jit-compiler/scripts/testlib_validate_expect_fail.txt` 已废弃，仅作说明占位。

### SUNDIALS 可选特性（CVODE / IDA / KINSOL / SUNLinSol）

#### 1) 安装与环境配置

Windows（MSVC）推荐：

1. 安装 LLVM/Clang（64-bit），确认 `libclang.dll` 存在于 x64 `bin` 目录。
2. 设置环境变量 `LIBCLANG_PATH` 指向该目录（必须是 64-bit）。
3. 使用 VS 2022 Build Tools（或完整 VS）+ CMake。

PowerShell 示例（当前会话生效）：

```powershell
$env:LIBCLANG_PATH = "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Tools\Llvm\x64\bin"
```

Linux/macOS（系统库模式）：

- 需要 `clang`/`libclang`、`cmake`、C/C++ toolchain。
- 通过系统包管理器安装 SUNDIALS，或使用 vendored 模式。

#### 2) 构建方式

- 默认（不含 SUNDIALS）：`cargo build --release -p rustmodlica`
- 启用 SUNDIALS（系统库或本地可发现）：`cargo build -p rustmodlica --features sundials --release`
- vendored SUNDIALS（无预装库时推荐）：`cargo build -p rustmodlica --features "sundials,sundials-vendor" --release`
- 启用 KLU 稀疏线性层：`cargo build -p rustmodlica --features "sundials,sundials-klu" --release`

#### 3) 使用说明（运行期）

- `--solver=cvode`、`--solver=ida` 需要编译时开启 `sundials` feature。
- `cvode/ida` 已支持 `when + zero-crossing + reinit` 事件主路径（含事件细化与去抖）。
- `ida` 当前按 index-1 子集使用（`IDASetId` 已接入，代数状态分量仍有限制）。
- 线性求解器可用环境变量控制：
  - `RUSTMODLICA_SUNDIALS_LINSOL=auto|dense|spgmr|klu`
  - 其中 `klu` 需 `sundials-klu` feature。
  - `petsc` / `umfpack` 目前给出提示并回落，不作为已接入后端。

- 事件去抖与扫描相关环境变量（SUNDIALS 路径）：
  - `RUSTMODLICA_EVENT_DEADBAND`
  - `RUSTMODLICA_EVENT_COUNT_DEADBAND`
  - `RUSTMODLICA_EVENT_MAX_SAME_HITS`
  - `RUSTMODLICA_TAIL_CROSSING_DEADBAND`
  - `RUSTMODLICA_TAIL_HEIGHT_DEADBAND`
  - `RUSTMODLICA_TAIL_VELOCITY_DEADBAND`
  - `RUSTMODLICA_SUNDIALS_EVENT_LOG=0|1`（关闭/开启事件日志）

- 过定残差检查配置（Newton 求解器）：
  - `RUSTMODLICA_OVERDET_CHECK=1|0`（启用/禁用过定残差一致性检查）
  - `RUSTMODLICA_OVERDET_RESIDUAL_TOL=<float>`（容差阈值，默认 1e-4）
  - CLI 参数：`--overdet-check=true|false`、`--overdet-tol=<float>`

- Newton 稀疏求解器配置：
  - `RUSTMODLICA_NEWTON_SPARSE_POLICY=auto|dense|sparse`（路径选择策略）
  - `RUSTMODLICA_SPARSE_MIN_SIZE=<int>`（最小稀疏尺寸，默认 4）
  - `RUSTMODLICA_SPARSE_DENSITY_THRESHOLD=<float>`（密度阈值，默认 0.3）
  - `RUSTMODLICA_NEWTON_PATH_TRACE=1`（启用路径选择日志）
  - 基准测试：`.\benchmark_newton_sparse.ps1 -TestHeuristic`

示例：

```powershell
# CVODE
cargo run -p rustmodlica --features sundials -- --solver=cvode --t-end=5 --dt=0.01 MyModel

# IDA
cargo run -p rustmodlica --features sundials -- --solver=ida --t-end=5 --dt=0.01 MyModel
```

#### 3.1) 事件参数扫描（event-scan）

Last updated: 2026-03-23

用于批量模型下对事件参数做网格扫描，输出每模型最优与全局最优组合。

常用参数：

- `--model=<name>`：单模型扫描。
- `--models=<m1,m2,...>`：多模型批量扫描。
- `--count-values=<v1,v2,...>`：扫描 `RUSTMODLICA_EVENT_COUNT_DEADBAND`。
- `--tail-velocity-values=<v1,v2,...>`：扫描 `RUSTMODLICA_TAIL_VELOCITY_DEADBAND`。
- `--aggregate-mode=sum|avg|max`：全局聚合评分策略。
- `--aggregate-report=full|compact`：输出详细度。
- `--output-file=<path>`：写 JSON 到文件（stdout 输出摘要）。
- `--quiet` 或 `--quiet=none|events|all`：控制扫描日志粒度。

参数速查表：

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--model` | 单模型扫描 | `BouncingBall` |
| `--models` | 多模型批量扫描（逗号分隔） | 空（未设置时走 `--model`） |
| `--count-values` | `RUSTMODLICA_EVENT_COUNT_DEADBAND` 扫描网格 | `0.0004,0.0005,0.0006,0.0008` |
| `--tail-velocity-values` | `RUSTMODLICA_TAIL_VELOCITY_DEADBAND` 扫描网格 | `0.02,0.03,0.04,0.05` |
| `--aggregate-mode` | 跨模型聚合评分：`sum`/`avg`/`max` | `sum` |
| `--aggregate-report` | 输出详细度：`full`/`compact` | `full` |
| `--quiet` | 静默模式，等价于 `--quiet=all` | 关闭 |
| `--quiet=none|events|all` | 日志粒度：无静默/仅事件日志/全部扫描日志 | `none` |
| `--top-n` | 输出候选数量 | `5` |
| `--output-file` | 结果 JSON 文件路径 | 空（输出到 stdout） |

常用预设：

```powershell
# 1) 单模型快速扫描（看完整 topN）
cargo run -p rustmodlica --features sundials -- event-scan `
  --lib-path="d:\source\repos\rustmodlica\jit-compiler\TestLib" `
  --model=BouncingBall `
  --aggregate-mode=sum `
  --aggregate-report=full `
  --top-n=5
```

```powershell
# 2) 批量模型推荐（稳定 + 输出精简）
cargo run -p rustmodlica --features sundials -- event-scan `
  --lib-path="d:\source\repos\rustmodlica\jit-compiler\TestLib" `
  --models=BouncingBall,YourOtherModel `
  --count-values=0.0004,0.0005,0.0006,0.0008 `
  --tail-velocity-values=0.02,0.03,0.04,0.05 `
  --aggregate-mode=sum `
  --aggregate-report=compact `
  --quiet=all `
  --output-file="d:\source\repos\rustmodlica\build_event_compare\event_scan_result.json"
```

```powershell
# 3) 风险规避取优（按最差模型 max 分数排序）
cargo run -p rustmodlica --features sundials -- event-scan `
  --lib-path="d:\source\repos\rustmodlica\jit-compiler\TestLib" `
  --models=BouncingBall,YourOtherModel `
  --aggregate-mode=max `
  --aggregate-report=compact `
  --quiet=events `
  --output-file="d:\source\repos\rustmodlica\build_event_compare\event_scan_result_max.json"
```

示例（Windows PowerShell）：

```powershell
cargo run -p rustmodlica --features sundials -- event-scan `
  --lib-path="d:\source\repos\rustmodlica\jit-compiler\TestLib" `
  --models=BouncingBall,YourOtherModel `
  --count-values=0.0004,0.0005,0.0006,0.0008 `
  --tail-velocity-values=0.02,0.03,0.04,0.05 `
  --aggregate-mode=sum `
  --aggregate-report=compact `
  --quiet=all `
  --output-file="d:\source\repos\rustmodlica\build_event_compare\event_scan_result.json"
```

#### 4) 常见问题

- `Unable to find libclang`：`LIBCLANG_PATH` 未设置或指向了 32-bit DLL。
- `pwsh.exe not found`（SUNDIALS vendored 构建日志中出现）：安装 PowerShell 7，或确保 `pwsh.exe` 在 `PATH` 中。
- KLU 符号缺失：构建时未启用 `sundials-klu`，或系统 SUNDIALS 未包含 KLU。

### 示例模型（complex.mo）

```modelica
model ComplexMath
  Real x;
  Real y;
  Real z;
  Real w;
equation
  x = 3.1415926;
  y = sin(x / 2.0);
  z = y * cos(x) + sqrt(4.0);
  w = exp(1.0);
end ComplexMath;
```

### ModAI IDE

```bash
cd modai-ide
npm install
npm run tauri dev
```

发布构建：`npm run tauri build`。

---

## 文档索引

| 分类           | 文档 |
|----------------|------|
| 编译器/规范     | `FMI_README.md`、`MSL_SUBSET.md`、`OPENMODELICA_VS_RUSTMODLICA.md`、`OPENMODELICA_FULL_ALIGNMENT_TASKS.md`、`FULL_MODELICA_SPEC_TASKS.md` |
| IDE/产品       | `iderequirest.md`、`modai-ide/JIT_SELF_ITERATE_FULL_TEST_CASE.md` |
| 优化/实现      | `OPTIMIZATION_PLAN_CN.md`、`OPTIMIZATION_PLAN.md`、`EXTERNAL_FUNCTION_ABI.md` |

