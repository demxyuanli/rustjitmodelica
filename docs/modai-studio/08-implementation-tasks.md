# Modai Studio 实现任务板

日期：2026-07-21  
阶段：**P0 MVP**（F01–F16 / R1–R6）  
机器可读状态：`build/modai_studio/tasks.json`  
推进脚本：`pwsh -File ./build/modai_studio/run_next.ps1`

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
P3 Future（`build/modai_studio/tasks_p3.json` P301–P305）已全部 `done`：Emit C/FMU、XY、Dashboard 预跑旋钮、Variant 切换；Pause/3D 仍为 Out/Blocked。  
F91 FMU 导入共仿：`studio_import_fmu` + 画布 `fmu` 节点 + FMI2 CS 仿真桥接已落地。

## 闭环规则

1. 每次取 `run_next.ps1` 指向的 `in_progress` 任务实现。
2. 完成后将该任务标 `done`，再取下一 pending。
3. P0 全部 `done` 后停止本阶段；P1 另开任务板。
4. 不自动生成测试程序；以构建与示例 Run 为门禁。
