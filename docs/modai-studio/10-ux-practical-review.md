# Modai Studio 功能与界面审查：实用差距与增强建议

日期：2026-08-15  
产品：Modai Studio（`modai-studio`）  
定位：参照 Simulink 交互、以 Modelica + `rustmodlica` 为语义底座的框图建模与仿真桌面应用  
关联：[01 需求](./01-requirements.md) · [02 设计](./02-design.md) · [04 功能](./04-features.md) · [06 Simulink 对照](./06-simulink-capability-map.md) · [08 任务板](./08-implementation-tasks.md) · [09 可视化](./09-visual-graphics.md)

原则不变：**交互形态学 Simulink，语义学 Modelica**。本文不要求复刻 MATLAB 生态，只评估「会用 Simulink 的人，能否在 Studio 里快速搭模型、跑起来、看懂、改完再跑」。

---

## 1. 审查结论

P0–P3 / Q4 / F89 / RATE / F90 的**能力清单已经铺得很宽**：库、画布、子系统、变体、物理域、传感器、FMU/C 导出、Fast Restart、Pause/Step、3D、Profiler 都有对应实现。

与实用的差距不在「缺某个 catalog ID」，而在**主路径手感与仿真观测闭环**：

| 用户以为自己在用 | 实际遇到的是 |
|------------------|--------------|
| Simulink 式 Library 拖拽 | 先点库项，再点画布放置；缩放后落点会偏 |
| 四区常驻：库 \| 画布 \| 参数 \| 曲线 | VS Code 活动栏会把库和属性互斥切走 |
| Run 过程中 Scope 跟着走 | 整段积分结束后才出 180px 曲线 |
| 双击模块改参数 | 双击子系统下钻；参数在侧栏，且可被活动栏藏掉 |
| 非法连线就地标红 | `window.alert` 打断编辑 |
| 变量按 `plant.sensor.y` 勾选 | 扁平编译器名（`body_r_0_1`）长清单 |
| 仪表盘在线调参 | 仅仿真前旋钮；改完不会自动 Fast Restart |

一句话：**功能面接近 Studio-Pro，日常建模仍停在「功能已有、路径不顺」。**  
下一阶段应从「继续扩块」转为 **P4 主路径产品化**：库手感、四区常驻、观测可用、诊断可行动。

---

## 2. 审查方法

对照三层，不以任务板 `done` 当作体验验收：

1. **规格**：`01` / `04` / `06` 的用户故事 U1–U10 与 P0 体验验收（10 分钟搭→跑→看）。
2. **实现**：`modai-studio/src/app/Workbench.tsx`、`LibraryPanel`、`StudioCanvas`、`ToolBar` / `MenuBar`、`ResultsDock`、`useStudioSim`、`DashboardPanel`、`catalog/*`。
3. **Simulink 日常动作**：Library 拖拽、双击改参、Config 对话框、Run 中看 Scope、Stop、改参再 Run、点诊断跳模块。

不纳入本审查（规格已 Out）：Stateflow、Embedded Coder 认证级、HIL / External Mode、MATLAB 工作区、完整 Bus Object。

---

## 3. 现状快照（能力已落地，手感未收口）

### 3.1 已具备、且对主路径有价值

