# Modai Studio 功能说明

日期：2026-07-21（修订：按 Simulink 能力域展开）  
关联：[需求说明](./01-requirements.md) · [设计说明](./02-design.md) · [Simulink 对照](./06-simulink-capability-map.md)

优先级：P0 = MVP；P1 = Studio-Core；P2 = Studio-Pro；P3 = Future。

## 1. 功能域导航（Simulink 映射）

| 域 | Simulink 参照 | 功能 ID 段 |
|----|---------------|------------|
| A 工程 | Project / .slx | F01–F03, F21 |
| B 库 | Library Browser | F04–F05, F40–F42 |
| C 编辑器 | Model Editor | F06–F10, F17–F18, F30–F39 |
| D 参数与资源 | Block Params / Model Explorer | F08–F09, F43–F45 |
| E 仿真配置 | Configuration Parameters | F11, F46–F48 |
| F 仿真控制 | Run / Stop | F12–F13, F19, F49 |
| G 观测 | Scope / SDI / To Workspace | F14–F15, F20, F50–F56 |
| H 层级 | Subsystem / Model Ref | F23, F60–F64 |
| I 物理域 | Simscape-like | F70–F74 |
| J 诊断 | Diagnostics / Advisor | F18, F80–F83 |
| K 导出与互联 | Code / FMU | F22, F90–F93 |
| L 示例与帮助 | Demos | F16, F94 |

---

## 2. 功能总表

### 2.1 P0 — MVP

| ID | 功能 | Simulink 类比 | 说明 |
|----|------|---------------|------|
| F01 | 新建项目 | New Model | manifest + 空 diagram + 骨架 mo |
| F02 | 打开项目 | Open | 读 schema；布局只信 diagram |
| F03 | 保存项目 | Save | diagram + emit mo |
| F04 | 组件库浏览 | Library Browser | 域/类别/搜索 |
| F05 | 拖拽添组件 | 拖 Block | 默认实例名与参数 |
| F06 | 端口连线 | 信号/物理连线 | 规则拒绝时提示 |
| F07 | 选择/移动/删除/框选 | 编辑基础 | 删节点级联删边 |
| F08 | 参数编辑 | Block Parameters | 右侧面板 |
| F09 | 实例重命名 | 改模块名 | 标识符与唯一性 |
| F10 | 画布视口 | 平移缩放 | 持久化 viewport |
| F11 | 仿真配置条 | Config 子集 | stopTime/intervals/solver/tol |
| F12 | 校验 Update | Update Diagram | validate |
| F13 | 运行仿真 | Run | simulate |
| F14 | 全局结果曲线 | Scope 面板 | 多变量勾选，不强制 Scope 块 |
| F15 | 消息/诊断面板 | Diagnostic Viewer 精简 | stage + errors |
| F16 | 示例工程 | Demo | >=2 |

### 2.2 P1 — Studio-Core

| ID | 功能 | Simulink 类比 |
|----|------|---------------|
| F17 | 撤销/重做 | Undo/Redo |
| F18 | 非法连线高亮 | 连接诊断 |
| F19 | 仿真取消 | Stop |
| F20 | 导出 CSV/JSON | To Workspace |
| F21 | 库路径配置 | 项目依赖 |
| F30 | 复制/粘贴/重复 | Copy-Paste |
| F31 | 网格吸附 | Snap to grid |
| F32 | 对齐/分布工具 | Align / Distribute |
| F33 | 画布注释 | Annotation |
| F34 | 信号线标签 | Signal label |
| F35 | 模块旋转 90° | Rotate |
| F36 | 画布查找 | Find |
| F37 | 自动正交路由/折点编辑 | 路由 |
| F38 | 错误定位到模块 | 点击诊断跳转 |
| F39 | 端口悬停类型提示 | 端口提示 |
| F43 | 实例树大纲 | Model Explorer 精简 |
| F44 | 顶层参数表 | Model Workspace 精简 |
| F45 | 多选批量改同一参数名 | 批量编辑（有限） |
| F46 | 高级求解选项 | Max/Min step 等 |
| F47 | 记录变量白名单 | Signal logging 配置 |
| F48 | 固定/可变步长模式展示 | Solver 类型 |
| F49 | 仿真进度条 | Progress |
| F50 | 画布 Sink/Scope 节点 | Scope block |
| F51 | 选中信号快速绘图 | Floating scope 精简 |
| F52 | Display 终值 | Display |
| F53 | 曲线缩放/光标读数 | Scope 工具 |
| F54 | 结果变量搜索过滤 | — |
| F40 | Catalog 收藏/最近使用 | Library 快捷 |
| F41 | 按关键字过滤库 | 搜索增强 |

### 2.3 P2 — Studio-Pro

