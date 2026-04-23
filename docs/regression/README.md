# Regression documentation

- **DIR / MSL + ModelicaTest analysis (2026-04-03):** [DIR_MSL_ModelicaTest_Report_20260403.md](./DIR_MSL_ModelicaTest_Report_20260403.md)
- **Baseline artifacts for that report:** `baseline/20260403/` (metrics, failure list, excerpts, log index)
- **DIR private incremental cache (design + implemented):** [DIR_Private_Incremental_Cache_Design.md](./DIR_Private_Incremental_Cache_Design.md) — use `-UsePrivateCache` on `run_modelica_dir_regression.ps1` or `-DirUsePrivateCache` on `run_regression.ps1`.
- **JIT validate-perf (`regress-harness` / `jit validate-perf`):** writes `report.json` under the chosen `out_dir`. Field **`stats.by_scenario.<scenario>.<model>`** includes:
  - **`cache_layer_stats`**: per scope (`L0` / `L1` / `L2`) totals: `hits`, `misses`, **`writes`** (SQLite rows written), `invalidations`, and per-stage maps (`stage_hits`, `stage_misses`, `stage_invalidations`).
  - **`cache_query_counters`**: summed counters from each run’s `RUSTMODLICA_CACHE_STATS_JSON` **`query_cache_counters`** object (e.g. `cache_L0_hits`, `cache_L0_writes`, `cache_stage_hits:L0:parse`, …).
  - **`cache_flat_full_layer_*` / `cache_array_sizes_layer_*`**: flatten / array-size hint rollups by scope.
  After a successful run, the CLI prints one line **`jit-validate-perf cache rollup:`** with L0/L1/L2 hits+writes and distinct `query_cache_counter` key count + sum when present.

## JIT parameter convergence summary

- Full specification is maintained in `jit-compiler/docs/regression/parameter-convergence.md`.
- Machine-readable assets:
  - `jit-compiler/docs/regression/parameter-metadata.json`
  - `jit-compiler/docs/regression/profile-templates.json`
- Implementation mapping guide:
  - `jit-compiler/docs/regression/CLI_TUI_Implementation_Guide.md`

Quick policy:
- Option precedence is `CLI > env > profile > default`.
- Use `DevFast` for local iteration, `CIGate` for gate runs, `PerfDiag` for regressions, `SolverStability` for convergence, and `FMIExport` for export workflows.
- For troubleshooting, prefer profile switch first, then parameter fine tuning.

## JIT settings reference (CLI + env)

### `rustmodlica` CLI (high impact options)

| Option | Values / Example | Default | Purpose | Typical use |
|---|---|---|---|---|
| `--validate` | switch | off | Compile-only validation, no simulation | Fast gate in CI |
| `--validate-tier` | `full \| parse \| flatten \| analyze` | `full` | Stop validation at a specific phase | Isolate parser/flatten/analyze issues |
| `--validation-mode` | `full \| quick \| superfast` | `full` | Speed vs strictness trade-off | Large batch validation |
| `--solver` | `rk4 \| rk45 \| implicit \| cvode \| ida` | `rk45` | Solver selection for simulation | Numerical stability comparison |
| `--index-reduction-method` | `none \| dummyDerivative \| pantelides \| pantelidesDummy \| debugPrint` | `none` | Index reduction strategy | High-index DAE tuning |
| `--tearing-method` | e.g. `first` | `first` | Nonlinear tearing strategy | Tearing behavior checks |
| `--generate-dynamic-jacobian` | e.g. `none` | `none` | Dynamic Jacobian policy | Newton path tuning |
| `--array-size-policy` | `legacy \| strict` | `legacy` | Handling unknown array dimensions | Strict flatten diagnostics |
| `--array-sizes-json` | `path/to/array_sizes.json` | empty | Explicit array size map input | Pair with strict policy |
| `--perf-json` | `path/to/perf.json` | empty | Structured compile/sim performance output | Perf collection |
| `--output-format` | `json` | text | JSON simulation output to stdout | Automation integration |
| `--emit-c` | output dir | empty | Emit C artifacts | Emit-C checks |
| `--emit-fmu` / `--emit-fmu-me` | output dir | empty | Emit FMI CS / ME artifacts | FMI integration checks |
| `--fmi-model-id` | identifier | empty | Override FMI `modelIdentifier` | Stable export naming |
| `--fmi-guid` | UUID/token | empty | Override FMI `guid` | Reproducible exports |