| 域 | 现状 | 实用评价 |
|----|------|----------|
| 工程 | 新建 / 打开 / 保存；`*.mo` + `*.diagram.json`；示例 9 个 | 闭环在；缺最近工程与未保存恢复 |
| 库 | 内置 catalog 覆盖 Sources/Math/Continuous/Discrete/Logical/Routing/Analog/Rotational/Translational/MultiBody/Sensors/Draw；收藏；搜索 | **能用，不好找、不好放** |
| 画布 | XYFlow 正交边、折点、junction、Mask SVG、域着色、采样色、网格吸附、对齐、旋转、查找 | 图元已工程化；放置与连线反馈弱 |
| 层级 | 子系统下钻、面包屑、选区封装、Variant、ModelRef、wrap `.mo` | 能力齐；入口藏在图标菜单 |
| 仿真 | Update / Run / Stop；Pause/Step；Fast Restart；进度条；rk45/rk4/implicit | 控制条有了；配置面太窄 |
| 观测 | 全局曲线、XY、3D、多 run 叠加、光标读数、CSV/JSON、Profiler | **事后查看器，不是 Scope** |
| 诊断 | Advisor 静态检查；Messages 可点聚焦；非法边红线 | 规则在；打断方式与文案不产品化 |
| 导出 | Export Suite（Coder / FMU CS/ME）、历史、打开目录 | 对主路径是加分项，不是阻塞项 |

Catalog 门禁 `catalog_gate_report.json`：signal/physical/routing 核心约 92 项，Analyze 全绿。块够用；**发现性不够**。

### 3.2 规格已写、实现未对齐的主路径项

| 规格承诺 | 实现偏差 | 影响 |
|----------|----------|------|
| F05「拖拽添组件」 | `LibraryPanel` 点击选中 + `StudioCanvas.onPaneClick` 放置；**无** `dataTransfer` / `onDrop` | Simulink 用户第一动作失败 |
| 四区常驻（`02` §6） | `ActivityBar` 在 explorer / properties / data 之间切换左侧；属性可同时出现在左栏与右栏 | 库经常消失 |
| F14 全局 Scope | `ResultsDock` 曲线高度写死 `180`；无 Data 表；x 轴用 category | 看波形费劲 |
| F47 记录变量白名单 | `loggedVariables` 只在 Data 活动栏逗号输入 | 与 Results 勾选脱节 |
| F11 仿真配置条 | 工具栏仅 solver / stopTime / intervals；tolerance 不在主条 | 调刚性问题要进深层面板 |
| 双击 = 参数（Simulink） | `onNodeDoubleClick` → `drillInto` | 普通块双击无反馈 |
| 连接失败就地提示 | `window.alert(decision.reason)` | 打断拖线 |
| 放置坐标跟视口 | `clientX - bounds`，未走 `screenToFlowPosition` | 缩放/平移后块落在错误位置 |

---

## 4. 界面与信息架构

### 4.1 壳层：VS Code 壳 vs Simulink 工作台

当前壳（`Workbench` + `ActivityBar`）是无边框 VS Code 布局：活动栏切换 Explorer / Properties / Data，底栏与右侧可关。

Simulink 用户的心理模型是**四区同时在**：

```text
Library（始终） | Canvas（始终） | Parameters（始终）
Results / Diagnostics（Run 后升起，可增高）
```

Studio 把「库」和「属性」做成互斥活动，导致：

- 拖块时要回到 Explorer，改参时库消失。
- 属性有两处入口（活动栏 Properties、右侧 Secondary），同一套 `propsDashboard` 复制渲染，认知重复。
- Pause / Step 在标题栏 `MenuBar`，Run / Stop 在画布上 `ToolBar`，仿真控制被拆开。

**建议：** 默认恢复规格四区；活动栏只切换「库的子页」（组件 / 大纲 / 数据），不拆掉库或属性。Pause/Step 回到仿真工具条，与 Run/Stop 同一组。

### 4.2 菜单不是菜单

`MenuBar` 是四组图标按钮（File / Edit / Export / Sim），不是下拉菜单。导出、包装、库包、Pause 全部平铺，主路径按钮被挤到认知边缘。

**建议：**

- 真正的 `File / Edit / Simulation / Results / Help` 下拉（可保留图标紧凑模式作选项）。
- 工具栏只留：Save、Update、Run、Pause/Resume、Step、Stop、Solver、StopTime、Fast Restart。
- 对齐 / 重布线 / Dashboard / Full JIT 收入「更多」或 Settings 默认隐藏。

### 4.3 组件库：能搜到，不像 Library Browser

`LibraryPanel` 的实用问题：

