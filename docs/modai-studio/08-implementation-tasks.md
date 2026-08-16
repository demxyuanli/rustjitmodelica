# Modai Studio 实现任务板

日期：2026-07-21（修订：2026-07-24 Rate Transition）  
阶段：**F95 StateGraph catalog 已落地；P4-D（频域 / External Mode）后做**  
P4 规格与优先级：[10-ux-practical-review.md](./10-ux-practical-review.md)  
历史阶段：P0 → P3 → Q4 → F89 → RATE → F90 均已收口（见下文）。
机器可读状态：`docs/modai-studio/tasks_rate.json`（当前）；另有 `tasks_f89.json` / `tasks_q4.json`  
本地副本：`build/modai_studio/tasks_*.json`（`build/` 被 gitignore）

## 里程碑

| ID | 焦点 | 任务 |
|----|------|------|
| M1 | 工程骨架 | S01–S02 |
| M2 | 后端 I/O + emit + 仿真桥 | S03–S04 |
| M3 | 前端四区工作台 | S05–S07 |
| M4 | 示例 + 构建验收 | S08–S09 |

## 任务列表

| ID | 状态 | 说明 |
|----|------|------|
| S01 | done | Tauri 2 + Vite + workspace member `modai-studio` |
| S02 | done | DiagramDocument / catalog / project manifest 类型与内置 catalog |
| S03 | done | `emit_mo`；`studio_project_open/save`；`studio_catalog_list` |
| S04 | done | `studio_validate` / `studio_simulate`（MSL lib-path） |
| S05 | done | Shell：Menu/Toolbar + Library \| Canvas \| Properties + 底栏 Tabs |
| S06 | done | XYFlow 拖拽、连线规则、框选删除、参数编辑 |
| S07 | done | Update/Run、Results 曲线、Messages |
| S08 | done | `examples/signal_feedback`、`examples/rc_circuit` |
| S09 | done | `cargo build -p modai-studio` + `npm run build` |

