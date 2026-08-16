use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stops compilation after the named phase when not `Full`. Used for tiered IDE validation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompileStopPhase {
    #[default]
    Full,
    Parse,
    Flatten,
    Analyze,
}

/// Summary returned when compilation stops after `Analyze` (no JIT).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationAnalyzedSummary {
    pub state_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub total_equations: usize,
    pub total_declarations: usize,
    pub alg_equation_count: usize,
    pub diff_equation_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompilerOptions {
    pub backend_dae_info: bool,
    pub index_reduction_method: String,
    pub tearing_method: String,
    pub generate_dynamic_jacobian: String,
    pub t_end: f64,
    pub dt: f64,
    pub atol: f64,
    pub rtol: f64,
    /// When running a function as entry: optional argument values (by input order). Missing => 0.0.
    pub function_args: Option<Vec<f64>>,
    /// RT1-3: Solver choice: "rk4" (fixed step) or "rk45" (adaptive when no when/zero-crossing).
    pub solver: String,
    /// RT1-5: Output/print interval for simulation results.
    pub output_interval: f64,
    /// RT1-5: Optional CSV result file path; when set, simulation writes time series to this file.
    pub result_file: Option<String>,
    /// JIT memory: when true, `Jit::compile()` leaks self via `Box::leak` so the
    /// code segment backing calc_derivs survives the caller. Used by background tier-up.
    pub jit_leak: bool,
    /// DBG-3: Warnings level: "all" | "none" | "error" (none = suppress, error = treat as error).
    pub warnings_level: String,
    /// CG1-1: When set, emit C source (model.c, model.h) to this directory.
    pub emit_c_dir: Option<String>,
    /// When true (and emit_c_dir set), also write coder-grade step API / CMake / interface.json.
    #[serde(default)]
    pub emit_coder_package: bool,
    /// EXT-1: Paths to shared libraries for external function symbols (e.g. .dll, .so).
    pub external_libs: Vec<String>,
    /// When true, suppress progress messages so only JSON (e.g. validate) is on stdout.
    pub quiet: bool,
    /// CLI `--validate`: relax function-as-root entry when scalar `expr_eval` cannot run the body.
    pub validate_only: bool,
    /// Tier S: write canonical flat JSON after flatten (before inline) to this path.
    pub emit_flat_snapshot: Option<String>,
    /// Stop compilation after writing `emit_flat_snapshot` (no JIT/simulation).
    pub flat_snapshot_only: bool,
    /// Use legacy string `constrainedby` check instead of extends-closure (see `instantiate` module).
    pub coarse_constrainedby_only: bool,
    /// `legacy` (default): unevaluated array dims fall back to scalar with optional warning. `strict`: error unless overridden.
    pub array_size_policy: String,
    /// Optional JSON file: `{"array_sizes":{"<flat_base_name>": N, ...}}` merged during flatten.
    pub array_sizes_json: Option<String>,
    /// When set and `RUSTMODLICA_JIT_POLICY_JSON` is unset, JIT loads this policy overlay path before the first JIT compile in the process.
    pub jit_policy_json: Option<String>,
    /// Validation-only speed/accuracy trade-off: "full" | "quick" | "superfast".
    /// Default is "full". This is intentionally a string to keep CLI/API wiring simple.
    #[serde(default)]
    pub validation_mode: String,
    /// Tiered validation: stop after parse, flatten, or analysis instead of running JIT.
    pub compile_stop: CompileStopPhase,
    /// Leyden: dual-compile produces speculative + generic paths for deopt fallback.
    #[serde(default)]
    pub dual_compile: bool,
    /// When true, this compile is a background warmup/precompile and must not bump the global compile epoch.
    #[serde(default)]
    pub warm_background: bool,
}

/// Structured cache miss event for diagnostics (L4-T08).
#[derive(Clone, Debug, serde::Serialize)]
pub struct CacheMissEvent {
    /// Cache layer: "artifact", "sim_bundle", "codegen", "flat_full", "external_resolve",
    /// "analysis_pipeline", "backend_dae", "analysis_summary".
    pub layer: String,
    /// Miss reason: "key_not_found", "deps_changed", "version_mismatch", "const_fold_mismatch",
    /// "dll_mtime_changed", "fingerprint_mismatch", "disabled", etc.
    pub reason: String,
    /// Optional extra detail (e.g. which param differed, which dep changed).
    pub detail: Option<String>,
}