### JIT-related environment variables (high impact set)

| Env var | Default behavior | Purpose | Notes |
|---|---|---|---|
| `RUSTMODLICA_SALSA` | validate path tends to use query pipeline by default | Query-based flatten switch | `0`: force legacy path, `1`: force query path |
| `RUSTMODLICA_QUERY_CACHE` | enabled | Global query cache on/off | `0/false/no` disables |
| `RUSTMODLICA_QUERY_CACHE_NAMESPACE` | empty | Cache namespace isolation | Use per scenario/job namespace |
| `RUSTMODLICA_FLATTEN_CACHE_DIR` | `<install_root>/cache` | Flatten/SQLite cache root | `0/false/no/none` disables disk root |
| `RUSTMODLICA_LIBS_EPOCH_CACHE` | enabled | Include dep-closure fingerprint in cache key | `0/false/no` disables |
| `RUSTMODLICA_CACHE_SQLITE` | enabled | SQLite tier cache on/off | Boolean parsed |
| `RUSTMODLICA_CACHE_STATS_JSON` | disabled | Export cache counters JSON | Read by validate-perf rollups |
| `RUSTMODLICA_DEP_GRAPH_JSON` | disabled | Export dependency graph JSON | Used by incremental analysis |
| `RUSTMODLICA_PERF_TRACE` | disabled | Print perf trace lines | Useful for local profiling |
| `RUSTMODLICA_STAGE_TRACE` | disabled | Print per-stage tracing | Useful for stage-level diagnostics |
| `RUSTMODLICA_CRANELIFT_OPT_LEVEL` | `speed` fallback | JIT Cranelift opt level | Unknown values fallback |
| `RUSTMODLICA_CRANELIFT_AOT_OPT_LEVEL` | implementation default | AOT Cranelift opt level | Affects AOT generation |
| `RUSTMODLICA_AOT_CACHE_DIR` | disabled when unset | AOT marker cache root | Empty string also disables |
| `RUSTMODLICA_JIT_POLICY_JSON` | unset | JIT policy overlay file | Runtime policy override |
| `RUSTMODLICA_JIT_CODEGEN_CACHE` | **on** when unset (`0`/`false`/`no` disables) | JIT codegen disk cache switch | AOT v2 merge reads reloc objects from this cache; see `CHANGELOG.md` |
| `RUSTMODLICA_JIT_CODEGEN_CACHE_DIR` | implementation default | JIT codegen cache root | Prefer fixed path in CI |
| `RUSTMODLICA_JIT_STUB_PARALLEL` | disabled | Parallel JIT stub compilation | Helps larger models |
| `RUSTMODLICA_JIT_PARTITION_SCAN_PARALLEL` | disabled | Parallel partition scanning | Helps heavy clock/partition models |
| `RUSTMODLICA_OVERDET_CHECK` | implementation default | Overdetermined checks on/off | Can be set by CLI too |
| `RUSTMODLICA_OVERDET_RESIDUAL_TOL` | implementation/annotation derived | Overdet residual tolerance | Can be set by CLI too |
| `RUSTMODLICA_EVENT_COUNT_DEADBAND` | implementation default | Event-count deadband | Tuned by event-scan |
| `RUSTMODLICA_TAIL_VELOCITY_DEADBAND` | implementation default | Tail-velocity deadband | Tuned by event-scan |
| `RUSTMODLICA_SUNDIALS_EVENT_LOG` | implementation default | Sundials event logging | Quiet scans may force `0` |
| `RUSTMODLICA_FMI_MODEL_ID` | unset | FMI model id override | Lower priority than `--fmi-model-id` |
| `RUSTMODLICA_FMI_MODEL_ID_PREFIX` | unset | FMI model id prefix | Used when no explicit override |
| `RUSTMODLICA_FMI_GUID` | unset | FMI guid override | Must pass format validation |
| `RUSTMODLICA_FMI_GENERATION_TOOL` | built-in default | FMI generation tool text | Metadata for generated FMU |

## Minimal recommended configuration templates

### 1) Development template (local quick iteration)

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

### 2) CI gate template (stable + reproducible)

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

### 3) Performance diagnostics template (trace + artifacts)

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

Recommended usage:
- Use template 1 for day-to-day local dev loops.
- Use template 2 as the baseline CI gate config.
- Use template 3 only when diagnosing regressions or cache/perf anomalies.