P0 任务板已全部 `done`。  
P1 Core（`build/modai_studio/tasks_p1.json` C01–C06）已全部 `done`。  
P2 Pro（`build/modai_studio/tasks_p2.json` P201–P205）已全部 `done`：子系统下钻、多 run 叠加、物理库扩充。  
P3 Future（`build/modai_studio/tasks_p3.json` P301–P312）含 Emit Coder/FMU、XY、Dashboard、Variant、FMU 共仿、Pause/Step、Fast Restart、Multibody 3D；MathWorks Embedded Coder 认证级仍为 Out。  
F91 FMU 导入共仿：`studio_import_fmu` + 画布 `fmu` 节点 + FMI2 CS 仿真桥接已落地。  
物理域+FMU：紧耦合固定步共积（plant RK4 + FMI2 CS 同环）优先，Jacobi 为回退；真 Pause/Step 已落地。  
Fast Restart（P310）：同结构改参复用 `CompiledModel` / JIT Artifacts，跳过重编译；工具栏开关 + `restartMode` 回报。  
Multibody 3D（P311）：底栏 3D 视口（three.js）；图结构预览 + 仿真 `r_0` 轨迹回放；Library MultiBody 目录 + `examples/multibody_preview`（单摆；编译器四元数 Frames / Axis 展开；`zeros(n)` 力平衡；Frame `R` 别名跟随关节；`if rooted` 折进 BLT；`der(resolve1)` 展开为 `w × r`，`der(v_0)` 再导成 `der(w)×r`；`resolve1`/`resolve2` 标量化使 `f=m(a-g)` 含 `der(w)`；`0 = e · t` 残差解 `der(w)`）。  
Coder-grade C（P312）：`Emit Coder` 写出 residual + `rml_step` 固定步 API + CMake + `interface.json`；CLI `--emit-coder=`。非 MathWorks Embedded Coder。  
Shell UI（P313）：菜单/工具栏图标化；系统设置（语言、字体、密度、tips、菜单/工具可见性）；快捷键 Ctrl+N/O/S、Ctrl+Enter。  
布局持久化（P314）：左右面板 / Library·Outline / Results 拖拽改尺寸，写入 settings.layout；运行状态与错误文案完整 i18n。  
快捷键帮助（P315）：Settings Shortcuts 页 + `?` 呼出；仿真进度阶段文案 `localizeProgress` i18n。  
仿真快捷键（P316）：Space 暂停/继续、`.` 单步、Ctrl+`.` 停止；已写入 Shortcuts 页。  
Frameless VS Code 壳（P317）：无边框标题栏、活动栏/侧栏/编辑器/底栏/状态栏；Ctrl+B/J/Alt+B 切换面板。  
壳层拆分（P318）：`useStudioSession` + `Workbench`，`App` 仅组装；再拆 `useStudioProject` / `useStudioEdit` / `useStudioSim`。  
Tauri 对齐：`tauri` crate 2.11.x 与 `@tauri-apps/api` 2.11.x。  
Studio 自动载入测试（P319）：`MODAI_STUDIO_OPEN` / `--project=` GUI 自动打开；`--smoke-open=` / `--smoke-example=` / `--smoke-all` 无头 open+validate+simulate；菜单 Examples + 欢迎页打开内置可视图可测示例；`test_studio_autoload.ps1`。  
经典主题管理（P320）：Dark/Light/Classic Blue/High Contrast；设置页色板；壳层/画布/图表 CSS token 适配。  
打开示例自动 Validate（P321）：成功切 Results 并勾选 diagram `plotHints`；Run 时优先沿用已选变量。  
Simulink 工程化框图（P322）：直角块体 + 侧端口标签 + 正交折线 + 工程网格；去掉流程图圆角卡片样式。  
Draw 图元库（P323）：Library Draw 目录；Rectangle/Ellipse/Line/Polygon/Text/Note；画布 SVG 渲染；不进 `.mo`。  
Draw 拖拽缩放（P324）：选中 Draw 节点显示 `NodeResizer`；缩放写回 `ui`；吸附开启时对齐网格。  
可视化对标说明（P325 doc）：`09-visual-graphics.md` 映射截图能力到 06 分期。  
块体 Mask SVG（P325）：按 catalogId 渲染 Gain/Add/Integrator/Step 等内嵌图标。  
Modelica Icon/SVG 分层说明（P326 规格）：见 `09-visual-graphics.md` §5；下一步实现读 Icon→块面（连线/仿真仍走 ports/equations/experiment）。  
包装 .mo 自动多 IO（P326）：`studio_wrap_mo` 从模型接口抽取多入/多出端口 + 参数，生成 Wrapped catalog 图元可拖拽连线。  
包装读 Icon 渲块面（P327）：wrap 时 `parse_icon` → `catalog.icon` / `node.ui.icon`；`IconFace` 渲染 Line/Rect/Ellipse/Polygon/Text。  
边标签布局 + 分支 junction（P328）：自定义 `studioStep` 边；短信号名标签；同源多扇出画 junction 点。  
打开案例 UI 挂起（P329）：`studio_validate` 改为 async + `spawn_blocking`；打开后先绘框图再 JIT 校验。  
校验停 Analyze（P330）：`validate_model` 默认 `CompileStopPhase::Analyze`，跳过 JIT codegen；Run 仍 Full。  
Messages Analyze + Full JIT 开关（P331）：Update 成功标明 Analyze only；工具栏 Cpu 开关可选 Full JIT 校验。  
In/Out 色签块（P332）：Library **Terminals** Inport（蓝）/ Outport（橙）；映射 `RealInput`/`RealOutput`；emit `connect(in1, gain.u)`。  
Mux/Bus 粗竖条（P333）：Library **Routing** Mux2–4 / Demux2–4 / Bus2–3；粗竖条 UI；映射 `Multiplex*` / `DeMultiplex*`。  
Goto/From 跳转（P334）：Library **Routing** Goto/From；同 `tag` 发射 `connect(goto.y, from.u)`；`RealPassThrough`；UI 色签。  
块名在下方（P335）：Properties **Label below** → `ui.labelBelow`；名称画在块外下方。  
边折点编辑（P336）：选中边显示折点；Alt+点击 / 双击线加点；拖拽移动；双击折点删除；持久化 `edge.vertices`。  
子系统边界 In/Out（P337）：创建子系统时割边自动插入 Inport/Outport；端口 `In1`/`Out1`；上钻/Save/Validate 时从子图 Terminals 同步父块端口；双线框子系统外观。  
信号库扩充（P338–P352）：Product/Division/Abs/Sqrt/Sign/Sin/Cos/Asin/Acos/Atan/Atan2；Limiter/DeadZone；Ramp/Pulse/ExpSine；Lookup1D；TF/Derivative/PI/PID；Display sink。  
图标+变体（P353–P354）：新块 Mask 图标；Continuous/Sources/Math/Nonlinear 变体组扩展。  
收藏+旋转+查找（P355–P357）：Library 收藏；Ctrl+R 旋转 90°；Ctrl+F 画布查找；端口悬停类型提示。  
参数字典（P358–P365）：`manifest.paramDictionary`；块参数 `$Name` 在 emit / Fast Restart 解析；活动栏 Data 面板；Properties Promote；Dashboard 字典旋钮；loggedVariables 编辑。  
From File + 激励源（P366–P372）：`CombiTimeTable` FromFile；CSV→`table=[...]`；Clock / BooleanStep/Pulse/Constant；图标。  
导出 SVG + 文档（P373–P378）：菜单 Export SVG；docs/CHANGELOG 对齐 F85/F86。  
静态检查 + 结构告警 + PNG（P379–P398）：Model Advisor 规则（未连端口/孤立块/缺 Ground/Goto 标签/空子系统/信号环）；编译结构类消息加 `[structure]`；Update 合并顾问结果；Messages 可点聚焦；Export PNG。  
Model Reference + 用户库包（P399–P418）：Wrap .mo → ModelRef catalog；emit 内联引用源；validate 自动注入 wrapped/userlib 路径；Import/Export Pack（catalog.json）；UserLib 分类与 Ref 图标。  
Configurable + Discrete + 采样色（P419–P438）：多选 Configurable 类型切换与批量参数；Discrete（Sampler/ZOH/FOH/UnitDelay/Discrete TF）与 Lookup2D 目录；F73 离散块虚线框/橙色条与 Ts 徽章；变体组 discrete/tables。  
物理库 + Logical（P439–P458）：Analog（Sine/Pulse 电压、电流源、IdealDiode、开合开关）；Rotational（Fixed/SpringDamper/IdealGear/Speed/Angle·TorqueSensor）；Logical And/Or/Not/Switch + Bool↔Real；采样时间图例；mechanical 域色。  
Variant Subsystem（P459–P478）：子系统多实现（ui.variants）；Add Variant 克隆活动子图；Properties 切换/重命名/删除；emit 仅展开 active；Advisor 端口不一致告警；块面 V 徽章。  
Translational + Multibody（P479–P498）：1D Mass/Spring/Damper/Fixed/Force/Speed/Position/Sensors/GearR2T；3D Prismatic/Spherical/Universal/FreeMotion/FixedRotation/BodyCylinder/PointMass/驱动关节；Configurable 变体组。  
信号线属性 + 域连线（P499–P518）：边 `unit`/`dimension`；按 domain 着色线宽/虚线/箭头；连线 Properties；画布域图例 / 采样时间 / 缩略图贴边悬停，点画布收成箭头。

