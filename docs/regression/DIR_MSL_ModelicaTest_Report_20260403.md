# DIR regression report: MSL + ModelicaTest (merged summary)

## Scope

This report summarizes the **directory-scale** run produced by `run_modelica_dir_regression.ps1` with merged output in `build_modelica_dir_regress/summary.txt` (parallel shards, run logs stamped `20260403_0013xx`–`001357`).

**Baseline bundle (separate from this narrative):** `build/regression_baseline/dir_20260403/` — machine-oriented artifacts for diffing and archival (`metrics.json`, full failure list, connector excerpt, log index).

## Executive summary

| Metric | Value |
|--------|------:|
| Total rows in merged `summary.txt` | 1802 |
| `OK` | 1607 |
| `!!` (failed) | 195 |
| Pass rate | **89.2%** (1607 / 1802) |

Under `run_modelica_dir_regression.ps1`, **any `!!` line increments the failure budget**; the merged file still contains **195** such lines, so the script **returns exit code 1** and `run_regression.ps1` (without `-SkipDir`) records **DIR / MISMATCH** unless policy changes.

## Pass/fail by library prefix

| Prefix | OK | Failed (`!!`) |
|--------|---:|--------------:|
| `Modelica.` | 1419 | 131 |
| `ModelicaTest.` | 188 | 62 |
| Parallel / infra (`shard_*`) | — | 2 |

`ModelicaTest` carries a **higher failure density** in this snapshot (~24.8% of `ModelicaTest` entries vs ~8.5% of `Modelica` entries), driven by Fluid tests, JIT stress cases, and MultiBody connector flatten errors.

## Failure reasons (from `reason=` field)

| Reason | Count |
|--------|------:|
| `sim_failed` | 191 |
| `newton_nonconverged` | 2 |
| `parallel_summary_missing` | 1 |
| `parallel_worker_failed` | 1 |

## Exit code buckets (failed rows)

| Pattern | Count | Note |
|---------|------:|------|
| `exit=-1073740791` | 98 | Often **fast-fail / stack guard** on Windows during heavy sims |
| `exit=-1` | 52 | Generic simulator failure |
| `exit=1` | 43 | Non-zero exit with stderr diagnostics |
| `exit=-1073741523` | 1 | Additional Windows NTSTATUS-style code |
| `exit=-1073741571` | 1 | Seen on `parallel_worker_failed` (DLL/init class failures are common in this range) |

## Thematic clusters (non-exhaustive)

1. **`FLATTEN_INCOMPATIBLE_CONNECTOR`** — **5** failures, all under `ModelicaTest.MultiBody.Forces.*`: `Interfaces.Frame_resolve` vs `Modelica.Mechanics.MultiBody.Interfaces.Frame_resolve`. Full lines are copied to `build/regression_baseline/dir_20260403/excerpt_flatten_incompatible_connector.txt`.
2. **Fluid** — many `Modelica.Fluid.*` and `ModelicaTest.Fluid.*` failures with `sim_failed` or abrupt process exit; see domain sample counts in `metrics.json`.
3. **Magnetic / electrical machines** — recurring `exit=-1073740791` on FundamentalWave and QuasiStatic machine examples.
4. **Parallel shard 5** — `parallel_summary_missing` and `parallel_worker_failed` indicate **one worker did not produce a clean shard summary**; treat as **infra noise** until re-run confirms.

## Key logs to open first

1. **Merged index:** `build_modelica_dir_regress/summary.txt`
2. **Structured index:** `build/regression_baseline/dir_20260403/KEY_LOGS.md`
3. **Per-case timing / CSV gate:** `build_modelica_dir_regress/parallel_shard_*/runlog_20260403_*.csv`
4. **Deep dive on a single model:** matching `build_modelica_dir_regress/parallel_shard_*/logs/<ModelName>.log`

## Suggested next actions

1. **Re-run or inspect shard 5** (`parallel_shard_5`) to see if the parallel failure is transient (AV, OOM, file lock).
2. **Triage `ModelicaTest.MultiBody.Forces.*` connector typing** as a single fix may collapse five failures.
3. **Compare future runs** against `build/regression_baseline/dir_20260403/metrics.json` (diff `totals`, `failed_by_prefix`, and domain samples).

## Related documents

- Baseline bundle: `build/regression_baseline/dir_20260403/`
- Upstream script: `run_modelica_dir_regression.ps1`
- Full regression wrapper: `run_regression.ps1` (DIR stage)
