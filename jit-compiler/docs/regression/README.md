# 回归案例总览 / Regression Case Guide

## 目的 / Purpose

本目录按功能对 `rustmodlica` 回归案例进行分类整理。  
This folder organizes `rustmodlica` regression cases by feature area.

该文档体系仅做说明与追溯，不新增测试程序。  
This documentation is for guidance and traceability only, without adding new test programs.

## 入口脚本 / Entry Scripts

- 主回归脚本 / Main regression script: `d:/source/repos/rustmodlica/run_regression.ps1`（默认走完整优化路径，不覆盖 `RUSTMODLICA_JIT_CODEGEN_CACHE` 与 `RUSTMODLICA_AOT_NATIVE_LOAD`；如需稳定性排障模式可加 `-DisableNativeAccelForStability`，脚本会强制两者为 `0`。CI 请用 `powershell -File ...` 以便进程退出码为 0/1。）
- TestLib 全量 `--validate` 门禁 / TestLib batch validate gate: `d:/source/repos/rustmodlica/jit-compiler/scripts/run_testlib_validate.ps1`（根目录 `.mo` 须通过；`TestLib/negative/*.mo` 须失败）
- 目录回归脚本 / Directory regression script: `d:/source/repos/rustmodlica/run_modelica_dir_regression.ps1`
- OMC 对比脚本 / OMC compare script: `d:/source/repos/rustmodlica/compare_omc.ps1`
- **JSON 回归调度器（Rust）/ JSON-driven harness (Rust)**: `d:/source/repos/rustmodlica/crates/regress-harness` 二进制 `regress-harness`，快速冒烟 `d:/source/repos/rustmodlica/crates/regress-harness/examples/smoke.json`，含参考 CSV 末行对比的示例 `d:/source/repos/rustmodlica/crates/regress-harness/examples/with_omc_compare.json`，JSON Schema `d:/source/repos/rustmodlica/crates/regress-harness/schema/regress_config.v1.json`。薄封装：`d:/source/repos/rustmodlica/run_regress_harness.ps1`。在仓库根目录执行：`cargo build -p regress-harness --release`，`regress-harness run --config ...`）。
  - TUI 入口 / TUI entry: **无子命令**或 `regress-harness interactive` 默认进入全屏 TUI（Actions / Chat / Tasks 三栏 + Command 区） / run without subcommand or use `regress-harness interactive` to open full-screen TUI (Actions / Chat / Tasks + Command bar)
  - 命令面板 / Command palette: `Ctrl+P` 或 `/` 打开；Leader 键默认 `Ctrl+X`（示例：`Ctrl+X` 后按 `q` 退出）
  - 命令面板交互 / Palette interaction: 支持别名与轻微拼写容错；`↑/↓` 选择候选、`Enter` 执行、`Esc` 关闭；右侧显示当前候选详情（Action/Aliases/Description）
  - 输入稳定性 / Input stability: TUI 输入仅处理按键按下阶段，避免字符重复输入
  - 配置文件 / Config file: 项目根可选 `tui.json`（`theme`、`keybinds.leader`、`scroll_speed`、`show_tips`、`tick_ms`）
  - Hybrid 变更就地检查清单 / Hybrid local change checklist:
  - 1) `crates/regress-harness/src/i18n.rs` 中 `hybrid_help_*` 键 / `hybrid_help_*` keys in `crates/regress-harness/src/i18n.rs`
  - 2) 本节 Hybrid 快捷键清单 / Hybrid hotkey list in this section
  - 3) README 中截图或示意图对应文字 / screenshot or mockup captions in README
  - 4) README 命令示例与参数说明 / command examples and parameter notes in README
  - 快捷键清单 / Hotkey list（固定顺序：与 `hybrid_help_*` 一致）:
  - `[1/r]` 运行 (run)、`[2/p]` 计划 (plan)、`[3/l]` 列表 (list)、`[4/s]` 状态 (status)
  - `[5/mt]` 监控最近 (monitor tail)、`[6/mf]` 监控追踪 (monitor follow)、`[7/a]` Agent 上下文 (agent context)、`[8/v]` 校验 (validate)
  - `[9/f]` 仅重跑最近失败 (rerun recent failed，流程中可筛选并选择单条精准重跑)、`[d]` 目录扫描案例 (scan cases)、`[c]` AI Agent Chat、`[z]` 切换语言 (switch language)、`[e]` 编辑上下文 (edit context)
  - `[j/k|tab]` 切焦点 (focus)、`[enter]` 执行焦点 (run focused)
  - `[t1/t2/t3]` 失败TOP联动 (failed TOP link)、`[n/p]` 切失败详情 (switch failed detail)、`[x]` 展开摘要 (expand summary)、`[q]` 退出 (quit)
  - 输入补全 / Input autocomplete: `Tab` 可用于命令补全与路径补全（Hybrid 命令输入、`scan cases` 目录输入、`edit context` 的 `config`/`data_root`；legacy interactive 的路径输入项同样支持）
  - Agent Chat 首轮上下文可选自动注入当前回归失败摘要（报告摘要 + 最近失败用例），用于开场即围绕当前失败状态分析
  - Agent Chat 会话内命令：`/ctx on|off` 动态切换失败摘要注入，`/reset` 清空对话历史并重新应用当前上下文
  - Agent Chat 完整命令：`/help`、`/history`、`/clear`、`/pin`、`/status`、`/ctx on|off`、`/reset`、`/model [deepseek-reasoner|deepseek-chat]`、`/quit`
  - Key 管理命令：`/key show`（掩码显示来源）、`/key set`（CLI 输入并可保存）、`/key clear`（清除本地与当前会话）
  - 若未设置 `DEEPSEEK_API_KEY`，进入 Agent Chat 时会在 CLI 内提示输入 key，并可选择保存到本机（`%USERPROFILE%/.regress-harness/credentials.json`）供后续自动读取
  - 行为说明 / Behavior notes:
  - 失败选择器展示失败原因摘要（优先 `classification`，其次 `exit_code`），支持关键字过滤（`case_id/reason`）与“仅显示最近 N 条失败” / failed selector shows summarized reason (prefer `classification`, fallback `exit_code`), supports keyword filter (`case_id/reason`) and recent-N filter
  - 监控面板支持失败详情下钻，`x` 开启后显示更长 `stderr/stdout` 摘要（summary expand） / monitor panel supports failed-detail drilldown, and `x` expands stderr/stdout summary
  - 界面包含树状输出、分割线、制表线、颜色图例，并对窄终端做宽度自适应与长文本截断 / UI provides tree/divider/table/color legend with adaptive width and ellipsis truncation in narrow terminals
  - 上下文编辑中 `tier` 与 `incremental` 使用分步选择器而非自由文本输入 / context editing uses step selectors for `tier` and `incremental`
  - 如需回退旧问答式菜单，可设置环境变量 `RUSTMODLICA_INTERACTIVE_LEGACY=1`
  - 脚本与 CI 仍可使用 `run`、`plan` 等子命令加参数
  - `defaults.repo_root` 相对路径相对于**进程当前目录**解析
  - **集中数据目录**：`--data-root`（默认 `build/regression_data`）下写入 `report.json`、`regress_manifest.json`（上次运行的 case 顺序与过滤条件）、`events.ndjson`（运行期事件流）、`cases.ndjson`、`summary_compat.txt`，CSV 等产物在 `artifacts/`。可选 `defaults.regression_data_root` 或旧参数 `--out-dir`（等同覆盖数据根）
  - **运行参数快照**：每次 `run` 生成 `runs/<run_id>/run_options.json`，用于参数优先级追溯与复现。
  - **减量回归**：`--incremental last_structure` 仅执行与上次 `regress_manifest.json` 中 `case_ids` 交集的用例；`last_structure_rerun_failed` 在该交集内再仅重跑基线 `report.json` 中非 pass 项（基线默认取 `--data-root/report.json`）。另支持 `rerun_failed` / `skip_unchanged`。现有 PS1 主回归仍保留；新工具用于逐步迁移与统一报告
  - 多语言入口：支持 `--lang en|zh-CN`，并支持环境变量 `RUSTMODLICA_LANG`（CLI 参数优先级更高）。
