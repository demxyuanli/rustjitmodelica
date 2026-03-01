use cranelift_jit::JITBuilder;
use std::sync::atomic::{AtomicBool, Ordering};

// F4-4: assert/terminate simulation behavior
static TERMINATE_REQUESTED: AtomicBool = AtomicBool::new(false);

#[allow(clippy::cast_possible_truncation)]
extern "C" fn modelica_assert(cond: f64, msg: f64) {
    if cond == 0.0 {
        eprintln!("Assertion failed: {}", msg);
    }
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

// Math Wrappers
extern "C" fn modelica_mod(x: f64, y: f64) -> f64 {
    x.rem_euclid(y)
}
extern "C" fn modelica_rem(x: f64, y: f64) -> f64 {
    x % y
}
extern "C" fn modelica_sign(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}
extern "C" fn modelica_min(x: f64, y: f64) -> f64 {
    x.min(y)
}
extern "C" fn modelica_max(x: f64, y: f64) -> f64 {
    x.max(y)
}
#[allow(clippy::cast_precision_loss)]
extern "C" fn modelica_div(x: f64, y: f64) -> f64 {
    if y == 0.0 { 0.0 } else { (x / y).trunc() }
}
#[allow(clippy::cast_precision_loss)]
extern "C" fn modelica_integer(x: f64) -> f64 {
    x.trunc()
}

extern "C" fn modelica_boolean(x: f64) -> f64 {
    if x != 0.0 { 1.0 } else { 0.0 }
}

extern "C" fn modelica_string(x: f64) -> f64 {
    x
}

/// Solves J * dx = -r for dx (dense n x n). Returns 0 on success, non-zero if singular.
/// Used by general Newton tearing (SolvableBlock with N > 3 residuals).
#[allow(clippy::cast_possible_truncation)]
extern "C" fn rustmodlica_solve_linear_n(n: i32, jac: *const f64, r: *const f64, dx: *mut f64) -> i32 {
    if n <= 0 || jac.is_null() || r.is_null() || dx.is_null() {
        return -1;
    }
    let n_usize = n as usize;
    let mut a = vec![0.0; n_usize * n_usize];
    let mut b = vec![0.0; n_usize];
    unsafe {
        std::ptr::copy_nonoverlapping(jac, a.as_mut_ptr(), n_usize * n_usize);
        std::ptr::copy_nonoverlapping(r, b.as_mut_ptr(), n_usize);
        for bi in &mut b {
            *bi = -*bi;
        }
    }
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
            return 1;
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
    unsafe {
        std::ptr::copy_nonoverlapping(b.as_ptr(), dx, n_usize);
    }
    0
}

pub fn register_symbols(builder: &mut JITBuilder) {
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
    builder.symbol("Modelica.Math.ceil", f64::ceil as *const u8);
    builder.symbol("Modelica.Math.floor", f64::floor as *const u8);
    builder.symbol("Modelica.Math.mod", modelica_mod as *const u8);
    builder.symbol("Modelica.Math.rem", modelica_rem as *const u8);
    builder.symbol("Modelica.Math.sign", modelica_sign as *const u8);
    builder.symbol("Modelica.Math.min", modelica_min as *const u8);
    builder.symbol("Modelica.Math.max", modelica_max as *const u8);
    builder.symbol("Modelica.Math.div", modelica_div as *const u8);
    builder.symbol("Modelica.Math.integer", modelica_integer as *const u8);

    builder.symbol("rustmodlica_solve_linear_n", rustmodlica_solve_linear_n as *const u8);

    builder.symbol("assert", modelica_assert as *const u8);
    builder.symbol("terminate", modelica_terminate as *const u8);
    builder.symbol("Boolean", modelica_boolean as *const u8);
    builder.symbol("String", modelica_string as *const u8);
}