fn serde_skip_false(b: &bool) -> bool {
    !*b
}

/// Nested dual-compile failure payload for perf JSON (stable field order; omit defaults).
#[derive(Clone, Debug, serde::Serialize)]
pub struct DualCompileErrorReport {
    pub code: String,
    #[serde(default, skip_serializing_if = "serde_skip_false")]
    pub registry_poisoned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cranelift_phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CompilePerfReport {
    pub model_name: String,
    pub load_model_ms: u64,
    pub flatten_inline_ms: u64,
    /// Wall-clock time spent in frontend flatten stage (includes cache hit path).
    pub flatten_wall_ms: u64,
    pub flatten_wall_us: u64,
    /// Wall-clock time spent in inline stage.
    pub inline_wall_ms: u64,
    pub inline_wall_us: u64,
    /// Wall-clock time spent writing flat snapshot (when `--emit-flat-snapshot` is set).
    pub snapshot_write_ms: u64,
    pub snapshot_write_us: u64,
    pub parse_ms: u64,
    pub parse_us: u64,
    pub inheritance_ms: u64,
    pub inheritance_us: u64,
    pub decl_expand_ms: u64,
    pub decl_expand_us: u64,
    pub eq_expand_ms: u64,
    pub eq_expand_us: u64,
    /// Breakdown inside `expand_declarations_with_mode` (sums across recursive Process tasks).
    #[serde(default)]
    pub decl_expand_param_pass_us: u64,
    #[serde(default)]
    pub decl_expand_array_dim_us: u64,
    #[serde(default)]
    pub decl_expand_decl_loop_us: u64,
    #[serde(default)]
    pub decl_expand_try_load_sub_model_us: u64,
    #[serde(default)]
    pub decl_expand_flatten_inheritance_us: u64,
    #[serde(default)]
    pub decl_expand_apply_modification_us: u64,
    #[serde(default)]
    pub decl_expand_param_substitute_fold_us: u64,
    #[serde(default)]
    pub eq_expand_prep_us: u64,
    #[serde(default)]
    pub eq_expand_equations_us: u64,
    #[serde(default)]
    pub eq_expand_algorithms_us: u64,
    #[serde(default)]
    pub eq_expand_initial_equations_us: u64,
    #[serde(default)]
    pub eq_expand_initial_algorithms_us: u64,
    /// Decl-expand inheritance-template cache counters (per compile).
    #[serde(default)]
    pub inherit_flat_template_cache_hit: u64,
    #[serde(default)]
    pub inherit_flat_template_cache_miss: u64,
    /// `hit / (hit + miss)` in range [0,1]; 0 when denominator is zero.
    #[serde(default)]
    pub inherit_flat_template_cache_hit_ratio: f64,
    pub resolve_connections_ms: u64,
    pub resolve_connections_us: u64,
    pub clock_infer_ms: u64,
    pub clock_infer_us: u64,
    pub constrainedby_ms: u64,
    pub constrainedby_us: u64,
    // Cache/IO visibility: time spent outside salsa compute on cache-hit paths.
    pub cache_deps_match_us: u64,
    pub cache_get_us: u64,
    pub cache_deserialize_us: u64,
    pub qcache_deps_match_us: u64,
    pub cache_l0_hits: u64,
    pub cache_l1_hits: u64,
    pub cache_l2_hits: u64,
    pub cache_l0_writes: u64,
    pub cache_l1_writes: u64,
    pub cache_l2_writes: u64,
    pub deps_mismatch: u64,
    pub cache_scope_stage_hits: BTreeMap<String, BTreeMap<String, u64>>,
    pub cache_scope_stage_misses: BTreeMap<String, BTreeMap<String, u64>>,
    pub cache_scope_stage_invalidations: BTreeMap<String, BTreeMap<String, u64>>,
    /// Aggregated flat_full tier hits across L0/L1/L2 (always populated when flatten cache runs).
    pub flat_full_cache_hits: u64,
    pub flat_full_cache_misses: u64,
    pub flat_full_cache_writes: u64,
    pub inline_us: u64,
    /// Aggregate time in `substitute_expr` during inline (`rewrite` / record dot extraction).
    pub inline_substitute_us: u64,
    pub inline_substitute_ms: u64,
    /// Time in `load_model` invoked from inline paths (cache misses only).
    pub inline_load_model_us: u64,
    pub inline_load_model_ms: u64,
    /// `Call(...)` nodes visited by `inline_expr`.
    pub inline_call_sites: u64,
    /// Successful single-output function body substitutions from a direct `Call`.
    pub inline_single_output_inlines: u64,
    /// Wall time per inline pass over flattened model slices (microseconds).
    pub inline_pass_decl_start_values_us: u64,
    pub inline_pass_decl_start_values_ms: u64,
    pub inline_pass_equations_us: u64,
    pub inline_pass_equations_ms: u64,
    pub inline_pass_initial_equations_us: u64,
    pub inline_pass_initial_equations_ms: u64,
    pub inline_pass_algorithms_us: u64,
    pub inline_pass_algorithms_ms: u64,
    pub inline_pass_initial_algorithms_us: u64,
    pub inline_pass_initial_algorithms_ms: u64,
    /// Candidate resolution metrics for function inlining.
    pub inline_resolve_calls: u64,
    pub inline_resolve_first_hit: u64,
    pub inline_resolve_candidates_total: u64,
    pub inline_resolve_probes_total: u64,
    pub inline_resolve_probe_1: u64,
    pub inline_resolve_probe_2: u64,
    pub inline_resolve_probe_3: u64,
    pub inline_resolve_probe_4: u64,
    pub inline_resolve_probe_ge5: u64,
    /// Flat model sizes at inline entry (same as `perf_record_add` snapshot keys).
    pub inline_input_declarations: usize,
    pub inline_input_equations: usize,
    pub inline_input_initial_equations: usize,
    pub inline_input_algorithms: usize,
    pub inline_input_initial_algorithms: usize,
    pub inline_declarations_with_start_value: usize,
    /// Whether inline frontend parallel PoC path was enabled for this compile.
    pub inline_parallel_poc_enabled: bool,
    /// Whether flatten-stage parallel PoC path was enabled for this compile.
    pub flatten_parallel_poc_enabled: bool,
    /// Eq-expand guard observability counters.
    pub guard_cooldown_enter: u64,
    pub guard_cooldown_active: u64,
    pub guard_cooldown_exit: u64,
    /// Human-readable last guard reason for this compile.
    pub guard_reason: String,
    pub analyze_ms: u64,
    pub backend_dae_ms: u64,
    pub external_resolve_ms: u64,
    /// Microseconds spent collecting raw external call sites before resolve.
    #[serde(default)]
    pub external_resolve_gather_us: u64,
    /// Microseconds spent in SQLite lookup for external resolve cache.
    #[serde(default)]
    pub external_resolve_lookup_us: u64,
    /// Microseconds spent resolving externals (loader / compute path).
    #[serde(default)]
    pub external_resolve_compute_us: u64,
    /// Microseconds spent persisting external resolve cache entry.
    #[serde(default)]
    pub external_resolve_store_us: u64,
    /// `hit` | `put` | `miss_compute` | `disabled` | `no_cache_dir` | `err_deser`
    pub external_resolve_cache_status: String,
    /// `hit` | `put` | `miss` | `disabled` | `no_cache_dir` | `err_deser`
    pub analysis_summary_cache_status: String,
    /// Classify + analyze SQLite cache: `not_run` | `disk_hit` | `disk_put` | `miss_compute` | `disabled` | `no_cache_dir` | `err_ser` | `err_deser`
    pub analysis_pipeline_cache_status: String,
    /// `backend_dae_v1` SQLite cache: `not_run` | `disk_hit` | `disk_put` | `miss_compute` | `disabled` | `no_cache_dir` | `err_deser`
    pub backend_dae_cache_status: String,
    /// End-to-end sim bundle: `hit` | `miss` | `disabled` | `skipped_stubs` | `skipped_codegen_disk`
    pub artifact_bundle_cache_status: String,
    /// Weighted cache warm ratio: 0.0 = fully cold, 1.0 = all stages cached.
    /// Computed from per-phase cache hit status with approximate cost weights.
    pub cache_warm_ratio: f64,
    /// Last artifact/codegen fast-path miss reason (e.g. `key_not_found`, `deps_changed`, `codegen_cache_miss`).
    pub cache_miss_reason: Option<String>,
    /// True when end-to-end artifact or sim-bundle structural reuse skipped JIT.
    pub structural_cache_hit: bool,
    /// True when only parameter vector was swapped (e.g. `reuse_compiled_with_new_params`).
    pub param_only_update: bool,
    /// When a full JIT compile ran, optional reason label.
    pub full_recompile_reason: Option<String>,
    /// L3-T04: aggregate count of structural cache hits (artifact or sim-bundle).
    pub cache_structural_hit_count: u32,
    /// L3-T04: aggregate count of param-only updates (no recompile).
    pub cache_param_update_count: u32,
    /// L3-T04: aggregate count of full recompiles (cold path).
    pub cache_full_recompile_count: u32,
    /// L4-T08: structured cache miss events collected across all cache layers.
    pub cache_miss_events: Vec<CacheMissEvent>,
    /// Fine-grained external-resolve miss detail when cache misses (L4-T08 / section 6.8-2).
    pub external_resolve_miss_detail: Option<String>,
    /// User function stub candidates collected from call graph.
    pub stub_candidate_count: usize,
    /// Wall-clock spent building user stubs.
    pub stub_compile_ms: u64,
    pub stub_compile_us: u64,
    /// Whether stub parallel prototype was enabled for this compile.
    pub stub_parallel_enabled: bool,
    /// Wall-clock spent scanning/building clock partition schedule.
    pub clock_partition_scan_ms: u64,
    pub clock_partition_scan_us: u64,
    /// Whether partition-scan parallel prototype was enabled for this compile.
    pub clock_partition_parallel_enabled: bool,
    /// Candidate parallel share estimate (0-100) from frontend-inline + stub/partition segments over total compile wall.
    pub parallel_candidate_share_pct: f64,
    /// Track B: wall time for Cranelift JIT compile (`jit.compile`), microseconds (pair with `jit_ms`).
    pub codegen_wall_us: u64,
    pub codegen_wall_ms: u64,
    pub jit_ms: u64,
    pub state_count: usize,
    pub discrete_count: usize,
    pub param_count: usize,
    pub alg_eq_count: usize,
    pub diff_eq_count: usize,
    pub blt_degrade_guard_triggered: bool,
    pub blt_degrade_guard_limit: Option<usize>,
    pub blt_degrade_guard_equation_count: Option<usize>,
    pub symbolic_index_signal_count: usize,
    pub implicit_derivative_constraint_count: usize,
    pub aot_cache_status: String,
    pub jit_compile_ok: bool,
    pub jit_error: Option<String>,
    pub fallback_total: u64,
    pub fallback_jit_builtin: u64,
    pub fallback_jit_variable: u64,
    pub fallback_jit_derivative: u64,
    pub fallback_jit_equation_skip: u64,
    pub fallback_jit_multi_assign: u64,
    pub fallback_newton_init_accept: u64,
    pub fallback_newton_event_accept: u64,
    pub fallback_clock_degrade: u64,
    pub adaptive_profile: String,
    pub adaptive_override_count: usize,
    pub adaptive_warning_count: usize,
    pub jit_incremental_enabled: bool,
    pub jit_cache_variant: String,
    pub jit_cache_partial_recompile: bool,
    pub jit_cache_skipped_functions: u64,
    pub jit_cache_recompiled_functions: u64,
    pub const_fold_enabled: bool,
    pub eq_dce_enabled: bool,
    pub const_fold_count: u64,
    pub eq_dce_removed: u64,
    #[serde(default, skip_serializing_if = "serde_skip_false")]
    pub const_fold_skipped_by_policy: bool,
    #[serde(default, skip_serializing_if = "serde_skip_false")]
    pub eq_dce_skipped_by_policy: bool,
    #[serde(default, skip_serializing_if = "serde_skip_false")]
    pub const_fold_cooldown_active: bool,
    #[serde(default, skip_serializing_if = "serde_skip_false")]
    pub jit_bypassed_tier0: bool,
    #[serde(default, skip_serializing_if = "serde_skip_false")]
    pub warmup_auto_enqueued: bool,
    /// Snapshot from last background warmup run (best-effort; set at compile start).
    #[serde(default)]
    pub warmup_populated_count: u32,
    /// Heuristic: prior warmup `ok` count surfaced as attributable signal.
    #[serde(default)]
    pub warmup_attributable_hits: u64,
    #[serde(default)]
    pub warmup_time_ms: u64,
    /// Approximate flat-full cache read path (get + deserialize), microseconds.
    #[serde(default)]
    pub salsa_flat_full_get_us: u64,
    /// True when process-local Salsa DB was reused for this compile's flatten query path.
    #[serde(default)]
    pub salsa_process_db_hit: bool,
    /// True when reused Salsa session carried the same codegen stable hash as this compile.
    #[serde(default)]
    pub salsa_codegen_reuse_eligible: bool,
    #[serde(default)]
    pub flatten_inline_subst_rewrite_us: u64,
    #[serde(default)]
    pub flatten_inline_resolve_us: u64,
    /// Number of parameter names participating in folded expressions.
    pub const_fold_param_count: usize,
    /// Comma-joined parameter names touched by folded expressions.
    pub const_fold_param_names: String,
    pub jit_inline_builtins_enabled: bool,
    pub jit_inline_builtin_hits: u64,
    pub hotspot_eval_count: u64,
    pub hotspot_threshold: u64,
    pub simd_step_enabled: bool,
    pub simd_step_hits: u64,
    pub simd_step_fallbacks: u64,
    /// Newton SolvableBlock path counters (filled after JIT translation).
    #[serde(default)]
    pub newton_sparse_blocks: u64,
    #[serde(default)]
    pub newton_dense_blocks: u64,
    #[serde(default)]
    pub newton_sparse_nnz_total: u64,
    /// Effective Newton sparse policy string: auto|dense|sparse.
    #[serde(default)]
    pub newton_sparse_policy: String,
    /// Background tier-up step threshold used / recommended for this model size.
    #[serde(default)]
    pub tierup_step_threshold: u64,
    pub type_specialization_enabled: bool,
    pub type_profile_hash: String,
    pub stack_scratch_enabled: bool,
    pub runtime_boundary_epoch: u64,
    /// Leyden condenser metrics.
    pub condenser_total_elapsed_us: u64,
    pub condenser_artifacts_written: u32,
    pub condenser_cache_hits: u32,
    pub condenser_errors: u32,
    /// Active compilation tier (from tiered compilation scheduler).
    pub compile_tier: String,
    /// Whether a training-run profile was used for this compilation.
    pub profile_guided: bool,
    /// Number of active speculative guards.
    pub speculation_guard_count: u32,
    /// Number of speculation invalidations during this session.
    pub speculation_invalidation_count: u32,
    /// CLI / options: dual speculative+generic compile requested.
    pub dual_compile_requested: bool,
    /// True when `dual_compile` produced both paths successfully.
    pub dual_compile_ok: bool,
    /// Active speculations count recorded for dual-compile (or 0).
    pub dual_compile_speculation_count: u32,
    /// `off` | `not_run` | `attempted` | `ok` | `failed` | `skipped_no_guards` | `skipped_aot_native`.
    pub dual_compile_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dual_compile_error: Option<DualCompileErrorReport>,
    /// AOT archive native fast path: `not_eligible` | `no_blob` | `loaded` | `load_failed` | `wrong_key` | `unsupported_os`.
    pub aot_native_load_status: String,
    /// Optional detail for AOT native load source/path (e.g. `toc_match` / `key_fallback`).
    pub aot_native_load_detail: Option<String>,
}

impl CompilePerfReport {
    pub const DUAL_COMPILE_STATUS_OFF: &'static str = "off";
    pub const DUAL_COMPILE_STATUS_NOT_RUN: &'static str = "not_run";
    pub const DUAL_COMPILE_STATUS_ATTEMPTED: &'static str = "attempted";
    pub const DUAL_COMPILE_STATUS_OK: &'static str = "ok";
    pub const DUAL_COMPILE_STATUS_FAILED: &'static str = "failed";
    pub const DUAL_COMPILE_STATUS_SKIPPED_NO_GUARDS: &'static str = "skipped_no_guards";
    pub const DUAL_COMPILE_STATUS_SKIPPED_AOT_NATIVE: &'static str = "skipped_aot_native";
    pub const DUAL_COMPILE_ERROR_DETAIL_MAX_LEN: usize = 256;

