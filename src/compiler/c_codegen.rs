// CG1-1: C code generation from DAE. Emits residual (and optional Jacobian) for compilation with external runtime.

use std::collections::HashMap;
use std::io::Write;

use crate::ast::{Equation, Expression, Operator};

/// Context for mapping variable names to C array access (x[], xdot[], p[], y[]).
/// Optional var_overrides: use a C expression for a variable (e.g. "local_tear" inside Newton block).
#[derive(Clone)]
pub struct CCodegenContext {
    pub state_index: HashMap<String, usize>,
    pub param_index: HashMap<String, usize>,
    pub output_index: HashMap<String, usize>,
    pub var_overrides: HashMap<String, String>,
}

impl CCodegenContext {
    pub fn new(
        state_vars: &[String],
        param_vars: &[String],
        output_vars: &[String],
    ) -> Self {
        let state_index: HashMap<String, usize> = state_vars
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect();
        let param_index: HashMap<String, usize> = param_vars
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect();
        let output_index: HashMap<String, usize> = output_vars
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect();
        Self {
            state_index,
            param_index,
            output_index,
            var_overrides: HashMap::new(),
        }
    }

    pub fn with_override(mut self, name: &str, c_expr: String) -> Self {
        self.var_overrides.insert(name.to_string(), c_expr);
        self
    }

    fn var_to_c(&self, name: &str) -> Result<String, String> {
        if let Some(expr) = self.var_overrides.get(name) {
            return Ok(expr.clone());
        }
        if name == "time" {
            return Ok("t".to_string());
        }
        if let Some(&i) = self.state_index.get(name) {
            return Ok(format!("x[{}]", i));
        }
        if name.starts_with("der_") {
            let base = &name[4..];
            if let Some(&i) = self.state_index.get(base) {
                return Ok(format!("xdot[{}]", i));
            }
        }
        if let Some(&i) = self.param_index.get(name) {
            return Ok(format!("p[{}]", i));
        }
        if let Some(&i) = self.output_index.get(name) {
            return Ok(format!("y[{}]", i));
        }
        Err(format!("C codegen: unknown variable '{}'", name))
    }
}

/// Convert Expression to C source string. Uses t, x[], xdot[], p[], y[] from context.
pub fn expr_to_c(expr: &Expression, ctx: &CCodegenContext) -> Result<String, String> {
    use Expression::*;
    match expr {
        Variable(name) => ctx.var_to_c(name),
        Number(n) => {
            if n.is_finite() {
                Ok(format!("{:?}", n))
            } else if *n == f64::INFINITY {
                Ok("(1.0/0.0)".to_string())
            } else if *n == f64::NEG_INFINITY {
                Ok("(-1.0/0.0)".to_string())
            } else {
                Ok("(0.0/0.0)".to_string())
            }
        }
        BinaryOp(l, op, r) => {
            let left = expr_to_c(l, ctx)?;
            let right = expr_to_c(r, ctx)?;
            if *op == Operator::Sub {
                if let Number(n) = l.as_ref() {
                    if n.abs() < 1e-15 {
                        return Ok(format!("(-{})", right));
                    }
                }
            }
            let op_str = match op {
                Operator::Add => "+",
                Operator::Sub => "-",
                Operator::Mul => "*",
                Operator::Div => "/",
                Operator::Less => "<",
                Operator::Greater => ">",
                Operator::LessEq => "<=",
                Operator::GreaterEq => ">=",
                Operator::Equal => "==",
                Operator::NotEqual => "!=",
                Operator::And => "&&",
                Operator::Or => "||",
            };
            Ok(format!("({} {} {})", left, op_str, right))
        }
        Der(inner) => {
            let base = expr_to_c(inner, ctx)?;
            if let Variable(name) = inner.as_ref() {
                if let Some(&i) = ctx.state_index.get(name) {
                    return Ok(format!("xdot[{}]", i));
                }
            }
            Err(format!("C codegen: der() only for state, got {}", base))
        }
        Call(name, args) => {
            let args_c: Vec<String> = args.iter().map(|a| expr_to_c(a, ctx)).collect::<Result<Vec<_>, _>>()?;
            let args_str = args_c.join(", ");
            match name.as_str() {
                "sin" => Ok(format!("sin({})", args_str)),
                "cos" => Ok(format!("cos({})", args_str)),
                "tan" => Ok(format!("tan({})", args_str)),
                "sqrt" => Ok(format!("sqrt({})", args_str)),
                "exp" => Ok(format!("exp({})", args_str)),
                "log" => Ok(format!("log({})", args_str)),
                "abs" => Ok(format!("fabs({})", args_str)),
                "min" if args.len() == 2 => Ok(format!("fmin({})", args_str)),
                "max" if args.len() == 2 => Ok(format!("fmax({})", args_str)),
                "mod" if args.len() == 2 => Ok(format!("fmod({})", args_str)),
                "sign" if args.len() == 1 => Ok(format!("(({}) >= 0.0 ? 1.0 : -1.0)", args_str)),
                "integer" if args.len() == 1 => Ok(format!("floor({})", args_str)),
                "floor" => Ok(format!("floor({})", args_str)),
                "ceil" => Ok(format!("ceil({})", args_str)),
                _ => Err(format!("C codegen: unsupported function '{}'", name)),
            }
        }
        If(cond, then_e, else_e) => {
            let c = expr_to_c(cond, ctx)?;
            let th = expr_to_c(then_e, ctx)?;
            let el = expr_to_c(else_e, ctx)?;
            Ok(format!("(({}) ? ({}) : ({}))", c, th, el))
        }
        ArrayAccess(arr, idx) => {
            if let Variable(arr_name) = arr.as_ref() {
                let idx_c = expr_to_c(idx, ctx)?;
                if let Some(&i) = ctx.state_index.get(arr_name) {
                    return Ok(format!("x[{} + (int)({})]", i, idx_c));
                }
                if let Some(&i) = ctx.output_index.get(arr_name) {
                    return Ok(format!("y[{} + (int)({})]", i, idx_c));
                }
                if let Some(&i) = ctx.param_index.get(arr_name) {
                    return Ok(format!("p[{} + (int)({})]", i, idx_c));
                }
            }
            Err("C codegen: array base must be known variable".to_string())
        }
        Dot(_, _) | Range(_, _, _) | ArrayLiteral(_) => {
            Err("C codegen: Dot/Range/ArrayLiteral not supported (flatten first)".to_string())
        }
    }
}

