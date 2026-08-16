# Modai Studio 设计说明

日期：2026-07-21（修订：2026-08-16 离散逻辑宿主）  
关联：[需求说明](./01-requirements.md) · [数据结构](./03-data-structures.md) · [功能说明](./04-features.md) · [实现论证](./05-implementation-rationale.md) · [Simulink 对照](./06-simulink-capability-map.md)

## 1. 设计目标

1. **独立产品边界**：`modai-studio` 与 `modai-ide` 并行存在，不共享 UI 状态机。
2. **仿真内核单一**：数值与语义以 `rustmodlica` 为准。
3. **图与文本职责分离**：`*.diagram.json` 管可视化编辑；`*.mo` 管可仿真模型。
4. **MVP 可交付**：优先打通拖拽 → 生成 → 仿真 → 曲线。
5. **Simulink 可发现性**：Library / Parameters / Run / Results 主路径对有 Simulink 经验的用户可猜测；不复制 MATLAB 生态。
6. **语义不克隆 Stateflow**：名字可以像 Rate Transition，发出去必须是 MSL 块。顺序图用 `Modelica.StateGraph` 实例 + `connect`，不新增状态图 JSON、不单独 Chart 画布。

## 2. 系统上下文

```text
+------------------+         Tauri IPC          +------------------------+
|  React Frontend  | <------------------------> |  modai-studio (Rust)   |
|  XYFlow Canvas   |                            |  - project I/O         |
|  Library / Props |                            |  - diagram sync        |
|  Plot Panel      |                            |  - mo emit             |
+------------------+                            |  - validate/simulate   |
                                                +-----------+------------+
                                                            |
                                                            | path dep
                                                            v
                                                +------------------------+
                                                |  rustmodlica           |
                                                |  (jit-compiler)        |
                                                +------------------------+
```

`modai-ide` 不在运行时依赖链上；仅作参考实现与行为对照。

## 3. 离散与顺序逻辑（非 Stateflow）

Stateflow 要解决的模式/顺序控制对 Modelica 用户仍有意义，但 **Modelica 不支持 Stateflow 产品**，Studio 也不做对等 Chart。能力拆到已有构造，不另做功能包：

| 用户意图 | 语义宿主 | Studio 形态 |
|----------|----------|-------------|
| 连续对象上切模式、跳变、复位 | `if` / `when` / `reinit`，或 `Modelica.Blocks.Logical` | 现有方程/块，无新 schema |
| 多步顺序（启停、灌装、故障处理） | **`Modelica.StateGraph`**（Grafcet：Step / Transition / `condition`） | 同一张 XYFlow 框图的 catalog 节点；`emit_mo` 仍是实例 + `connect` |
| 时钟化离散控制器 | MLS 第 17 章 `state` / `transition` | **不排期**。JIT 无该语法；实现后也不等于 StateGraph 库 |

版本不要混：JIT **库**对齐 MSL 4（bundled `StateGraph.mo` 仍在，是单文件包，不是「主库删了」）；**语言门禁**是 MLS 3.4 core 子集。第 17 章从 MLS 3.3 写入规范、3.4+ 仍在，但本编译器未实现。`Modelica_StateGraph2` 更强但不合规，不用作宿主。

做第 17 章 ≠ 实现 StateGraph；把 StateGraph 跑绿 ≠ 有第 17 章。

禁止：`statechart.json`、独立状态图画布、把 Transition 做成「语言」而只允许以后做菱形外观（落盘仍是 Transition 节点）。

## 4. 仓库与包结构（目标）

```text
rustmodlica/                          # existing monorepo
├── jit-compiler/                     # package: rustmodlica
├── modai-ide/                        # existing IDE (unchanged ownership)
├── modai-studio/                     # NEW product root
│   ├── package.json                  # Vite + React frontend
│   ├── src/                          # UI
│   │   ├── app/                      # shell, menu, toolbar (Run/Stop)
│   │   ├── canvas/                   # XYFlow nodes, edges, rules, annotations
│   │   ├── library/                  # Library Browser panel
│   │   ├── outline/                  # P1 Model Explorer-lite
│   │   ├── properties/               # Block Parameters panel
│   │   ├── simulation/               # config + progress + bridge API
│   │   ├── results/                  # Scope panel, table, runs (P2)
│   │   ├── project/
│   │   ├── i18n/
│   │   └── api/tauri/
│   ├── src-tauri/                    # package: modai-studio
│   │   ├── Cargo.toml                # depends on rustmodlica (path)
│   │   └── src/
│   │       ├── main.rs / lib.rs
│   │       ├── commands/
│   │       ├── project/
│   │       ├── diagram/
│   │       ├── emit_mo/
│   │       ├── catalog/
│   │       └── sim_bridge/
│   └── examples/                     # sample projects
└── Cargo.toml                        # workspace members += modai-studio/src-tauri
```

