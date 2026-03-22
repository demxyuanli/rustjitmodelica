use crate::ast::Expression;
use crate::jit::{CalcDerivsFunc};
use crate::solver::System;
use std::collections::HashMap;

use super::jacobian::eval_jac_expr_at_state;

pub fn maybe_print_numeric_jacobian(
    numeric_ode_jacobian: bool,
    time: f64,
    epsilon: f64,
    states: &[f64],
    calc_derivs: CalcDerivsFunc,
    params: &[f64],
    discrete_vals: &mut [f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    pre_states: &[f64],
    pre_discrete_vals: &[f64],
    t_end: f64,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    state_var_index: &HashMap<String, usize>,
    homotopy_lambda_ptr: *const f64,
) {
    if !(numeric_ode_jacobian && (time - 0.0).abs() < epsilon) {
        return;
    }
    let n = states.len();
    if n == 0 {
        return;
    }
    let mut system = System {
        calc_derivs,
        params,
        discrete: discrete_vals,
        outputs,
        when_states,
        crossings,
        pre_states,
        pre_discrete: pre_discrete_vals,
        t_end,
        diag_residual: std::ptr::null_mut(),
        diag_x: std::ptr::null_mut(),
        eval_call_index: std::ptr::null_mut(),
        last_eval_time: std::ptr::null_mut(),
        last_eval_state: std::ptr::null_mut(),
        last_eval_state_len: 0,
        scratch_outputs: None,
        homotopy_lambda_ptr,
        buf_discrete: Vec::new(),
        buf_when: Vec::new(),
        buf_crossings: Vec::new(),
        buf_outputs: Vec::new(),
    };
    let mut jac = vec![0.0_f64; n * n];
    if let Err(code) = system.compute_ode_jacobian_numeric(time, states, &mut jac, 1e-6) {
        eprintln!(
            "Warning: numeric ODE Jacobian computation failed at t={:.4} with status {}",
            time, code
        );
        return;
    }

    println!("Numeric ODE Jacobian at t={:.4} (size {} x {}):", time, n, n);
    for i in 0..n {
        print!("  row {}:", i);
        for j in 0..n {
            print!(" {:+.4}", jac[i * n + j]);
        }
        println!();
    }
    if let Some(jac_exprs) = symbolic_ode_jacobian {
        if jac_exprs.len() == n && jac_exprs.iter().all(|row| row.len() == n) {
            let mut max_diff = 0.0_f64;
            for i in 0..n {
                for j in 0..n {
                    let v_sym = eval_jac_expr_at_state(&jac_exprs[i][j], state_var_index, states);
                    let v_num = jac[i * n + j];
                    let diff = (v_sym - v_num).abs();
                    if diff > max_diff {
                        max_diff = diff;
                    }
                }
            }
            println!(
                "Max difference between symbolic and numeric ODE Jacobian at t={:.4}: {:.6}",
                time, max_diff
            );
        } else {
            eprintln!(
                "Warning: symbolic ODE Jacobian matrix size ({:?}x?) does not match state dimension {}",
                jac_exprs.len(),
                n
            );
        }
    }
}