1. **交互**：点选再点画布；提示语还是英文 `"Select a block, then click canvas"`。
2. **外观**：纯文字 + 缩写（`TSpringDamper`、`PrismaticA`、`GearR2T`）；画布已有 `BlockIcon`，库列表不用。
3. **结构**：按 `category` 平铺，分类不可折叠；库一大就变成长滚动条。
4. **命名**：`displayKey` 已在 catalog 里，UI 未走 i18n，中文用户看到的是内部前缀。
5. **发现**：有收藏，无「最近使用」；无按端口类型过滤（选中信号口时只显示可连块）。

**建议（P4-A 核心）：**

- 库项 **拖到画布**（保留点击放置作回退）。
- 分类可折叠，默认展开 Sources / Math / Continuous / Analog。
- 每项显示 Mask 小图标 + 中英显示名 + 一行 typeName。
- 搜索同时匹配显示名、`typeName`、端口类型。
- 最近使用 8 项；与收藏并列。

### 4.4 画布：图元够，编辑反馈不够

已有正交路由、折点、域图例、采样图例，图面不「业余」。缺口在操作闭环：

| 问题 | 位置 | 建议 |
|------|------|------|
| 放置不跟 zoom/pan | `StudioCanvas.onPaneClick` | `screenToFlowPosition`（折点编辑已在用） |
| 无库拖放 | 画布无 `onDrop` | 接受 `application/x-modai-catalog` |
| 非法连线弹窗 | `window.alert` | 画布横幅 / 状态栏 reasonKey，边标红可撤销 |
| 无右键菜单 | 全前端无 `onContextMenu` | 添加块、创建子系统、Plot signal、删除、旋转 |
| 双击语义 | 仅子系统下钻 | 普通块：聚焦属性；子系统：下钻；Alt+双击反向 |
| 无自动连线辅助 | 拖近端口无吸附提示 | 靠近兼容口时高亮 + 松手连接 |

### 4.5 属性面板：字段在，不像 Block Parameters

`PropertiesPanel` 能改名、参数、`$Name` 提升、CSV 进表、Variant。差距：

- `Name` / `Type` 等标签未走 i18n。
- 无参数说明、单位、取值范围（catalog 只有 `name/value/valueType`）。
- 多选时主面板空白，只靠 `SelectionToolsPanel`，不像 Simulink 多选改同一参数。
- Dashboard 自动把**所有数值参数**做成滑条，范围用 `0.2x–2x` 猜测，容易把 `startTime` 和 `J` 混在一堆里。

**建议：** catalog 为常用参数补 `description` / `unit` / `min` / `max`；属性按「常用 / 全部」分组；Dashboard 仅绑定用户钉选或字典项。

### 4.6 结果区：有图，不是示波器

`ResultsDock` 把 Results / XY / 3D / Messages / Runs / Profiler / Export 挤在同一底栏。曲线区：

- 高度 `180px`，多通道几乎不可读。
- 变量是全量 `Object.keys(result.series)` 复选框，无搜索、无按实例分组。
- x 轴 `type: "category"`，时间当字符串，缩放/读数弱于数值轴。
- 规格里的 **Data 表**未做。
- 无框选缩放、无导出当前图、无「只显示 Scope 绑定」。
- 仿真中不更新；`busy` 只显示进度条。

**建议（P4-B）：**

1. Results 默认增高（例如 280–360，可拖）；曲线用数值时间轴 + dataZoom。
2. 变量树：`instance / port`，搜索过滤；默认只勾 `plotHints` 与 Scope/边选择。
3. 恢复 Data 页：时间序列表，选中行与光标同步。
4. 中期：积分过程推送增量 `series` 点，曲线活起来（这才配「实时仿真平台」）。

### 4.7 欢迎页与可发现性

`WelcomeOverlay` 只有打开 / 新建 / 示例列表。缺：

