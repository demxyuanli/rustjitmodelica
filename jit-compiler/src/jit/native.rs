use cranelift_jit::JITBuilder;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;

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

// Math Wrappers
extern "C" fn modelica_mod(x: f64, y: f64) -> f64 {
    x.rem_euclid(y)
}
extern "C" fn modelica_rem(x: f64, y: f64) -> f64 {
    x % y
}
extern "C" fn modelica_sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}
extern "C" fn modelica_min(x: f64, y: f64) -> f64 {
    x.min(y)
}
extern "C" fn modelica_max(x: f64, y: f64) -> f64 {
    x.max(y)
}
#[allow(clippy::cast_precision_loss)]
extern "C" fn modelica_div(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        0.0
    } else {
        (x / y).trunc()
    }
}
#[allow(clippy::cast_precision_loss)]
extern "C" fn modelica_integer(x: f64) -> f64 {
    x.trunc()
}

/// smooth(Real) -> Real: identity for testing; Modelica uses for continuity hint.
extern "C" fn modelica_smooth(x: f64) -> f64 {
    x
}

extern "C" fn modelica_boolean(x: f64) -> f64 {
    if x != 0.0 {
        1.0
    } else {
        0.0
    }
}

extern "C" fn modelica_not(x: f64) -> f64 {
    if x != 0.0 {
        0.0
    } else {
        1.0
    }
}

extern "C" fn modelica_pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

extern "C" fn modelica_string(x: f64) -> f64 {
    x
}

thread_local! {
    static DENSE_NEWTON_JRD_WORKSPACE: std::cell::RefCell<Vec<f64>> =
        std::cell::RefCell::new(Vec::new());
    static JIT_RAW_U64_WORKSPACE: std::cell::RefCell<Vec<u64>> =
        std::cell::RefCell::new(Vec::new());
}

/// Contiguous [J row-major n*n][r n][dx n], 8-byte aligned. Thread-local; reused across calls.
#[allow(clippy::cast_possible_truncation)]
extern "C" fn rustmodlica_dense_newton_workspace(n: i32) -> *mut f64 {
    if n <= 0 {
        return std::ptr::null_mut();
    }
    let n_usize = n as usize;
    if n_usize > crate::solvable_limits::MAX_SOLVABLE_RESIDUALS {
        return std::ptr::null_mut();
    }
    let len = n_usize
        .saturating_mul(n_usize)
        .saturating_add(n_usize)
        .saturating_add(n_usize);
    DENSE_NEWTON_JRD_WORKSPACE.with(|cell| {
        let mut v = cell.borrow_mut();
        v.resize(len, 0.0);
        v.as_mut_ptr()
    })
}

/// Scratch memory for large sparse-Newton packed buffers; pointer is 8-byte aligned.
#[allow(clippy::cast_possible_truncation)]
extern "C" fn rustmodlica_jit_workspace_bytes(min_bytes: i32) -> *mut u8 {
    if min_bytes <= 0 {
        return std::ptr::null_mut();
    }
    let b = min_bytes as usize;
    let words = (b + 7) / 8;
    JIT_RAW_U64_WORKSPACE.with(|cell| {
        let mut v = cell.borrow_mut();
        if v.len() < words {
            v.resize(words, 0);
        }
        v.as_mut_ptr() as *mut u8
    })
}

