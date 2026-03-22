// CG1-1: C code generation from DAE. Emits residual (and optional Jacobian) for compilation with external runtime.
// CG1-4: Array preservation: use NAME_START + index in generated C when array layout is provided.
// FUNC-6: Emit extern declarations for user/external functions called from equations.

use std::collections::{HashMap, HashSet};
use std::io::Write;

mod context;
mod equation_emit;
mod expr_emit;
mod external;
mod files;
mod solvable_emit;

use crate::ast::{Equation, Expression};
use crate::compiler::equation_convert::parse_array_index as parse_array_index_impl;
use context::CCodegenContext;

fn is_c_builtin(name: &str) -> bool {
    matches!(
        name,
        "sin"
            | "cos"
            | "tan"
            | "sqrt"
            | "exp"
            | "log"
            | "abs"
            | "min"
            | "max"
            | "mod"
            | "sign"
            | "integer"
            | "floor"
            | "ceil"
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgKind {
    Scalar,
    Array,
    /// FUNC-7: string literal -> const char* in C
    String,
}

pub(super) fn parse_array_index(name: &str) -> Option<(String, usize)> {
    parse_array_index_impl(name)
}

/// Emit C residual function: void residual(double t, const double* x, double* xdot, const double* p, double* y).
/// Supports Simple equations and SolvableBlock with 1 to 32 residuals (Newton in C; IR4-1 aligned with JIT).
pub fn emit_residual(
    _state_vars: &[String],
    _param_vars: &[String],
    _output_vars: &[String],
    sorted_eqs: &[Equation],
    ctx: &CCodegenContext<'_>,
    external_sigs: &HashMap<String, Vec<ArgKind>>,
    external_names: Option<&HashSet<String>>,
    user_function_bodies: Option<&HashMap<String, (Vec<String>, Expression)>>,
    out: &mut dyn Write,
) -> Result<(), String> {
    equation_emit::emit_residual(
        _state_vars,
        _param_vars,
        _output_vars,
        sorted_eqs,
        ctx,
        external_sigs,
        external_names,
        user_function_bodies,
        out,
    )
}

/// Emit C ODE Jacobian: void jacobian(double t, const double* x, const double* p, double* J).
/// J is row-major, n x n; J[i*n+j] = d(xdot_i)/d(x_j).
pub fn emit_jacobian(
    jac_dense: &[Vec<Expression>],
    ctx: &CCodegenContext<'_>,
    out: &mut dyn Write,
) -> Result<(), String> {
    files::emit_jacobian(jac_dense, ctx, out)
}

/// Emit model.h with residual (and optional jacobian) declaration.
/// CG1-4: Emit array layout defines for state (x[]), output (y[]), and parameter (p[]) when provided.
pub fn emit_header(
    has_jacobian: bool,
    state_array_layout: Option<&[(String, usize, usize)]>,
    output_array_layout: Option<&[(String, usize, usize)]>,
    param_array_layout: Option<&[(String, usize, usize)]>,
    out: &mut dyn Write,
) -> Result<(), String> {
    files::emit_header(
        has_jacobian,
        state_array_layout,
        output_array_layout,
        param_array_layout,
        out,
    )
}

/// Write model.c and model.h to the given directory. Returns paths written.
/// If ode_jacobian is Some, also emits jacobian() in C and declares it in the header.
/// CG1-4: Array layouts enable NAME_START/SIZE in header and symbolic indices in residual/jacobian.
/// EXT-5: external_c_names maps modelica name -> C name for extern declarations and calls.
pub fn emit_c_files(
    dir: &std::path::Path,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    sorted_eqs: &[Equation],
    ode_jacobian: Option<&[Vec<Expression>]>,
    state_array_layout: Option<&[(String, usize, usize)]>,
    output_array_layout: Option<&[(String, usize, usize)]>,
    param_array_layout: Option<&[(String, usize, usize)]>,
    external_c_names: Option<HashMap<String, String>>,
    external_names: Option<&HashSet<String>>,
    user_function_bodies: Option<&HashMap<String, (Vec<String>, Expression)>>,
) -> Result<Vec<std::path::PathBuf>, String> {
    files::emit_c_files(
        dir,
        state_vars,
        param_vars,
        output_vars,
        sorted_eqs,
        ode_jacobian,
        state_array_layout,
        output_array_layout,
        param_array_layout,
        external_c_names,
        external_names,
        user_function_bodies,
    )
}