- **从 PS1 扫描生成的初始配置 / PS1-scanned seed configs**: 生成器 `d:/source/repos/rustmodlica/crates/regress-harness/scripts/Export-RegressConfigFromPs1.ps1` 读取 `run_regression.ps1` 的 `$cases` 与 `$caseExtraArgs`，以及 `jit-compiler/scripts/run_mos_regression.ps1` 中的 MOS 路径，写出 `d:/source/repos/rustmodlica/crates/regress-harness/examples/testlib_from_run_regression.json`（129 条 `kind: model`）与 `d:/source/repos/rustmodlica/crates/regress-harness/examples/mos_from_run_mos_regression.json`（14 条 `kind: mos`）。在仓库根执行：`powershell -NoProfile -ExecutionPolicy Bypass -File crates/regress-harness/scripts/Export-RegressConfigFromPs1.ps1`。`defaults.rustmodlica_exe` 默认 `jit-compiler/target_regression/release/rustmodlica.exe`，与 `run_regression.ps1` 使用的隔离 `target_regression` 产物一致；运行前需在 `jit-compiler` 下用相同 `target-dir` 构建出该可执行文件（与主 PS1 回归前置条件一致）。
- **phase1 手工补齐映射 / phase1 manual mapping extension**: `d:/source/repos/rustmodlica/crates/regress-harness/examples/jit_phase1_from_run_regression.json` is an overlay config that composes the PS1-scanned baseline table via `includes` (e.g. `testlib_from_run_regression.json`), then adds the first batch of PS1 special sections: `JIT_RULES/TestLibValidateBatch`, 12x `ScriptMode/*`, `EmitC/RecursiveFunc`, `FUNC-7/EmitC/StringArgExtFunc`, `SYNC-2/ClockedPartitionTest`. It keeps `target_regression` and uses `custom_command` to preserve text assertions and emitted-C artifact assertions.
- **regress-harness 扩展子命令 / Extended CLI**（仓库根、`cargo build -p regress-harness --release` 后使用）：
  - `validate-config --config <path>`：校验 JSON（版本、重复 `id`）。
  - `list-cases --config ... [--tier ...] [--tags a,b] [--format human|json]`：列出筛选后的用例，不落盘。
  - `plan`：与 `run` 相同的 `--config`、`--data-root`、`--incremental`、`--baseline`、`--manifest` 等；只打印计划（run / skipped_unchanged / skipped_scope），支持输出格式 `human`（可读文本）或 `json`（含 `rows` 数组）。
  - `status --data-root ... [--format human|json]`：读取 `{data-root}/report.json` 摘要与失败项。
  - `monitor --data-root ... [--tail N] [--follow] [--source auto|event|ndjson]`：支持“监控最近行”（`--tail`）与“监控追踪”（`--follow`）两种模式。`--source auto` 默认优先读取 `events.ndjson`，失败时自动回退 `cases.ndjson`；`--source event` 强制读取事件流；`--source ndjson` 读取结果流（需此前 `run` 时开启 `--ndjson`）。`--follow` 为轮询追加（Ctrl+C 结束）。
  - `agent-context --data-root ... [--config <path>]`：向 stdout 输出**单块 JSON**（供脚本或 AI agent 消费）；在交互菜单中对应“Agent 上下文（JSON）”。
  - `run ... --progress`：每完成一例向 **stderr** 打印一行进度（`case_id`、status、`duration_ms`）。
  - 交互 UI 由 `inquire` 驱动（替代旧的 `dialoguer`），默认直接执行 `regress-harness` 即可进入交互菜单。
  - `agent repl`：AI Agent 可通过**每行一条 JSON**与 CLI 持续会话：输入 `{"cmd":"..."}`，输出 `{"ok":true|false,...}`。  
    最小可用示例（MVP）：
    - `{"cmd":"set_context","config":"crates/regress-harness/examples/smoke.json","data_root":"build/regression_data"}`
    - `{"cmd":"plan"}`
    - `{"cmd":"run","ndjson":true}`
    - `{"cmd":"status"}`
    - `{"cmd":"quit"}`

    常用扩展示例：
    - `{"cmd":"monitor_tail","tail":50,"max_lines":50}`
    - `{"cmd":"monitor_follow","follow_seconds":5,"tail":20,"max_lines":50}`
    - `{"cmd":"deepseek_chat","prompt":"summarize current failures"}`
    - `{"cmd":"deepseek_chat","prompt":"summarize current failures","normalize":false}`
    - `{"cmd":"deepseek_reasoner","prompt":"find top 3 failure patterns"}`

    参数说明（与当前实现一致）：
    - `set_context` 持久上下文字段：`config`、`data_root`、`tier`、`tags`、`incremental`、`workers`。后续命令可省略；若同字段再次传入则以当前命令值覆盖上下文值。
    - `monitor_tail`：`tail` 默认 `20`；`max_lines` 仅保留输出末尾 N 行（同时作用于 `stdout`/`stderr`）；`max_lines=0` 返回空文本。
    - `monitor_follow`：`follow_seconds` 默认 `5`，最小 `1`；可选 `tail`（仅在传入时生效）；`max_lines` 行截断规则同上；返回中含 `timed_out` 与 `follow_seconds`。
    - `deepseek_chat` / `glm_chat` / `deepseek_reasoner`：都走 DeepSeek API（读取环境变量 `DEEPSEEK_API_KEY`，可选 `api_base`）。
    - `deepseek_reasoner` 默认模型 `deepseek-reasoner`；其他默认 `deepseek-chat`；传入 `model` 可覆盖默认。
    - `normalize` 默认 `true`：返回精简 `provider/model/answer/thinking`；`normalize=false` 额外返回 `api_base` 与完整 `result` 便于调试。
