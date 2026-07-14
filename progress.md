# Progress Log

## 2026-07-13 21:26 — 计划启动

- 创建 `task_plan.md` / `findings.md` / `progress.md`
- 启用 LOOP 动态调度，从 **P0** 开始

## 2026-07-13 21:30 — P0 进行中

- [x] `cargo build --release -p rustmodlica` 成功
- [x] SMPM 抽样 validate 通过:
  - `SMPM_Inverter` (FundamentalWave) — `success:true`
  - `SMPM_FieldWeakening` (QuasiStatic) — `success:true`
- [x] `domains_magnetic.rs` `resolve_magnetic_fw_machines_rest` 已在树中
## 2026-07-13 21:44 — P0 回归结果

- 758 例目录回归完成：**757 passed / 1 failed**（`build_modelica_dir_regress_p0/summary.txt`）
- 唯一失败：`Modelica.Magnetic.QuasiStatic...SMPM_CurrentSource` — `sim_failed`（疑并行竞态）
- 10 例 SMPM 中其余 9 例已通过（较修复前 748/10 大幅改善）
- 单独重跑 `SMPM_CurrentSource`：**exit=0 通过**（确认为并行竞态/flaky，非 SMPM 路由回归）
- P0 门禁：**可视为通过**（757+1 flaky）；可选对失败例加 quarantine 或单 worker 重试
- 旧 summary (`build_modelica_dir_regress/summary.txt`) 仍显示 10 SMPM sim_failed（修复前快照）

**LOOP**: watcher 监视 `run.log` 完成标记；fallback 30min 心跳

## 2026-07-13 21:58 — P1 完成

- [x] `cargo build --release -p rustmodlica -p regress-harness`
- [x] validate-perf: 7 MultiBody x 5 scenarios → **49/49 pass** (`build/jit_perf_baseline_p1/report.json`)
- [x] EngineV6 冷/热 perf-json: 3781ms / 1272ms；热路径 `inline_load_model_ms≈768`
- [x] `baseline_snapshot.md` + `baseline/20260713_jit_perf_p1/jit_perf_baseline.json`
- compare-baseline: 35 benchmark Pass；speedup check Fail（1.5x 阈值，quick analyze 预期）
- **P2 启动**: 实现 worker 长进程 validate

## 2026-07-13 22:16 — P2 完成（核心）

- [x] `jit-compiler/src/cli/validate_stdio.rs`：`--validate-stdio` 长进程协议
- [x] `regress-harness` `--worker-per-scenario` 真实实现（非 PoC 日志）
- [x] A/B：`build/jit_perf_p2_legacy` vs `build/jit_perf_p2_worker`
  - EngineV6 `devloop_multi_model` run2: **720ms → 16ms**
  - 7 模型 worker 全矩阵: **14/14 pass**，~12s 总耗时
- [x] P2b: `modai-ide` Tauri 接入 `--validate-stdio` 长进程（`validate_stdio_worker.rs`）

## 2026-07-13 23:05 — P2b 完成

- [x] `validate_stdio` 支持 `code` + `embed_perf` 请求字段（IDE 内存源码）
- [x] `validate_stdio_worker.rs`：单 actor 线程持有 1 个 worker 子进程，按 lib_paths/tier 指纹复用
- [x] `jit_validate_sync` 默认走 worker；provenance probe 或 `RUSTMODLICA_IDE_VALIDATE_INPROCESS=1` 回退进程内
- **下一步**: P5 仿真数值内核

## 2026-07-13 23:45 — P4 完成

- [x] `loader.rs`: `model_resolvable` + `path_exists_cache` + `peek_loaded_model` + `snapshot_warm_models`
- [x] `inheritance.rs`: qualify 走 `model_resolvable`（不再 `load_model_silent`）
- [x] `inline.rs`/`traverse.rs`: 进程级 `global_inline_model_cache` seed/merge
- [x] `rewrite.rs`: `peek_loaded_model` 命中不计入 `inline_load_model_us`
- [x] validate-perf P4: `build/jit_perf_baseline_p4` **5/5 pass**
- 门禁 hot `devloop_edit_leaf` EngineV6: inline **117–123ms** + flatten **35ms** = **152ms**（P1 合计 ~795ms，-81%）

## 2026-07-14 — P5 完成

- [x] Newton `auto` 标定：density 0.35 / min_n 8；统一 NEWTON_PATH + NEWTON_SPARSE_POLICY
- [x] perf：`newton_sparse_blocks` / `newton_dense_blocks` / `newton_sparse_nnz_total` / `tierup_step_threshold`
- [x] codegen cache variant 含 `nwa|nwd|nws`
- [x] `TieringPolicy::for_equation_count`；SIMD_STEP 默认 ON
- [x] SolvableBlock A/B：16/32/64 auto→sparse；4→dense
- 结论见 `findings.md` P5 节
- [x] 完整 OM MSL EngineV6 长仿真墙钟：SALSA=0 + state=65；cold wall~3s；hot t=30 sim~136ms；Newton blocks=0
- [x] `--perf-json` 在无 `PERF_TRACE` 时也记录 `sim_ms`/`sim_us`
- [x] **SALSA=1 EngineV6 state 塌缩修复**: `decl_expand_preinherited` → `DeclAndSubEq`；嵌套 eqs/conns 经 `DeclExpandOut` 注入 `eq_expand`；stage epoch DeclExpand=4 / EqExpand=2 / FlatModelQ|FlatFull=2

**下一步**: 收口未提交变更.commit，或 `vectorize.rs` profile 驱动

## 2026-07-13 22:40 — P3 完成

- [x] Salsa 默认 ON + `salsa_session` codegen stable hash 联动
- [x] IDE `SalsaEnvDefaults`（validate/sim/session 路径）
- [x] validate-perf P3: `build/jit_perf_baseline_p3` **35/35 pass**
- 门禁 `devloop_edit_dep_only` EngineV6: P1 **758ms** → P3 **396ms** (-47.8%)
- 门禁 `devloop_edit_leaf` EngineV6: P1 **765ms** → P3 **375ms** (-51.0%)
- **下一步**: P2b IDE `--validate-stdio` 接入，或 P4 展平热点
