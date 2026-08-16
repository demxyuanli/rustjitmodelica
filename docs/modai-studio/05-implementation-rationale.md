# Modai Studio 实现论证

日期：2026-07-21（修订：2026-08-16 离散逻辑宿主）  
目的：在编码前论证“为什么这样做、先做什么、风险如何收口”。  
关联：本目录其余规格文档。

## 1. 结论摘要

采用 **monorepo 内独立 Tauri 应用（`modai-studio`）+ XYFlow 画布 + 双文件工程 + path 依赖 `rustmodlica` 薄仿真桥**。  
该路径在交付速度、产品边界清晰度、与现有编译器复用之间平衡最好；不在 MVP 抽取跨 IDE 的共享画布内核。

## 2. 关键决策论证

### 2.1 为何独立产品而不是改 modai-ide

| 选项 | 优点 | 缺点 |
|------|------|------|
| 增强 IDE | 少一个包 | IDE 已是代码工作台，强行改成 Simulink 会撕裂信息架构 |
| 从 IDE 拆画布 | 复用代码 | JointJS 会话/性能约束与 XYFlow 目标不符，拆分成本≈重写 |
| **独立 Studio** | 交互可按框图产品设计；失败不影响 IDE | 短期有重复 UI |

**选择独立 Studio**：需求已确认产品形态是 Simulink 式，不是 IDE 皮肤。

### 2.2 为何 XYFlow 而不是 JointJS

- IDE 的 JointJS 路径已暴露 WebView 性能敏感点（大量 reconcile / link routing）。
- XYFlow 更贴“节点编辑器”模型，生态与自定义 node 成本更适合信号流+多端口组件。
- 独立产品无历史包袱，可直接选更匹配的画布。

代价：不能复用 IDE 的 Joint 渲染代码——可接受，因本就是独立产品。

### 2.3 为何双文件而不是纯 annotation

| 方案 | 优点 | 缺点 |
|------|------|------|
| 纯 JSON 生成临时 mo | 实现快 | 用户资产不可移植 |
| 纯 `.mo` annotation | 标准 | annotation 往返成本高，易与 OMEdit 细节缠斗 |
| **`.mo` + `.diagram.json`** | 仿真路径干净；布局迭代快 | 两文件需保持一致 |

**选择双文件**：MVP 核心是仿真正确 + 画布好用；标准 annotation 互通放到 P2。

一致性策略：Save / Run 时由同一 `emit_mo` 覆盖写 `.mo`，以 diagram 为编辑真相。

### 2.4 为何库 API 桥而不是只调 CLI

| 方案 | 优点 | 缺点 |
|------|------|------|
| CLI 子进程 | 隔离强 | 进度/诊断/取消差；Windows 进程开销与路径细节多 |
| **path 依赖库 API** | 与 IDE 同级能力；类型共享 `SimulationResult` | 需注意 IDE 与 Studio 同时链同一 crate 的构建时间 |

Studio 命令面保持**薄**：不复制 IDE 的方程图、自迭代、监控会话等。

### 2.5 为何内置 Catalog 而不是运行时扫全 MSL

- 全量 MSL 扫描与 icon/端口推断本身是大型工程（IDE 已在做且复杂）。
- MVP 只需十几个组件证明闭环。
- Catalog 静态表可测、可评审、可按编译器真实支持度裁剪。

后续可增加“从库导入组件到 catalog”的工具链，不阻塞产品骨架。

### 2.6 MSL 依赖策略（必须事先说清）

信号/物理组件类型名指向 Modelica Standard Library。实现时三选一（**推荐 S1**）：

| 策略 | 做法 | 适用 |
|------|------|------|
| **S1 推荐** | 复用仓库已有 MSL pack / 环境库路径；示例假定 MSL 可用 | 与现有工具链一致 |
| S2 | MVP 自带 `StudioLib` 极简 `.mo` 组件，类型名挂自家包 | 无 MSL 也能演示，但偏离标准 |
| S3 | 混合：信号用 Blocks，物理用自备简化模型 | 折中，文档成本高 |

规格默认 **S1**；若 CI/开发机缺少 MSL，示例与文档给出库路径配置（F21 可提前到 MVP）。

## 3. 建议模块落地顺序（论证用，非详细计划）

编码前仍需单独的实现计划；此处只论证依赖顺序：

```text
1. Workspace + Tauri 空壳 + IPC ping
2. project I/O (manifest + diagram schema v1)
3. catalog JSON + Library panel
4. XYFlow canvas CRUD + connect rules (signal first)
5. emit_mo (signal subset)
6. sim_bridge validate/simulate + plot
7. physical subset ports/rules + rc example
8. polish: i18n, messages, example pack
```

理由：每一步都有可运行增量；避免先做物理域导致连线规则与生成器同时爆炸。