| ID | 功能 | Simulink 类比 |
|----|------|---------------|
| F22 | 只读查看 .mo | — |
| F23 | 子系统下钻 | Subsystem |
| F60 | 选区创建子系统 | Create Subsystem |
| F61 | 子系统端口自动提升 | 自动 In/Out |
| F62 | 模型引用 | Model Reference |
| F63 | replaceable 类型切换 | Configurable 精简 |
| F64 | 面包屑层级导航 | 上钻下钻 |
| F55 | 多 run 历史 | SDI 精简 |
| F56 | 多 run 曲线叠加对比 | SDI compare |
| F42 | 用户扩展 catalog 包 | Custom library |
| F70 | 物理库扩充（更多 Analog/Mechanics） | Simscape libs |
| F71 | 传感器/探头组件 | PS-Simulink Converter 类比（Modelica 传感器） |
| F72 | 物理连线样式区分 | 域着色 |
| F80 | 静态检查清单 | Model Advisor 精简 |
| F81 | 代数环/结构告警展示 | Algebraic loop 诊断 |
| F82 | 导出框图 PNG/SVG | Print/Export |
| F83 | Goto / From | Goto/From |
| F84 | 一维查表块 | Lookup Table |
| F85 | 项目参数字典 | Data Dictionary |
| F86 | From File 激励 | From Workspace 精简 |

### 2.4 P3 — Future

| ID | 功能 | Simulink 类比 |
|----|------|---------------|
| F90 | FMU 导出向导 | FMU export |
| F91 | FMU 导入为节点 | FMU block |
| F92 | C 代码 emit 向导 | Coder 极简 |
| F93 | Dashboard 控件绑参 | Dashboard |
| F49b | Fast Restart | Fast Restart |
| F49c | 真暂停/单步 | Pause / Step |
| F73 | 采样时间着色 | Sample time colors |
| F74 | Multibody/3D 相关 | Simscape Multibody |
| F87 | Variant 组件 | Variant Subsystem |
| F88 | XY 相平面图 | XY Graph |
| F89 | 仿真 Profiler | Profiler |

---

## 3. 组件库

### 3.1 P0 信号流（目标 8，≈ Sources/Math/Continuous）

| catalogId | typeName（建议） | Simulink 直觉 |
|-----------|------------------|---------------|
| `blocks.sources.step` | `Modelica.Blocks.Sources.Step` | Step |
| `blocks.sources.sine` | `Modelica.Blocks.Sources.Sine` | Sine Wave |
| `blocks.sources.constant` | `Modelica.Blocks.Sources.Constant` | Constant |
| `blocks.math.gain` | `Modelica.Blocks.Math.Gain` | Gain |
| `blocks.math.add` | `Modelica.Blocks.Math.Add` | Add |
| `blocks.math.feedback` | `Modelica.Blocks.Math.Feedback` | Sum/Feedback |
| `blocks.continuous.integrator` | `Modelica.Blocks.Continuous.Integrator` | Integrator |
| `blocks.continuous.firstOrder` | `Modelica.Blocks.Continuous.FirstOrder` | Transfer Fcn 降级默认 |

说明：原 TransferFunction 若支持不稳，**默认用 FirstOrder**；TransferFunction 升为 P1 可选。

### 3.2 P0 物理域（目标 6，≈ Simscape 入门）

| catalogId | typeName（建议） | 域 |
|-----------|------------------|-----|
| `analog.basic.ground` | `Modelica.Electrical.Analog.Basic.Ground` | electrical |
| `analog.basic.resistor` | `Modelica.Electrical.Analog.Basic.Resistor` | electrical |
| `analog.basic.capacitor` | `Modelica.Electrical.Analog.Basic.Capacitor` | electrical |
| `analog.sources.constantVoltage` | `Modelica.Electrical.Analog.Sources.ConstantVoltage` | electrical |
| `mechanics.rotational.inertia` | `Modelica.Mechanics.Rotational.Components.Inertia` | mechanical |
| `mechanics.rotational.spring` | `Modelica.Mechanics.Rotational.Components.Spring` | mechanical |

电气示例优先；机械可作第二示例或 P1 加固。

### 3.3 P1+ 建议扩充（不冻结类型名）

- Sinks：`RealOutput` 终端、Scope 绑定节点、Display
- Math：Product、Divide、Abs、Saturation、Relay
- Continuous：Derivative、PI/PID（若有）、StateSpace（远期）
- Discrete：UnitDelay、ZeroOrderHold（编译器就绪后）
- Routing：Mux/Demux 等价（数组）、Goto/From（P2）
- Electrical：Inductor、CurrentSensor、VoltageSensor
- Mechanical：Damper、Torque source

任一类型必须先过 `rustmodlica` 可仿真门禁再进 catalog。

---

## 4. 功能详述

### 4.1 工程（F01–F03）

同前：目录项目；Save 时 emit；Run 默认 **保存并仿真**。

### 4.2 库与画布（F04–F07, F30–F39）

**连线规则（P0 最低集）：**

1. 信号：`out` → `in`，`kind=signal`
2. 物理：`kind=connector`，`typeName` 兼容表
3. 禁止跨域混连
4. 禁止自环

**P1 增强：** 悬停提示、非法边保留为 ghost 或标红、正交路由、对齐、注释、信号标签。

### 4.3 参数与大纲（F08–F09, F43–F45）

- P0：单选参数面板
- P1：左侧/侧栏实例树；顶层 `parameter` 列表可编辑并参与 emit
- P2：参数字典文件，多模型共享