- 最近工程
- 「10 分钟路径」引导（对着 `signal_feedback` 高亮 Library → 连线 → Run → 勾曲线）
- 示例按域分组（信号 / 电气 / 机械 / 多体 / 速率）

---

## 5. 业务逻辑与仿真闭环

### 5.1 「实时仿真」名实不符

产品叙述是实时仿真平台，实现是 **批处理**：`doRun` 保存 → `simulateModel` 阻塞到结束 → 一次写入 `result.series` → 绘图。`listenSimProgress` 只更新阶段文案与进度条。

Dashboard 注释写明：`Pre-run knobs (not in-sim online tuning — engine has no mid-run param API)`。

这与 Simulink 用户的「Run 后立刻看 Scope、必要时停、改增益再 Fast Restart」不是同一产品。

**建议分两档：**

| 档 | 内容 | 依赖 |
|----|------|------|
| B1 准实时 | 积分按区间推送点；Results 增量画；Pause 时曲线停在当前 t | `sim_bridge` 进度事件带 `time` + 少量 series |
| B2 在线调参 | Fast Restart 已落地；Dashboard 改参后一键「应用并续跑」或自动重跑 | 现有 Fast Restart + 明确 UI，不必真 External Mode |

B2 可先做（工作量小于真中途改状态）；B1 才是「实时」的最小诚实定义。

### 5.2 Fast Restart 与调参迭代未闭合

工具栏有 Fast Restart 开关，Run 会带 `fastRestart`。但：

- 改属性 / 拖 Dashboard **不会**提示「结构未变，可热重启」。
- Run 记录只有 `Run N (Fast Restart)`，不展示参数快照，多 run 对比看不出改了什么。
- 结构变化（增删块、换线）失败时，用户看不到「已回退全量编译」。

**建议：** 每次 Run 把 `paramDictionary` + 改过的 `node.params` 摘要写入 `RunRecord`；Runs 页显示 diff；结构变化时状态栏说明 restartMode。

### 5.3 变量记录与命名

编译器扁平名与框图实例名之间已有 `plotHints` / `normalizeResultKey`，打开示例会预勾 hint。日常新建模型仍会面对上百个 `*_der_*`。

`loggedVariables` 与 Results 勾选是两套状态：改 Data 面板不会限制仿真输出，勾 Results 也不写回 manifest。

**建议：** 以 Results 勾选 + Scope/边选择为唯一「要记录的量」；保存时写入 `loggedVariables`；仿真请求带白名单（引擎若暂不裁剪，至少 UI 一致）。显示名用 `instance.port`，tooltip 给扁平名。

### 5.4 诊断可行动性

Advisor 能查重名、未连端口、缺 Ground、Goto 标签、域/单位冲突、Ts 不一致。实用缺口：

- 文案英文硬编码（`Duplicate instance name...`）。
- Update 把顾问错误与编译错误混在一个列表，没有规则 ID / 修复建议。
- 连接失败用 alert，成功路径没有「刚刚连的是 electrical / 单位 V」的轻量确认。

**建议：** Messages 分 Compiler / Advisor / Connection；每条带 `rule`、严重度、一键聚焦；中英与 i18n 对齐。非法连线不弹窗。

### 5.5 求解器与实验配置

主条只有三个求解器内部名。tolerance / maxStep / minStep 不在主路径。没有「固定步长 vs 可变步长」说明。刚性问题（电气开关、多体）用户容易停在 rk45 然后怪曲线。

**建议：** Simulation → Configuration 对话框（规格 F11/F46）：StopTime、区间、solver 及人话说明、tolerance、步长、记录变量。工具栏保留 StopTime + solver 缩写。solver 选项按编译器实际能力列出（含 sundials 构建时的 CVODE/IDA），不可用项禁用并写明原因。

### 5.6 工程会话

- 无最近工程列表（IDE 侧已有，Studio 欢迎页没有）。
- 无自动保存 / 崩溃恢复。
- 未保存退出无系统级确认（脏标记有 `*`）。
- 打开示例会自动 Validate，大模型会让首屏停在「校验中」，像卡死。

