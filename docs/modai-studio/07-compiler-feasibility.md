# Modai Studio × jit-compiler 可行性评估

日期：2026-07-21  
依据：`jit-compiler`（`rustmodlica`）源码、TestLib、回归文档；对照 [06-simulink-capability-map.md](./06-simulink-capability-map.md)  
结论口径：**不是**“能否做成完整 Simulink”，而是“对照表里各分期功能，编译器侧能否支撑、缺口在哪”。

## 1. 总判断

| 问题 | 判断 |
|------|------|
| 能否**完整实现** Simulink 产品级功能？ | **不能**。Stateflow / Embedded Coder / HIL / 工作区生态 / 全库 Blockset 等已标 Out，且编译器亦无对等能力。 |
| 对照表中的 **P0 MVP** 能否落地？ | **能（编译器侧基本就绪）**。依赖 `validate_from_source` / `simulate_from_source` + MSL Blocks/物理子集。 |
| **P1 Studio-Core**？ | **大部分能（UI）+ 部分受阻（引擎）**。撤销/对齐/CSV/标签不依赖编译器；**中途取消、真·仿真进度、离散块可信度**受编译器限制。 |
| **P2 Studio-Pro**？ | **部分能**。嵌套/replaceable flatten 有基础；子系统产品化、SDI、查表严格语义、Goto/Mux 需联调与门禁。 |
| **P3 Future（FMU/C/暂停单步）**？ | **产物级 Partial**。已有 `--emit-fmu` / `--emit-c`；真暂停/单步/完整 FMI 部署链未就绪。 |

一句话：**完整 Simulink 对等不可行；按现规格分期的“交互学 Simulink”主路径可行，P0 可开工，P1 需补 cancel/进度，P2+ 需 catalog 门禁与编译器增量。**

## 1.1 为何实现不了完整 Simulink（论证）

这里的“实现不了”指：**无法在本仓库技术路线上交付与 MathWorks Simulink 产品族功能对等、可替换的系统**。  
不是指“画布 + 跑曲线”做不到——那恰恰是 Studio P0 要做的。

### A. 产品边界不同：Simulink 不是一个画布，是一整条产品族

用户口头说的“Simulink”通常包含（或强依赖）以下整包能力：

| 产品/能力 | 作用 | 本仓库是否具备对等物 |
|-----------|------|----------------------|
| Simulink 本体 | 因果框图、采样时间、求解器配置、Scope/SDI | **部分**：可用 Modelica.Blocks + 自研 UI 近似交互 |
| MATLAB | 工作区、脚本、矩阵、App、数据字典运行时 | **无**：刻意不引入 MATLAB 运行时 |
| Stateflow | 状态机 / 流程图逻辑 | **无**：仅有 Modelica `when`/`if`/Clock 子集 |
| Simscape | 物理网络（另套连接语义与求解栈） | **部分近似**：Modelica 非因果 `connect`，不是 Simscape 引擎 |
| Embedded Coder / HDL / PLC Coder | 生产级代码生成与认证链 | **无**：仅有实验级 `--emit-c` / `--emit-fmu` |
| Simulink Test / Coverage / Requirements | 测试与追溯工具链 | **无**（回归用 `regress-harness`，不是同产品） |
| 第三方 Blockset / 硬件支持包 | 生态模块与板级联调 | **无** |
| External Mode / PIL / HIL | 在环与实时 | **无** |

即使把 Studio UI 做得再像 Simulink，**缺少 MATLAB + Stateflow + Coder + 生态**，也不能称为“实现了 Simulink”。规格里将这些标为 Out，不是偷懒，而是承认产品边界。

### B. 语义模型不同：因果框图 ≠ Modelica 方程系统

