# Studio 可视化图形对标（相对 Simulink 截图）

日期：2026-07-23  
关联：[06 Simulink 能力对照](./06-simulink-capability-map.md) · [04 功能说明](./04-features.md) · [07 编译器可行性](./07-compiler-feasibility.md)

## 1. 有没有专门说明书？

**有，且以本仓库文档为准**（不复制 MathWorks 闭源 UI 规格）：

| 文档 | 作用 |
|------|------|
| [06-simulink-capability-map.md](./06-simulink-capability-map.md) | **主说明书**：Library / Editor / Subsystem / Mux·Bus / Mask 图标等分期 |
| [04-features.md](./04-features.md) | 功能 ID（F06 连线、F23 子系统、Routing 等） |
| [07-compiler-feasibility.md](./07-compiler-feasibility.md) | 哪些能做、哪些永远不对齐 Simulink 语义 |
| [03-data-structures.md](./03-data-structures.md) | 节点/边/Draw 图元落盘 |
| MathWorks 公开文档 | 仅作交互直觉参考（Block / Port / Signal / Bus）；**不**作实现兼容验收 |

原则不变：**交互形态学 Simulink，语义学 Modelica**。

## 2. 截图能力 → 对照表 → Studio 现状

对照典型控制框图（多端口 Switch、Saturation 内嵌曲线、Gain 三角、粗 Bus/Mux 竖条、In/Out 色签、正交分支、反馈环）：

| 截图现象 | 06 对照项 | 阶段 | Studio 现状 | 丰富方向 |
|----------|-----------|------|-------------|----------|
| 直角块 + 左右端口 | 端口连线 | P0 | 已有工程化块体/端口 | 继续细化端口几何 |
| Gain 三角 / 运算符小方块 | Mask 图标脚本 | P1–P2 | 文本块为主 | **catalog SVG Mask** |
| Saturation 内嵌 S 曲线 | Mask 图标 | P1–P2 | 无内嵌 glyph | SVG 模板按 `catalogId` |
| Switch 多输入口 | 端口连线 | P0 | 多端口垂直分布已有 | 强化端口间距/标签 |
| Inport 蓝 / Outport 橙签 | Goto/From 或 In/Out | P1 | Terminals 色签 + Routing Goto/From | 已落地 P332/P334 |
| 正交折线 + 箭头 | 智能引导线 | P0–P1 | `studioStep` 正交边 + junction + 折点 | 已落地 P328/P336 |
| 粗竖条 Mux/Bus | Mux/Demux/Bus | P1 | Library **Routing** 粗竖条 | 数组维扩展 / Bus Object |
| 反馈环绕线 | 端口连线 | P0 | 可连，路由一般 | 自动正交避让 |
| 子系统大块自定义图标 | Subsystem + Mask | P2 | 双线框子系统 | 子图 + icon |
| 线上信号名 | 信号线命名 | P1 | `StudioStepEdge` 短标签 + tooltip 全名 | 可调偏移 / 隐藏 |
| 分支连接点 | 信号分支 junction | P1 | 同源多目标实心圆点 | 已落地 P328 |
| 块名在块下方 | 模块标签 | P1 | `ui.labelBelow` | 已落地 P335 |
| 离散块采样色 / Ts | 采样时间着色 F73 | P2 | `domain-discrete` 虚线框 + `Ts=` 徽章 + 贴边悬停图例 | 已落地 P419–P458 |
| Configurable 多选换型 | Configurable F63 | P2 | 多选同族切换 type + 批量参数 | 已落地 P419–P438 |
| 物理库扩充 | Analog/Rotational F70 | P2 | 理想开关/二极管/齿轮/传感器等 | 已落地 P439–P458 |
| Variant 子系统 | Variant Subsystem F87 | P2 | 多子图切换，emit 仅活动 | 已落地 P459–P478 |
| Translational / Multibody | F70/F74 | P2 | 1D 平动 + 3D 关节/体 | 已落地 P479–P498 |
| 域连线样式 + 单位维度 | F72 | P2 | 信号/电气/机械线型与 unit；连线/采样时间/缩略图贴边箭头 | 已落地 P499–P518 |
| 观测闭环加深 | F50–F53 | Q4-A | Scope 绑定 / 边快绘 / Display 终值 / 光标 | 任务板 P526–P529 |
| 传感器桥 + 混模示例 | F71 | Q4-B | 物理→信号 RealOutput；unit 推断 | 已落地 P531–P538 |
| 自动避让路由 + Icon 覆盖 | F37 / Mask | Q4-C | 新边避块、反馈环、拖动松手重布相关连线、Mask 补齐 | 已落地 P539–P546 |

下一轮视觉/体验切片：Q4、F89、Rate Transition、**F90 Export Suite** 已落地。延后项：真多时钟分区 UI、逐块 residual、频域/线性化。

## 3. 可参照丰富吗？

**可以，且应按 06 分期切片，不要按截图像素复刻。**

推荐落地顺序（工作量从大到可交付）：