---

## Q4 下一轮任务板（三档）

原则：不再以「扩 catalog ID」为主，按必要性三档推进。  
闭环粒度：优先整档收口（A → B → C），单档内可按 `run_next -Board q4` 逐条推进。

### 里程碑

| ID | 档 | 焦点 | 任务 | 必要性 |
|----|----|------|------|--------|
| Q4-A | A | Catalog/emit 可信门禁 + 观测闭环 | P519–P530 | 最高：敢用 + 能排错 |
| Q4-B | B | 传感器桥 + 混模示例 + unit/域一致性 | P531–P538 | 高：机电验收 |
| Q4-C | C | 自动路由避让 + Icon/标签专业化 | P539–P546 | 中高：采纳体验 |

### 明确延后（本板不排）

- ~~F89 Profiler~~ / ~~Rate Transition~~ / ~~F90 Export~~ → 见对应任务板
- 真多时钟分区调度 UI
- 频域 / 线性化工具
- 逐块 residual（JIT 钩子）

### 档 A — 可信门禁 + 观测闭环（P519–P530）— done

| ID | 状态 | 说明 | 验收要点 |
|----|------|------|----------|
| P519 | done | Catalog FQN 盘点表：`docs/modai-studio/catalog_gate_inventory.json` | `--dump-catalog-gate=` |
| P520 | done | 门禁脚本：`modai-studio/scripts/run_catalog_gate.ps1` | validate 报告 JSON |
| P521 | done | 信号库门禁：signal-core Analyze 全绿 | 见 `catalog_gate_report.json` |
| P522 | done | 物理库门禁：physical-core(+routing) Analyze 全绿 | 同上 |
| P523 | done | emit 黄金：嵌套子系统前缀 `plant_*` | `--check-emit-golden=` |
| P524 | done | emit 黄金：Variant 仅 active + Goto/From connect | 同上 |
| P525 | done | Messages `[fallback]` 标注 placeholder/fallback | `validate_model` 注解 |
| P526 | done | F50 Scope/Display 选中绑定 Results | `blocks.sinks.scope` |
| P527 | done | F51 选中边快速绘图 | `seriesKeysForEdge` |
| P528 | done | F52 Display 块面显示 Run 终值 | `displayEndValues` |
| P529 | done | F53 结果曲线光标读数 | axisPointer + readout |
| P530 | done | F81 消息聚焦增强；档 A 构建验收 | cargo + npm build 绿 |