- **`run_regression.ps1` 中尚未映射到 JSON 的段落 / PS1 sections not yet covered by JSON configs**: 已有 phase1 映射覆盖 `JIT_RULES`、`INT-2`、`FUNC-6/7`、`SYNC-2`。当前仍由 `run_regression.ps1`（或其所调子流程）负责，后续可继续迁入：`PERF-SMOKE`（`RUSTMODLICA_PERF_SMOKE` 门控）；`SYNC-DET` 重复运行 CSV 稳定性；`SYNC-TRACE-ASSERT` 时钟分区跟踪断言；`[FMI]` `--emit-fmu` 产物检查；`[DIR]` `run_modelica_dir_regression.ps1`（可用 `-SkipDir` 跳过）；覆盖率门禁刷新 `coverage_status.json` 等。

## JIT 配置模板与参数优先级 / JIT templates and option precedence

### 参数优先级说明 / Option precedence

- 优先级统一按 **CLI > env > default** 执行。
- 当同一个行为既有命令行参数又有环境变量时，以命令行参数为准（例如 `--fmi-model-id` 高于 `RUSTMODLICA_FMI_MODEL_ID`）。
- 对于仅存在 env 开关的行为（如部分 trace/cache 开关），未设置时使用实现内默认值。
- 建议在 CI 和批量回归里显式设置关键参数，避免依赖进程环境残留。

### 最小推荐模板 1：开发迭代 / Minimal template 1: local development