命名说明：产品对外可称 “Modai Studio”；crate/目录用 `modai-studio`（ASCII）。

## 5. 逻辑架构

### 5.1 前端分层

| 层 | 职责 |
|----|------|
| App Shell | 窗口布局：库 \| 画布 \| 属性；底/侧结果区 |
| Project Session | 当前项目路径、脏标记、保存/加载编排 |
| Canvas Session | XYFlow nodes/edges 与 diagram document 的投影 |
| Catalog UI | 展示内置组件元数据，拖拽 payload |
| Sim UI | 配置、运行、错误、曲线选择 |

状态建议：React Context + 轻量 session 对象（类似 IDE 的 session 思路，但独立实现，不 import IDE 源码）。

### 5.2 后端分层

| 模块 | 职责 |
|------|------|
| `commands` | Tauri IPC 入口，参数校验，错误映射 |
| `project` | 项目目录约定、读写 `.mo` / `.diagram.json` / `project.json` |
| `diagram` | DiagramDocument 校验、与前端 JSON 对齐 |
| `catalog` | 内置组件目录（端口、默认参数、域标签） |
| `emit_mo` | DiagramDocument → Modelica 源码生成 |
| `sim_bridge` | 封装 `rustmodlica` validate/simulate，返回序列化结果 |

### 5.3 数据流（编辑 → 仿真）

```text
User edits canvas
  -> Frontend updates DiagramDocument (in memory)
  -> Save / Auto-prepare:
       write model.diagram.json
       emit_mo(DiagramDocument) -> model.mo
  -> Simulate:
       read model.mo (or in-memory emitted string)
       rustmodlica compile + run
       return SimulationResult { time, series }
  -> Plot panel binds selected variables
```

关键不变量：

- **仿真输入只消费 Modelica 文本**（及库路径），不直接读 diagram JSON。
- **画布恢复只消费 diagram JSON**（及 catalog 元数据），不把 `.mo` 当布局源（MVP）。

## 6. 与现有系统的边界

| 主题 | modai-studio | modai-ide |
|------|--------------|-----------|
| 主交互 | 框图优先 | 代码优先 |
| 画布 | XYFlow | JointJS structure editor |
| 仿真 | 薄 `sim_bridge` | 完整 JIT 命令面 + 监控/方程图等 |
| AI / 自迭代 | MVP 不做 | 已有 |
| 库管理 | 内置 catalog + 可选本地路径 | 完整 LibrariesBrowser |

共享允许：

- `rustmodlica` 公共 API / `SimulationResult` 等类型
- 未来可选抽取 `modai-canvas-core`（**不在 MVP**）

禁止：

- Studio 直接 `import` IDE 前端模块
- 为 Studio 去改 IDE 核心交互（除非修公共编译器 bug）

## 7. UI 信息架构

主工作台刻意对齐 Simulink 四区（Library | Editor | Parameters | Results），单构图而非仪表盘：

```text
+------------------------------------------------------------------+
| Menu: File  Edit  Simulation  Results  Help                      |
| Toolbar: Save | Update | Run/Stop | solver | stopTime            |
+----------+-----------------------------------+-------------------+
| Library  |           Canvas (XYFlow)         | Properties        |
| Outline  |                                   | Block params      |
| (P1)     |                                   |                   |
+----------+-----------------------------------+-------------------+
| Tabs: Results (plots) | Messages | Data | Runs (P2)              |
+------------------------------------------------------------------+
```

视觉与交互原则：

- 首屏聚焦建模画布，不堆统计卡片
- P0 Results = 全局曲线面板（不强制放置 Scope 块；见对照表 Scope 策略）
- 信号流边与物理 connector 边视觉区分（线型/颜色）
- i18n：`en` / `zh` 键对齐；保留 Run/Library/Results 等稳定概念键