### 档 B — 物理可读 + 传感器桥（P531–P538）

F71 约定：MSL `*.Sensors.*` 自带 RealOutput，**无**独立 PS 转换块；物理法兰/针脚 → Sensor → 信号块。  
新增 catalog：`catalog/sensors.rs`（Potential/Power、Rot Speed/Acc/Rel*、Trans Speed/Acc/Rel*）。  
示例：`rot_sensor_control`、`elec_sensor_control`。

| ID | 状态 | 说明 | 验收要点 |
|----|------|------|----------|
| P531 | done | F71 补齐常用电气/机械传感器 catalog | `extra_sensor_blocks` |
| P532 | done | 传感器 → Real 桥接约定 + sensor 图标 | 见上约定；`blockIcons` sensor |
| P533 | done | 连线时从端口推断默认 `unit`/`dimension` | `sensorMeta.ts` + onConnect |
| P534 | done | Advisor：域不一致 / unit 冲突提示 | `domainMismatch` / `unitConflict` |
| P535 | done | 示例：控制 + 旋转机械混模 | `examples/rot_sensor_control` |
| P536 | done | 示例：电气 + 控制混模 | `examples/elec_sensor_control` |
| P537 | done | 混模示例纳入门禁脚本 | `run_catalog_gate.ps1` smoke |
| P538 | done | docs/CHANGELOG 对齐 F71；档 B 构建验收 | 本文 + CHANGELOG |

### 档 C — 画布专业化（P539–P546）

| ID | 状态 | 说明 | 验收要点 |
|----|------|------|----------|
| P539 | done | 自动正交路由：新连线避块 | 新边不穿主块心 |
| P540 | done | 反馈环绕线 / 重布线命令 | 工具栏 Reroute；选中边或全部；拖动模块松手后重布相关连线 |
| P541 | done | 顶频信号块 Mask SVG 覆盖补齐 | Math/Sources/Continuous + min/max/2nd |
| P542 | done | 顶频物理块 Mask/IconFace 覆盖补齐 | Analog/Mechanics 域占位稳定 |
| P543 | done | wrap/MSL 常用类型 Icon 回退策略 | wrap/userlib/Modelica → 域 glyph |
| P544 | done | Mux/Bus 维数徽章 + 标签布局 | 条面 `n→m` 徽章 |
| P545 | done | 边标签偏移 / 显隐设置 | Settings：显隐 + 垂直偏移 |
| P546 | done | 更新 `09-visual-graphics`；档 C 构建验收 | 文档 + 构建绿 |