/// Solves J * dx = -r for dx (dense n x n). Returns 0 on success, non-zero if singular.
/// Used by general Newton tearing (SolvableBlock with N > 3 residuals).
#[allow(clippy::cast_possible_truncation)]
extern "C" fn rustmodlica_solve_linear_n(
    n: i32,
    jac: *const f64,
    r: *const f64,
    dx: *mut f64,
) -> i32 {
    thread_local! {
        static BUF_JAC: std::cell::RefCell<Vec<f64>> = std::cell::RefCell::new(Vec::new());
        static BUF_RHS: std::cell::RefCell<Vec<f64>> = std::cell::RefCell::new(Vec::new());
        static BUF_A: std::cell::RefCell<Vec<f64>> = std::cell::RefCell::new(Vec::new());
        static BUF_B: std::cell::RefCell<Vec<f64>> = std::cell::RefCell::new(Vec::new());
    }
    if n <= 0 || jac.is_null() || r.is_null() || dx.is_null() {
        return -1;
    }
    let n_usize = n as usize;
    BUF_JAC.with(|cell| {
    BUF_RHS.with(|cell_rhs| {
    BUF_A.with(|cell_a| {
    BUF_B.with(|cell_b| {
    let mut jac_base = cell.borrow_mut();
    let mut rhs_base = cell_rhs.borrow_mut();
    let mut a = cell_a.borrow_mut();
    let mut b = cell_b.borrow_mut();
    jac_base.resize(n_usize * n_usize, 0.0);
    rhs_base.resize(n_usize, 0.0);
    a.resize(n_usize * n_usize, 0.0);
    b.resize(n_usize, 0.0);
    unsafe {
        std::ptr::copy_nonoverlapping(jac, jac_base.as_mut_ptr(), n_usize * n_usize);
        std::ptr::copy_nonoverlapping(r, rhs_base.as_mut_ptr(), n_usize);
        for bi in &mut *rhs_base {
            *bi = -*bi;
        }
    }

    let mut lambda = 0.0_f64;
    let max_lm_retries = 10_u32;
    for _retry in 0..=max_lm_retries {
        a.copy_from_slice(&jac_base);
        b.copy_from_slice(&rhs_base);
        if lambda > 0.0 {
            for i in 0..n_usize {
                a[i * n_usize + i] += lambda;
            }
        }

        let mut ok = true;
        for k in 0..n_usize {
            let mut max_row = k;
            let mut max_val = a[k * n_usize + k].abs();
            for i in (k + 1)..n_usize {
                let v = a[i * n_usize + k].abs();
                if v > max_val {
                    max_val = v;
                    max_row = i;
                }
            }
            if max_val < 1e-14 {
                ok = false;
                break;
            }
            if max_row != k {
                for j in 0..n_usize {
                    a.swap(k * n_usize + j, max_row * n_usize + j);
                }
                b.swap(k, max_row);
            }
            let inv = 1.0 / a[k * n_usize + k];
            a[k * n_usize + k] = 1.0;
            for j in (k + 1)..n_usize {
                a[k * n_usize + j] *= inv;
            }
            b[k] *= inv;
            for i in 0..n_usize {
                if i == k {
                    continue;
                }
                let f = a[i * n_usize + k];
                a[i * n_usize + k] = 0.0;
                for j in (k + 1)..n_usize {
                    a[i * n_usize + j] -= f * a[k * n_usize + j];
                }
                b[i] -= f * b[k];
            }
        }

        if ok {
            unsafe {
                std::ptr::copy_nonoverlapping(b.as_ptr(), dx, n_usize);
            }
            return 0;
        }

        lambda = if lambda == 0.0 { 1e-6 } else { lambda * 10.0 };
    }

    1
    })})})})
}

