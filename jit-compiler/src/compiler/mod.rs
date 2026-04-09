mod c_codegen;
mod equation_convert;
mod initial_conditions;
pub(crate) mod inline;
pub(crate) mod adaptive;
mod jacobian;
mod pipeline;
mod compile_model;
mod solvable_scale_warn;

use std::collections::{HashMap, HashSet};
use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::analysis::ProvenanceIndex;
use crate::ast::{AlgorithmStatement, Equation, Expression, Model};
use crate::backend_dae::ClockPartition as BackendClockPartition;
use crate::diag::WarningInfo;
use crate::equation_graph;
use crate::equation_graph::EquationGraphMode;
use crate::expr_eval;
use crate::flatten::{ArraySizePolicy, Flattener};
use crate::jit::{CalcDerivsFunc, Jit};
use crate::loader::ModelLoader;
use pipeline::{flatten_and_inline, stage_trace_enabled};
pub use pipeline::geometric_default_for_name;

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
    /// DBG-3: Warnings level: "all" | "none" | "error" (none = suppress, error = treat as error).
    pub warnings_level: String,
    /// CG1-1: When set, emit C source (model.c, model.h) to this directory.
    pub emit_c_dir: Option<String>,
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
    /// 1.0 when sim-bundle skipped JIT entirely; else 0.0 (coarse metric).
    pub cache_warm_ratio: f64,
    /// Last artifact/codegen fast-path miss reason (e.g. `key_not_found`, `deps_changed`, `codegen_cache_miss`).
    pub cache_miss_reason: Option<String>,
    /// True when end-to-end artifact or sim-bundle structural reuse skipped JIT.
    pub structural_cache_hit: bool,
    /// True when only parameter vector was swapped (e.g. `reuse_compiled_with_new_params`).
    pub param_only_update: bool,
    /// When a full JIT compile ran, optional reason label.
    pub full_recompile_reason: Option<String>,
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
    pub type_specialization_enabled: bool,
    pub type_profile_hash: String,
    pub stack_scratch_enabled: bool,
    pub runtime_boundary_epoch: u64,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        CompilerOptions {
            backend_dae_info: false,
            index_reduction_method: "none".to_string(),
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
        }
    }
}

/// EXT-1/EXT-2: Keeps loaded libraries alive; resolved symbol pointers for JIT.
pub(crate) struct ExternalLibs(pub Vec<libloading::Library>);

pub struct Compiler {
    pub loader: ModelLoader,
    pub options: CompilerOptions,
    pub(crate) warnings: Vec<WarningInfo>,
    pub(crate) external_libraries: ExternalLibs,
    /// EXT-2: Resolved external function symbols (modelica_name -> ptr); valid while external_libraries is alive.
    pub(crate) external_symbol_ptrs: HashMap<String, *const u8>,
    /// Global string interner for variable name deduplication across compilation stages.
    pub interner: crate::string_intern::StringInterner,
    /// Structured compile-time performance report for the last compile call.
    pub last_compile_perf: Option<CompilePerfReport>,
    /// Equation/component provenance for the last successful flatten+inline (same flat as last compile).
    pub last_provenance_index: Option<Arc<ProvenanceIndex>>,
    /// Deferred strict-mode artifact write to flush on explicit confirmation.
    pub(crate) deferred_artifact: Option<DeferredArtifactWrite>,
}

impl Default for ExternalLibs {
    fn default() -> Self {
        ExternalLibs(Vec::new())
    }
}

/// EXT-4: Parse annotation string for Library="..." (e.g. annotation(Library="mylib")).
fn parse_annotation_library(annotation: Option<&String>) -> Option<String> {
    let s = annotation.as_ref()?.as_str();
    let s = s.trim();
    let idx = s.find("Library")?;
    let rest = s[idx + 7..].trim_start();
    let rest = rest
        .strip_prefix('=')
        .map(|r| r.trim_start())
        .unwrap_or(rest);
    let start = rest.find('"')?;
    let rest = &rest[start + 1..];
    let end = rest.find('"')?;
    let lib = rest[..end].trim();
    if lib.is_empty() {
        None
    } else {
        Some(lib.to_string())
    }
}