---

## F89 仿真 Profiler 任务板

目标：Run 后给出 wall-clock 阶段耗时（emit / compile / apply_params / integrate），底栏可查看。  
机器可读：`docs/modai-studio/tasks_f89.json`  
延后：逐块 residual 剖析（需 JIT 钩子）、多速率、FMU 导出打磨。

| ID | 状态 | 说明 | 验收要点 |
|----|------|------|----------|
| P547 | done | 任务板 + `SimProfile` 类型（Rust/TS） | JSON + 类型可序列化 |
| P548 | done | `studio_simulate` 计时 emit | profile.emitMs |
| P549 | done | worker 计时 compile / Fast Restart 跳过 | compileMs + skipped |
| P550 | done | 计时 apply_params + integrate | 两阶段 ms |
| P551 | done | `SimulateResponse.profile` + progress phases | 前端可读 |
| P552 | done | RunRecord / lastProfile 保存 | Runs 可回看 |
| P553 | done | ResultsDock Profiler 页：表 + 条形图 | 阶段占比可读 |
| P554 | done | i18n + progress 阶段文案 | 中英对齐 |
| P555 | done | docs/CHANGELOG；构建验收 | cargo + npm 绿 |

### F89 闭环规则

1. 按 `tasks_f89.json` 顺序推进；整板收口后停止本阶段。
2. 不自动生成测试程序；以构建 + 一次示例 Run 有 profile 为准。
3. 禁止项不变。

---

## Rate Transition / 多速率任务板（P556–P562）

目标：显式速率转换块（Simulink Rate Transition 直觉），映射 MSL Discrete ZOH / UnitDelay；Advisor 提示 Ts 不一致；示例可 Run。  
机器可读：`docs/modai-studio/tasks_rate.json`  
约定：无独立 JIT 多时钟调度 UI；`samplePeriod` 为输出保持/采样周期。

| ID | 状态 | 说明 | 验收要点 |
|----|------|------|----------|
| P556 | done | 任务板 + 范围（ZOH/UD 映射） | tasks_rate.json |
| P557 | done | Catalog RateTransition + RateTransitionUD | Discrete 目录可见 |
| P558 | done | Configurable 变体组 + Library 标签 | 可切换族内类型 |
| P559 | done | Mask + 采样色 / Ts 徽章 | rateTransition 识别 |
| P560 | done | Advisor：Ts 不一致且无桥接块 | sampleRateMismatch |
| P561 | done | 示例 `rate_transition` | open + Run |
| P562 | done | docs/CHANGELOG；构建验收 | cargo + npm 绿 |

### RATE 闭环规则

1. 按 `tasks_rate.json` 推进；整板收口后停止。
2. 不自动生成测试；以构建 + 示例为准。
3. 禁止项不变。

---

## F90 Export Suite 任务板（P563–P574）

目标：整板收口导出体验——向导（Coder / FMU CS / FMU ME）、标识符与 GUID、产物预览、导出历史、打开目录、smoke emit。  
机器可读：`docs/modai-studio/tasks_f90.json`

| ID | 状态 | 说明 | 验收要点 |
|----|------|------|----------|
| P563 | done | 任务板 + EmitOptions 类型 | tasks_f90.json |
| P564 | done | 后端 CS/ME + FmiExportOptions | ME 可出包 |
| P565 | done | EmitRequest 扩展字段 | fmiKind/modelId/guid |
| P566 | done | ExportWizard 对话框 | Coder/CS/ME 可选 |
| P567 | done | 导出前产物预览列表 | 向导内可见 |
| P568 | done | 菜单 Emit 走向导 | 不再裸选目录 |
| P569 | done | 上次导出选项持久化 | settings.lastExport |
| P570 | done | Results Export 历史页 | EmitRecord |
| P571 | done | 打开导出目录 | opener revealItemInDir |
| P572 | done | `--smoke-emit-example=` | JSON ok |
| P573 | done | i18n | EN (+ ZH keys) |
| P574 | done | docs/CHANGELOG + 构建冒烟 | 绿 |

