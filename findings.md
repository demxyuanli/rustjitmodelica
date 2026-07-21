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

## JIT 覆盖/性能审查 + strict placeholder 抽样（2026-07-19）

审查（code-simplifier）：主瓶颈仍 flatten/inline；最大语义风险是 policy placeholder；P2–P5 收益仍成立。

Strict 扫描（`RUSTMODLICA_JIT_STRICT_PLACEHOLDERS=1` + `JIT_VAR_STRICT` + `POLICY_STRICT=variable,pre,dot,function_builtin,algorithm`，OM MSL，`--validate`）：

| 子集 | n | pass | fail | Top missing builtins |
|------|---|------|------|----------------------|
| clocked（40） | 40 | 0 | 40 | `sample`(17), `Clock`(14), `shiftSample`(5), `superSample`(2), `not`(2) |
| fluid（13） | 13 | 7 | 6 | `fill`(3), `not`(2), `initial`(1) |
| multibody（30） | 30 | 2 | 28 | `not`(16), `Frames.planarRotation`(6), `Frames.nullRotation`(4), `Frames.absoluteRotation`(1), `Internal.getNextTimeEvent`(1) |

产物：`build/strict_placeholder_scan_{prio,fluid,mb}/`；脚本 `build/scan_strict_placeholders.ps1`。

优先消减：`not`（跨域高频，疑似算子误走 Call）、时钟 `Clock/sample/shiftSample`、Frames.*、Fluid `fill`。

**已跟进修复（同日）**：`call.rs` 将 `strict function-builtin` 硬失败移到 unknown-import 回退前（不再误杀手写 `fill`/`assert`/…）；并补 `not(e)`。复测：`Tanks`/`Sample1` strict 下 success；`HeatingSystem`→`cat`，`Pendulum`→`zeros` 仍待消减。

## Strict builtin 下一刀（2026-07-20）

根因：`default_function_builtin_rules.json` 缺逗号 → 规则表为空；且 `POLICY_STRICT=function_builtin` 曾直接关掉 JSON 匹配。

| 模型 | strict success | 备注 |
|------|----------------|------|
| Pendulum / DoublePendulum / ForceAndTorque | yes | zeros→Frames→cross→from_deg→gravity/normalize |
| TanksWithOverflow | yes | Fluid |
| Sample1 / AbsoluteClocks / ShiftSample / SuperSample | yes | Clock/sample 族 |
| HeatingSystem | yes | if/when `var_map` 分支快照恢复，消除 then→else SSA 泄漏 |

根因（SSA）：then 经 `var_map.insert` 挂上 `f64const`，else 再 `stack_store` 同一 `Value` → Cranelift `non-dominating`（与 fluid_reg/Newton 无关）。已修于 `control_flow.rs`。

### Strict 子集复扫（2026-07-20 收口）

| 子集 | n | pass | fail | 对比此前 |
|------|---|------|------|----------|
| clocked | 87 | **87** | 0 | 曾 0/40（sample/Clock） |
| fluid | 13 | **13** | 0 | 曾 7/13 |
| multibody | 63 | **63** | 0 | 曾 2/30；末例 HeatLosses=`from_degC` 已补 |

产物：`build/strict_placeholder_scan_{prio3,fluid3,mb3}/`。

### Strict electrical / magnetic 扩大扫描（2026-07-20）

| 轮次 | electrical | magnetic | 主要消减 |
|------|------------|----------|----------|
| 初扫 | 130/213 | 19/79 | Complex / convertAlpha / Thermal / everett |
| *2 | 148/213 | 30/79 | brushVoltageDrop / conj / transpose / wrapAngle |
| *3 | 204/213 | (中断) | ComplexMath.* / external JSON 优先 / substring 免 link |
| *5 | **212/213** | **79/79** | `Functions.state*`、`from_rpm`；仅余 1× model_not_found |

关键修复：
- `try_compile_builtin_call`：JSON rules 优先于 `external_modelica_names` 短路
- `entry.rs`：有 builtin rule 的 external 不再要求 shared library
- ComplexMath 族 / conj / fromPolar / transpose / wrapAngle / brushVoltageDrop / quasiRMS / `Functions.state*` / `from_rpm`

产物：`build/strict_placeholder_scan_{electrical,magnetic}5/`。

下一步最大项：非 validate 仿真对标（`compare_omc.ps1` / 选一批 electrical+magnetic Examples 跑 sim CSV）。

### EM sim smoke（2026-07-20）