    pub const AOT_NATIVE_STATUS_NOT_ELIGIBLE: &'static str = "not_eligible";
    pub const AOT_NATIVE_STATUS_NO_MATCHING_ENTRY: &'static str = "no_matching_entry";
    pub const AOT_NATIVE_STATUS_NO_BLOB: &'static str = "no_blob";
    pub const AOT_NATIVE_STATUS_LOADED: &'static str = "loaded";
    pub const AOT_NATIVE_STATUS_LOAD_FAILED: &'static str = "load_failed";
    pub const AOT_NATIVE_STATUS_UNSUPPORTED_OS: &'static str = "unsupported_os";
    pub const AOT_NATIVE_STATUS_WRONG_KEY: &'static str = "wrong_key";
    pub const AOT_NATIVE_STATUS_MODEL_NOT_IN_ARCHIVE: &'static str = "model_not_in_archive";
    pub const AOT_NATIVE_STATUS_ARCHIVE_READ_FAILED: &'static str = "archive_read_failed";
    pub const AOT_NATIVE_STATUS_NO_ARCHIVE_FILE: &'static str = "no_archive_file";
    pub const AOT_NATIVE_STATUS_NO_ARCHIVE_PATH: &'static str = "no_archive_path";
    pub const AOT_NATIVE_STATUS_DISABLED_BY_ENV: &'static str = "disabled_by_env";