### F90 闭环规则

1. 整板推进，不拆成单点功能收口。
2. 不自动生成测试；以构建 + smoke-emit 为准。
3. 禁止项不变。

## P4 实用手感（已收口）

规格：[10-ux-practical-review.md](./10-ux-practical-review.md) §7–§8。  
P4-A 主路径手感已落地（库拖拽、落点、四区常驻、连线不弹窗、双击/右键、仿真条 Pause/Step）。  
P4-B 观测闭环已落地（数值时间轴 / 变量树 / Data 表 / Run 参数快照 / loggedVariables 对齐 / Dashboard 钉选+应用并 Run）。  
P4-17 准实时 Scope 已落地（积分输出行写入 `LiveSeriesHandle`，`studio-sim-progress` 每 250ms 推增量点；Pause 停画）。  
P4-18 Configuration 对话框已落地（StopTime / solver 说明 / tolerance / 步长 / 记录变量；CVODE/IDA 在未开 sundials 时禁用）。  
P4-19 Messages 分栏已落地（Compiler / Advisor / Connection；顾问规则 i18n + 修复提示；点击聚焦）。  
P4-22 属性参数 unit/description 已落地（catalog `CatalogParam` 补字段；Properties 显示 `R` 旁 Ohm；Dashboard 旋钮带单位）。  
P4-20 最近工程已落地（欢迎页最近 8 个；打开失败剔除；脏关闭 / 换工程确认）。  
P4-21 示例打开先出图已落地（Validate 后台跑；状态栏可取消；不挡画布与 Run）。

## P5 主路径补全（已收口）

P4 已收口后仍缺的实用项（不含 P4-D 延后项）：未落盘 Run 先保存；属性标签 i18n；拖线吸附兼容口；旋转后端口/连线跟随；下拉菜单；自动保存。

| ID | 状态 | 说明 | 验收要点 |
|----|------|------|----------|
| P5-01 | done | 无工程目录时 Run/Update/导出打开保存向导 | 不跳过保存直接仿真 |
| P5-02 | done | 属性面板 Name/Type/尺寸/标签位置 i18n | 中文环境无裸英文标签 |
| P5-03 | done | 拖线高亮可连端口 + 更大吸附半径 | 从 Gain.y 拖出时 Integrator.u 发光 |
| P5-04 | done | 旋转 90° 后门位与连线跟随 | 端口改到上/下/左/右；松手后重布相关连线 |
| P5-05 | done | File/Edit/Export/Simulation 真正下拉菜单 | 标题栏文字菜单，不再是图标组 |
| P5-06 | done | 脏工程定时自动保存 / 崩溃恢复 | 有目录时 30s 写盘；localStorage 草稿，异常退出可恢复 |

## IDE 单向导入（F94）

File → Import IDE diagram：读取 IDE `DiagramModel` / `GraphicalDocumentModel` JSON，适配为 Studio `DiagramDocument` 后替换当前根图（脏画布先确认）。不反向导出、不共享类型定义。适配器：`modai-studio/src/import/ideDiagram.ts`。

## StateGraph catalog（F95，done）

不另做 Stateflow / JSON 状态图 / 第 17 章编辑器。Library 增加 `Modelica.StateGraph` 的 `InitialStep` / `Step` / `Transition` / `*WithSignal`；emit 在出现图节点时补 `inner StateGraphRoot`。验收示例：`modai-studio/examples/stategraph_first`（FirstExample 式启停环）。MSL 4 源码在 `jit-compiler/Modelica/StateGraph.mo`。

## 历史闭环规则（P0）

1. 每次取 `run_next.ps1` 指向的 `in_progress` 任务实现。
2. 完成后将该任务标 `done`，再取下一 pending。
3. P0 全部 `done` 后停止本阶段；后续阶段另开任务板。
4. 不自动生成测试程序；以构建与示例 Run 为门禁。