- curated 7 例 short sim：**7/7**（`build/em_sim_smoke/`）
- 扩到 30 Examples：**30/30**（`build/em_sim_smoke30/summary.json`，rk4 t_end=0.1）
- 扩到 60 Examples：**59/60**（唯一失败 `MeasureRiseTime`；已在 `regress_msl_broad.json` 标 `missing_in_om_msl_4_1_0`）
- 全量 electrical+magnetic Examples（排除 missing）：**286/291** → `build/em_sim_all_examples/summary.json`
  - 5 fail 曾为理想开关 Newton；现 short-sim：**5/5 已通**（含 ChopperBuckBoost DutyCycle / ExampleTemplates rk4+implicit）
  - 已修 `regress_msl_broad.json` 中误合并的 MeasureRiseTime/Multivibrator 条目
- 本机无 `omc`：rk4 vs rk45 last-row 自洽门禁 → `build/em_sim_cross_solver/summary.json`（7/7 maxDiff=0）

### Ideal-switch Newton warm-start（2026-07-20）

- 事件微步：回退 `save_outputs` + clamp 互补变量 `s→0`（`prepare_event_step_algebraic_guess`）
- JIT：tearing/dense Newton 对理想开关 `s` 固定从 0 起算；失败路径不再把 |x|~1e8 write_back 到 outputs
- solver-step：homotopy 重试 + **隐式微步优先** + dt 二分定位 + sticky `dt_active` + 有界 discontinuity nudge；事件迭代 force-s0 / fail-open；assert storm reset
- Chopper PWM：`commit_solver_algebraic_guess`（scratch→outputs）、nudge 按时间推进重置、event-step `dt` 下限
- **Fail-open 收紧**：默认关闭；仅 `RUSTMODLICA_SWITCH_FAIL_OPEN=1` 可在 `|r|>tol` 时降级继续并打 `[degraded:switch-fail-open]`
- short-sim：DCPM / HBridge / Chopper ExampleTemplates / **DutyCycle** 均在严格路径绿（`--solver=implicit`，无 fail-open）
- **BLT**：禁止对 `time` / `*_start` / PWM clock start 做 Newton tearing；跳过仅含冻结量的 residual-only 块（根因修复 DutyCycle `|r|~1`）
- **分拆**：`simulation/driver.rs` + `switch_drive.rs`；`simulation.rs` 仅作模块门面
- **OMC CSV**：本机无 `omc`；`compare_omc.ps1` 已支持 `-LibPath`/`-OutDir`/`-CrossSolverSelfGate`（rk4 vs rk45 末行）。有 OMC CSV 时放到 OutDir 的 `*.omc.csv` 或传 `-OmcOut`

下一步最大项：OMC CSV 对标，或其它门禁回归。

说明文档：[`docs/Ideal_Switch_Simulation.md`](docs/Ideal_Switch_Simulation.md)。

## Mechanics / Media 域收敛（2026-07-21）

Strict validate（同 Discrete/Tables 环境）：

| 子集 | resolvable | pass | 备注 |
|------|------------|------|------|
| Rotational Examples + Test | 30 | **30** | 含 EddyCurrentBrake / LossyGear / GenerationOfFMUs |
| Translational Examples + Test | 25 | **25** | 含 Friction / EddyCurrentBrake |
| Media（正确 FQN） | 15 | **15** | `TestsWithFluid` / `TestAllProperties` / `TestOnly` |
| short-sim 抽样 6 | 6 | **6**，`fallback_total=0` | First / EddyCurrent / Friction / Oscillator / SimpleAir×2 |

新增 builtin：
- `is_equal_fn`（`Math.isEqual` / `Matrices.isEqual`）
- `move_position`（`Sources.Move.position`）
- `thermal_convert` 扩 `linearTemperatureDependency`；`convertAlpha` 按 MSL `(alpha1,T2,T1=293.15)`
- `stribeck_table_f0`（`GenerateStribeckFrictionTable` 标量 f(v=0)）

产物：`build/strict_scan_mech_media/round2/`。

## Media.Examples loader + Noise/ComplexBlocks（2026-07-21）

根因：`Media/package.mo` 在 `redeclare replaceable record/partial model extends/...` 处 PARSE 失败，inner `Examples` 未注册。  
修复：`modelica.pest` + `decl_parse` 支持 `redeclare` 前缀的 class / short-class / extends（含 replaceable/partial、record extends）。

| 集合 | 结果 | 备注 |
|------|------|------|
| Media.Examples（含 Utilities） | **20/20** | SimpleLiquidWater…ShortPipe；`--lib-path`=OM MSL 或 `jit-compiler` |
| Clocked/Blocks Noise | **7/7** | UniformNoise / Xorshift / SampleWithADeffects + 4 Noise blocks |
| ComplexBlocks + ComplexMath | **4/4** | TestConversionBlock / ShowTransferFunction / TestComplex* |
| short-sim 抽样 5 | **5/5**，`fallback_total=0` | Media / UniformNoise / Complex / Noise block |

产物：`build/strict_scan_media_noise_complex/`。