## 4. 与 `rustmodlica` 的接口契约（论证）

最小需要的能力（名称以现有库为准，实现计划再钉死具体函数）：

1. 从源码字符串 + modelName + 库路径 → validate
2. 同上 → simulate → `SimulationResult { time, series }`
3. 错误信息可展示（字符串或结构化）

参考对照：`modai-ide` 的 `jit_validate` / `run_simulation_cmd`，但 Studio **重写薄封装**，避免拖入 IDE command 巨石文件。

若库 API 不便直接链：允许内部暂时 spawn 同仓库构建出的 CLI，但规格仍要求最终以库调用为主（方案 1）。

## 5. 风险登记与收口

| ID | 风险 | 等级 | 收口 |
|----|------|------|------|
| K1 | 所选 Blocks/Analog 类型编译器支持不全 | 高 | Catalog 以 TestLib/回归已绿类型为准；表可替换 |
| K2 | XYFlow 多端口物理组件难用 | 中 | 自定义 node；限制 MVP 端口数 |
| K3 | emit_mo 与手写 mo 差异导致用户困惑 | 中 | UI 标明“由框图生成”；P2 只读预览 |
| K4 | 双产品维护成本 | 中 | 严格边界；共享仅编译器 |
| K5 | 文件 >800/1000 行失控 | 中 | 按目录拆分 commands/canvas/emit |
| K6 | schema 无迁移即改字段 | 低 | schemaVersion + 拒绝未知主版本 |

## 6. 明确不做（防范围蔓延）

实现阶段若出现下列请求，应回到规格变更流程，不得静默做进 MVP：

1. 画布 ↔ 文本双向同步
2. 完整 OMEdit annotation 兼容
3. 把 Studio 嵌进 IDE 或反过来合并仓库产品
4. AI 自迭代改编译器
5. 自动生成测试工程/审计脚本
6. 为了“好看”重做 IDE 图表栈

## 7. 验证论证

用户约束：不自动生成测试程序。因此 MVP 验证集定义为：

1. **编译验证**：`cargo build -p modai-studio`；前端 production build
2. **示例验证**：两示例手动 Run 出曲线
3. **格式验证**：保存后 JSON schema 字段完整；`.mo` 可被 `rustmodlica --validate` 接受

详细命令写入实现计划；规格阶段只锁定验收口径。

## 8. 文档与代码的门禁关系

```text
文档评审通过
  -> 编写实现计划 (docs/modai-studio/ or docs/superpowers/plans/)
  -> 按计划编码
  -> 编译验证
  -> 示例验收
```

**当前停止点**：等待本目录文档评审通过。未通过前不创建 `modai-studio/` 应用代码骨架以外的实现（若需空目录占位，也应在评审后）。

## 9. 开放问题（评审时拍板）

| # | 问题 | 建议默认 | 状态 |
|---|------|----------|------|
| Q1 | 产品显示名中文？ | “Modai Studio” | 待确认 |
| Q2 | TransferFunction 支持差？ | **默认 FirstOrder**；TF 升 P1 | 规格已采纳默认 |
| Q3 | 曲线库 | ECharts | 待确认 |
| Q4 | MVP 是否带 MSL 引导？ | 是（精简 bootstrap） | 待确认 |
| Q5 | 自动保存？ | 仅 Run 前保存；Ctrl+S | 待确认 |
| Q6 | P0 Scope 是否强制画布块？ | **否**，全局 Results 面板 | 规格已采纳 |
| Q7 | 子系统数据形态？ | 预留 `diagramId`/`subsystemRef`；P2 再选嵌套 model | 规格已预留 |
| Q8 | 顺序逻辑 /「像 Stateflow」？ | **不做 Chart**。宿主：`when`/Logical + 同图画 `Modelica.StateGraph` 块。不用 StateGraph2；第 17 章不进 Studio 排期 | 规格已采纳 |

评审记下结论后更新 [00-index.md](./00-index.md) 决策基线。

## 10. Simulink 对标后的实现含义

1. **不要**按 Simulink 菜单一一抄齐再开工；按 P0→P1→P2 切片。
2. **要**在 P0 数据结构留下层级/标签/run 扩展点，避免 Core/Pro 推倒 schema。
3. Catalog 名单以“编译器能跑”为硬门禁，名称上贴近 Simulink 用户直觉即可。
4. 完整对照与 Out 清单见 [06-simulink-capability-map.md](./06-simulink-capability-map.md)。
5. **不要**为顺序逻辑发明第三种语言或 JSON 状态图；需要 Grafcet 时只加 StateGraph catalog（F95）。
6. **不要**把第 17 章语言状态机当成 StateGraph 的实现，也**不要**以为做了第 17 章就有了 StateGraph。
