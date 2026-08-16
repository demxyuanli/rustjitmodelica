# Modai Studio Design Spec (Pointer)

日期：2026-07-21  
状态：完整规格见独立文档集（编码前评审）

本文件为索引指针。正式规格位于：

**[docs/modai-studio/](../../modai-studio/00-index.md)**

| 文档 | 路径 |
|------|------|
| 索引与决策基线 | `docs/modai-studio/00-index.md` |
| 需求说明 | `docs/modai-studio/01-requirements.md` |
| 设计说明 | `docs/modai-studio/02-design.md` |
| 数据结构说明 | `docs/modai-studio/03-data-structures.md` |
| 功能说明 | `docs/modai-studio/04-features.md` |
| 实现论证 | `docs/modai-studio/05-implementation-rationale.md` |
| Simulink 能力对照 | `docs/modai-studio/06-simulink-capability-map.md` |

产品决策摘要：独立 Tauri 应用 `modai-studio`；XYFlow；`*.mo` + `*.diagram.json`；path 依赖 `rustmodlica`；交互学 Simulink / 语义学 Modelica；P0 全局 Results 面板（非强制 Scope 块）；信号流+物理域极小子集；不做 Stateflow / Embedded Coder / 文本双向同步（MVP）。