```powershell
$env:RUSTMODLICA_SALSA = "1"
$env:RUSTMODLICA_QUERY_CACHE = "1"
$env:RUSTMODLICA_QUERY_CACHE_NAMESPACE = "dev-local"
$env:RUSTMODLICA_PERF_TRACE = "0"
$env:RUSTMODLICA_STAGE_TRACE = "0"

.\target\release\rustmodlica.exe `
  --validate `
  --validate-tier=analyze `
  --validation-mode=quick `
  --lib-path=.\jit-compiler `
  ModelicaTest.JitStress.ComplexJitRegression
```

### 最小推荐模板 2：CI 门禁 / Minimal template 2: CI gate

```powershell
$env:RUSTMODLICA_SALSA = "1"
$env:RUSTMODLICA_QUERY_CACHE = "1"
$env:RUSTMODLICA_QUERY_CACHE_NAMESPACE = "ci-$env:BUILD_BUILDID"
$env:RUSTMODLICA_FLATTEN_CACHE_DIR = "$pwd\build\ci_cache"
$env:RUSTMODLICA_JIT_CODEGEN_CACHE = "1"
$env:RUSTMODLICA_JIT_CODEGEN_CACHE_DIR = "$pwd\build\ci_codegen_cache"
$env:RUSTMODLICA_PERF_TRACE = "0"
$env:RUSTMODLICA_STAGE_TRACE = "0"

cargo run -p regress-harness --release -- `
  run `
  --config crates/regress-harness/examples/smoke.json `
  --data-root build/regression_data `
  --incremental last_structure_rerun_failed
```

### 最小推荐模板 3：性能排查 / Minimal template 3: performance diagnostics

```powershell
$env:RUSTMODLICA_SALSA = "1"
$env:RUSTMODLICA_QUERY_CACHE = "1"
$env:RUSTMODLICA_QUERY_CACHE_NAMESPACE = "perf-nsA"
$env:RUSTMODLICA_PERF_TRACE = "1"
$env:RUSTMODLICA_STAGE_TRACE = "1"
$env:RUSTMODLICA_CACHE_STATS_JSON = "$pwd\build\jit_validate_perf\cache_stats.json"
$env:RUSTMODLICA_DEP_GRAPH_JSON = "$pwd\build\jit_validate_perf\dep_graph.json"

cargo run -p regress-harness --release -- `
  jit validate-perf `
  --out-dir build/jit_validate_perf `
  --validate-tier=analyze `
  --validation-mode=full `
  --models ModelicaTest.JitStress.ComplexJitRegression `
  --hot-runs 2 `
  --perf-trace `
  --stage-trace
```

### 故障到参数对照表 / Symptom-to-parameter quick map

| 故障现象 / Symptom | 先看参数（Top 3） | 建议动作（最小闭环） |
|---|---|---|
| `flatten` 失败或结果不符合预期 | `--validate-tier=flatten`, `--validation-mode`, `RUSTMODLICA_SALSA` | 先用 `--validate --validate-tier=flatten` 复现；切换 `RUSTMODLICA_SALSA=0/1` 对比路径；必要时改 `validation-mode=full` 排除 quick/superfast 影响 |
| `analyze` 失败或提前停止 | `--validate-tier=analyze`, `--index-reduction-method`, `--tearing-method` | 固定到 `--validate-tier=analyze`；对比 `index-reduction-method`（`none` vs `pantelides`）；记录 `tearing-method` 差异并保留失败样本 |
| 仿真失败或数值不稳定 | `--solver`, `--atol/--rtol`, `--output-interval` | 先切换求解器（`rk45`/`ida`）；收紧或放宽容差；固定输出间隔后再比较结果 |
| 性能明显变慢（编译或仿真） | `RUSTMODLICA_PERF_TRACE`, `RUSTMODLICA_STAGE_TRACE`, `--perf-json` | 开 `PERF_TRACE/STAGE_TRACE`；增加 `--perf-json` 固化证据；对同一模型做 A/B 对比（同环境、同参数） |
| 缓存命中率低或反复 miss | `RUSTMODLICA_QUERY_CACHE`, `RUSTMODLICA_QUERY_CACHE_NAMESPACE`, `RUSTMODLICA_FLATTEN_CACHE_DIR` | 确认缓存未被关闭；为不同任务显式设置 namespace；固定 cache dir 后再跑两轮观察热启动命中 |
| 增量回归选例异常（跑太多或漏跑） | `--incremental`, `--baseline`, `--manifest` | 明确增量策略（如 `last_structure_rerun_failed`）；核对 baseline/manifest 路径是否指向同一 data-root 产物 |
| overdetermined 相关报错 | `--overdet-check`, `--overdet-tol`, `RUSTMODLICA_OVERDET_RESIDUAL_TOL` | 先显式开启/关闭 `overdet-check` 对比；调大 `overdet-tol` 观察边界；避免 CLI 与 env 同时冲突配置 |
| FMI 导出不一致（ID/GUID） | `--fmi-model-id`, `--fmi-guid`, `RUSTMODLICA_FMI_MODEL_ID` | 优先使用 CLI 固定导出；仅在无 CLI 时使用 env；保证同一流水线内 ID/GUID 来源单一 |
| 事件扫描结果波动大 | `RUSTMODLICA_EVENT_COUNT_DEADBAND`, `RUSTMODLICA_TAIL_VELOCITY_DEADBAND`, `RUSTMODLICA_SUNDIALS_EVENT_LOG` | 用 event-scan 扫 deadband 组合；固定日志开关；对同模型重复运行并比较 top-N 组合稳定性 |

### Full-tier regression diff: r6 -> r7 (2026-04-08)

Run command (same for both baselines):

```powershell
cargo run -p regress-harness --release -- `
  jit validate-perf `
  --out-dir build/jit_validate_perf_r7 `
  --validate-tier=full `
  --validation-mode=full `
  --models=ModelicaTest.JitStress.ComplexJitRegression,ModelicaTest.JitStress.MslBroadCoverage,ModelicaTest.JitStress.RobotElectricalControl,TestLib.BigFor,TestLib.MSLBlocksTest,TestLib.MultiTopCombined `
  --hot-runs 2 `
  --perf-trace `
  --stage-trace
