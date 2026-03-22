use cranelift_jit::JITBuilder;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sparse_solve::CsrMatrix;

// F4-4: assert/terminate simulation behavior
static TERMINATE_REQUESTED: AtomicBool = AtomicBool::new(false);
static ASSERT_HIT_COUNT: AtomicU64 = AtomicU64::new(0);
static ASSERT_PRINTED_COUNT: AtomicU64 = AtomicU64::new(0);
static ASSERT_SUPPRESS: AtomicBool = AtomicBool::new(false);
const ASSERT_PRINT_LIMIT: u64 = 32;

#[allow(clippy::cast_possible_truncation)]
extern "C" fn modelica_assert(cond: f64, msg: f64) {
    if cond == 0.0 {
        if ASSERT_SUPPRESS.load(Ordering::Relaxed) {
            return;
        }
        let hit_idx = ASSERT_HIT_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
        let printed = ASSERT_PRINTED_COUNT.load(Ordering::SeqCst);
        if printed < ASSERT_PRINT_LIMIT {
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
            let raw = *row_ptr.add(idx);
            if raw < 0 {
                return -1;
            }
            *row = raw as usize;
        }
        for (idx, col) in col_idx_vec.iter_mut().enumerate() {
            let raw = *col_idx.add(idx);
            if raw < 0 {
                return -1;
            }
            *col = raw as usize;
        }
        std::ptr::copy_nonoverlapping(values, values_vec.as_mut_ptr(), nnz_usize);
        std::ptr::copy_nonoverlapping(r, rhs.as_mut_ptr(), n_usize);
    }

    for value in &mut rhs {
        *value = -*value;
    }

    let mut lambda = 0.0_f64;
    let max_lm_retries = 10_u32;
    for _retry in 0..=max_lm_retries {
        let mut vals = values_vec.clone();
        if lambda > 0.0 {
            for i in 0..n_usize {
                let start = row_ptr_vec[i];
                let end = row_ptr_vec[i + 1];
                for k in start..end {
                    if col_idx_vec[k] == i {
                        vals[k] += lambda;
                    }
                }
            }
        }

        let matrix = CsrMatrix {
            n: n_usize,
            row_ptr: row_ptr_vec.clone(),
            col_idx: col_idx_vec.clone(),
            values: vals,
        };
        let mut solution = vec![0.0; n_usize];
        if matrix.solve_in_place(&rhs, &mut solution).is_ok() {
            unsafe {
                std::ptr::copy_nonoverlapping(solution.as_ptr(), dx, n_usize);
            }
            return 0;
        }

        lambda = if lambda == 0.0 { 1e-8 } else { lambda * 10.0 };
    }

    1
}

/// SYNC-3: sample(interval) - returns 1.0 at sample instants (0, interval, 2*interval, ...), else 0.0.
/// Approximates "at sample point" by fmod(t, interval) near zero or t near zero.
extern "C" fn rustmodlica_sample(t: f64, interval: f64) -> f64 {
    if interval <= 0.0 {
        return 0.0;
    }
    let phase = t / interval;
    let k = phase.floor();
    let frac = phase - k;
    if frac < 1e-12 || (1.0 - frac) < 1e-12 {
        1.0
    } else if t < 1e-12 {
        1.0
    } else {
        0.0
    }
}

