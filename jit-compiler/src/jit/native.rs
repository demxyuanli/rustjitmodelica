use cranelift_jit::JITBuilder;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::sparse_solve::CsrMatrix;

// F4-4: assert/terminate simulation behavior
static TERMINATE_REQUESTED: AtomicBool = AtomicBool::new(false);
static ASSERT_HIT_COUNT: AtomicU64 = AtomicU64::new(0);
static ASSERT_PRINTED_COUNT: AtomicU64 = AtomicU64::new(0);
static ASSERT_SUPPRESS: AtomicBool = AtomicBool::new(false);
const ASSERT_PRINT_LIMIT: u64 = 32;
static NEWTON_DUAL_VALIDATE_ENABLED: OnceLock<bool> = OnceLock::new();
static NEWTON_SPARSE_DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();
static ASSERT_TRACE_ENABLED: OnceLock<bool> = OnceLock::new();
static TABLE_ID_ALLOC: AtomicU64 = AtomicU64::new(1);
static EXTERNAL_OBJECT_ID_ALLOC: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Default)]
struct TableData {
    time_col: Vec<f64>,
    data_cols: Vec<Vec<f64>>,
}

fn table_registry() -> &'static Mutex<HashMap<i64, TableData>> {
    static REG: OnceLock<Mutex<HashMap<i64, TableData>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

fn external_object_registry() -> &'static Mutex<HashMap<i64, i64>> {
    static REG: OnceLock<Mutex<HashMap<i64, i64>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            t == "1" || t == "true" || t == "on" || t == "yes"
        })
        .unwrap_or(false)
}

fn dual_validate_enabled() -> bool {
    *NEWTON_DUAL_VALIDATE_ENABLED.get_or_init(|| env_flag("RUSTMODLICA_NEWTON_DUAL_VALIDATE"))
}

fn sparse_debug_enabled() -> bool {
    *NEWTON_SPARSE_DEBUG_ENABLED.get_or_init(|| env_flag("RUSTMODLICA_NEWTON_SPARSE_DEBUG"))
}

fn assert_trace_enabled() -> bool {
    *ASSERT_TRACE_ENABLED.get_or_init(|| env_flag("RUSTMODLICA_ASSERT_TRACE"))
}

#[allow(clippy::cast_possible_truncation)]
extern "C" fn modelica_assert(cond: f64, msg: f64) {
    if cond == 0.0 {
        if ASSERT_SUPPRESS.load(Ordering::Relaxed) {
            return;
        }
        let hit_idx = ASSERT_HIT_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
        let printed = ASSERT_PRINTED_COUNT.load(Ordering::SeqCst);
        if assert_trace_enabled() && printed < ASSERT_PRINT_LIMIT {
            ASSERT_PRINTED_COUNT.fetch_add(1, Ordering::SeqCst);
            eprintln!("Assertion failed: {}", msg);
            if hit_idx == ASSERT_PRINT_LIMIT {
                eprintln!(
                    "Assertion print limit reached ({}); suppressing further assert logs.",
                    ASSERT_PRINT_LIMIT
                );
            }
        }
    }
}

extern "C" fn rustmodlica_assert_suppress_begin() {
    ASSERT_SUPPRESS.store(true, Ordering::Relaxed);
}

extern "C" fn rustmodlica_assert_suppress_end() {
    ASSERT_SUPPRESS.store(false, Ordering::Relaxed);
}

extern "C" fn modelica_terminate(_msg: f64) {
    TERMINATE_REQUESTED.store(true, Ordering::SeqCst);
}

/// FUNC-7 / EXT-3: Host symbol for `external "C"` functions whose C name is `extLog`
/// (`input String` -> `const char*`, `output Real` -> `double`). TestLib uses this pattern.
/// Returns 0.0; `--external-lib` / extra JIT symbols can supply a real implementation.
extern "C" fn rustmodlica_builtin_ext_log(_msg: *const std::ffi::c_char) -> f64 {
    0.0
}