#[allow(clippy::cast_possible_truncation)]
extern "C" fn rustmodlica_solve_linear_csr(
    n: i32,
    nnz: i32,
    row_ptr: *const i32,
    col_idx: *const i32,
    values: *const f64,
    r: *const f64,
    dx: *mut f64,
) -> i32 {
    let dual_validate = dual_validate_enabled();
    let sparse_debug = sparse_debug_enabled();
    if n <= 0
        || nnz < 0
        || row_ptr.is_null()
        || col_idx.is_null()
        || values.is_null()
        || r.is_null()
        || dx.is_null()
    {
        return -1;
    }

    let n_usize = n as usize;
    let nnz_usize = nnz as usize;
    let mut row_ptr_vec = vec![0usize; n_usize + 1];
    let mut col_idx_vec = vec![0usize; nnz_usize];
    let mut values_vec = vec![0.0; nnz_usize];
    let mut rhs = vec![0.0; n_usize];

    unsafe {
        for (idx, row) in row_ptr_vec.iter_mut().enumerate() {
            let raw = std::ptr::read_unaligned(row_ptr.add(idx));
            if raw < 0 {
                return -1;
            }
            *row = raw as usize;
        }
        for (idx, col) in col_idx_vec.iter_mut().enumerate() {
            let raw = std::ptr::read_unaligned(col_idx.add(idx));
            if raw < 0 {
                return -1;
            }
            *col = raw as usize;
        }
        for (idx, dst) in values_vec.iter_mut().enumerate() {
            *dst = std::ptr::read_unaligned(values.add(idx));
        }
        for (idx, dst) in rhs.iter_mut().enumerate() {
            *dst = std::ptr::read_unaligned(r.add(idx));
        }
    }

    for value in &mut rhs {
        *value = -*value;
    }

    let mut matrix = CsrMatrix {
        n: n_usize,
        row_ptr: row_ptr_vec,
        col_idx: col_idx_vec,
        values: values_vec.clone(),
    };
    let mut solution = vec![0.0; n_usize];
    let mut lambda = 0.0_f64;
    let max_lm_retries = 10_u32;
    for _retry in 0..=max_lm_retries {
        matrix.values.copy_from_slice(&values_vec);
        if lambda > 0.0 {
            for i in 0..n_usize {
                let start = matrix.row_ptr[i];
                let end = matrix.row_ptr[i + 1];
                for k in start..end {
                    if matrix.col_idx[k] == i {
                        matrix.values[k] += lambda;
                    }
                }
            }
        }

        solution.fill(0.0);
        if matrix.solve_in_place(&rhs, &mut solution).is_ok() {
            if dual_validate {
                let mut dense = vec![0.0_f64; n_usize * n_usize];
                for row in 0..n_usize {
                    let start = matrix.row_ptr[row];
                    let end = matrix.row_ptr[row + 1];
                    for k in start..end {
                        let col = matrix.col_idx[k];
                        if col < n_usize {
                            dense[row * n_usize + col] = matrix.values[k];
                        }
                    }
                }
                let mut dense_dx = vec![0.0_f64; n_usize];
                let dense_status = rustmodlica_solve_linear_n(
                    n,
                    dense.as_ptr(),
                    r,
                    dense_dx.as_mut_ptr(),
                );
                if dense_status == 0 {
                    let mut max_delta = 0.0_f64;
                    for i in 0..n_usize {
                        let d = (dense_dx[i] - solution[i]).abs();
                        if d > max_delta {
                            max_delta = d;
                        }
                    }
                    if sparse_debug {
                        eprintln!(
                            "[newton-sparse] dual-validate n={} nnz={} max|dx_sparse-dx_dense|={:.3e}",
                            n_usize, nnz_usize, max_delta
                        );
                    }
                } else if sparse_debug {
                    eprintln!(
                        "[newton-sparse] dual-validate dense fallback failed status={}",
                        dense_status
                    );
                }
            }
            unsafe {
                std::ptr::copy_nonoverlapping(solution.as_ptr(), dx, n_usize);
            }
            if sparse_debug && lambda > 0.0 {
                eprintln!(
                    "[newton-sparse] csr lm accepted lambda={:.3e} n={} nnz={}",
                    lambda, n_usize, nnz_usize
                );
            }
            return 0;
        }

        lambda = if lambda == 0.0 { 1e-8 } else { lambda * 10.0 };
        if sparse_debug {
            eprintln!(
                "[newton-sparse] csr lm retry lambda={:.3e} n={} nnz={}",
                lambda, n_usize, nnz_usize
            );
        }
    }

    if sparse_debug {
        eprintln!(
            "[newton-sparse] csr lm failed n={} nnz={} retries={}",
            n_usize, nnz_usize, max_lm_retries
        );
    }
    1
}

/// SYNC-3: sample(interval) - returns 1.0 at sample instants (0, interval, 2*interval, ...), else 0.0.
/// Approximates "at sample point" by fmod(t, interval) near zero or t near zero.
extern "C" fn rustmodlica_sample(t: f64, interval: f64) -> f64 {
    const SAMPLE_EPS: f64 = 1e-9;
    if interval <= 0.0 {
        return 0.0;
    }
    let phase = t / interval;
    let k = phase.floor();
    let frac = phase - k;
    if frac < SAMPLE_EPS || (1.0 - frac) < SAMPLE_EPS {
        1.0
    } else if t.abs() < SAMPLE_EPS {
        1.0
    } else {
        0.0
    }
}

#[no_mangle]
pub extern "C" fn sample(t: f64, interval: f64) -> f64 {
    rustmodlica_sample(t, interval)
}

// SYNC builtins (first-version numeric semantics, matching C codegen expr_emit.rs)
#[no_mangle]
pub extern "C" fn interval(x: f64) -> f64 {
    x
}

#[no_mangle]
pub extern "C" fn subSample(clock: f64, factor: f64) -> f64 {
    clock * factor
}

#[no_mangle]
pub extern "C" fn superSample(clock: f64, factor: f64) -> f64 {
    if factor == 0.0 {
        clock
    } else {
        clock / factor
    }
}

#[no_mangle]
pub extern "C" fn shiftSample(clock: f64, n: f64) -> f64 {
    clock + n
}

#[no_mangle]
pub extern "C" fn hold(x: f64) -> f64 {
    x
}

#[no_mangle]
pub extern "C" fn previous(x: f64) -> f64 {
    x
}