/// Emit one equation to C (LHS = RHS). Uses ctx for var mapping; LHS can be xdot[], y[], or an override name.
fn emit_one_equation(lhs: &Expression, rhs_c: &str, ctx: &CCodegenContext, out: &mut dyn Write) -> Result<(), String> {
    let lhs_str = match lhs {
        Expression::Variable(name) => {
            if name.starts_with("der_") {
                let base = &name[4..];
                if let Some(&i) = ctx.state_index.get(base) {
                    format!("xdot[{}]", i)
                } else {
                    return Err(format!("C codegen: der_ variable '{}' not in state set", name));
                }
            } else if let Some(ov) = ctx.var_overrides.get(name) {
                ov.clone()
            } else if let Some(&i) = ctx.output_index.get(name) {
                format!("y[{}]", i)
            } else {
                return Err(format!("C codegen: LHS variable '{}' not der_ or output", name));
            }
        }
        _ => return Err("C codegen: LHS must be variable".to_string()),
    };
    writeln!(out, "  {} = {};", lhs_str, rhs_c).map_err(|e| e.to_string())?;
    Ok(())
}

/// Emit C residual function: void residual(double t, const double* x, double* xdot, const double* p, double* y).
/// Supports Simple equations and SolvableBlock with exactly one residual (Newton in C).
pub fn emit_residual(
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    sorted_eqs: &[Equation],
    out: &mut dyn Write,
) -> Result<(), String> {
    let ctx = CCodegenContext::new(state_vars, param_vars, output_vars);

    writeln!(out, "/* Generated by rustmodlica CG1-1. Do not edit. */").map_err(|e| e.to_string())?;
    writeln!(out, "#include <math.h>").map_err(|e| e.to_string())?;
    writeln!(out, "void residual(double t, const double* x, double* xdot, const double* p, double* y) {{").map_err(|e| e.to_string())?;

    for eq in sorted_eqs {
        match eq {
            Equation::Simple(lhs, rhs) => {
                let rhs_c = expr_to_c(rhs, &ctx)?;
                emit_one_equation(lhs, &rhs_c, &ctx, out)?;
            }
            Equation::SolvableBlock {
                unknowns,
                tearing_var: Some(ref t_var),
                equations: inner,
                residuals,
            } if residuals.len() == 1 => {
                let tear_idx = ctx.output_index.get(t_var).ok_or_else(|| {
                    format!("C codegen: tearing var '{}' not in output list", t_var)
                })?;
                writeln!(out, "  {{").map_err(|e| e.to_string())?;
                writeln!(out, "    double local_tear = y[{}];", tear_idx).map_err(|e| e.to_string())?;
                writeln!(out, "    for (int iter = 0; iter < 50; iter++) {{").map_err(|e| e.to_string())?;
                let ctx_inner = ctx.clone().with_override(t_var, "local_tear".to_string());
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                        emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                    }
                }
                let res_c = expr_to_c(&residuals[0], &ctx_inner)?;
                writeln!(out, "      double res = {};", res_c).map_err(|e| e.to_string())?;
                writeln!(out, "      if (fabs(res) < 1e-8) break;").map_err(|e| e.to_string())?;
                writeln!(out, "      double eps = 1e-6, old_tear = local_tear;").map_err(|e| e.to_string())?;
                writeln!(out, "      local_tear += eps;").map_err(|e| e.to_string())?;
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                        emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                    }
                }
                let res_pert_c = expr_to_c(&residuals[0], &ctx_inner)?;
                writeln!(out, "      double res_pert = {};", res_pert_c).map_err(|e| e.to_string())?;
                writeln!(out, "      double J = (res_pert - res) / eps;").map_err(|e| e.to_string())?;
                writeln!(out, "      if (fabs(J) < 1e-12) break;").map_err(|e| e.to_string())?;
                writeln!(out, "      local_tear = old_tear - res / J;").map_err(|e| e.to_string())?;
                writeln!(out, "    }}").map_err(|e| e.to_string())?;
                writeln!(out, "    y[{}] = local_tear;", tear_idx).map_err(|e| e.to_string())?;
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        let rhs_c = expr_to_c(rhs, &ctx)?;
                        emit_one_equation(lhs, &rhs_c, &ctx, out)?;
                    }
                }
                writeln!(out, "  }}").map_err(|e| e.to_string())?;
            }
            Equation::SolvableBlock { residuals, .. } => {
                return Err(format!(
                    "C codegen: SolvableBlock with {} residuals not supported (only 1 residual); use JIT backend",
                    residuals.len()
                ));
            }
            _ => {
                return Err(format!("C codegen: equation type not supported: {:?}", eq));
            }
        }
    }

    writeln!(out, "}}").map_err(|e| e.to_string())?;
    Ok(())
}