/// TestLib `printStringExternal`: C name `rustmodlica_print_string` (see TestLib/Resources).
/// Registered under the Modelica call-site name via `jit_stub_for_external_c_name` + compile_model.
extern "C" fn rustmodlica_print_string_jit_stub(_msg: *const std::ffi::c_char) -> f64 {
    0.0
}

/// TestLib `sumArrayExternal`: `double rustmodlica_sum_array(const double*, double n)`.
#[allow(clippy::cast_precision_loss)]
extern "C" fn rustmodlica_sum_array_jit_stub(arr: *const f64, n: f64) -> f64 {
    if arr.is_null() || n <= 0.0 {
        return 0.0;
    }
    let len = n as usize;
    let s = unsafe { std::slice::from_raw_parts(arr, len) };
    let sum: f64 = s.iter().sum();
    if std::env::var("RUSTMODLICA_JIT_SUM_ARRAY_TRACE")
        .ok()
        .map(|v| v.trim() == "1")
        .unwrap_or(false)
    {
        eprintln!(
            "[jit-sum-array-stub] n={} len={} sum={} first={:?}",
            n,
            len,
            sum,
            s.first().copied()
        );
    }
    sum
}

/// When no `--external-lib` provides a symbol, map known C entry points to in-process stubs.
/// The JIT still links imports under the **Modelica** function name from the call site.
pub fn jit_stub_for_external_c_name(c_name: &str) -> Option<*const u8> {
    match c_name {
        "rustmodlica_print_string" => Some(rustmodlica_print_string_jit_stub as *const u8),
        "rustmodlica_sum_array" => Some(rustmodlica_sum_array_jit_stub as *const u8),
        _ => None,
    }
}

/// Diagnostic callback for residual consistency gate failures.
/// Called by JIT-compiled code when the max absolute residual exceeds the configured tolerance.
/// `max_abs`: largest |residual| after solving, `n_residuals`: total residual count,
/// `tol`: configured tolerance threshold.
#[allow(clippy::cast_possible_truncation)]
extern "C" fn rustmodlica_residual_gate_fail(max_abs: f64, n_residuals: f64, tol: f64) {
    let nr = n_residuals as i64;
    eprintln!(
        "SolvableBlock residual consistency gate FAILED: max|residual|={:.6e} > tol={:.1e}, residuals={}",
        max_abs, tol, nr
    );
}

pub fn terminate_requested() -> bool {
    TERMINATE_REQUESTED.load(Ordering::SeqCst)
}

pub fn reset_terminate_flag() {
    TERMINATE_REQUESTED.store(false, Ordering::SeqCst);
}

pub fn reset_assert_counter() {
    ASSERT_HIT_COUNT.store(0, Ordering::SeqCst);
    ASSERT_PRINTED_COUNT.store(0, Ordering::SeqCst);
    ASSERT_SUPPRESS.store(false, Ordering::Relaxed);
}

pub fn assert_hit_count() -> u64 {
    ASSERT_HIT_COUNT.load(Ordering::SeqCst)
}

pub fn suppress_assert_begin() {
    ASSERT_SUPPRESS.store(true, Ordering::Relaxed);
}

pub fn suppress_assert_end() {
    ASSERT_SUPPRESS.store(false, Ordering::Relaxed);
}

include!("native_math.rs");

pub fn register_symbols(builder: &mut JITBuilder) {
    visit_builtin_symbols(|name, ptr| {
        builder.symbol(name, ptr);
    });
}

pub fn builtin_jit_symbol_ptrs() -> std::collections::HashMap<String, *const u8> {
    let mut m = std::collections::HashMap::new();
    visit_builtin_symbols(|name, ptr| {
        m.insert(name.to_string(), ptr);
    });
    m
}

/// Names of symbols registered by register_symbols(); used to avoid JIT panic when external is missing.
pub fn builtin_jit_symbol_names() -> std::collections::HashSet<&'static str> {
    let mut set = std::collections::HashSet::new();
    visit_builtin_symbols(|name, _| {
        set.insert(name);
    });
    set
}