**建议：** 最近 8 个工程；脏关闭确认；示例改为「打开即出图，Validate 后台跑，可取消」。

---

## 6. 对标 Simulink 主路径（U1–U10）

| # | Simulink 动作 | Studio 今天 | 实用缺口 | 阶段 |
|---|----------------|-------------|----------|------|
| U1 | 库拖 Source/Gain，连线，Run，看 Scope | 点选放置；跑完再勾曲线 | 拖拽 + 放置坐标 + Scope 高度/预勾 | P4-A/B |
| U2 | 双击改参再 Run | 侧栏改参；双击下钻 | 双击语义；改参后 Fast Restart 提示 | P4-A |
| U3 | Configuration Parameters | 工具栏 3 个字段 | Config 对话框 | P4-C |
| U4 | Subsystem 整理 | 菜单图标，需先多选 | 右键「创建子系统」 | P4-A |
| U5 | 多次 run 对比 | Runs 复选叠加 | 缺参数快照 | P4-B |
| U6 | Simscape 搭 RC 看电压 | 示例能跑；库名难认 | 库图标/中文名；传感器 hint | P4-A |
| U7 | 导出数据 | CSV/JSON 菜单 | Data 表 + 从图导出 | P4-B |
| U8 | 查找并批量改 Gain | Find + Configurable | 可发现性差 | 已有，补入口 |
| U9 | 可复用库 | wrap / userlib pack | 向导文案偏开发者 | 维持，不优先 |
| U10 | 生成代码 | Export Suite | 主路径不挡；保持 Out 边界 | 维持 |

---

## 7. 增强建议（按优先级）

工作量：L = 跨前后端 / 多文件；M = 单域改造；S = 局部修复。  
收益按「10 分钟主路径」计。

### P4-A 主路径手感（先做，工作量最大、收益最大）

目标：有 Simulink 经验的人不再问「块怎么拖上去」。

| ID | 项 | 工作量 | 验收 |
|----|----|--------|------|
| P4-01 | 库 → 画布 HTML5 拖放；点击放置保留 | M | 从 Sources 拖 Step 到画布生成实例 |
| P4-02 | 放置使用 `screenToFlowPosition` | S | 缩放 0.5 / 2.0 落点仍准 |
| P4-03 | 库：可折叠分类 + `BlockIcon` + i18n 显示名 | L | 中文环境见「阶跃 / 增益」，不是前缀 |
| P4-04 | 最近使用 8 项 | S | 放置后出现在 Recents |
| P4-05 | 四区常驻：库与属性同时可见；活动栏不拆库 | M | 改参时仍能拖新块 |
| P4-06 | 非法连线：横幅/状态栏，禁止 `alert` | S | 跨域连线不丢焦点 |
| P4-07 | 画布右键：添加（搜索）、子系统、Plot、删除 | M | 不靠顶栏也能封装选区 |
| P4-08 | 双击：普通块聚焦属性；子系统下钻 | S | Gain 双击不再无反应 |
| P4-09 | Pause/Step 并入仿真工具条 | S | Run 组完整 |
| P4-10 | 工具栏瘦身：对齐/JIT 默认进「更多」 | S | 首屏能看见 Run 与 StopTime |

### P4-B 观测闭环（「实时」的最小诚实定义）

| ID | 项 | 工作量 | 验收 |
|----|----|--------|------|
| P4-11 | Results 数值时间轴 + dataZoom；高度随底栏 | M | 拖高底栏曲线跟着长 |
| P4-12 | 变量树 + 搜索；显示 `instance.port` | M | 百变量可在 3 秒内勾到 `c.v` |
| P4-13 | Data 表页，与光标同步 | M | 能复制一列数值 |
| P4-14 | RunRecord 参数快照 + Runs 对比说明 | M | 两次 Run 看出 `gain.k` 从 1→2 |
| P4-15 | 勾选 ↔ `loggedVariables` 双向 | S | 保存后再开勾选仍在 |
| P4-16 | Dashboard 仅钉选参数；改参提供「应用并 Run」 | M | 调 k 一次点击出新曲线 |
| P4-17 | 积分增量推送（准实时 Scope） | L | Run 中曲线随 t 增长；Pause 停画 |