/// FUNC-2: Collect all function names called from equations and algorithms (after inlining).
pub(super) fn collect_all_called_names(
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
) -> HashSet<String> {
    let mut names = HashSet::new();
    fn normalize_called_name(name: &str) -> Option<String> {
        match name {
            "valveCharacteristic" => {
                Some("Modelica.Fluid.Valves.BaseClasses.ValveCharacteristics.linear".to_string())
            }
            "regRoot" | "regRoot2" | "regSquare2" | "regFun3" | "regStep" | "spliceFunction" => {
                Some(format!("Modelica.Fluid.Utilities.{name}"))
            }
            "arg" => Some("Modelica.ComplexMath.arg".to_string()),
            "distribution" => Some("Modelica.Blocks.Noise.Interfaces.distribution".to_string()),
            "oneTrue" => Some("Modelica.Electrical.Batteries.Utilities.oneTrue".to_string()),
            "isPowerOf2" => Some("Modelica.Electrical.Polyphase.Functions.isPowerOf2".to_string()),
            "numberOfSymmetricBaseSystems" => {
                Some("Modelica.Electrical.Polyphase.Functions.numberOfSymmetricBaseSystems".to_string())
            }
            "factorY2DC" => Some("Modelica.Electrical.Polyphase.Functions.factorY2DC".to_string()),
            "exlin" => Some("Modelica.Electrical.Analog.Semiconductors.exlin".to_string()),
            "exlin2" => Some("Modelica.Electrical.Analog.Semiconductors.exlin2".to_string()),
            _ => None,
        }
    }
    fn collect_calls_expr(expr: &Expression, out: &mut HashSet<String>) {
        match expr {
            Expression::Call(name, _) => {
                out.insert(name.clone());
                if let Some(normalized) = normalize_called_name(name) {
                    out.insert(normalized);
                }
            }
            Expression::BinaryOp(l, _, r) => {
                collect_calls_expr(l, out);
                collect_calls_expr(r, out);
            }
            Expression::Der(inner) => collect_calls_expr(inner, out),
            Expression::If(c, t, e) => {
                collect_calls_expr(c, out);
                collect_calls_expr(t, out);
                collect_calls_expr(e, out);
            }
            Expression::ArrayAccess(a, i) => {
                collect_calls_expr(a, out);
                collect_calls_expr(i, out);
            }
            Expression::Sample(inner)
            | Expression::Interval(inner)
            | Expression::Hold(inner)
            | Expression::Previous(inner) => collect_calls_expr(inner, out),
            Expression::SubSample(c, n)
            | Expression::SuperSample(c, n)
            | Expression::ShiftSample(c, n)
            | Expression::BackSample(c, n) => {
                collect_calls_expr(c, out);
                collect_calls_expr(n, out);
            }
            _ => {}
        }
    }
    fn collect_calls_eq(eq: &Equation, out: &mut HashSet<String>) {
        match eq {
            Equation::Simple(lhs, rhs) => {
                collect_calls_expr(lhs, out);
                collect_calls_expr(rhs, out);
            }
            Equation::CallStmt(expr) => {
                collect_calls_expr(expr, out);
            }
            Equation::SolvableBlock {
                equations,
                residuals,
                ..
            } => {
                for e in equations {
                    if let Equation::Simple(l, r) = e {
                        collect_calls_expr(l, out);
                        collect_calls_expr(r, out);
                    }
                }
                for r in residuals {
                    collect_calls_expr(r, out);
                }
            }
            _ => {}
        }
    }
    fn collect_calls_alg(stmt: &AlgorithmStatement, out: &mut HashSet<String>) {
        match stmt {
            AlgorithmStatement::Assignment(_, rhs) => collect_calls_expr(rhs, out),
            AlgorithmStatement::MultiAssign(_, rhs) => collect_calls_expr(rhs, out),
            AlgorithmStatement::CallStmt(expr) => collect_calls_expr(expr, out),
            AlgorithmStatement::NoOp => {}
            AlgorithmStatement::Break => {}
            AlgorithmStatement::Return(v) => {
                if let Some(expr) = v {
                    collect_calls_expr(expr, out);
                }
            }
            AlgorithmStatement::If(cond, t, eifs, els) => {
                collect_calls_expr(cond, out);
                for s in t {
                    collect_calls_alg(s, out);
                }
                for (c, b) in eifs {
                    collect_calls_expr(c, out);
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
                if let Some(b) = els {
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::For(_, range, body) => {
                collect_calls_expr(range, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
            }
            AlgorithmStatement::When(cond, body, elses) => {
                collect_calls_expr(cond, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
                for (c, b) in elses {
                    collect_calls_expr(c, out);
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::Reinit(_, e) => collect_calls_expr(e, out),
            AlgorithmStatement::Assert(cond, msg) => {
                collect_calls_expr(cond, out);
                collect_calls_expr(msg, out);
            }
            AlgorithmStatement::Terminate(msg) => collect_calls_expr(msg, out),
            AlgorithmStatement::While(cond, body) => {
                collect_calls_expr(cond, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
            }
        }
    }
    for eq in alg_equations.iter().chain(diff_equations) {
        collect_calls_eq(eq, &mut names);
    }
    for stmt in algorithms {
        collect_calls_alg(stmt, &mut names);
    }
    names
}

/// Raw call-site names from equations/algorithms (before external resolution / loader work).
pub(crate) fn collect_external_raw_call_sites(
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
) -> HashSet<String> {
    let mut names = HashSet::new();
    fn collect_calls_expr(expr: &Expression, out: &mut HashSet<String>) {
        match expr {
            Expression::Call(name, _) => {
                out.insert(name.clone());
            }
            Expression::BinaryOp(l, _, r) => {
                collect_calls_expr(l, out);
                collect_calls_expr(r, out);
            }
            Expression::Der(inner) => collect_calls_expr(inner, out),
            Expression::If(c, t, e) => {
                collect_calls_expr(c, out);
                collect_calls_expr(t, out);
                collect_calls_expr(e, out);
            }
            Expression::ArrayAccess(a, i) => {
                collect_calls_expr(a, out);
                collect_calls_expr(i, out);
            }
            Expression::Sample(inner)
            | Expression::Interval(inner)
            | Expression::Hold(inner)
            | Expression::Previous(inner) => collect_calls_expr(inner, out),
            Expression::SubSample(c, n)
            | Expression::SuperSample(c, n)
            | Expression::ShiftSample(c, n)
            | Expression::BackSample(c, n) => {
                collect_calls_expr(c, out);
                collect_calls_expr(n, out);
            }
            _ => {}
        }
    }
    fn collect_calls_eq(eq: &Equation, out: &mut HashSet<String>) {
        match eq {
            Equation::Simple(lhs, rhs) => {
                collect_calls_expr(lhs, out);
                collect_calls_expr(rhs, out);
            }
            Equation::SolvableBlock {
                equations,
                residuals,
                ..
            } => {
                for e in equations {
                    if let Equation::Simple(l, r) = e {
                        collect_calls_expr(l, out);
                        collect_calls_expr(r, out);
                    }
                }
                for r in residuals {
                    collect_calls_expr(r, out);
                }
            }
            _ => {}
        }
    }
    for eq in alg_equations.iter().chain(diff_equations) {
        collect_calls_eq(eq, &mut names);
    }
    fn collect_calls_alg(stmt: &AlgorithmStatement, out: &mut HashSet<String>) {
        match stmt {
            AlgorithmStatement::Assignment(_, rhs) => collect_calls_expr(rhs, out),
            AlgorithmStatement::MultiAssign(_, rhs) => collect_calls_expr(rhs, out),
            AlgorithmStatement::CallStmt(expr) => collect_calls_expr(expr, out),
            AlgorithmStatement::NoOp => {}
            AlgorithmStatement::Break => {}
            AlgorithmStatement::Return(v) => {
                if let Some(expr) = v {
                    collect_calls_expr(expr, out);
                }
            }
            AlgorithmStatement::If(cond, t, eifs, els) => {
                collect_calls_expr(cond, out);
                for s in t {
                    collect_calls_alg(s, out);
                }
                for (c, b) in eifs {
                    collect_calls_expr(c, out);
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
                if let Some(b) = els {
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::For(_, range, body) => {
                collect_calls_expr(range, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
            }
            AlgorithmStatement::When(cond, body, elses) => {
                collect_calls_expr(cond, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
                for (c, b) in elses {
                    collect_calls_expr(c, out);
                    for s in b {
                        collect_calls_alg(s, out);
                    }
                }
            }
            AlgorithmStatement::Reinit(_, e) => collect_calls_expr(e, out),
            AlgorithmStatement::Assert(cond, msg) => {
                collect_calls_expr(cond, out);
                collect_calls_expr(msg, out);
            }
            AlgorithmStatement::Terminate(msg) => collect_calls_expr(msg, out),
            AlgorithmStatement::While(cond, body) => {
                collect_calls_expr(cond, out);
                for s in body {
                    collect_calls_alg(s, out);
                }
            }
        }
    }
    for stmt in algorithms {
        collect_calls_alg(stmt, &mut names);
    }
    names
}

/// EXT-2: Collect (modelica_name, c_name, library_hint) for functions that have external_info and are called from eqs/alg.
/// EXT-4: library_hint is from annotation(Library="...").
pub(super) fn collect_external_calls(
    loader: &mut ModelLoader,
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
) -> Vec<(String, String, Option<String>)> {
    let names = collect_external_raw_call_sites(alg_equations, diff_equations, algorithms);
    let mut out = Vec::new();
    for call_site in names {
        // Avoid mis-classifying builtin operators as external calls.
        // In particular, the unqualified TestLib.* fallback below would otherwise map
        // `sample(...)` / clock-derived operators to TestLib stubs (if present) and force
        // the JIT down the generic Import path, which can panic on missing symbols.
        let lower = call_site.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "sample"
                | "interval"
                | "subsample"
                | "supersample"
                | "shiftsample"
                | "backsample"
                | "hold"
                | "previous"
                | "ones"
                | "zeros"
                | "fill"
                | "pre"
                | "edge"
                | "change"
                | "assert"
                | "terminate"
                | "size"
        ) {
            continue;
        }
        // Do not skip on `is_builtin_function` here: that helper treats any
        // package-qualified name with an uppercase first segment (e.g. TestLib.*)
        // as a "builtin", which would drop real external declarations from EXT
        // collection and let JIT fall through to namespace passthrough (wrong).
        let mut resolved: Option<Arc<Model>> = None;
        if let Ok(m) = loader.load_model_silent(&call_site, true) {
            if m.is_function && m.external_info.is_some() {
                resolved = Some(m);
            }
        }
        if resolved.is_none() && !call_site.contains('.') {
            let q = format!("TestLib.{call_site}");
            if let Ok(m) = loader.load_model_silent(&q, true) {
                if m.is_function && m.external_info.is_some() {
                    resolved = Some(m);
                }
            }
        }
        let Some(model) = resolved else {
            continue;
        };
        if let Some(ref ext) = model.external_info {
            let default_c = call_site
                .rsplit_once('.')
                .map(|(_, t)| t)
                .unwrap_or(call_site.as_str());
            let c_name = ext.c_name.as_deref().unwrap_or(default_c).to_string();
            let lib_hint = parse_annotation_library(model.annotation.as_ref());
            out.push((call_site, c_name, lib_hint));
        }
    }
    out
}

/// Result of compilation: either simulation artifacts or a single function result (F3-1).
pub enum CompileOutput {
    Simulation(Artifacts),
    FunctionRun(f64),
    /// Flat Tier S snapshot written; compilation stopped before analysis/JIT.
    FlatSnapshotDone,
    /// Stopped after load/parse; no flatten.
    ValidationParseOk,
    /// Stopped after flatten; no variable analysis or JIT.
    ValidationFlattenOk {
        total_equations: usize,
        total_declarations: usize,
    },
    /// Stopped after analysis; no external stub resolution or JIT.
    ValidationAnalyzed(ValidationAnalyzedSummary),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockPartitionTrigger {
    Always,
    Sample { start: f64, interval: f64 },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClockPartitionScheduleEntry {
    pub id: String,
    pub trigger: ClockPartitionTrigger,
    pub var_names: Vec<String>,
    pub algorithm_indices: Vec<usize>,
    pub alg_equation_indices: Vec<usize>,
    pub diff_equation_indices: Vec<usize>,
}

pub struct Artifacts {
    pub calc_derivs: CalcDerivsFunc,
    pub states: Vec<f64>,
    pub discrete_vals: Vec<f64>,
    pub params: Vec<f64>,
    pub state_vars: Vec<String>,
    pub param_vars: Vec<String>,
    pub discrete_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub output_start_vals: Vec<f64>,
    pub state_var_index: HashMap<String, usize>,
    /// SYNC-2: Clock partitions for event/solver (e.g. clocked state handling).
    #[allow(dead_code)]
    pub clock_partitions: Vec<BackendClockPartition>,
    pub clock_partition_schedule: Vec<ClockPartitionScheduleEntry>,
    pub when_count: usize,
    pub crossings_count: usize,
    pub t_end: f64,
    pub dt: f64,
    pub numeric_ode_jacobian: bool,
    pub symbolic_ode_jacobian: Option<Vec<Vec<Expression>>>,
    /// Tearing variable names per SolvableBlock (for Newton failure diagnostics).
    pub newton_tearing_var_names: Vec<String>,
    pub atol: f64,
    pub rtol: f64,
    /// Backend DAE differential index (for IDA / warnings).
    pub differential_index: u32,
    /// IDA `IDASetId` vector; same length as `states`, from `SimulationDae` layout.
    pub ida_component_id: Vec<f64>,
    pub solver: String,
    pub output_interval: f64,
    pub result_file: Option<String>,
    /// FUNC-2: Keep user function stub JITs alive so calc_derivs call targets remain valid.
    #[allow(dead_code)]
    pub user_stub_jits: Vec<Jit>,
    /// Keeps disk-backed `calc_derivs` memory valid when returning from sim-bundle fast path.
    #[allow(dead_code)]
    pub(crate) calc_derivs_codegen_keepalive: Option<Box<crate::jit::codegen_cache::CachedFunction>>,
    /// Set by `reuse_compiled_with_new_params` when only the parameter vector was replaced.
    pub param_only_update: bool,
}

include!("compiler_impl.rs");