## Fluid Examples strict（2026-07-21 续）

- 裸名 `specificEnthalpy_pTX` 等未命中 `Medium.`/`Media.` 规则 → 补 `medium_props` equals 规则
- Leaf Examples：**19/19** strict（HeatingSystem / PumpingSystem / Tanks.* / DrumBoiler / HeatExchangerSimulation / BatchPlant_StandardWater 等）
- 产物：`build/strict_scan_fluid_examples/summary_correct_fqn.json`
- MultiBody Examples：**63/63** strict → `build/strict_scan_multibody_examples/`
- Electrical `MeasureRiseTime`：OM MSL 4.1.0 无该文件；bundled `jit-compiler/Modelica` 有 → 计为库版本差

## Machines / PowerConverters / QuasiStatic（2026-07-21）

| 集合 | 结果 | 产物 |
|------|------|------|
| Machines + PowerConverters + Polyphase Examples | **125/125** | `build/strict_scan_machines_power/` |
| QuasiStatic Machines/Polyphase/SinglePhase + Batteries Examples | **19/19** | `build/strict_scan_quasistatic_batteries/` |
| short-sim 抽样 5（DCPM / Dimmer / PolyphaseRectifier） | **5/5**，`fallback_total=0` | 同目录 `*.sim2.err.txt` |

## Thermal Examples（2026-07-21）

| 集合 | 结果 | 产物 |
|------|------|------|
| HeatTransfer + FluidHeatFlow Examples | **20/20** | `build/strict_scan_thermal_stategraph/` |
| short-sim TwoMasses / SimpleCooling | **2/2**，`fallback_total=0` | 同目录 |

注：本机 OM MSL 4.1.0 树无 `Modelica/StateGraph`。

## MSL 全绿 LOOP 收口（2026-07-21）

任务板 `build/msl_full_green/tasks.json`：**T01–T16 全部 done**。  
聚合：`build/msl_full_green/AGGREGATE.md`。

本轮 T14 补丁：
- `positiveMax` / `positiveMin` → `max_without_event`（`max(x,1e-10)`）
- `Fluid.Dissipation` / `dp_volumeFlowRate*` → `fluid_dissipation`
- `cardinality` 缺连接度时 STRICT 下返回 0（warn），避免 TestTankWithTopPorts 卡死

已知排除：`MeasureRiseTime` OM 4.1.0 无源（版本差）。

## Discrete / Tables 域收敛（2026-07-21）

Strict validate（`STRICT_PLACEHOLDERS` + `POLICY_STRICT=variable,pre,dot,function_builtin,algorithm`，OM MSL）：

| 集合 | 结果 | 备注 |
|------|------|------|
| 初抽检 Discrete+Tables 18 | **18/18** | Filter / BooleanNetwork / Discrete / Combi* |
| 扩扫 106（含奇数号 Tables + Noise + Clocked） | **98/98 可解析** | 8× `model_not_found` = MSL 源内 `/* ... */` 注释掉的 Test9/10/13… |
| short-sim Discrete / CombiTimeTable / CombiTable / UniformNoise | **4/4**，`fallback_total=0` | rk4 t_end=0.2 |

Fallback 收紧：
- `ext_combitimetable_warn0` → `ext_combitable_lookup`：真实 `rustmodlica_table_get_value` 路径不再 `inc_jit_builtin`
- pre() 表占位在 `STRICT_PLACEHOLDERS` 下 fail-closed
- `Clock()` 空参不再误计 `[fallback:jit-builtin] empty-args`（先 JSON handler 再 warn）

产物：`build/strict_scan_discrete_tables/`。

## 产品约束（2026-07-21 确认）

| 约束 | 状态 |
|------|------|
| 全 MSL 未绿 | 仍成立；继续按域收敛 |
| 存在 fallback | 仍成立（policy / switch-fail-open 等） |
| cancel / 真暂停 | **cancel+进度已落地**（`SimControl`）；真暂停/单步仍无（P3） |
| emit-c / FMU ≠ Embedded Coder | Coder 能力 Out；不追求 EC 级 |

## SimControl（2026-07-21）

- 引擎：`simulation/control.rs` → driver `poll_step`
- IDE：`cancel_simulation_run` + Stop；进度字段 `simTime`/`tEnd`
- CVODE/IDA：仅入口协作检查，非回调内抢断

## 关键文件

- `jit-compiler/src/loader.rs`
- `jit-compiler/src/flatten/inheritance.rs`, `utils.rs`
- `jit-compiler/src/jit/tiered.rs`, `codegen_cache/`
- `jit-compiler/src/compiler/compile_model/compile/entry.rs`
- `crates/regress-harness/src/jit_validate/runner_tail.rs`
- `modai-ide/src-tauri/src/commands/` (jit validate IPC)
