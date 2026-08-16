# Modai Studio 需求说明

日期：2026-07-21（修订：2026-08-16 离散逻辑宿主）  
产品：Modai Studio（工作目录名 `modai-studio`）  
类型：基于 Modelica 的可视化建模与仿真桌面应用（Simulink 风格交互）  
对照详表：[06-simulink-capability-map.md](./06-simulink-capability-map.md)

## 1. 背景与问题

RustModlica 已具备：

- `jit-compiler`：Modelica 解析、展平、JIT、仿真
- `modai-ide`：代码 IDE + JointJS 结构图编辑（偏开发者工具）

缺口：缺少以**框图拖拽**为一等公民的独立产品。控制与机电用户熟悉的是 Simulink 路径——Library → 连线 → 配求解器 → Run → Scope——而不是先写 `.mo`。

## 2. 产品目标

### 2.1 总目标

提供 Windows 优先的桌面应用，以 **Simulink 式工作流** 完成 Modelica 模型的可视化建模与仿真：

1. 从组件库拖拽搭建框图（信号流 ≈ Simulink；物理网络 ≈ Simscape）
2. 配置模块参数与仿真配置（≈ Configuration Parameters）
3. 运行 / 停止仿真（≈ Simulation 控制条）
4. 以示波器/数据检视方式查看与导出结果（≈ Scope / SDI 精简版）

### 2.2 对标声明

| 对标 | 不对标 |
|------|--------|
| Simulink 建模编辑手感 | MATLAB 语言工作区 |
| Simulink 仿真与看波形主路径 | Stateflow 产品/语言（Chart、动作语言、`after`、junction） |
| Simscape 式物理连接（Modelica connector） | Embedded Coder / HDL 全流程 |
| 文本友好的项目落盘 | 闭源 `.slx` 二进制生态 |

口诀：**交互形态学 Simulink，语义学 Modelica。**

## 3. 目标用户

| 角色 | 诉求 | Simulink 类比 |
|------|------|----------------|
| 控制/信号流工程师 | 源、增益、积分、反馈、看阶跃响应 | Sources/Math/Continuous + Scope |
| 机电/电气入门用户 | R/C/源/地或转动惯量等 | Simscape Electrical/Fundamentals |
| 系统工程师（后期） | 子系统分层、多 run 对比、参数字典 | Subsystem + SDI + Data Dictionary |
| 内部验证人员 | 极小模型验证编译器与 UI | Demo models |

非目标用户（本产品线不服务）：依赖 Stateflow 的逻辑架构师、需要生产级自动代码生成流水线的嵌入式团队、纯文本 Modelica 库维护者（请用 `modai-ide`）。

## 4. 产品分期与范围

### 4.1 阶段定义

| 阶段 | 名称 | 目标一句话 |
|------|------|------------|
| P0 | MVP | 会用 Simulink 的人 10 分钟内完成“搭→跑→看” |
| P1 | Studio-Core | 日常小中型框图建模手感（撤销、对齐、标签、取消、导出） |
| P2 | Studio-Pro | 层级、多 run、数据字典、模型引用、物理库扩充 |
| P3 | Future | FMU/C、变体、仪表盘、高级诊断 |

### 4.2 P0（MVP）必须有

对应 Simulink：Library Browser 精简版、Model Editor 基础、Run、Scope 面板、Config 基础项。

1. **项目**：新建 / 打开 / 保存；`*.mo` + `*.diagram.json` + `studio-project.json`
2. **组件库**：信号流子集 + 物理域子集；分类与搜索
3. **画布**：拖入、连线、移动、删除、框选、缩放平移
4. **参数面板**：模块参数与实例名（≈ Block Parameters）
5. **仿真配置**：`stopTime`、步长/区间数、求解器、容差（≈ Configuration Parameters 子集）
6. **Update/Validate + Run**：经 `rustmodlica`；错误进消息面板
7. **结果面板**：多变量时间序列曲线（全局 Scope，不强制画布 Scope 块）
8. **示例**：信号流闭环 + 电气 RC（或等价）各至少 1 个

### 4.3 P1（Studio-Core）应该有

对应 Simulink：编辑手感、基础诊断、数据导出、模型大纲。

1. 撤销 / 重做；复制粘贴；网格对齐/分布
2. 连线规则强化 + 非法边高亮；仿真错误映射到模块
3. 信号线标签；画布注释 Annotation
4. 实例树（≈ Model Explorer 精简）
5. 仿真进度与取消；记录变量白名单
6. 画布 Sink/Scope 节点（可选绑定曲线页）；Display 终值
7. 导出 CSV/JSON；库路径配置；顶层参数表
8. 模块旋转（90°）；画布内查找

### 4.4 P2（Studio-Pro）可以有

对应 Simulink：Subsystem、SDI、Data Dictionary、Model Reference。

1. 子系统下钻 / 选区封装；模型引用
2. 多 run 结果对比（SDI-lite）
3. 项目参数字典；Goto/From；查表块
4. `.mo` 只读预览；导出框图 PNG/SVG
5. 物理库扩充与传感器块；静态检查清单（Advisor-lite）
6. 与 `modai-ide` 项目互操作（单向导入优先；File → Import IDE diagram 已落地）
7. 多步顺序控制：Library 提供 `Modelica.StateGraph` 的 Step/Transition（F95），落在现有 `diagram.json` + `connect`；不是独立 Chart

### 4.5 P3（Future）

FMU 导入导出、C emit 向导、Variant、Dashboard 控件、Fast Restart、采样时间着色、Profiler、相平面 XY Graph 等——详见对照表，不进入当前实现计划默认范围。

## 5. 非目标（Out of Scope）

