# Baseline snapshots

Each run exports under **`baseline/<YYYYMMDD>/`** (date folder name). Typical files: `metrics.json`, `failures_all_lines.txt`, `KEY_LOGS.md`, and focused excerpts.

**JIT validate-perf / `regress-harness jit compare-baseline`:** default baseline JSON (when `--baseline` is omitted) is **`baseline/20260417_jit_cranelift_none/jit_perf_baseline.json`**, captured with **`RUSTMODLICA_CRANELIFT_OPT_LEVEL=none`** on the child `rustmodlica` process (release binary). Its stored **`speedup_min_ratio` is 1.0** so cold vs hot checks stay meaningful when hot runs are wall-time dominated (not strictly codegen). Older **`baseline/20260408`** numbers are not comparable to current full JIT/cache behavior.

For new captures under **`baseline/`**, prefer committing only **`report.json`** plus **`jit_perf_baseline.json`**. Root **`.gitignore`** ignores validate-perf bulk under `baseline/**` (`cache_*` trees, **`*.sqlite*`**, `perf_*.json`, `stdout_*.txt`, `stderr_*.txt`, `dep_graph_*.json`, `cache_stats_*.json`, `run_manifest.json`, etc.) so local re-runs do not clutter `git status`.