## 8. 模型生成策略

### 8.1 生成目标形态

生成扁平、可读的单一 `model`：

- `parameter` / 组件实例声明
- 修改后的参数赋值（`component.param = value`）
- `equation` 段中的 `connect(a.port, b.port)`
- 顶层 `annotation(experiment(...))` 可选写入仿真默认值

### 8.2 命名

- 组件实例名：用户可编辑；默认 `typeName` 小写 + 序号（如 `gain1`）
- 连接：由边的 source/target 端口映射为 `instance.port`

### 8.3 失败策略

- 生成前静态检查（未连接的强制端口、重名、未知 type）
- 生成成功但编译失败：展示编译器错误，保留 diagram，不静默改图

## 9. 仿真桥设计

参考 IDE 的请求形状，但 Studio 命令面保持最小：

| Command | 作用 |
|---------|------|
| `studio_project_open` | 打开项目，返回 diagram + 元数据 |
| `studio_project_save` | 保存 diagram + 生成 mo |
| `studio_catalog_list` | 返回内置组件目录 |
| `studio_validate` | 对当前模型 validate |
| `studio_simulate` | 仿真并返回 `SimulationResult` |
| `studio_emit_preview` | 可选：预览生成 mo（调试用） |

`sim_bridge` 内部：

1. 取得 model 源码字符串与 `model_name`
2. 调用与 CLI/`run_simulation` 等价的库入口
3. 映射错误为结构化 `{ stage, message, path?, line? }`

进度与取消：P1；MVP 可用阻塞式 async 命令 + UI loading。

## 10. 组件目录设计

内置 `catalog`（静态数据，可 JSON 加载）：

- 每条：`typeName`、`domain`（`signal` \| `electrical` \| `mechanical`）、显示名 i18n key、默认参数、端口列表（名、方向、种类）
- 画布节点渲染：信号流用简洁方块；物理域用带端口锚点的自定义 node
- MVP 不依赖完整 Icon annotation 解析；可用简单几何/SVG 占位
- Library **Draw** 图元库（Rectangle/Ellipse/Line/Polygon/Text/Note）用于画布注释，不进 `.mo`

首版数量目标（可在功能说明中 enumerated）：

- Signal：约 8
- Physical：约 6

## 11. 错误处理

| 层级 | 策略 |
|------|------|
| UI 输入 | 禁用非法操作；toast/inline 提示 |
| Diagram 校验 | Save/Simulate 前返回问题列表 |
| Emit | 确定性错误，不写半残 mo（或写前回滚） |
| Compiler | 原样/整理后展示；标记相关节点 `hasError`（若能映射实例名） |
| Panic/IO | Tauri 命令返回 `Err(String)` 或结构化错误 envelope |

## 12. 安全与本地性

- 纯本地应用；不上传模型
- 文件访问限制在用户选择的项目目录及配置的库路径
- 不执行模型外任意 shell（仿真走库 API）

## 13. 扩展点（有意留白）

1. `emit_mo` 替换为“增量补丁手写 mo”（P2）
2. catalog 从已安装 MSL 扫描生成（P2）
3. 抽取共享 crate 供 IDE 复用（post-MVP）
4. 子系统层级（P2）
5. `Modelica.StateGraph` catalog（F95）：同一张图上的 Step/Transition，不新增 schema

禁止作为扩展点：JSON 状态图、Stateflow Chart、以 StateGraph2 或第 17 章为 Studio 语义宿主。

## 14. 测试与验证策略（规格层）

按用户约束：**不自动生成测试程序**。验证以编译与手工/脚本回归为主：

1. `cargo build -p modai-studio`
2. 前端 `npm run build`
3. 示例工程手动仿真出图
4. 可选：后续将示例纳入 `regress-harness`（另开任务，不阻塞 MVP 规格）

## 15. 设计待决（实现前需关掉）

当前决策已足够开始写实现计划；下列项在实现计划中给默认值即可：

| 项 | 默认建议 |
|----|----------|
| 项目清单文件名 | `studio-project.json` |
| diagram schema version | `1` |
| 默认求解器 | `rk45` |
| 曲线库 | ECharts 或轻量 canvas（与 IDE 类似即可，独立依赖） |