| 维度 | Simulink | Modai Studio / rustmodlica |
|------|----------|----------------------------|
| 核心语义 | 有向信号流（端口 out→in），采样时间一等公民 | Modelica：**方程 + connect**；Blocks 只是 MSL 里的因果子集 |
| 物理域 | 另走 Simscape（守恒连接、域专用求解） | 同一套 Modelica 非因果 connector（电气/机械等） |
| 离散/多速率 | Sample Time 颜色、Rate Transition、离散库成熟 | Clock/`sample`/`when` 有子集；**不是** Simulink 采样时间体系 |
| 代数环 | 专用诊断与处理策略 | 依赖 BLT/撕裂/牛顿；消息与策略不同 |
| “模块”实现 | Block 方法（mdlOutputs 等）+ 代码生成 | 展开为方程 → JIT/`calc_derivs` |

因此：

- **可以**用 Blocks 做出“很像 Simulink”的控制框图体验；  
- **不能**在不引入第二套执行引擎的前提下，保证每个 Simulink 块、每种采样时间规则、每种代数环行为与 MATLAB 数值一致。

口诀仍然成立，但也解释了上限：**交互可以学 Simulink，语义不能变成 Simulink。**

### C. 执行与工具链不同：没有 MATLAB 运行时就没有一批“默认能力”

完整 Simulink 工作流默认假定：

1. 参数/轨迹来自 MATLAB 工作区（From Workspace / To Workspace / `.mat`）  
2. 可用 MATLAB Function / S-Function / MEX 扩展行为  
3. 仿真中可调参、Dashboard、External Mode  
4. 结果进 MATLAB 再做控制设计（线性化、Bode、PID Tuner 等）

本路线是：

1. 本地项目文件 + `SimulationResult` 曲线/CSV  
2. 扩展靠 Modelica 模型/函数，不靠 MEX  
3. `run_simulation` 当前是**跑完再返回**的批式驱动（无真暂停、无仿真中改参 API）  
4. 无内置线性化/频域工具链

这些不是“再写几个面板”能补齐的，而是**运行时与科学计算宿主**缺失。

### D. 编译器成熟度不同：即使只做 Modelica，也达不到“全 MSL = 全 Simulink 库”

即使用 Modelica 去“翻译” Simulink 直觉，仍受 `rustmodlica` 现实约束：

1. **全量 MSL 未绿**：精选集高通过率 ≠ `regress_msl_broad` 级全库可用；不能承诺 Library Browser 挂全 MSL。  
2. **Fallback/placeholder**：部分外部表、未知调用可能被规则打成 0/警告却仍“跑通”——这与 Simulink 块语义确定性不兼容。  
3. **仿真控制 API 缺口**：无协作取消、无 `time/t_end` 进度、无求解器内 Pause/Step——对不齐 Simulink 仿真条。  
4. **代码生成级别**：`--emit-fmu`/`--emit-c` 是工具链能力，不是 Embedded Coder 的生产/认证级生成。  
5. **进阶物理**：理想开关、电力电子等有专项恢复路径，但仍是风险面，不是 Simscape 全库体验。

所以：**不是“差几个块”，而是库覆盖、语义可信度、仿真交互、代码生成四个维度同时低于 Simulink 产品线。**

### E. 工程与合规现实（为何不应宣称对等）

1. **版权与品牌**：Simulink 是 MathWorks 商标与闭源产品；本项目是 Modelica 工具链，对标的是工作流，不是克隆。  
2. **验证成本**：Simulink 兼容意味着海量模型/基线；本仓库回归是 Modelica/MSL/ModelicaTest 口径。  
3. **目标用户不同**：Studio 要服务“会搭框图、用 Modelica 仿真”的用户，而不是替换已绑定 MATLAB 生态的产线。

### F. 那为什么还要对照 Simulink？

对照的是**用户可感知的主路径**，用于排优先级，而不是验收“兼容 Simulink”：

```text
Library → 连线 → 改参数 → 配 stopTime/solver → Run → 看曲线/导出
```

这一条在 P0 **可以实现**（见本文 §3–§4）。  
超出这条主路径的 Stateflow/Coder/HIL/工作区/全库，**实现不了，也不应写进成功标准**。

### G. 一句话边界（评审用）

