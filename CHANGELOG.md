# Changelog

All notable changes to this project are documented in this file.

Format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### JIT perf baseline

- **`regress-harness jit validate-perf`**: repeatable **`--set-env KEY=VAL`** for child `rustmodlica` processes; manifest records `child_env`.
- **Default `jit compare-baseline` baseline path** (when **`--baseline`** is omitted): **`baseline/20260417_jit_cranelift_none/jit_perf_baseline.json`** (six models, `cold_empty_nsCOLD` + `hot_nsA`, **`RUSTMODLICA_CRANELIFT_OPT_LEVEL=none`**). That JSON stores **`speedup_min_ratio`: 1.0** so harness speedup checks do not false-fail when hot wall time tracks cold.
- **`.gitignore`**: under **`baseline/**`**, ignore JIT validate-perf bulk (`cache_*`, **`*.sqlite*`**, perf/log/stats/manifest artifacts); version control only **`report.json`** + **`jit_perf_baseline.json`** for the default JIT perf baseline folder.

### JIT cache / warmup (Leyden-style)

- **Warmup**: cross-process `flatten_cache_dir/.warmup.lock`, `RUSTMODLICA_WARMUP_TIER=auto`, optional `RUSTMODLICA_WARMUP_DEP_GRAPH`, ranked candidates (`model_hotness_v1.json` + `.mo` size), compile-epoch cancel for foreground compiles (`CompilerOptions.warm_background`), periodic global budget checks during warmup loops.
- **Precompile**: `precompile_models_parallel` uses **rayon** when more than one model.
- **Codegen disk**: `codegen_path_index.sqlite` under JIT cache root for faster `global_budget` scans; updated on cache write.
- **Path hash index**: `path_hash_index.sqlite` under flatten cache root; `closure_hash::unified_file_hash` consults it after the in-process LRU.
- **CLI**: `--cache-stats` is read-only; **`--cache-gc`** runs `enforce_global_budget`; **`--cache-invalidate`** (`soft|hard|model`); **`--cache-stats --miss-breakdown`** prints aggregated `cache_miss_agg_v1.json`.
- **Perf / observability**: `CompilePerfReport` adds `warmup_populated_count`, `warmup_attributable_hits`, `warmup_time_ms`; end-of-compile hook persists `cache_miss_agg` + `model_hotness_record`.
- **Invalidation**: `CompileFlagsChanged` maps to **soft** invalidation; soft deletes stage-keyed SQLite rows (flat_full + array_sizes) without full project DB wipe.
- **Artifact cache**: `RUSTMODLICA_ARTIFACT_PRUNE_INTERVAL_MS` (default 60s) throttles SQLite prune on get/put.
- **AOT**: `try_load_default_archive` logs skip reason on load failure.

### Adaptive CONST_FOLD / EQ_DCE and compile perf extensions

- Optional **`RUSTMODLICA_ADAPTIVE_FOLD_POLICY=1`**: SQLite-backed per-flatten-hash record (`fold_benefit_record`) skips const-fold/DCE when the last run had zero benefit, and applies a short cooldown when external-resolve time regresses; **`persist_and_tierup_flags`** also arms tier-up recompiles via `tierup_skip_const_fold` (see `simulation.rs`).
- **`--perf-json`**: new fields include `external_resolve_*_us` sub-spans, `const_fold_skipped_by_policy`, `const_fold_cooldown_active`, `jit_bypassed_tier0`, `warmup_auto_enqueued`, and flatten inline sub-phase proxies (`salsa_flat_full_get_us`, etc.).
- **External resolve cache key** now uses **pre-fold** call-site lists so CONST_FOLD on/off shares the same SQLite key where sites are unchanged.
- Scripts: **`scripts/phase_split_negative_three.ps1`** (10-round medians + `summary.json`), **`scripts/msl_complex_opt_compare.ps1`** (`-AdaptiveFoldPolicyOnOpt`), **`scripts/cache_warm_kpi.ps1`** (cold vs hot KPI sample).

### Leyden-style AOT archive (v2) and JIT codegen disk cache

- **AOT archive format** is at **version 2** (`jit-compiler/src/jit/aot_archive.rs`, magic `RMJITAOT`). Table-of-contents entries now store `when_count` and `crossings_count` in addition to `codegen_key_hash` and optional `import_symbols`, so the **AOT native fast path** can skip in-process `jit.compile` while still wiring the simulation loop correctly.
- **Version 1** `aot_archive.bin` files are rejected with a version mismatch error; delete the file or re-run a full compile to regenerate the archive.
- **Non-empty code blobs** in the archive are populated from the **JIT codegen disk cache** object file (`.bin` next to `.json` under the codegen cache root, keyed by `CodegenCacheKey::stable_hash()`). For this to work in practice:
  - Keep **`RUSTMODLICA_JIT_CODEGEN_CACHE` enabled** (default when unset: on; set to `0` / `false` / `no` to disable). Disabling the cache prevents writing/reading the relocatable object bytes used when merging into `aot_archive.bin`.
  - After at least one successful compile that **writes** a relocatable object for the model, the AOT merge step can embed those bytes; otherwise the TOC entry may carry an **empty** blob and the AOT native loader will fall back to normal JIT.
- **Optional cache directory**: `RUSTMODLICA_JIT_CODEGEN_CACHE_DIR`; default follows `dirs::cache_dir()` / `rustmodlica/jit-codegen` (see `codegen_cache_key.rs`).
- **Relocation loaders**:
  - **Windows**: COFF relocation in `jit-compiler/src/jit/codegen_cache/coff_reloc.rs`.
  - **Linux**: ELF64 relocation in `jit-compiler/src/jit/codegen_cache/elf_reloc.rs` (undefined symbols resolved via `dlsym`; the JIT crate links `libdl` on Linux).
  - **Other Unix** (e.g. macOS): AOT native load remains **unsupported** in this release (object artifacts are ELF on Linux; macOS typically uses Mach-O from the toolchain).

### `compile_perf` / `--perf-json` (dual compile + AOT native)

Structured compile performance JSON (via `--perf-json=...`) includes Leyden-related fields such as `dual_compile_requested`, `dual_compile_ok`, `dual_compile_speculation_count`, `dual_compile_status`, and `aot_native_load_status` for regression tracking.
