# Modai Studio 数据结构说明

日期：2026-07-21（修订：2026-08-16 离散逻辑宿主）  
关联：[设计说明](./02-design.md) · [功能说明](./04-features.md) · [Simulink 对照](./06-simulink-capability-map.md)

本文定义落盘格式、内存模型与 IPC 载荷。字段名在 JSON 中使用 **camelCase**；Rust 侧用 `serde(rename_all = "camelCase")` 对齐。

## 1. 项目目录约定

```text
MyPlant/
├── studio-project.json          # project manifest
├── Plant.mo                     # simulatable Modelica source (truth for sim)
├── Plant.diagram.json           # canvas document (truth for layout/edit)
├── subsystems/                  # P2: optional child diagrams
│   └── ctrl_loop.diagram.json
├── results/                     # P1+: exports and run history
│   ├── last_run.csv
│   └── runs/
│       └── 20260721T120000Z/
│           ├── meta.json
│           └── series.json
└── data/                        # P2: parameter dictionary / input trajectories
    └── params.json
```

规则：

- 一个 Studio 项目对应一个目录。
- 主模型基名一致：`{modelName}.mo` 与 `{modelName}.diagram.json`。
- P0 仅顶层单 diagram；`subsystems/`、`results/runs/`、`data/` 可延后创建。

## 2. `studio-project.json`