1. **P325 Mask SVG**：按 `catalogId`/`typeName` 渲染内嵌图标（Gain / Add / Product / Integrator / Step…）— 已落地
2. **P328 边标签与分支点**：短信号名可读；同源多目标时画 junction 点 — 已落地
3. **P332 In/Out 色签块**：Library Terminals（RealInput/RealOutput）— 已落地
4. **P333 Mux/Bus 条**：粗竖条 Mux/Demux/Bus → Multiplex/DeMultiplex — 已落地
5. **P334 Goto/From**：同 tag 语义跳转 + 色签 — 已落地
6. **P335 labelBelow**：块名可选画在下方 — 已落地
7. **P336 边折点**：Alt/双击加点、拖拽、持久化 vertices — 已落地
8. **P337 子系统边界**：创建时自动 In/Out；上钻同步端口；双线框 + 色柄 — 已落地
9. **P338–P357**：信号库 20 项扩充 + 收藏/旋转/查找 — 已落地
10. **P358–P378**：参数字典 F85 + From File F86 + Export SVG — 已落地
11. **P379–P398**：Model Advisor F80 + 结构告警 F81 + Export PNG F82 — 已落地
12. **P399–P418**：Model Reference F62 + 用户 catalog 包 — 已落地

禁止项（见 07）：Stateflow Chart、JSON 状态图、完整 Bus Object 生态、与 MATLAB 数值逐块对等。顺序图若做，是同画布上的 StateGraph 块，不是独立状态图画布。

## 4. 本轮实现

P338–P357：约 20 项闭环——Math/Nonlinear/Sources/Tables/Continuous/Display catalog；图标与变体；Favorites；Rotate；Find；端口 tip。  
P358–P378：`paramDictionary` + `$Name`；FromFile CSV→table；Clock/Boolean 源；Data 活动栏；Export SVG。  
P379–P398：静态检查清单；结构/代数环消息标注；Export PNG。  
P399–P418：ModelRef（Wrap .mo）；userlib pack Import/Export；emit 内联引用类；库路径自动注入。

## 5. 包装模型为图元：多输入/多输出与属性（澄清）

### 5.1 你要的能力（正确理解）

不是「SVG 文件本身定义仿真」，而是：

1. 把一个 Modelica 模型**包装成一个可拖拽图元**（外观可用 Icon 渲成的 SVG）
2. 包装时**自动生成多个输入端口、多个输出端口**，供画布连线
3. 选中该图元时，在属性面板配置**参数**（以及可选的运行相关设置）

**可以做。** 多 IO 端口必须来自模型的**公共接口声明**，不是从 SVG 路径里“识别”出来。

### 5.2 自动产生端口：数据从哪来

| 来源 | 自动生成什么 | Studio 现状 |
|------|--------------|-------------|
| `.mo` 中 `input` / `output` / `connector` | 左/右多个 `PortDef`（可多入多出） | 内置 catalog **手工**写死 ports；子系统封装时已按切开边**自动**生成 `u1..`/`y1..` |
| FMU `modelDescription.xml` | 多入多出信号口 | `studio_import_fmu` 已自动提取 |
| Icon / SVG 图形 | 仅块面外观；可选 Placement 提示端口锚点 | 手工 Mask；Icon 解析未接 Studio |

标准写法示例（多入多出接口 → 包装后应变成多个连线把手）：

```modelica
block PlantWrapper
  Modelica.Blocks.Interfaces.RealInput u1;
  Modelica.Blocks.Interfaces.RealInput u2;
  Modelica.Blocks.Interfaces.RealOutput y1;
  Modelica.Blocks.Interfaces.RealOutput y2;
  parameter Real gain = 1.0;
  ...
end PlantWrapper;
```

包装流水线应为：

```text
.mo 类型
  ├─ public input/output/connector ──▶ ports[]（自动，多入多出）
  ├─ parameter ──────────────────────▶ params[]（属性面板可改）
  └─ Icon graphics（可选）───────────▶ 块面 SVG
画布：拖拽实例 → 按 ports 连线 → emit connect → 仿真
```

### 5.3 属性设置 vs 仿真运行条件

| 设置类型 | 挂在哪 | 作用 |
|----------|--------|------|
| 图元参数（`parameter`，如 gain、R） | 该图元 `node.params` | 改实例修饰符，影响方程；Properties 已支持 |
| 运行条件（StopTime、步数/Interval、solver） | 通常项目 `simulationDefaults` / `experiment` | 对整次 Run；不是每个图元一份（除非把顶层模型当图元且约定暴露） |

结论：

- **图元参数配置：应当且已经按块做。**
- **步长/StopTime：默认整图一次 Run 配置**；若包装的是“可运行子模型”，可另增“运行覆盖”字段，但语义上仍是仿真选项，不是 SVG。

### 5.5 Icon 块面（P327）

包装 `.mo` 时若存在 `annotation(Icon(...))`：

1. `rustmodlica::annotation::parse_icon` → JSON 存入 `CatalogEntry.icon`
2. 放置节点时复制到 `node.ui.icon`
3. `StudioBlockNode` 优先用 `IconFace` 渲染；无 Icon 时仍用 Mask SVG

端口仍只来自模型接口，与 Icon 图形无关。