    /// AOT native load detail: archive TOC entry matched model+key.
    pub const AOT_NATIVE_DETAIL_TOC_MATCH: &'static str = "toc_match";
    /// AOT native load detail: fallback to direct key lookup in archive map.
    pub const AOT_NATIVE_DETAIL_KEY_FALLBACK: &'static str = "key_fallback";

    /// Compute a weighted cache warm ratio from per-phase cache hit flags.
    /// Weights reflect approximate contribution of each phase to cold compile time
    /// for typical medium/large models.
    pub fn compute_warm_ratio(&self, jit_skipped: bool) -> f64 {
        // Phase weights (sum = 1.0):
        //   flatten/inline: 0.40, JIT/codegen: 0.35, external_resolve: 0.10,
        //   analysis_pipeline: 0.10, backend_dae: 0.05
        let mut w = 0.0_f64;
        if self.flat_full_cache_hits > 0 {
            w += 0.40;
        }
        if jit_skipped {
            w += 0.35;
        }
        if self.external_resolve_cache_status == "hit" {
            w += 0.10;
        }
        if self.analysis_pipeline_cache_status == "disk_hit" {
            w += 0.10;
        }
        if self.backend_dae_cache_status == "disk_hit" {
            w += 0.05;
        }
        w.clamp(0.0, 1.0)
    }