| 说法 | 是否成立 |
|------|----------|
| “我们要做一个像 Simulink 的 Modelica 可视化仿真工具” | **成立**（产品目标） |
| “我们要实现完整 Simulink” | **不成立**（语义、产品族、运行时、编译器成熟度均不支撑） |
| “P0 能搭 Blocks/简单物理并仿真出图” | **成立**（相对 jit-compiler 可行） |

## 2. 证据锚点（编译器）

| 能力 | 位置 / 证据 |
|------|-------------|
| 库 API 校验/仿真 | `jit-compiler/src/api.rs`：`validate_from_source`、`simulate_from_source` |
| 仿真结果 | `SimulationResult { time, series }`（`simulation/types.rs`）；`lib.rs` 导出 |
| 驱动入口 | `simulation/driver/run.rs`：`run_simulation` + optional `SimControl`（协作 cancel + `time/t_end` 进度）；**真暂停/单步仍无** |
| 求解器 | CLI/选项：`rk4` / `rk45` / `implicit` / `radau` / `qss`；`cvode`/`ida` 需 `sundials` feature |
| FMU/C | `--emit-c` / `--emit-fmu` / `--emit-fmu-me`；`fmi/` 模块；文档 `toolchain-fmi-emitc-script.md` |
| Blocks 覆盖样例 | `TestLib/ComponentLibraryCoverage.mo`（Step/Sine/Constant/Add/Feedback/Integrator/FirstOrder/TransferFunction/PID/…） |
| Blocks 小样例 | `TestLib/MSLBlocksTest.mo`、`MSLTransferFunctionTest.mo` |
| 旋转机械 MSL | 同上 Coverage 中 `Mechanics.Rotational.*`；回归中有 `Rotational.Examples.First` 类条目 |
| 电气注意 | Coverage 里电路多为**本地** `Resistor`/`Capacitor`/`Ground`，**不是** `Modelica.Electrical.Analog.*`；MSL Analog 另有回归样例（如 Cauer / Resistor），catalog 必须单独门禁 |
| 离散/时钟 | 多组 Clocked TestLib + `events-and-clock.md`；MSL `Blocks.Discrete` 成熟度仍属 Partial |
| 查表风险 | ExternalCombi* 已走 `ext_combitable_lookup` + `rustmodlica_table_get_value`；strict 下 pre 占位 fail-closed；抽检 `fallback_total=0` |
| 严格语义 | `RUSTMODLICA_JIT_POLICY_STRICT`、`RUSTMODLICA_JIT_STRICT_PLACEHOLDERS` — “绿”可能被 placeholder 掩盖 |

口径警告：精选集高通过率 ≠ `regress_msl_broad` 全量 MSL；Studio catalog **必须**对每个 FQN 做 validate+短时 simulate 准入。

## 3. 按 Simulink 对照域评估

图例：**Full** = 编译器可支撑规格所述；**Partial** = 能做子集或有坑；**Blocked** = 引擎/语义缺口挡住规格；**UI** = 几乎不依赖编译器；**Out** = 产品已排除。

### 3.1 库与组件

| Studio / Simulink 项 | 阶段 | 可行性 | 说明 |
|----------------------|------|--------|------|
| Library 面板 + 内置 catalog | P0 | **UI + Partial** | UI 无阻；条目必须门禁 |
| Sources Step/Sine/Constant | P0 | **Full** | Coverage / MSLBlocksTest |
| Math Gain | P0 | **Partial→Full** | 语义简单；Coverage 未显式列 Gain，准入后可 Full |
| Math Add / Feedback | P0 | **Full** | Coverage |
| Continuous Integrator / FirstOrder | P0 | **Full** | Coverage；FirstOrder 作默认 |
| Continuous TransferFunction | P0→P1 | **Partial** | 有测试；不稳时已规划降级 |
| Discrete UnitDelay/ZOH | P1 | **Partial** | 有修复与 ModelicaTest 线索；勿当 P0 |
| Lookup / CombiTable | P2 | **Partial** | 运行时查表已接；文件/MAT/Akima 等仍非完整 OM 对标 |
| Analog Ground/R/C/V | P0 | **Partial→Full** | 依赖 **MSL FQN** 门禁，勿用 TestLib 本地 TwoPin 冒充 |
| Rotational Inertia/Spring | P0 | **Full** | Coverage + 回归样例 |
| 第三方 Blockset | Out | **Out** | — |