```json
{
  "schemaVersion": 1,
  "product": "modai-studio",
  "modelName": "Plant",
  "libraryPaths": [],
  "simulationDefaults": {
    "stopTime": 1.0,
    "numberOfIntervals": 500,
    "solver": "rk45",
    "tolerance": 1e-6,
    "maxStep": null,
    "minStep": null,
    "loggedVariables": []
  },
  "ui": {
    "locale": "zh"
  }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `schemaVersion` | number | 清单版本，从 1 起 |
| `product` | string | 固定 `modai-studio` |
| `modelName` | string | 主模型名，匹配文件基名与 `model` 声明 |
| `libraryPaths` | string[] | 额外 Modelica 库搜索路径 |
| `simulationDefaults` | object | 默认仿真设置（≈ Configuration Parameters 子集） |
| `simulationDefaults.loggedVariables` | string[] | P1：空 = 使用启发式全量/常用输出 |
| `ui.locale` | string | `zh` \| `en` |

## 3. Diagram Document（`*.diagram.json`）

画布唯一真相源。

### 3.1 顶层

```json
{
  "schemaVersion": 1,
  "modelName": "Plant",
  "diagramId": "root",
  "parentDiagramId": null,
  "nodes": [],
  "edges": [],
  "annotations": [],
  "viewport": { "x": 0, "y": 0, "zoom": 1 }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `schemaVersion` | number | diagram schema 版本 |
| `modelName` | string | 必须与项目 `modelName` 一致（子系统可为内部名） |
| `diagramId` | string | P0 固定 `root`；P2 子系统唯一 ID |
| `parentDiagramId` | string\|null | 顶层为 null；子系统指向上级 |
| `nodes` | Node[] | 组件实例 |
| `edges` | Edge[] | 连接 |
| `annotations` | Annotation[] | P1：画布注释；P0 可 `[]` |
| `viewport` | object | XYFlow 视口，可选但建议保存 |

### 3.2 Node

```json
{
  "id": "n_gain1",
  "instanceName": "gain1",
  "typeName": "Modelica.Blocks.Math.Gain",
  "domain": "signal",
  "catalogId": "blocks.math.gain",
  "position": { "x": 120, "y": 80 },
  "rotation": 0,
  "params": [
    { "name": "k", "value": "1.0" }
  ],
  "ports": [
    { "id": "u", "kind": "signal", "direction": "in", "typeName": "RealInput" },
    { "id": "y", "kind": "signal", "direction": "out", "typeName": "RealOutput" }
  ],
  "nodeKind": "component",
  "subsystemRef": null,
  "plotHints": ["y"],
  "ui": {
    "label": "gain1",
    "width": 96,
    "height": 48
  }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | string | 稳定节点 ID（UUID 或 `n_` 前缀） |
| `instanceName` | string | Modelica 实例名；同 diagram 内唯一 |
| `typeName` | string | 完整类型名（生成 `Type instance(...)`） |
| `domain` | string | `signal` \| `electrical` \| `mechanical` \| `other` |
| `catalogId` | string | 内置目录条目 ID |
| `position` | `{x,y}` | 画布坐标（像素） |
| `rotation` | number | 度，P0 可固定 0；P1 支持 90° 步进 |
| `params` | `{name,value}[]` | 值以 Modelica 字面量文本存储 |
| `ports` | Port[] | 可冗余自 catalog；落盘以便离线恢复 |
| `nodeKind` | string | `component` \| `subsystem` \| `sink` \| `annotation`（P0 仅 component） |
| `subsystemRef` | string\|null | P2：指向子 `diagramId` 或文件 |
| `plotHints` | string[] | 结果面板默认勾选的端口/变量后缀（`instance.hint` 匹配扁平 `instance_hint` / `r_0[1]`→`r_0_1`） |
| `ui` | object | 纯展示字段，不影响 `.mo` |

### 3.3 Port

```json
{
  "id": "p",
  "kind": "connector",
  "direction": "bidirectional",
  "typeName": "Modelica.Electrical.Analog.Interfaces.Pin"
}
```

| 字段 | 允许值 | 说明 |
|------|--------|------|
| `id` | string | 端口名，对应 `instance.port` |
| `kind` | `signal` \| `connector` \| `stream` \| `other` | 连线规则用 |
| `direction` | `in` \| `out` \| `bidirectional` | 信号流强制 in→out；物理 connector 多为 bidirectional |
| `typeName` | string | 兼容检查用 |

### 3.4 Edge

```json
{
  "id": "e_1",
  "sourceNodeId": "n_step1",
  "sourcePortId": "y",
  "targetNodeId": "n_gain1",
  "targetPortId": "u",
  "signalName": null,
  "vertices": [],
  "routing": "orthogonal",
  "ui": { "showLabel": false }
}
```

| 字段 | 说明 |
|------|------|
| `sourceNodeId` / `targetNodeId` | 端点节点 |
| `sourcePortId` / `targetPortId` | 端点端口 |
| `signalName` | P1：信号标签（示波器快速绘图用）；P0 可 null |
| `vertices` | 可选折点 `{x,y}[]` |
| `routing` | `default` \| `orthogonal`；P0 可忽略 |
| `ui.showLabel` | 是否在画布显示 `signalName` |

生成 `connect` 映射：

```text
connect(source.instanceName.sourcePortId, target.instanceName.targetPortId);
```

### 3.5 Annotation / Draw 图元（P1）

画布装饰图元走 **Library → Draw** 目录，落盘为 `nodeKind: "draw"` 的节点（无端口），不参与 `emit_mo`。

内置图元：`Rectangle` / `Ellipse` / `Line` / `Polygon` / `Text` / `Note`（`Studio.Draw.*`）。

```json
{
  "id": "n_note1_…",
  "instanceName": "note1",
  "typeName": "Studio.Draw.Note",
  "domain": "draw",
  "catalogId": "draw.note",
  "position": { "x": 40, "y": 20 },
  "nodeKind": "draw",
  "ports": [],
  "params": [
    { "name": "text", "value": "Plant loop" },
    { "name": "fill", "value": "…" },
    { "name": "stroke", "value": "…" }
  ],
  "ui": { "label": "Plant loop", "width": 160, "height": 72 }
}
```

顶层 `annotations: []` 仍保留；新图元优先用 draw 节点，便于与 XYFlow 选中/删除/属性一致。

### 3.6 Run 元数据（P1/P2，`results/runs/*/meta.json`）

```json
{
  "runId": "20260721T120000Z",
  "createdAt": "2026-07-21T12:00:00Z",
  "modelName": "Plant",
  "options": { "stopTime": 1.0, "solver": "rk45" },
  "paramSnapshot": [{ "instanceName": "gain1", "name": "k", "value": "2.0" }],
  "success": true,
  "seriesPath": "series.json"
}
```

用于 SDI-lite 多 run 对比；P0 可不落盘，仅内存展示最近一次 `SimulationResult`。

## 4. 生成的 Modelica（`*.mo`）

由 `emit_mo` 从 DiagramDocument 生成。示例形状：

```modelica
model Plant
  Modelica.Blocks.Sources.Step step1(height = 1.0, startTime = 0.1);
  Modelica.Blocks.Math.Gain gain1(k = 2.0);
  Modelica.Blocks.Continuous.Integrator integrator1;
equation
  connect(step1.y, gain1.u);
  connect(gain1.y, integrator1.u);
  annotation(experiment(StopTime = 1.0, Interval = 0.002));
end Plant;
```

约定：

- 不生成与仿真无关的复杂 annotation 图形（布局只在 diagram JSON）。
- 参数仅输出用户相对 catalog 默认值有覆盖的项（或 MVP 简化为始终写出已知参数）。
- `instanceName` 必须是合法 Modelica 标识符。

## 5. Catalog Entry（内置组件目录）

运行时由 `studio_catalog_list` 返回；也可落在 `src-tauri/catalog/*.json`。

```json
{
  "id": "blocks.math.gain",
  "typeName": "Modelica.Blocks.Math.Gain",
  "domain": "signal",
  "category": "Math",
  "displayKey": "catalog.blocks.math.gain",
  "defaultInstancePrefix": "gain",
  "requiresLibrary": "Modelica",
  "defaultParams": [
    { "name": "k", "value": "1", "valueType": "Real" }
  ],
  "ports": [
    { "id": "u", "kind": "signal", "direction": "in", "typeName": "RealInput" },
    { "id": "y", "kind": "signal", "direction": "out", "typeName": "RealOutput" }
  ],
  "tags": ["signal", "gain"]
}
```

| 字段 | 说明 |
|------|------|
| `requiresLibrary` | 提示依赖；MVP 假定 MSL 可用或提供 stub 包策略（见实现论证） |
| `valueType` | `Real` \| `Integer` \| `Boolean` \| `String` \| `Other` |
| `displayKey` | i18n 键 |

## 6. 内存 Session 模型（前端）

```typescript
interface StudioProjectSession {
  rootDir: string;
  manifest: StudioProjectManifest;
  diagram: DiagramDocument;
  dirty: boolean;
  lastValidation?: ValidationReport;
  lastSimulation?: SimulationResultView;
  selectedNodeIds: string[];
  selectedEdgeIds: string[];
}
```

XYFlow 投影：

- `nodes` ← diagram.nodes（`position` / `data` 携带 ports、params）
- `edges` ← diagram.edges（`sourceHandle`/`targetHandle` = port id）

双向：XYFlow `onNodesChange` / `onConnect` → 更新 `DiagramDocument` → 置 `dirty`。

## 7. IPC 载荷

### 7.1 共用仿真结果

与 `rustmodlica::SimulationResult` 对齐：

```json
{
  "time": [0.0, 0.002, 0.004],
  "series": {
    "integrator1.y": [0.0, 0.004, 0.008],
    "gain1.y": [0.0, 2.0, 2.0]
  }
}
```

### 7.2 `studio_simulate` 请求

```json
{
  "rootDir": "D:/models/MyPlant",
  "modelName": "Plant",
  "code": null,
  "options": {
    "stopTime": 1.0,
    "numberOfIntervals": 500,
    "solver": "rk45",
    "tolerance": 1e-6
  },
  "emitFromDiagram": true
}
```

| 字段 | 说明 |
|------|------|
| `code` | 若非 null，直接仿真该字符串（调试） |
| `emitFromDiagram` | true 时先用当前 diagram 生成 mo 再仿真 |

### 7.3 错误 / 报告

```json
{
  "success": false,
  "stage": "compile",
  "errors": [
    { "message": "...", "path": "Plant.mo", "line": 12, "column": 3 }
  ],
  "warnings": []
}
```

`stage`：`io` \| `diagram` \| `emit` \| `validate` \| `compile` \| `simulate`。

## 8. 连线规则数据（逻辑结构）

不落盘，运行时由规则引擎使用：

```text
RuleContext {
  sourcePort: Port
  targetPort: Port
  sourceDomain: Domain
  targetDomain: Domain
}

Decision = Allow | Deny { reasonKey }
```

MVP 规则摘要：

1. signal：`out` → `in`，且双方 `kind == signal`
2. electrical/mechanical：`kind == connector`，`typeName` 兼容（MVP：字符串相等或同族表）
3. 禁止信号端口连物理 connector
4. 自环禁止

## 9. Schema 演进

- 所有顶层文件含 `schemaVersion`
- 读取时：高版本拒绝并提示升级；低版本用显式迁移函数升到当前
- MVP 只实现 version `1`，迁移框架留空实现点

## 10. 与 modai-ide DiagramModel 的关系

IDE 现有 `DiagramModel`（components/connections/layout）语义相近，但：

- Studio **不复用** IDE 的 TypeScript/Rust 类型定义文件
- 字段可对照，便于未来互通
- Studio 以 node/edge（XYFlow 友好）建模；IDE 以 component/connection 建模

若未来互通，增加导入适配器：`DiagramModel` → `DiagramDocument`（单向）。Studio 不复用 IDE 源文件。

当前实现：`modai-studio/src/import/ideDiagram.ts`。File → Import IDE diagram 读取 IDE 导出的 JSON（`components`/`connections`，或带 `graphical` 的 `GraphicalDocumentModel`）。按 `typeName` 匹配 Studio catalog 填 `domain`/`catalogId`/`ports`；连线 `from`/`to` 按首个 `.` 拆成实例与端口。未匹配类型仍导入为 `imported.*` 节点。已是 Studio `DiagramDocument` 的 JSON 原样加载。不写回 IDE，也不做双向同步。

## 11. 标识符与合法性

| 名称 | 规则 |
|------|------|
| `modelName` | Modelica 标识符 |
| `instanceName` | Modelica 标识符；项目内唯一 |
| `node.id` / `edge.id` | 任意稳定字符串；可选 UUID |
| 参数 `value` | 合法 Modelica 表达式文本（MVP 以字面量为主） |

非法标识符在 Save/Simulate 前拦截。

## 12. 没有 JSON 状态图

`DiagramDocument` 只有框图 `nodes`/`edges`（component / subsystem / draw / fmu）。**不**增加 `states`/`transitions`/`junctions` schema，也不另存 `statechart.json`。

顺序逻辑若进入 Studio，`InitialStep`/`Step`/`Transition` 就是普通 catalog 节点，连线仍是 `connect`。这不是 Stateflow Chart，也不是 MLS 第 17 章语法的视图。
