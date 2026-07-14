# JIT 性能分析 — Findings

**日期**: 2026-07-13

## 瓶颈分布（EngineV6 为代表）

| 阶段 | 冷缓存 | 热缓存 | 是否主瓶颈 |
|------|--------|--------|------------|
| `qualify_short_type_names` + loader | 曾 141s → **~5s**（已修） | 低 | 冷启动仍敏感 |
| `flatten_wall` / `inline_wall` | 中 | **~13–15s/例** MultiBody | **validate 热路径主瓶颈** |
| Cranelift `jit_ms` | ~15ms | ~15ms | 否 |
| 仿真 Newton + 积分 | 视 t_end | 大模型主导 | 长仿真主瓶颈 |

## 已落地优化

- `loader.rs`: `scanned_file_items` + `absent_name_epoch`（CHANGELOG a6971b3）
- `compile/entry.rs`: `jit_module_keepalive` 修复热缓存 UAF
- flatten: `qualify_short_type_names` 作用域修复、`NonSI` 别名（5a2b307）
- codegen cache: COFF REL32、raw blob 拒载、object_emit 外部符号重映射

## 未吃满的 Leyden 能力

- Tier 0–3: `profile_available=false`，PGO 浅
- ~~`worker_per_scenario`: **仅 flag + 日志**~~ → **已实现 `--validate-stdio`**（2026-07-13 P2）
- Salsa: 进程内 DB 在，codegen 失效联动弱（`legacy_salsa0` ~52–63s）
- `block_compile`: 默认关闭
- `CODEGEN_CACHE_MAX_EQUATIONS`: 大模型跳过落盘
- macOS AOT: unsupported

## validate-perf 历史对照（16 MultiBody, quick）

| 场景 | legacy_default | worker_shared_forcefull |
|------|----------------|-------------------------|
| hot_nsA | 14814.9 ms | **13286.7 ms** (~-10%) |
| legacy_salsa0 | 59957.8 ms | **51859.8 ms** (~-14%) |

注: worker 收益来自当时 PoC 路径；当前源码 worker 实现待 P2 补齐。

## 仿真限制

- RK45 自适应: 仅 `when_count==0`
- Newton: `MAX_SOLVABLE_RESIDUALS=2048`；`auto` 默认（密度<=0.35 且 n>=8 走 CSR）
- CVODE/IDA: 需 sundials feature；与 Newton 撕裂共存不完整

## P5 数值内核结论（2026-07-14）

| 项 | 结论 |
|----|------|
| `NEWTON_SPARSE_POLICY=auto` | 生产默认保留；阈值标定 density<=0.35、min_n=8 |
| SolvableBlock16/32/64 | auto → sparse（nnz 46/94/190）；n=3/4 保持 dense |
| codegen cache | variant 附加 `nwa/nwd/nws`，策略切换不再串缓存 |
| tierup | 按方程规模 50/100/200；`RUSTMODLICA_TIERUP_STEP_THRESHOLD` 可覆盖 |
| SIMD_STEP | RK4 默认开启 |
| MultiBody EngineV6 本机树 / SALSA | **已修**（2026-07-14）：salsa `decl_expand` 改用 `DeclAndSubEq`，嵌套方程传入 `eq_expand`；SALSA=1 与 legacy 同为 state=65/eqs=332 |

## EngineV6 完整 MSL 长仿真墙钟（2026-07-14）

**库根**: `third_party/om_msl_4_1_0/OpenModelica-ModelicaStandardLibrary-7a4bf7de77a3986e8eb1e88cbb515d646f78f834`  
**脚本**: `build/run_p5_engine_long_ab.ps1` → `build/jit_perf_p5_enginev6/enginev6_long_ab_summary.json`  
**前置**: 完整 OM MSL；隔离 SHM/std/user 缓存。SALSA=1 嵌套方程塌缩已修（见上），长仿真可用默认 salsa。

| 相位 | policy | t_end | wall_ms | flatten | sim_ms | sim_us | state |
|------|--------|-------|---------|---------|--------|--------|-------|
| cold | auto | 1 | 4578 | 761 | 5 | 5386 | 65 |
| cold | auto | 5 | 3105 | 914 | 32 | 32221 | 65 |
| cold | auto | 10 | 3163 | 883 | 52 | 52163 | 65 |
| cold | auto | 30 | 3026 | 892 | 120 | 120693 | 65 |
| cold | dense | 30 | 3385 | 934 | 166 | 166306 | 65 |
| hot | auto | 30 | 2218 | 33 | 135 | 135935 | 65 |
| hot | dense | 30 | 2223 | 24 | 137 | 137858 | 65 |

结论:
- 规模正确: **state=65 alg=93 diff=65**（非树内 SALSA 塌缩态）
- 冷墙钟 ≈ 编译（flatten+inline ~1s）；`sim` 随 t_end 近似线性（~4ms/s @ rk4 dt=0.002）
- hot t=30 仍以 inline/JIT 为主（~2.2s），仿真仅 ~136ms
- `newton_sparse_blocks=0`：EngineV6 走撕裂环，**auto vs dense 无 Newton CSR 差异**；sim 差在噪声内
- `--perf-json` 现同时写 `sim_ms`/`sim_us`（不再依赖 `PERF_TRACE`）

## 关键文件

- `jit-compiler/src/loader.rs`
- `jit-compiler/src/flatten/inheritance.rs`, `utils.rs`
- `jit-compiler/src/jit/tiered.rs`, `codegen_cache/`
- `jit-compiler/src/compiler/compile_model/compile/entry.rs`
- `crates/regress-harness/src/jit_validate/runner_tail.rs`
- `modai-ide/src-tauri/src/commands/` (jit validate IPC)