### 3.2 模型编辑

| 项 | 阶段 | 可行性 | 说明 |
|----|------|--------|------|
| 拖拽/连线/框选/视口 | P0 | **UI** | 与编译器无关 |
| 对齐/撤销/注释/标签/旋转 | P1 | **UI** | — |
| Goto/From、Mux/Bus | P2–P3 | **Partial** | 靠 emit 约定 + 数组；非一等公民 |
| 导出 PNG/SVG、Diff | P2–P3 | **UI** | — |

### 3.3 层级与封装

| 项 | 阶段 | 可行性 | 说明 |
|----|------|--------|------|
| 参数面板 Mask 精简 | P0 | **UI** | — |
| Subsystem / 选区封装 | P2 | **Partial** | flatten 支持嵌套 model；emit/UI 产品工作量大 |
| Model Reference | P2 | **Partial** | 库路径 + 实例化已有基础 |
| replaceable / Variant | P2–P3 | **Partial** | TestLib 有 replaceable；Variant 产品化不足 |
| Stateflow | Out | **Out / Blocked** | when/if/Clock 子集 ≠ Stateflow |

### 3.4 仿真配置与控制

| 项 | 阶段 | 可行性 | 说明 |
|----|------|--------|------|
| stopTime / solver / tol | P0 | **Full** | `CompilerOptions` + `simulate_from_source` |
| max/min step、记录白名单 | P1 | **Partial** | 选项面需与驱动对齐；白名单可在结果侧过滤 |
| Run + 曲线 | P0 | **Full** | `SimulationResult` |
| Stop / 取消 | P1 | **Partial→Full** | `SimControl` + IDE `cancel_simulation_run`；CVODE/IDA 路径仅入口检查 |
| 进度条（仿真时间） | P1 | **Partial→Full** | 驱动 `poll_step` + `modai-jit-progress.simTime/tEnd` |
| Pause / Step | P3 | **Blocked** | 现有 step 多为跑完后回放，非求解器内暂停 |
| Fast Restart | P2 | **Done (Studio)** | `CompiledModel` 会话缓存 + `with_params`；结构指纹失效时全量编译 |

### 3.5 观测与数据

| 项 | 阶段 | 可行性 | 说明 |
|----|------|--------|------|
| 全局 Results 曲线 | P0 | **Full** | series 字典 |
| CSV/JSON 导出 | P1 | **UI** | 基于 `SimulationResult` |
| 画布 Scope / 点选信号 | P1 | **UI + Full** | 变量名映射即可 |
| 多 run SDI | P2 | **UI** | 落盘 run 元数据即可 |
| From File 激励 | P2 | **Partial** | 需生成 CombiTimeTable 或等价；查表风险 |
| Dashboard 在线调参 | P3 | **Partial / Blocked** | 无仿真中改参 API |

### 3.6 物理域 / 诊断 / 代码

| 项 | 阶段 | 可行性 | 说明 |
|----|------|--------|------|
| connector 物理连线 | P0 | **Full** | flatten `connect` |
| 理想开关 / 电力电子进阶 | P2+ | **Partial / Risky** | 有 `Ideal_Switch_Simulation` 路径，非入门 RC |
| Multibody 3D | P3 | **Partial (Studio + Frames lowering)** | 视口回放 `r_0`；JIT 四元数 `resolve1`/`planarRotation`/`gravityAcceleration`；分析期 `resolve1`/`resolve2` 标量化使 `a_0` 含 `der(w)`；`zeros(n)` 力平衡展开；Frame `R` 别名跟随关节；`if rooted` 折进 BLT；`der(resolve1)` 用 `R.w`（含二阶导耦合 `der(w)`）；`0 = e · t` 残差解关节 `der(w)`；非完整 MSL `Orientation.T[3,3]+w` |
| Update/Validate | P0 | **Full** | `validate_from_source` |
| 代数环诊断 | P2 | **Partial** | 错误信息质量依赖分析阶段 |
| FMU/C 导出 | P3 | **Done (coder package)** | residual + `rml_step`/CMake/`interface.json`；非 MathWorks Coder |
| FMI 协仿导入 | P3 | **Partial / Blocked** | 导出强于导入节点产品化 |
| External Mode / PIL | Out | **Out** | — |