### 4.4 仿真配置与控制（F11–F13, F19, F46–F49）

| 项 | P0 | P1 |
|----|----|----|
| stopTime | 是 | 是 |
| numberOfIntervals / interval | 是 | 是 |
| solver | rk4/rk45/(implicit) | + 展示说明 |
| tolerance | 基础 | 是 |
| maxStep/minStep | — | 是 |
| 变量记录白名单 | 默认启发 | 用户可配 |
| 进度/取消 | loading | 是 |

### 4.5 观测（F14–F15, F20, F50–F56）

**P0 策略（刻意不同于必须放 Scope 的习惯）：**

- 仿真结束 → 结果面板列出 `series` 键 → 勾选绘图
- 默认可勾选 `*.y` / catalog `plotHints`

**P1：**

- 可选画布 Scope 节点：仅声明“关注变量”，不参与方程（或映射到可忽略输出）
- 选中边 → “Plot signal”
- CSV/JSON 导出到 `results/`

**P2：**

- 保留多次 run 元数据（时间戳、参数快照摘要）
- 叠加对比同一变量

### 4.6 层级（F23, F60–F64）— P2

- 子系统 = 子 `DiagramDocument` 或嵌入 `children` + 边界端口
- emit：生成嵌套 `model`/`block` 或 `expandable` 组件（实现计划定一种，默认嵌套 model）
- UI：双击下钻；面包屑返回

### 4.7 物理域（F70–F74）

- P0：四件套电路可仿真
- P1：传感器 + 连线域着色
- P2：库扩充；P3：多体/3D

### 4.8 诊断（F18, F80–F83）

- P0：消息列表
- P1：点击跳转模块；连线高亮
- P2：检查清单（未连接端口、孤立源、缺 Ground 等）

### 4.9 示例（F16）

| 示例 | 内容 | 类比 |
|------|------|------|
| `examples/signal_feedback` | Step–Gain–Integrator / Feedback | 经典教学模型 |
| `examples/rc_circuit` | V–R–C–Ground | Simscape 入门 |

---

## 5. 关键交互流程

### 5.1 Simulink 式首次路径（P0）

```text
New Project
 -> Library: drag Step, Gain, FirstOrder/Integrator
 -> connect out->in
 -> Parameters: set gain.k
 -> Simulation: stopTime=1, solver=rk45
 -> Run
 -> Results: plot *.y
 -> Save
```

### 5.2 调参迭代（P1）

```text
Edit params -> Run (save+sim) -> compare curve
 -> Undo geometry edit if needed
 -> Export CSV
```

### 5.3 子系统整理（P2）

```text
Select group -> Create Subsystem
 -> drill in -> edit -> up
 -> Run still works on top-level model
```

### 5.4 失败路径

```text
Run -> emit/compile/simulate error
 -> Messages: stage + message
 -> Jump to node if instance name resolved
 -> fix -> Run
```

---

## 6. UI 信息架构（功能视角）

```text
+------------------------------------------------------------------+
| Menu: File  Edit  Simulation  Results  Help                      |
| Toolbar: Save | Update | Run/Stop | Solver combo | stopTime      |
+----------+-----------------------------------+-------------------+
| Library  |           Canvas                  | Properties        |
| (tree)   |  (XYFlow + optional Scope sinks)  | Parameters        |
|          |                                   | Signal (if edge)  |
| Outline  |                                   |                   |
+----------+-----------------------------------+-------------------+
| Tabs: Results (plots) | Messages | Data (table) | Runs (P2)      |
+------------------------------------------------------------------+
```

菜单名可与 Simulink 不完全一致，但 **Run / Library / Parameters / Results** 四词应稳定出现在 i18n 中。

---

## 7. 非功能（功能视角）

| ID | 项 | 目标 |
|----|----|------|
| NF1 | 启动 | 开发可拉起；发布可运行 |
| NF2 | 小模型反馈 | 典型示例通常 < 10s 出结果（机器相关） |
| NF3 | 保存安全 | 失败不丢内存图 |
| NF4 | i18n | 全 UI 键对齐 |
| NF5 | 可维护 | 模块化；控文件体量 |
| NF6 | 可发现性 | 有 Simulink 经验用户可猜测主路径 |
| NF7 | Git 友好 | 文本落盘，减少无意义字段抖动（序列化排序稳定） |

---

## 8. 验收对照

| 阶段 | 关门条件 |
|------|----------|
| P0 | F01–F16 + 需求 R1–R6 |
| P1 | 上表 P1 核心：F17–F21, F30–F34, F38, F43, F47, F49, F50 或 F51, F20 |
| P2 | F23+F60+F64 子系统闭环；F55 多 run；至少一项物理库扩充 |

细节标准见 [需求说明 §6](./01-requirements.md)。

---

## 9. 文档变更记录

| 日期 | 变更 |
|------|------|
| 2026-07-21 | 初版功能表 |
| 2026-07-21 | 按 Simulink 域拆分 A–L；扩展 P1–P3 ID；Scope 策略与 FirstOrder 默认 |
