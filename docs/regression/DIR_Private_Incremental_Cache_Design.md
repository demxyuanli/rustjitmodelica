# DIR private incremental cache — design

## Goals

- **Speed up local DIR re-runs** (`run_modelica_dir_regression.ps1`) by reusing compiler/query/flatten artifacts when inputs are unchanged.
- **Private**: cache lives **outside shared CI artifacts** and ideally **outside git** (per-machine or per-clone ignored path).
- **Incremental**: safe reuse only when **dependency closure**, **toolchain identity**, and **run parameters** match a recorded key (no silent stale hits).
- **Not a substitute for cold regression**: release gates should still allow a **cold** path periodically; this design targets **developer iteration** and optional **warm** CI shards.

## What already exists (reuse, do not reinvent)

| Mechanism | Env / entry | Role |
|-----------|-------------|------|
| Query + layered SQLite | `RUSTMODLICA_CACHE_SQLITE=1`, `RUSTMODLICA_QUERY_CACHE_NAMESPACE`, optional disable `RUSTMODLICA_QUERY_CACHE=0` | AST / `query_db` reuse across processes |
| Flatten on-disk hints | `RUSTMODLICA_FLATTEN_CACHE_DIR` (optional; default `<install_root>/cache` via `current_exe` or `RUSTMODLICA_INSTALL_ROOT`), `RUSTMODLICA_FLATTEN_FULL_CACHE`, `RUSTMODLICA_FLATTEN_CACHE_TTL_MS` | Flattened model hints; TTL / invalidation via `RUSTMODLICA_CACHE_INVALIDATE_TRIGGER` (see `frontend.rs`) |
| SHM index (optional) | `RUSTMODLICA_CACHE_SHM`, `RUSTMODLICA_CACHE_SHM_NAME`, … | Faster cross-process index; optional for DIR |
| AOT marker | `RUSTMODLICA_AOT_CACHE_DIR` | Lightweight compile fingerprint markers (not full binary reuse) |

IR / schema generation is already tied to **`IR_SCHEMA_EPOCH`** (`jit-compiler/src/cache/ir_epoch.rs`); any bump must **invalidate** logical cache compatibility. When a disk cache root is active (explicit `RUSTMODLICA_FLATTEN_CACHE_DIR` or the default `<install_root>/cache`), the compiler writes **`ir_schema_epoch.txt`** there and, on epoch mismatch, **removes the entire cache root** (SQLite + JSON hints) before continuing, so stale on-disk artifacts are not mixed across IR generations. Set `RUSTMODLICA_FLATTEN_CACHE_DIR=0` to disable the disk root.

## Private storage layout (proposed)

Default root (never commit):

- **Windows**: `%LOCALAPPDATA%\rustmodlica\dir_cache\<repo_fingerprint>\`
- **Override**: `RUSTMODLICA_DIR_PRIVATE_CACHE_ROOT` (single absolute path)

Under root:

```
<root>/
  run_key_<short_hash>/          # one "run profile": exe + libs + script version
    L0/ L1/ L2/                  # optional mirror of layered SQLite dirs if we want separation
    flatten/                     # RUSTMODLICA_FLATTEN_CACHE_DIR points here
    aot_markers/                 # RUSTMODLICA_AOT_CACHE_DIR
    manifest.json                # optional: last run metadata for humans / tooling
