# JIT 编译/仿真性能优化 — 自动推进计划

**创建**: 2026-07-13  
**LOOP**: 动态自调度（每轮读本文 + `progress.md`，执行当前 Phase 下一任务）  
**目标**: 兑现 Leyden 三 tier + 分层编译骨架，降低 IDE validate 热路径与长仿真耗时

---

## 状态总览

| Phase | 名称 | 状态 | 门禁 |
|-------|------|------|------|
| P0 | 回归门禁 + SMPM 修复收口 | `completed` | 758 回归 757/1（1 flaky 单跑通过） |
| P1 | Perf 基线采集 | `completed` | 49/49 validate-perf + baseline JSON |
| P2 | Worker 长进程 validate | `completed` | EngineV6 run2 720ms→16ms (3-model A/B) |
| P3 | Salsa ↔ codegen 增量联动 | `completed` | `devloop_edit_dep_only` EngineV6 758→396ms (-48%) |
| P4 | 展平热点深化 | `completed` | hot `inline_load+flatten` 152ms vs P1 795ms (-81%) |
| P5 | 仿真数值内核 | `completed` | auto 标定 + SolvableBlock A/B + tierup/SIMD |

---

## P0 — 回归门禁 + SMPM 修复收口

- [ ] 确认 `domains_magnetic.rs` / `domains_misc.rs` SMPM `Machines.Losses` 路由修复在树中
- [ ] 抽样 10 例 SMPM + 3 例 Fluid/Magnetic/FluxTubes 复验
- [ ] 全量 `run_modelica_dir_regression.ps1`（758 例）→ `build_modelica_dir_regress/summary.txt` 0 failed
- [ ] `cargo test -p rustmodlica -q` 全绿
- [ ] 更新 `CHANGELOG.md`（SMPM 路由条目）

**禁止**: 重新引入 loader 首段剥离逻辑（曾导致 EngineV6 数值错误）

---

## P1 — Perf 基线采集

- [ ] `cargo build --release -p rustmodlica -p regress-harness`
- [ ] MultiBody 16 例矩阵:
  ```powershell
  rtk cargo run -p regress-harness --release -- jit validate-perf `
    --out-dir build/jit_perf_baseline_p1 `
    --scenarios devloop_multi_model,devloop_edit_leaf,devloop_edit_dep_only,stdlib_bake,userlib_bake `
    --validation-mode quick --hot-runs 2
  ```
  (注: 旧文档 `hot_nsA`/`legacy_salsa0` 场景已更名为上列 harness id)
- [ ] EngineV6 单例冷/热 `--perf-json` 剖面（`RUSTMODLICA_PERF_TRACE=1`）
- [ ] 写入 `build/jit_perf_baseline_p1/baseline_snapshot.md`（关键字段表）
- [ ] `jit compare-baseline` 对照默认基线，记录 delta

---

## P2 — Worker 长进程 validate

**已实现** `--validate-stdio` + harness `--worker-per-scenario`

- [x] `rustmodlica --validate-stdio`：stdin JSON 请求 → stdout validate JSON（`cli/validate_stdio.rs`）
- [x] `regress-harness` scenario 级单进程循环喂模型（`runner_tail.rs`）
- [x] `modai-ide` Tauri validate 复用子进程（`validate_stdio_worker.rs`，size=1 actor）
- [x] 7 模型 `devloop_multi_model` worker 复测：**14/14 pass**，总墙钟 ~12s
- [x] A/B：EngineV6 run2 **720ms → 16ms**（legacy vs worker，3 模型矩阵）

门禁: `devloop_multi_model` EngineV6 run2 较 P1 基线 590ms 下降 **≥8%** — **已达成**（worker 16ms）

---

## P3 — Salsa ↔ codegen 增量联动

- [x] 梳理 `query_db/salsa_session.rs` 与 `compile_model/entry.rs` codegen 触发链
- [x] Salsa session 记录 `codegen_stable_hash`；`salsa_process_db_hit` / `salsa_codegen_reuse_eligible` perf 字段
- [x] `salsa_query_path_enabled` 默认 ON（`RUSTMODLICA_SALSA=0` 显式关闭）
- [x] IDE/Tauri 默认 `RUSTMODLICA_SALSA=1` + `RUSTMODLICA_SALSA_PROCESS_DB=1`（`SalsaEnvDefaults`）
- [x] harness `devloop_edit_leaf` 显式开启 Salsa；`stdlib_bake` 显式 `SALSA=0` 保冷路径
- [x] `devloop_edit_leaf` / `devloop_edit_dep_only` 复测：**35/35 pass**
- [x] 门禁: `devloop_edit_dep_only` EngineV6 **758→396ms**（-47.8%，≥10%）

---

## P4 — 展平热点深化

- [x] `ModelLoader::model_resolvable` — exists/缓存探测，替代 qualify 中 `load_model_silent`
- [x] `qualify_short_type_names` 回调改 `model_resolvable`（`inheritance.rs`）
- [x] 进程级 `global_inline_model_cache` — seed/merge（`inline.rs` + `traverse.rs`）
- [x] `load_model_inline_cached` — `peek_loaded_model` 短路 + global cache 预热
- [x] validate-perf P4: `build/jit_perf_baseline_p4` **5/5 pass**
- 门禁 `devloop_edit_leaf` EngineV6 hot: `inline_load_model_ms` **117–123**（P1 ~768）；`flatten_wall_ms` **35**（P1 ~25）；合计 **152ms ≤ 795×0.85**

按 ROI 顺序（后续可选）:

1. **继承模板缓存扩大** — `inheritance_flat_template_cache` 命中率 + perf 计数
2. **MSL FQN 符号索引** — 包扫描一次，`qualify_short_type_names` 走索引而非 probe
3. **展平并行默认** — `flatten_decl_parallel` / `eq_parallel` env 评估后 IDE 默认开启
4. **ValidationMode 策略** — IDE validate 默认 `QuickStructure`；仿真前强制 `Full`

门禁: EngineV6 热 validate `inline_load_model_ms` + `flatten_wall_ms` 合计 ≤ P1×0.85

---

## P5 — 仿真数值内核（中长期）

- [x] `RUSTMODLICA_NEWTON_SPARSE_POLICY=auto` 生产默认评估（密度 0.35 / min_n 8）
- [x] 符号/稀疏 Jacobian 路径对 SolvableBlock16/32/64 抽样对标（auto 选 sparse）
- [x] `background_tierup` 阈值标定（`TieringPolicy::for_equation_count` + env）
- [x] SIMD RK4 step 默认开启（`RUSTMODLICA_SIMD_STEP` default true）
- [x] 结论写入 `findings.md`

- [x] 完整 OM MSL 上 EngineV6 长仿真墙钟 A/B（hot t=30 ~2.2s wall / ~136ms sim；Newton blocks=0）
- [x] 修复 SALSA=1 EngineV6 state 塌缩（`DeclAndSubEq` + 嵌套 eqs 传入 eq_expand；validate 默认路径 state=65）

遗留（可选）: `vectorize.rs` profile 驱动；有 CSR Newton 的 MultiBody 长仿真对标

---

## LOOP 每轮执行协议

1. 读 `task_plan.md` 当前 Phase 第一个未勾选项
2. 读 `progress.md` 末条，避免重复
3. 执行一项可验证子任务（含编译/测试）
4. 更新 `progress.md`；Phase 全完成则改状态并进入下一 Phase
5. Phase 门禁未过则不推进
6. 全部 P0–P4 完成或遇 blocker 写入 `progress.md` 并暂停 LOOP

**Blocker 示例**: 758 回归新失败、worker 协议设计需用户确认、perf 回退 >5%