```

Top-line verdict:

- `r6`: `30 total / 29 pass / 1 fail`
- `r7`: `30 total / 30 pass / 0 fail`
- Failure was reduced to zero; previously failing case: `legacy_salsa0 + ModelicaTest.JitStress.MslBroadCoverage`

Per-model hot-path (`hot_nsA`, run2) cache-field diff:

| model | artifact_bundle_cache_status (r6->r7) | external_resolve_cache_status (r6->r7) | cache_warm_ratio (r6->r7) | duration_ms (r6->r7) | delta_ms |
|---|---|---|---|---:|---:|
| ModelicaTest.JitStress.ComplexJitRegression | `hit -> hit` | `not_run -> hit` | `1 -> 1` | `1991 -> 2005` | `+14` |
| ModelicaTest.JitStress.MslBroadCoverage | `hit -> hit` | `not_run -> hit` | `1 -> 1` | `2030 -> 1806` | `-224` |
| ModelicaTest.JitStress.RobotElectricalControl | `hit -> hit` | `not_run -> hit` | `1 -> 1` | `1974 -> 1789` | `-185` |
| TestLib.BigFor | `hit -> hit` | `not_run -> hit` | `1 -> 1` | `2650 -> 1789` | `-861` |
| TestLib.MSLBlocksTest | `hit -> hit` | `not_run -> hit` | `1 -> 1` | `2505 -> 1827` | `-678` |
| TestLib.MultiTopCombined | `hit -> hit` | `not_run -> hit` | `1 -> 1` | `2101 -> 1841` | `-260` |

Scenario aggregate diff:

| scenario | failed_cases (r6->r7) | avg_duration_ms (r6->r7) | delta_ms | avg_cache_warm_ratio (r6->r7) | flat_full_cache_hits total (r6->r7) |
|---|---|---:|---:|---:|---:|
| cold_empty_nsCOLD | `0 -> 0` | `6362.33 -> 5534.00` | `-828.33` | `0.0 -> 0.0` | `0 -> 0` |
| cold_qcache0 | `0 -> 0` | `6153.50 -> 5546.83` | `-606.67` | `0.0 -> 0.0` | `0 -> 0` |
| hot_nsA | `0 -> 0` | `4356.42 -> 3602.50` | `-753.92` | `0.5 -> 0.5` | `0 -> 6` |
| legacy_salsa0 | `0 -> 0` | `3394.80 -> 10015.50` | `+6620.70` | `0.0 -> 0.0` | `0 -> 0` |

Interpretation notes:

- `legacy_salsa0` avg time rises because the former failing case now executes complete pipeline and reports success instead of terminating with JIT error.
- `hot_nsA` run2 keeps full `artifact_bundle_cache_status=hit` coverage (`6/6`) and now reports `external_resolve_cache_status=hit` (`6/6`), indicating deeper cache-path observability after fix.
- `flat_full_cache_*` improvement appears in `hot_nsA` (`hits: 0 -> 6`), while cold/legacy scenarios remain miss/write dominated as expected.

### Full-tier regression diff: r7 -> r8 (2026-04-08)

Top-line verdict:

- `r7`: `30 total / 30 pass / 0 fail`
- `r8`: `30 total / 30 pass / 0 fail`
- pass/fail remains stable; comparison focuses on cache-field consistency and timing deltas.

Per-model hot-path (`hot_nsA`, run2) cache-field diff:

| model | artifact_bundle_cache_status (r7->r8) | external_resolve_cache_status (r7->r8) | cache_warm_ratio (r7->r8) | duration_ms (r7->r8) | delta_ms |
|---|---|---|---|---:|---:|
| ModelicaTest.JitStress.ComplexJitRegression | `hit -> hit` | `hit -> hit` | `1 -> 1` | `2005 -> 1736` | `-269` |
| ModelicaTest.JitStress.MslBroadCoverage | `hit -> hit` | `hit -> hit` | `1 -> 1` | `1806 -> 1728` | `-78` |
| ModelicaTest.JitStress.RobotElectricalControl | `hit -> hit` | `hit -> hit` | `1 -> 1` | `1789 -> 1818` | `+29` |
| TestLib.BigFor | `hit -> hit` | `hit -> hit` | `1 -> 1` | `1789 -> 1715` | `-74` |
| TestLib.MSLBlocksTest | `hit -> hit` | `hit -> hit` | `1 -> 1` | `1827 -> 1697` | `-130` |
| TestLib.MultiTopCombined | `hit -> hit` | `hit -> hit` | `1 -> 1` | `1841 -> 1741` | `-100` |

Scenario aggregate diff:

| scenario | failed_cases (r7->r8) | avg_duration_ms (r7->r8) | delta_ms | avg_cache_warm_ratio (r7->r8) | flat_full_cache_hits total (r7->r8) |
|---|---|---:|---:|---:|---:|
| cold_empty_nsCOLD | `0 -> 0` | `5534.00 -> 5876.50` | `+342.50` | `0.0 -> 0.0` | `0 -> 0` |
| cold_qcache0 | `0 -> 0` | `5546.83 -> 6302.17` | `+755.34` | `0.0 -> 0.0` | `0 -> 0` |
| hot_nsA | `0 -> 0` | `3602.50 -> 3910.17` | `+307.67` | `0.5 -> 0.5` | `6 -> 6` |
| legacy_salsa0 | `0 -> 0` | `10015.50 -> 11851.83` | `+1836.33` | `0.0 -> 0.0` | `0 -> 0` |

#### legacy_salsa0 split table A: success-rate repair benefit

| metric | r7 | r8 | delta |
|---|---:|---:|---:|
| total_cases | 6 | 6 | 0 |
| passed_cases | 6 | 6 | 0 |
| failed_cases | 0 | 0 | 0 |
| success_rate | 100% | 100% | 0% |

#### legacy_salsa0 split table B: pure performance (success-only)

| metric | r7 | r8 | delta |
|---|---:|---:|---:|
| avg_duration_ms_success_only | 10015.50 | 11851.83 | +1836.33 |

Interpretation notes:

- The split confirms `r7 -> r8` has no success-rate repair effect in `legacy_salsa0`; observed change is purely timing variance under successful runs.
- Cache-field consistency remains stable across hot run2 (`artifact=hit`, `external_resolve=hit`, `warm_ratio=1` for all six models).

### Full-tier regression diff: r8 -> r9 (3-sample) (2026-04-08)

Sampling setup:

- `r8`: single baseline run (`build/jit_validate_perf_r8`)
- `r9`: three repeated runs (`build/jit_validate_perf_r9_1..3`) with identical command/options

Top-line verdict:

- `r8`: `30 total / 30 pass / 0 fail`
- `r9`: each sample run is `30 total / 30 pass / 0 fail`
- cache consistency on `hot_nsA run2` remains stable across all three r9 runs: `artifact_hit=6/6`, `external_resolve_hit=6/6`, `cache_warm_ratio=1` for all models

Scenario aggregate with median/variance (r9 across 3 runs):

| scenario | r8 avg_duration_ms | r9 mean_duration_ms | r9 median_duration_ms | r9 variance_ms2 | median_delta_vs_r8_ms |
|---|---:|---:|---:|---:|---:|
| cold_empty_nsCOLD | 5876.50 | 5267.39 | 5331.00 | 3647.34 | -545.50 |
| cold_qcache0 | 6302.17 | 5189.06 | 5254.00 | 4119.18 | -1048.17 |
| hot_nsA | 3910.17 | 3558.78 | 3663.17 | 8215.03 | -247.00 |
| legacy_salsa0 | 11851.83 | 9922.22 | 10453.00 | 212183.81 | -1398.83 |

Hot-path per-model (`hot_nsA`, run2) with median/variance (r9 across 3 runs):

| model | r8 duration_ms | r9 mean_ms | r9 median_ms | r9 variance_ms2 | median_delta_vs_r8_ms |
|---|---:|---:|---:|---:|---:|
| ModelicaTest.JitStress.ComplexJitRegression | 1736 | 1724.00 | 1735 | 103.00 | -1 |
| ModelicaTest.JitStress.MslBroadCoverage | 1728 | 1871.67 | 2024 | 23104.33 | +296 |
| ModelicaTest.JitStress.RobotElectricalControl | 1818 | 1838.33 | 2053 | 34757.33 | +235 |
| TestLib.BigFor | 1715 | 1753.33 | 1813 | 2942.33 | +98 |
| TestLib.MSLBlocksTest | 1697 | 1779.33 | 1849 | 3640.33 | +152 |
| TestLib.MultiTopCombined | 1741 | 1867.67 | 1999 | 12940.33 | +258 |

#### legacy_salsa0 split table A: success-rate repair benefit (r8 vs r9-3run)

| metric | r8 | r9 mean | r9 median | r9 variance |
|---|---:|---:|---:|---:|
| success_rate(%) | 100.00 | 100.00 | 100.00 | 0.00 |

#### legacy_salsa0 split table B: pure performance (success-only, r8 vs r9-3run)

| metric | r8 | r9 mean | r9 median | r9 variance | median_delta_vs_r8 |
|---|---:|---:|---:|---:|---:|
| avg_duration_ms_success_only | 11851.83 | 9922.22 | 10453.00 | 212183.81 | -1398.83 |

Interpretation notes:

- Adding median and variance reduces single-run bias: scenario-level medians show `r9` is faster than `r8`, while model-level hot-path variance reveals a few models with unstable run2 latency.
- `legacy_salsa0` split confirms no success-rate repair effect in `r8 -> r9`; the observed difference is pure performance distribution change under all-success runs.

### Full-tier regression diff: r9 -> r10 (3-sample, with P90/MAD) (2026-04-08)

Top-line verdict:

- `r10_1/r10_2/r10_3`: all `30 total / 30 pass / 0 fail`
- `hot_nsA run2` cache consistency keeps full coverage in all r10 samples: `artifact_hit=6/6`, `external_resolve_hit=6/6`, `cache_warm_ratio=1`

Scenario aggregate distribution (`avg_duration_ms` per scenario per run):

| scenario | r9 median_ms | r10 median_ms | median_delta_ms | r9 variance_ms2 | r10 variance_ms2 | r9 P90_ms | r10 P90_ms | r9 MAD_ms | r10 MAD_ms |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| cold_empty_nsCOLD | 5331.00 | 5674.83 | +343.83 | 3647.34 | 24201.79 | 5316.87 | 5646.73 | 120.17 | 310.67 |
| cold_qcache0 | 5254.00 | 5751.00 | +497.00 | 4119.18 | 61098.34 | 5240.70 | 5688.03 | 128.33 | 487.50 |
| hot_nsA | 3663.17 | 3601.00 | -62.17 | 8215.03 | 4246.20 | 3633.15 | 3596.55 | 163.08 | 122.33 |
| legacy_salsa0 | 10453.00 | 10152.50 | -300.50 | 212183.81 | 34449.79 | 10299.73 | 10091.43 | 826.00 | 335.50 |

Hot-path per-model distribution (`hot_nsA`, run2, 3 samples):

| model | r9 median_ms | r10 median_ms | median_delta_ms | r9 variance_ms2 | r10 variance_ms2 | r9 P90_ms | r10 P90_ms | r9 MAD_ms | r10 MAD_ms |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| ModelicaTest.JitStress.ComplexJitRegression | 1735 | 2165 | +430 | 103.00 | 54704.33 | 1732.4 | 2089.8 | 20 | 429 |
| ModelicaTest.JitStress.MslBroadCoverage | 2024 | 1732 | -292 | 23104.33 | 37.00 | 1993.4 | 1731.8 | 304 | 11 |
| ModelicaTest.JitStress.RobotElectricalControl | 2053 | 1782 | -271 | 34757.33 | 702.33 | 1991.4 | 1776.6 | 336 | 53 |
| TestLib.BigFor | 1813 | 1758 | -55 | 2942.33 | 902.33 | 1798.4 | 1757.6 | 106 | 53 |
| TestLib.MSLBlocksTest | 1849 | 1776 | -73 | 3640.33 | 1183.00 | 1828.2 | 1773.4 | 105 | 65 |
| TestLib.MultiTopCombined | 1999 | 1725 | -274 | 12940.33 | 274.33 | 1960.0 | 1721.2 | 199 | 33 |

Interpretation notes:

- `P90` + `MAD` confirms long-tail jitter reduction in `hot_nsA` and `legacy_salsa0` at scenario level (`variance`, `P90`, `MAD` all decreased), while cold scenarios show wider spread in this sample window.
- At model granularity, most hot-run2 cases become tighter (`MAD` down), but `ComplexJitRegression` shows a tail expansion in r10 and should be tracked separately as a hotspot outlier.

Quick triage notes:
- 先最小化变量：固定模型、固定输入、固定参数，再做单因子对比。
- 涉及覆盖关系时遵循 `CLI > env > default`，避免“看起来改了但未生效”。
- 性能问题优先留痕：`report.json`、`perf-json`、`cache_stats`、`dep_graph` 至少保留一组可复现工件。

## 分类文档 / Category Documents

- [核心仿真 / Core Simulation](./core-simulation.md)
- [事件与时钟 / Events And Clock](./events-and-clock.md)
- [函数与多输出 / Functions And Multioutput](./functions-and-multioutput.md)
- [展平连接与OOP / Flatten Connect OOP](./flatten-connect-oop.md)
- [工具链FMI EmitC Script / Toolchain FMI EmitC Script](./toolchain-fmi-emitc-script.md)（含 `modelDescription` 字段、CLI/`RUSTMODLICA_FMI_*` 与 `run_regression.ps1` FMI 断言） / includes `modelDescription` fields, CLI and `RUSTMODLICA_FMI_*` env, and `run_regression.ps1` FMI checks
- [MSL与ModelicaTest目录回归 / MSL And ModelicaTest Directory Regression](./msl-modelicatest-dir-regression.md)

## 参数收敛与选择规范 / Parameter convergence and selector spec

- 主规范 / Main spec: `parameter-convergence.md`
- 参数元数据 / Parameter metadata: `parameter-metadata.json`
- 场景模板 / Profile templates: `profile-templates.json`
- 实施映射指南 / CLI-TUI guide: `CLI_TUI_Implementation_Guide.md`
- 集成检查清单 / Integration checklist: `parameter-integration-checklist.md`

使用建议 / Recommended reading order:
1) `parameter-convergence.md`（规则与流程）  
2) `profile-templates.json`（场景参数包）  
3) `parameter-metadata.json`（参数级元信息）  
4) `CLI_TUI_Implementation_Guide.md`（落地实现映射）

## 统一判定规则 / Unified Verdict Rules

- `pass` 用例 / case: 命令退出码为 `0`
- `fail` 用例 / case: 命令退出码为非 `0`
- 产物检查 / artifact check: 需要输出文件的功能必须产生对应文件
- 稳定性检查 / stability check: 确定性场景重复执行结果文件应一致

## Cargo features convention

- Use `RUSTMODLICA_CARGO_FEATURES` (comma-separated) as the single source of truth for `cargo --features ...` in scripts and `custom_command` cases.
- Do not hardcode `--features sundials` in new scripts; prefer reading `RUSTMODLICA_CARGO_FEATURES` and defaulting to `sundials` when unset.

Custom command template (feature-dependent):

```json
{
  "id": "my_featured_case",
  "kind": "custom_command",
  "target": "MY-TARGET",
  "program": "powershell",
  "env": {
    "RUSTMODLICA_CARGO_FEATURES": "sundials"
  },
  "args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "<script.ps1>"],
  "tags": ["jit-phase1"],
  "expect": { "kind": "exit_zero" }
}
```

## Config composition (`includes`)

- Use top-level `includes` to compose a phase config from one or more base configs.
- Include paths are resolved relative to the including config file (or absolute).
- Merge order:
  - Included configs are loaded first (in listed order).
  - Then the current config overlays `defaults` / `execution` / `incremental` / `tiers`.
  - Then the current `cases` are appended.
- Recommendation:
  - Keep PS1-scanned baseline configs as stable “tables” (e.g. `testlib_from_run_regression.json`, `mos_from_run_mos_regression.json`).
  - Keep phase configs as “overlay” configs: `includes` + only the phase-specific `custom_command` / `mos` additions.

Phase config template (include baseline + add phase custom/mos):

```json
{
  "version": 1,
  "includes": [
    "testlib_from_run_regression.json",
    "mos_from_run_mos_regression.json"
  ],
  "defaults": {
    "working_dir": "jit-compiler",
    "cargo_run_models": true,
    "cargo_target_dir_primary": "target_regression"
  },
  "tiers": {
    "phase1": { "include_tags": ["jit-phase1"] }
  },
  "cases": [
    {
      "id": "phase1_my_custom_case",
      "kind": "custom_command",
      "target": "PHASE1/MY-CHECK",
      "program": "powershell",
      "args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "<script.ps1>"],
      "tags": ["jit-phase1"],
      "expect": { "kind": "exit_zero" }
    }
  ]
}
```

## 追溯基线 / Traceability Baseline

分类映射与以下来源保持一致。  
Category mapping is aligned with the following sources.

- `run_regression.ps1` 的用例清单与专项检查 / case list and special checks
- `JIT_DEVELOPMENT_ANALYSIS.md` 的回归覆盖统计 / regression coverage sections
- `README.md` 的求解器与工具链入口说明 / solver and toolchain usage sections

## 更新规范 / Update Rules

- 新增案例时，写入对应分类文档 / Add new cases to the corresponding category document
- 命令示例保持 Windows PowerShell 兼容 / Keep command examples Windows PowerShell compatible
- 期望结果与失败模式需和脚本行为同步 / Keep expected verdict and failure mode synchronized with script behavior
- Hybrid 快捷键或交互行为发生变更时，必须同步更新 `crates/regress-harness/src/i18n.rs` 中 `hybrid_help_*` 键与本文件 Hybrid 快捷键清单 / When Hybrid hotkeys or interaction behavior changes, update both `hybrid_help_*` keys in `crates/regress-harness/src/i18n.rs` and the Hybrid hotkey list in this document
- Hybrid 文案统一使用语义化术语：`monitor tail`、`monitor follow`、`rerun recent failed`、`run focused`、`failed TOP link`、`switch failed detail`、`expand summary` / Keep Hybrid wording aligned to semantic terms: `monitor tail`, `monitor follow`, `rerun recent failed`, `run focused`, `failed TOP link`, `switch failed detail`, `expand summary`
- 若 README 中包含 Hybrid 界面示意图或截图说明，新增/修改快捷键后必须同步更新截图对应的文字清单，确保图文一致 / If README includes Hybrid UI mockups or screenshot captions, update the screenshot-side text checklist whenever hotkeys are added or changed
- Hybrid 变更固定检查清单（每次改动都执行）：1) `i18n` 的 `hybrid_help_*` 键；2) README Hybrid 快捷键清单；3) README 中截图/示意图对应文字；4) README 命令示例与参数说明 / Hybrid change mandatory checklist (run on every change): 1) `hybrid_help_*` keys in `i18n`; 2) Hybrid hotkey list in README; 3) screenshot/mockup captions in README; 4) command examples and parameter notes in README
