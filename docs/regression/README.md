# Regression documentation

- **DIR / MSL + ModelicaTest analysis (2026-04-03):** [DIR_MSL_ModelicaTest_Report_20260403.md](./DIR_MSL_ModelicaTest_Report_20260403.md)
- **Baseline artifacts for that report:** `baseline/20260403/` (metrics, failure list, excerpts, log index)
- **DIR private incremental cache (design + implemented):** [DIR_Private_Incremental_Cache_Design.md](./DIR_Private_Incremental_Cache_Design.md) — use `-UsePrivateCache` on `run_modelica_dir_regression.ps1` or `-DirUsePrivateCache` on `run_regression.ps1`.
- **JIT validate-perf (`regress-harness` / `jit validate-perf`):** writes `report.json` under the chosen `out_dir`. Field **`stats.by_scenario.<scenario>.<model>`** includes:
  - **`cache_layer_stats`**: per scope (`L0` / `L1` / `L2`) totals: `hits`, `misses`, **`writes`** (SQLite rows written), `invalidations`, and per-stage maps (`stage_hits`, `stage_misses`, `stage_invalidations`).
  - **`cache_query_counters`**: summed counters from each run’s `RUSTMODLICA_CACHE_STATS_JSON` **`query_cache_counters`** object (e.g. `cache_L0_hits`, `cache_L0_writes`, `cache_stage_hits:L0:parse`, …).
  - **`cache_flat_full_layer_*` / `cache_array_sizes_layer_*`**: flatten / array-size hint rollups by scope.
  After a successful run, the CLI prints one line **`jit-validate-perf cache rollup:`** with L0/L1/L2 hits+writes and distinct `query_cache_counter` key count + sum when present.