## 4. P0 Catalog 准入建议（安全 / 风险）

| catalog 项 | 判定 | 动作 |
|------------|------|------|
| Step, Sine, Constant | **Safe** | 可进 P0 |
| Add, Feedback | **Safe** | 可进 P0 |
| Integrator, FirstOrder | **Safe** | FirstOrder 作默认连续块 |
| Gain | **Low risk** | 补一条短仿真门禁后进库 |
| TransferFunction | **Risky as default** | P1 可选；P0 不用作默认 |
| Analog.*（MSL FQN） | **Gate required** | 用 MSL 小电路门禁，不用本地 TwoPin 冒充 |
| Rotational Inertia/Spring | **Safe** | 可进 P0 |
| 任意查表/离散块 | **Not P0** | 等 strict 门禁 |

## 5. 挡住“完整对等”的 Top 缺口（编译器）

1. **无真暂停（求解器内 hold）** — cancel/进度已落地；Pause/Step 仍挡 P3。  
2. **Placeholder / fallback 过厚** — “能跑”≠语义正确；挡可信 catalog 与查表。  
3. **离散与多速率产品化不足** — 挡 Discrete / Rate Transition / 采样色。  
4. **子系统+路由一等公民不足** — flatten 能嵌套，Mux/Bus/Goto/Variant 发射未产品化。  
5. **混合物理（理想开关等）鲁棒性** — 挡进阶 Simscape 路径；入门 RC 仍可做。

## 6. 对规格的修订建议（已可直接采纳）

1. 在对照表增加 **Compiler** 列：`Full | Partial | Blocked | UI | Out`（见下文补丁意图）。  
2. P1「仿真取消 / 真进度」标为 **依赖编译器增量**，否则只能做“关闭窗口杀任务”级降级。  
3. P0 电气 catalog **强制 MSL FQN 门禁**；文档中写明 TestLib 本地电路不可直接映射。  
4. 保持 Out：Stateflow、Embedded Coder、HIL、MATLAB 工作区。  
5. “完整实现 Simulink”从成功标准中排除；成功标准维持分期验收。

## 7. 建议的编译器前置工作（若要解锁 P1 引擎项）

按工作量排序：

1. **P0 catalog 全量 FQN**：`validate` + 短时 `simulate` 门禁表写入 `docs/modai-studio/` 或 catalog JSON。  
2. ~~`run_simulation` cancel + `time/t_end` 进度回调~~（已落地：`simulation/control.rs` + IDE cancel）。  
3. ~~strict 模式下的 Blocks.Discrete / Tables 抽检~~（已做：98/98 + short-sim `fallback_total=0`）。  
3b. ~~Mechanics Rotational/Translational + Media（ModelicaTest FQN）strict~~（30+25+15 / short-sim `fallback_total=0`）。  
4. 子系统 emit 约定（嵌套 `model` vs `block`）与黄金样例（P2）。

## 8. 结论句（给评审用）

- **不能**承诺完整实现 Simulink。  
- **能**按现有 `06` 对照表实现 **P0**，编译器不是主风险（主风险是 catalog 门禁与 emit 质量）。  
- **P1** 中纯 UI 功能可完整做；**取消/进度**需先改 `jit-compiler`。  
- **P2–P3** 只能 Partial，且必须接受与 OMEdit/Simulink 的语义差距（fallback、FMU 成熟度、无 Stateflow）。