pub fn register_symbols(builder: &mut JITBuilder) {
    builder.symbol("rustmodlica_sample", rustmodlica_sample as *const u8);
    // Register symbols for math functions
    builder.symbol("sin", f64::sin as *const u8);
    builder.symbol("cos", f64::cos as *const u8);
    builder.symbol("tan", f64::tan as *const u8);
    builder.symbol("asin", f64::asin as *const u8);
    builder.symbol("acos", f64::acos as *const u8);
    builder.symbol("atan", f64::atan as *const u8);
    builder.symbol("atan2", f64::atan2 as *const u8);
    builder.symbol("sinh", f64::sinh as *const u8);
    builder.symbol("cosh", f64::cosh as *const u8);
    builder.symbol("tanh", f64::tanh as *const u8);
    builder.symbol("sqrt", f64::sqrt as *const u8);
    builder.symbol("pow", modelica_pow as *const u8);
    builder.symbol("exp", f64::exp as *const u8);
    builder.symbol("log", f64::ln as *const u8);
    builder.symbol("log10", f64::log10 as *const u8);
    builder.symbol("abs", f64::abs as *const u8);
    builder.symbol("ceil", f64::ceil as *const u8);
    builder.symbol("floor", f64::floor as *const u8);

    // Extended Math
    builder.symbol("mod", modelica_mod as *const u8);
    builder.symbol("rem", modelica_rem as *const u8);
    builder.symbol("sign", modelica_sign as *const u8);
    builder.symbol("min", modelica_min as *const u8);
    builder.symbol("max", modelica_max as *const u8);
    builder.symbol("div", modelica_div as *const u8);
    builder.symbol("integer", modelica_integer as *const u8);
    builder.symbol("smooth", modelica_smooth as *const u8);

    // Modelica.Math Aliases
    builder.symbol("Modelica.Math.sin", f64::sin as *const u8);
    builder.symbol("Modelica.Math.cos", f64::cos as *const u8);
    builder.symbol("Modelica.Math.tan", f64::tan as *const u8);
    builder.symbol("Modelica.Math.asin", f64::asin as *const u8);
    builder.symbol("Modelica.Math.acos", f64::acos as *const u8);
    builder.symbol("Modelica.Math.atan", f64::atan as *const u8);
    builder.symbol("Modelica.Math.atan2", f64::atan2 as *const u8);
    builder.symbol("Modelica.Math.sinh", f64::sinh as *const u8);
    builder.symbol("Modelica.Math.cosh", f64::cosh as *const u8);
    builder.symbol("Modelica.Math.tanh", f64::tanh as *const u8);
    builder.symbol("Modelica.Math.exp", f64::exp as *const u8);
    builder.symbol("Modelica.Math.log", f64::ln as *const u8);
    builder.symbol("Modelica.Math.log10", f64::log10 as *const u8);
    builder.symbol("Modelica.Math.sqrt", f64::sqrt as *const u8);
    builder.symbol("Modelica.Math.pow", modelica_pow as *const u8);
    builder.symbol("Modelica.Math.ceil", f64::ceil as *const u8);
    builder.symbol("Modelica.Math.floor", f64::floor as *const u8);
    builder.symbol("Modelica.Math.mod", modelica_mod as *const u8);
    builder.symbol("Modelica.Math.rem", modelica_rem as *const u8);
    builder.symbol("Modelica.Math.sign", modelica_sign as *const u8);
    builder.symbol("Modelica.Math.min", modelica_min as *const u8);
    builder.symbol("Modelica.Math.max", modelica_max as *const u8);
    builder.symbol("Modelica.Math.div", modelica_div as *const u8);
    builder.symbol("Modelica.Math.integer", modelica_integer as *const u8);

    builder.symbol(
        "rustmodlica_solve_linear_n",
        rustmodlica_solve_linear_n as *const u8,
    );
    builder.symbol(
        "rustmodlica_solve_linear_csr",
        rustmodlica_solve_linear_csr as *const u8,
    );

    builder.symbol("assert", modelica_assert as *const u8);
    builder.symbol("terminate", modelica_terminate as *const u8);
    builder.symbol("Boolean", modelica_boolean as *const u8);
    builder.symbol("not", modelica_not as *const u8);
    builder.symbol("String", modelica_string as *const u8);
    builder.symbol(
        "rustmodlica_assert_suppress_begin",
        rustmodlica_assert_suppress_begin as *const u8,
    );
    builder.symbol(
        "rustmodlica_assert_suppress_end",
        rustmodlica_assert_suppress_end as *const u8,
    );
}

/// Names of symbols registered by register_symbols(); used to avoid JIT panic when external is missing.
pub fn builtin_jit_symbol_names() -> std::collections::HashSet<&'static str> {
    let mut set = std::collections::HashSet::new();
    set.insert("rustmodlica_sample");
    set.insert("sin");
    set.insert("cos");
    set.insert("tan");
    set.insert("asin");
    set.insert("acos");
    set.insert("atan");
    set.insert("atan2");
    set.insert("sinh");
    set.insert("cosh");
    set.insert("tanh");
    set.insert("sqrt");
    set.insert("pow");
    set.insert("exp");
    set.insert("log");
    set.insert("log10");
    set.insert("abs");
    set.insert("ceil");
    set.insert("floor");
    set.insert("mod");
    set.insert("rem");
    set.insert("sign");
    set.insert("min");
    set.insert("max");
    set.insert("div");
    set.insert("integer");
    set.insert("smooth");
    set.insert("Modelica.Math.sin");
    set.insert("Modelica.Math.cos");
    set.insert("Modelica.Math.tan");
    set.insert("Modelica.Math.asin");
    set.insert("Modelica.Math.acos");
    set.insert("Modelica.Math.atan");
    set.insert("Modelica.Math.atan2");
    set.insert("Modelica.Math.sinh");
    set.insert("Modelica.Math.cosh");
    set.insert("Modelica.Math.tanh");
    set.insert("Modelica.Math.exp");
    set.insert("Modelica.Math.log");
    set.insert("Modelica.Math.log10");
    set.insert("Modelica.Math.sqrt");
    set.insert("Modelica.Math.pow");
    set.insert("Modelica.Math.ceil");
    set.insert("Modelica.Math.floor");
    set.insert("Modelica.Math.mod");
    set.insert("Modelica.Math.rem");
    set.insert("Modelica.Math.sign");
    set.insert("Modelica.Math.min");
    set.insert("Modelica.Math.max");
    set.insert("Modelica.Math.div");
    set.insert("Modelica.Math.integer");
    set.insert("rustmodlica_solve_linear_n");
    set.insert("rustmodlica_solve_linear_csr");
    set.insert("rustmodlica_assert_suppress_begin");
    set.insert("rustmodlica_assert_suppress_end");
    set.insert("assert");
    set.insert("terminate");
    set.insert("Boolean");
    set.insert("not");
    set.insert("String");
    set
}