    pub fn truncate_dual_compile_error_detail(e: &str) -> String {
        e.chars()
            .take(Self::DUAL_COMPILE_ERROR_DETAIL_MAX_LEN)
            .collect()
    }
}

impl Default for CompilerOptions {
    fn default() -> Self {
        CompilerOptions {
            jit_leak: false,
            backend_dae_info: false,
            index_reduction_method: "pantelides".to_string(),
            tearing_method: "first".to_string(),
            generate_dynamic_jacobian: "none".to_string(),
            t_end: 10.0,
            dt: 0.01,
            atol: 1e-6,
            rtol: 1e-3,
            function_args: None,
            solver: "rk45".to_string(),
            output_interval: 0.05,
            result_file: None,
            warnings_level: "all".to_string(),
            emit_c_dir: None,
            emit_coder_package: false,
            external_libs: Vec::new(),
            quiet: false,
            validate_only: false,
            emit_flat_snapshot: None,
            flat_snapshot_only: false,
            coarse_constrainedby_only: false,
            array_size_policy: "legacy".to_string(),
            array_sizes_json: None,
            jit_policy_json: None,
            validation_mode: "full".to_string(),
            compile_stop: CompileStopPhase::Full,
            dual_compile: false,
            warm_background: false,
        }
    }
}