### 5.1 MVP / Core 明确不做

1. 改造或拆分 `modai-ide` JointJS 编辑器
2. 画布与 `.mo` **双向**实时同步 / 完整 annotation 往返
3. 完整 MSL 浏览器与全量 icon
4. 云协作、插件市场、多用户权限
5. Electron 或其他桌面壳
6. 自动生成测试程序与无关审计脚本

### 5.2 产品线级不做（或极远期）

1. Stateflow 等价独立状态机 IDE / Chart 画布 / 动作语言 / JSON 状态图 schema
2. 以 `Modelica_StateGraph2` 为语义宿主（更强但不完全符合 Modelica，进不了 MSL）
3. Studio 排期实现 MLS 第 17 章 `state`/`transition`（时钟化状态机；JIT 尚未 parse/flatten；做成也不等于 StateGraph）
4. Embedded Coder / HDL Coder 级生产代码链
5. HIL / External Mode / PIL
6. MATLAB 工作区与 `.mat` 互操作（可用 CSV/JSON）
7. 需求管理 / 覆盖率工具（回归交给 `regress-harness`）

顺序/模式逻辑在 Modelica 里已经拆开，不另做「像 Stateflow 的功能组合」：连续对象上的切模式用 `when`/`if`/`reinit` 或 Logical 块；Grafcet 多步用 MSL `StateGraph` 块画在**同一张**框图上。仿真真相仍是 `.mo`。

## 6. 成功标准（验收）

### 6.1 P0 功能验收

| ID | 标准 | Simulink 类比 |
|----|------|----------------|
| R1 | 空项目可拖入 >= 3 个信号流组件并连线保存 | 搭模型 |
| R2 | 保存后存在一致的 `Model.mo` 与 `Model.diagram.json` | 工程落盘 |
| R3 | Run：合法模型出曲线；非法模型可读错误 | Run + Diagnostic |
| R4 | 信号流示例与物理域示例默认求解器可跑通 | Demo |
| R5 | `cargo build -p modai-studio` + 前端 `npm run build` 通过 | 可构建 |
| R6 | 无画布 Scope 块也能在结果面板看到时间序列 | Scope 面板 |

### 6.2 P0 体验验收

- 主路径无需文本编辑器完成“搭模型 → 仿真 → 看曲线”
- 布局固定为：库 | 画布 | 属性，下部/侧部结果与消息（单构图，非仪表盘堆叠）
- 对有 Simulink 经验的用户，核心按钮语义可猜测（Library / Run / 参数 / 曲线）

### 6.3 技术验收

- `modai-studio/src-tauri` 为 workspace member，path 依赖 `rustmodlica`
- 仿真真相源为 `.mo`；画布真相源为 `*.diagram.json`
- 图编辑状态可完整从 diagram 恢复
- 数据结构为子系统预留扩展字段（即使 P0 不启用）

### 6.4 P1 关门追加（Core）

| ID | 标准 |
|----|------|
| R7 | 连续编辑支持撤销回到拖拽前状态 |
| R8 | 仿真中可取消并恢复 UI |
| R9 | 至少一种格式导出时间序列（CSV） |
| R10 | 非法连线不可创建或立即标红并给出原因 |

## 7. 约束

| 约束 | 说明 |
|------|------|
| 平台 | Windows 优先 |
| 语言 | UI 中英文 i18n；代码标识符与注释英文 |
| 内核 | 只绑定本仓库 `jit-compiler`，不引入第二套 Modelica 编译器 |
| 体量 | 单文件避免长期 >800 行；与 IDE 解耦 |
| 版本控制 | 落盘文本化，便于 Git diff |
| 文档 | 规格以 `docs/modai-studio/` 为准 |

## 8. 假设与依赖

1. `rustmodlica` 提供 validate/simulate 库能力（或可薄封装），结果含 `time` + `series`。
2. 首版组件以**内置 catalog** 描述端口与参数；类型名以编译器实际支持为准可替换。
3. 生成 `.mo` 使用标准实例声明与 `connect`，可被现有编译路径消费。
4. MSL 可用（策略 S1，见实现论证）；否则示例需配置 `libraryPaths`。
5. P0 不要求仿真真暂停/单步；取消即可。

## 9. 风险（需求层）

| 风险 | 影响 | 缓解 |
|------|------|------|
| 用户期望完整 Simulink | 范围爆炸 | 对照表分期 + Out 清单 |
| 物理域 connector 复杂 | 连线难用 | P0 限组件；规则引擎 |
| Scope 交互与 Simulink 不一致 | 学习摩擦 | 文档说明全球结果面板；P1 补 Sink |
| 子系统晚做导致数据模型返工 | 推倒重来 | diagram 预留层级字段 |
| Blocks 类型支持缺口 | 示例失败 | Catalog 可替换，禁止静默生成不可仿真类型 |
| 与 IDE 双产品成本 | 维护分叉 | 共享仅编译器；画布不复用 JointJS |

## 10. 里程碑

| 里程碑 | 内容 |
|--------|------|
| M0 | 规格评审通过（含 Simulink 对照） |
| M1 | 空壳 + 项目 I/O |
| M2 | 信号流 Library/Editor/Run/结果面板 |
| M3 | 物理子集 + 连线诊断 |
| M4 | Studio-Core（P1） |
| M5 | Studio-Pro（P2） |
| M6 | Future（P3）按需 |

编码不得早于 M0 关门。

## 11. 文档变更记录

| 日期 | 变更 |
|------|------|
| 2026-07-21 | 初版 MVP 需求 |
| 2026-07-21 | 按 Simulink 能力域扩展分期、验收、Out 清单；引用对照表 |