/// Emit C ODE Jacobian: void jacobian(double t, const double* x, const double* p, double* J).
/// J is row-major, n x n; J[i*n+j] = d(xdot_i)/d(x_j).
pub fn emit_jacobian(
    jac_dense: &[Vec<Expression>],
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    out: &mut dyn Write,
) -> Result<(), String> {
    let ctx = CCodegenContext::new(state_vars, param_vars, output_vars);
    let n = jac_dense.len();
    writeln!(out, "void jacobian(double t, const double* x, const double* p, double* J) {{").map_err(|e| e.to_string())?;
    for (i, row) in jac_dense.iter().enumerate() {
        for (j, expr) in row.iter().enumerate() {
            let c_expr = expr_to_c(expr, &ctx)?;
            writeln!(out, "  J[{} * {} + {}] = {};", i, n, j, c_expr).map_err(|e| e.to_string())?;
        }
    }
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    Ok(())
}

/// Emit model.h with residual (and optional jacobian) declaration.
/// CG1-4: If state_array_layout is Some, emit comments / defines for logical array layout (x[] indices).
pub fn emit_header(
    has_jacobian: bool,
    state_array_layout: Option<&[(String, usize, usize)]>,
    out: &mut dyn Write,
) -> Result<(), String> {
    writeln!(out, "/* Generated by rustmodlica CG1-1. */").map_err(|e| e.to_string())?;
    writeln!(out, "#ifndef MODEL_H").map_err(|e| e.to_string())?;
    writeln!(out, "#define MODEL_H").map_err(|e| e.to_string())?;
    if let Some(layout) = state_array_layout {
        writeln!(out, "/* CG1-4: state x[] array layout (name, start_index, size) */").map_err(|e| e.to_string())?;
        for (name, start, size) in layout {
            let safe = name.replace('.', "_");
            writeln!(out, "#define {}_START {}", safe.to_uppercase(), start).map_err(|e| e.to_string())?;
            writeln!(out, "#define {}_SIZE {}", safe.to_uppercase(), size).map_err(|e| e.to_string())?;
        }
    }
    writeln!(out, "void residual(double t, const double* x, double* xdot, const double* p, double* y);").map_err(|e| e.to_string())?;
    if has_jacobian {
        writeln!(out, "void jacobian(double t, const double* x, const double* p, double* J);").map_err(|e| e.to_string())?;
    }
    writeln!(out, "#endif").map_err(|e| e.to_string())?;
    Ok(())
}

/// Write model.c and model.h to the given directory. Returns paths written.
/// If ode_jacobian is Some, also emits jacobian() in C and declares it in the header.
/// state_array_layout: (array_name, start_index_in_x, size) for CG1-4 array preservation comments in header.
pub fn emit_c_files(
    dir: &std::path::Path,
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    sorted_eqs: &[Equation],
    ode_jacobian: Option<&[Vec<Expression>]>,
    state_array_layout: Option<&[(String, usize, usize)]>,
) -> Result<Vec<std::path::PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let c_path = dir.join("model.c");
    let h_path = dir.join("model.h");
    let mut c_file = std::fs::File::create(&c_path).map_err(|e| e.to_string())?;
    let mut h_file = std::fs::File::create(&h_path).map_err(|e| e.to_string())?;
    emit_residual(state_vars, param_vars, output_vars, sorted_eqs, &mut c_file)?;
    if let Some(jac) = ode_jacobian {
        emit_jacobian(jac, state_vars, param_vars, output_vars, &mut c_file)?;
    }
    emit_header(ode_jacobian.is_some(), state_array_layout, &mut h_file)?;
    Ok(vec![c_path, h_path])
}