```

`<repo_fingerprint>`: e.g. SHA256 of `git rev-parse --show-toplevel` path + remote URL truncated, or simply a **user-supplied** `RUSTMODLICA_DIR_CACHE_INSTANCE` to avoid collisions when multiple clones share one `%LOCALAPPDATA%` tree.

`<short_hash>`: hash of:

- `rustmodlica.exe` SHA256 (or `target_replace` release path)
- Contents or hash of `build_modelica_dir_regress/libraries.lock.json` when present, else hash of ordered `LibPath` roots + `git HEAD`
- `IR_SCHEMA_EPOCH` (read from sources at packaging time **or** stamped at build — if only runtime, use binary hash as proxy)
- Fixed **policy version** string bumped when CLI semantics change (e.g. default `--index-reduction-method`)

This yields a **new subdirectory** when the toolchain or libraries change — **no cross-version reuse**.

## Namespace strategy (parallel-safe)

`run_modelica_dir_regression.ps1` uses **multiple PowerShell workers**. SQLite and flatten dirs **must not** contend on the same files.

**Rule**: each shard sets a **distinct** `RUSTMODLICA_QUERY_CACHE_NAMESPACE`, e.g. `DIR_S<shard>W_<run_key_suffix>` and points `RUSTMODLICA_FLATTEN_CACHE_DIR` to `.../run_key_.../flatten/shard_<n>/`.

Parent merge **does not** need a unified cache; only **human wall time** matters per shard.

Serial mode (`ParallelWorkers=1`) uses a single namespace: `DIR_SERIAL_<run_key_suffix>`.

## Wiring in `run_modelica_dir_regression.ps1` (Phase 1 — env only)

Parameters (proposal):

- `-PrivateCacheRoot` — optional; default from `RUSTMODLICA_DIR_PRIVATE_CACHE_ROOT` or `%LOCALAPPDATA%\...`
- `-DisablePrivateCache` — force cold behavior for this invocation
- `-PrivateCacheKeyExtra` — optional string mixed into `run_key` (e.g. `experimentA`)

Before each `& $exe @cliArgs`:

1. Resolve `exe` path and compute `$exeHash`.
2. Load or compute **library lock** fingerprint (reuse logic aligned with `libraries.lock.json` from full regression if available).
3. ` $runKey = short_hash( $exeHash, $libFp, $IR_SCHEMA_EPOCH, $policyVersion, $PrivateCacheKeyExtra ) `
4. Set:

   - `RUSTMODLICA_CACHE_SQLITE=1`
   - `RUSTMODLICA_QUERY_CACHE_NAMESPACE=DIR_...` (per shard or serial)
   - `RUSTMODLICA_FLATTEN_CACHE_DIR=<root>/run_key_.../flatten[/shard_n]`
   - `RUSTMODLICA_AOT_CACHE_DIR=<root>/run_key_.../aot_markers[/shard_n]` (optional)

5. Do **not** set `RUSTMODLICA_CACHE_INVALIDATE_TRIGGER` unless user asks for purge.

**Child processes** in parallel mode must receive the same env: extend `Start-Process` to pass `-Environment` (PowerShell 7+) or set machine/user env temporarily — if older PS5-only, document using **one wrapper script** that sets env then calls self — or inject via a tiny launcher `.cmd`.

## Phase 2 — optional "skip unchanged model" (higher risk)

A **manifest** row per model: key = `(model, solver, dt, t_end, lib_fp, exe_hash)` → `(exit_code, reason, output_csv_sha256, wall_ms)`.

**Skip** re-invoking `rustmodlica` only if:

- Key matches, and
- **No** `.mo` in the **closure** (or library roots) has mtime newer than manifest time **and** closure hash matches stored value (requires Rust or external tool to compute closure hash — ideally **reuse** existing dep graph export if wired in DIR runs).

**Recommendation**: treat Phase 2 as **off by default**; enable with `-DirTrustManifest` only for inner-loop tuning, never for sole CI gate.

## Cold vs warm policy

| Mode | Use case |
|------|-----------|
| Cold | `Remove-Item env:RUSTMODLICA_*` / `-DisablePrivateCache` / fresh `run_key` |
| Warm | same `run_key`, same shards, incremental library edits |
| Purge | set `RUSTMODLICA_CACHE_INVALIDATE_TRIGGER` to documented value, or delete `run_key_*` dir |

## Observability

- Reuse existing `RUSTMODLICA_CACHE_STATS_JSON` from validate-perf harness **optionally** in DIR: one stats file per shard under `OutDir` for post-run aggregation. Payload includes **`query_cache_counters`** (every in-process `cache_*` counter, including `cache_L0_hits` / `cache_L1_hits` / `cache_L2_hits`, misses, per-layer writes, and `cache_stage_*` breakdown keys), plus **`cache_scope_stage_hits` / `misses` / `invalidations`** maps, and SQLite **`layers` / `rows`** when `RUSTMODLICA_FLATTEN_CACHE_DIR` is set (otherwise those arrays are empty but the file is still written). The JSON includes SQLite `layers`/`rows` when `RUSTMODLICA_FLATTEN_CACHE_DIR` is set, plus **`query_cache_counters`** (all in-process `cache_*` keys from `query_db` perf, including `cache_L0_hits` / `cache_L1_hits` / `cache_L2_hits`, misses, per-layer writes, and `cache_stage_*` breakdowns) and **`cache_scope_stage_*`** maps even when no flatten cache dir is configured.
- `manifest.json` under `run_key_*` can record: timestamp, worker count, git HEAD, `libraries.lock` hash, list of shard namespaces.

## Security / hygiene

- Private cache may contain **paths and model names** from your machine; **do not** upload without review.
- Add `**/dir_cache/` or `%LOCALAPPDATA%/rustmodlica/dir_cache/` to contributor docs / `.gitignore` if any script drops a symlink under repo `build/`.

## Implementation status (landed)

`run_modelica_dir_regression.ps1`:

- **`-UsePrivateCache`** or **`RUSTMODLICA_USE_DIR_PRIVATE_CACHE=1`** (with `-DisablePrivateCache` to force cold).
- **`-PrivateCacheRoot`** optional; else **`RUSTMODLICA_DIR_PRIVATE_CACHE_ROOT`**, else `%LOCALAPPDATA%\rustmodlica\dir_cache\<repoPathHash8>\`.
- **`-PrivateCacheKeyExtra`** mixed into the run key (A/B experiments).
- **`PrivateCacheRunKey` / `PrivateCacheShard` / `PrivateCacheRoot`**: internal; parallel parent passes per-shard namespace and paths.
- Sets **`RUSTMODLICA_CACHE_SQLITE=1`**, **`RUSTMODLICA_QUERY_CACHE_NAMESPACE`**, **`RUSTMODLICA_FLATTEN_CACHE_DIR`**, **`RUSTMODLICA_AOT_CACHE_DIR`** before preflight and all model runs.
- Writes **`manifest_serial.json`** / **`manifest_shard_N.json`** under `run_<key>/`.

`run_regression.ps1`: **`-DirUsePrivateCache`** and optional **`-DirPrivateCacheRoot`** forward to DIR only.

`.gitignore`: **`build/dir_private_cache/`** (fallback when `LOCALAPPDATA` is missing).

**Sharing cache with the next DIR run or other tools**

- **`-WriteDirCacheEnvScript <path>`** (relative to repo root or absolute): after `Apply-DirPrivateCacheEnv` succeeds, writes a small PowerShell script that sets `RUSTMODLICA_CACHE_SQLITE`, `RUSTMODLICA_QUERY_CACHE_NAMESPACE`, `RUSTMODLICA_FLATTEN_CACHE_DIR`, and `RUSTMODLICA_AOT_CACHE_DIR` for the current `run_<key>`. Dot-source it in the same shell before `rustmodlica --validate` or another driver so the **same** query/flatten namespace is reused without re-running DIR.
- **`scripts/run_dir_cache_shared_smoke.ps1`**: three-step smoke — (1) DIR with `-UsePrivateCache` and a fixed `build\dir_cache_shared_test` root, (2) second DIR pass with a different `-OutDir` but the same `PrivateCacheRoot`, (3) dot-source `build\dir_cache_shared_env.ps1` and run `rustmodlica --validate` on a small model. Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run_dir_cache_shared_smoke.ps1` (optional `-MaxCases`, `-Root`).

## Residual checklist

1. (Optional) Extend team onboarding with env vars; validate-perf / `report.json` cache fields are summarized in `docs/regression/README.md`.
2. Phase-2 manifest short-circuit remains **not implemented** (off by default in design).
3. Rollout plan residual: optional SHM **segment** pools per scope (keys already carry `L0`/`L1`/`L2` via `CacheKeyV2`); `sqlite_put_batch` and Phase-5 transaction/IR-epoch work are in tree — see `three-layer-cache-rollout_9b931642.plan.md` for Phase-4 row-level **Done** notes.

## Related files

- `run_modelica_dir_regression.ps1` — DIR driver, parallel shards; `-WriteDirCacheEnvScript` for exported env.
- `scripts/run_dir_cache_shared_smoke.ps1` — shared-cache smoke (two DIR out-dirs + standalone validate).
- `jit-compiler/src/flatten/flatten_cache.rs`, `flatten/cache_sqlite.rs`, `query_db/mod.rs` — cache behavior.
- `jit-compiler/src/compiler/pipeline/frontend.rs` — invalidation trigger.
- `baseline/<YYYYMMDD>/metrics.json` — baseline comparisons (orthogonal to cache).