P4-17 是唯一必须动 `sim_bridge` / 仿真循环的项，也是「实时平台」与示波器产品的分界。

### P4-C 配置、诊断、会话

| ID | 项 | 工作量 | 验收 |
|----|----|--------|------|
| P4-18 | Configuration 对话框（tol / 步长 / solver 说明） | M | 不改代码能把容差从 1e-6 改到 1e-4 |
| P4-19 | Messages 分栏 + Advisor i18n + 修复提示 | M | 缺 Ground 有中文说明并可跳转 |
| P4-20 | 最近工程 + 脏关闭确认 | S | 欢迎页能打开昨天的项目 |
| P4-21 | 示例打开先出图，Validate 后台 | S | 首屏不假死 |
| P4-22 | 属性参数 description/unit（catalog 补字段） | L | Resistor.R 旁显示 Ohm |

### P4-D 明确后做 / 不做

后做：真多时钟分区 UI、频域/线性化、逐块 residual、在线 External Mode、完整最近块推荐引擎。  
不做：Stateflow 产品化、认证级 Coder、HIL、MATLAB 工作区。

---

## 8. 建议实施顺序

按「主路径能否独立完成」排序，不按 catalog 数量：

```text
P4-A 库拖拽 + 落点 + 四区常驻 + 连线不弹窗 + 双击/右键
  -> P4-B 曲线可读（轴/树/表/快照）+ Dashboard 一键再跑
    -> P4-17 准实时推点（实时平台的技术含义）
      -> P4-C Config / 诊断 i18n / 最近工程
```

P4-A / P4-B / P4-C（含 P4-17–P4-22）已在 2026-08-15 落地。P4-D 为明确后做 / 不做，不纳入本板。P5-01–P5-06 已落地（未落盘先保存、属性 i18n、拖线吸附、旋转跟随、下拉菜单、自动保存/崩溃恢复）。

### 8.1 P4-A 关门条件

1. 从库拖拽 Step–Gain–Integrator，缩放画布后仍落在指针处。
2. 左侧库与右侧属性同时可见，改 `gain.k` 时仍能继续拖块。
3. 电气针脚拖到信号口：画布提示原因，无系统对话框。
4. 双击 Gain 聚焦属性；双击子系统下钻。
5. 中文 UI 下库名、放置提示、连接失败、结果空态均走 i18n。
6. `npm run build`（`modai-studio`）通过。

---

## 9. 对已有文档的含义

| 文档 | 建议 |
|------|------|
| `00-index` | 增加本文；阶段状态改为「能力清单已收口，进入 P4 实用手感」 |
| `04-features` | F05 验收改为「拖拽或点击放置」；F14 补充曲线高度/变量树/Data 表 |
| `06` U1 | 在「库 → 画布」上明确 **drag** |
| `08` | P0–F90 保持 done；新增 P4 板，指针指向本文 §7 |
| `09` | 下一轮视觉切片让位于库列表图标与 Scope 可读，而不是再补 Mask |

---

## 10. 文档变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-15 | 初版：对照实现与 Simulink 主路径，给出 P4-A/B/C 增强建议 |
| 2026-08-15 | P4-A/B/P4-17 落地；下一阶段 P4-C |
| 2026-08-15 | P4-18/19/22 落地；P4-C 剩余最近工程与示例后台 Validate |
| 2026-08-15 | P4-20 落地；P4-C 剩余示例后台 Validate |
| 2026-08-15 | P4-21 落地；P4-A/B/C 收口 |
