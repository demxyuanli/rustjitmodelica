# Modai Studio 文档索引

日期：2026-07-21  
状态：P0–P3 已落地；F91 FMU 导入共仿已落地（见 `08-implementation-tasks.md` / `build/modai_studio/tasks*.json`）；Pause/3D 仍 Out/Blocked
  
产品代号：`modai-studio`  
决策基线：方案 1（独立 Tauri 应用 + 薄仿真桥）

## 文档清单

| 序号 | 文档 | 说明 |
|------|------|------|
| 01 | [需求说明](./01-requirements.md) | 目标用户、分期范围、非目标、验收标准 |
| 02 | [设计说明](./02-design.md) | 架构、模块边界、Simulink 式信息架构 |
| 03 | [数据结构说明](./03-data-structures.md) | 项目文件、图模型、预留层级/run 字段、IPC |
| 04 | [功能说明](./04-features.md) | 功能域 A–L、P0–P3 清单、交互流程 |
| 05 | [实现论证](./05-implementation-rationale.md) | 选型理由、风险、门禁、开放问题 |
| 06 | [Simulink 能力对照](./06-simulink-capability-map.md) | 与 Simulink/Simscape 功能映射与分期 |
| 07 | [编译器可行性评估](./07-compiler-feasibility.md) | 对照 jit-compiler：Full/Partial/Blocked |
| 08 | [实现任务板](./08-implementation-tasks.md) | P0 里程碑 S01–S09；状态见 `build/modai_studio/tasks.json` |

## 已确认决策

1. **产品定位**：独立桌面产品，不从 `modai-ide` 拆仓；复用 `jit-compiler`（`rustmodlica`）。
2. **MVP 交互**：框图拖拽为主；不做首版文本双向同步。
3. **技术栈**：Tauri 2 + React + TypeScript；画布 XYFlow。
4. **组件域**：信号流 + 物理域，首版各做极小子集。
5. **落盘**：`model.mo` + `model.diagram.json`；仿真以 `.mo` 为准。
6. **工程形态**：monorepo `modai-studio/`，Cargo workspace，path 依赖 `rustmodlica`。
7. **对标策略**：交互学 Simulink，语义学 Modelica；不对标 Stateflow / Embedded Coder / HIL。
8. **Scope 策略（P0）**：全局 Results 曲线面板；不强制画布 Scope 块。
9. **连续块默认**：P0 用 `FirstOrder` 作为传递函数直觉降级；TransferFunction 视支持升 P1。
10. **编译器完整度**：不承诺完整 Simulink（产品族/语义/运行时/成熟度四层原因见 `07` §1.1）；P0 可行；P1 取消/进度依赖 `run_simulation` 增量。

## 阅读顺序

需求 → Simulink 对照 → **编译器可行性** → 设计 → 数据结构 → 功能 → 实现论证。  
全部评审通过后再进入实现计划与编码。

## 评审门禁

- [ ] 需求说明评审通过
- [ ] Simulink 对照评审通过（分期与 Out）
- [ ] 编译器可行性评估认可（P0 可开工 / 完整对等不做）
- [ ] 设计说明评审通过
- [ ] 数据结构说明评审通过
- [ ] 功能说明评审通过
- [ ] 实现论证评审通过（含 Q1/Q3/Q4/Q5）
- [ ] 允许进入实现计划（writing-plans）与编码