fn visit_builtin_symbols(mut f: impl FnMut(&'static str, *const u8)) {
    f("rustmodlica_sample", rustmodlica_sample as *const u8);
    f("sample", sample as *const u8);
    f("sin", f64::sin as *const u8);
    f("cos", f64::cos as *const u8);
    f("tan", f64::tan as *const u8);
    f("asin", f64::asin as *const u8);
    f("acos", f64::acos as *const u8);
    f("atan", f64::atan as *const u8);
    f("atan2", f64::atan2 as *const u8);
    f("sinh", f64::sinh as *const u8);
    f("cosh", f64::cosh as *const u8);
    f("tanh", f64::tanh as *const u8);
    f("sqrt", f64::sqrt as *const u8);
    f("pow", modelica_pow as *const u8);
    f("exp", f64::exp as *const u8);
    f("log", f64::ln as *const u8);
    f("log10", f64::log10 as *const u8);
    f("abs", f64::abs as *const u8);
    f("ceil", f64::ceil as *const u8);
    f("floor", f64::floor as *const u8);
    f("mod", modelica_mod as *const u8);
    f("rem", modelica_rem as *const u8);
    f("sign", modelica_sign as *const u8);
    f("min", modelica_min as *const u8);
    f("max", modelica_max as *const u8);
    f("div", modelica_div as *const u8);
    f("integer", modelica_integer as *const u8);
    f("smooth", modelica_smooth as *const u8);
    f("interval", interval as *const u8);
    f("subSample", subSample as *const u8);
    f("superSample", superSample as *const u8);
    f("shiftSample", shiftSample as *const u8);
    f("hold", hold as *const u8);
    f("previous", previous as *const u8);
    f("Modelica.Math.sin", f64::sin as *const u8);
    f("Modelica.Math.cos", f64::cos as *const u8);
    f("Modelica.Math.tan", f64::tan as *const u8);
    f("Modelica.Math.asin", f64::asin as *const u8);
    f("Modelica.Math.acos", f64::acos as *const u8);
    f("Modelica.Math.atan", f64::atan as *const u8);
    f("Modelica.Math.atan2", f64::atan2 as *const u8);
    f("Modelica.Math.sinh", f64::sinh as *const u8);
    f("Modelica.Math.cosh", f64::cosh as *const u8);
    f("Modelica.Math.tanh", f64::tanh as *const u8);
    f("Modelica.Math.exp", f64::exp as *const u8);
    f("Modelica.Math.log", f64::ln as *const u8);
    f("Modelica.Math.log10", f64::log10 as *const u8);
    f("Modelica.Math.sqrt", f64::sqrt as *const u8);
    f("Modelica.Math.pow", modelica_pow as *const u8);
    f("Modelica.Math.ceil", f64::ceil as *const u8);
    f("Modelica.Math.floor", f64::floor as *const u8);
    f("Modelica.Math.mod", modelica_mod as *const u8);
    f("Modelica.Math.rem", modelica_rem as *const u8);
    f("Modelica.Math.sign", modelica_sign as *const u8);
    f("Modelica.Math.min", modelica_min as *const u8);
    f("Modelica.Math.max", modelica_max as *const u8);
    f("Modelica.Math.div", modelica_div as *const u8);
    f("Modelica.Math.integer", modelica_integer as *const u8);
    f("rustmodlica_solve_linear_n", rustmodlica_solve_linear_n as *const u8);
    f(
        "rustmodlica_dense_newton_workspace",
        rustmodlica_dense_newton_workspace as *const u8,
    );
    f("rustmodlica_jit_workspace_bytes", rustmodlica_jit_workspace_bytes as *const u8);
    f("rustmodlica_solve_linear_csr", rustmodlica_solve_linear_csr as *const u8);
    f("assert", modelica_assert as *const u8);
    f("terminate", modelica_terminate as *const u8);
    f("extLog", rustmodlica_builtin_ext_log as *const u8);
    f(
        "rustmodlica_residual_gate_fail",
        rustmodlica_residual_gate_fail as *const u8,
    );
    f("Boolean", modelica_boolean as *const u8);
    f("not", modelica_not as *const u8);
    f("String", modelica_string as *const u8);
    f(
        "rustmodlica_assert_suppress_begin",
        rustmodlica_assert_suppress_begin as *const u8,
    );
    f(
        "rustmodlica_assert_suppress_end",
        rustmodlica_assert_suppress_end as *const u8,
    );
    f("rustmodlica_math_real_fft", crate::math_fft::rustmodlica_math_real_fft as *const u8);
    f(
        "rustmodlica_math_random_msl",
        crate::modelica_random::rustmodlica_math_random_msl as *const u8,
    );
    f(
        "rustmodlica_real_fft_write_to_file",
        crate::math_fft::rustmodlica_real_fft_write_to_file as *const u8,
    );
}

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
