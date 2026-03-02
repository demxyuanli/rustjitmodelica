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

    pub fn with_overrides(mut self, overrides: &[(String, String)]) -> Self {
        for (k, v) in overrides {
            self.var_overrides.insert(k.clone(), v.clone());
        }
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

/// Emit solve_dense helper for NxN Newton (n from 4 to 32). Solves J*x = b, overwrites b with x.
fn emit_solve_dense_helper(out: &mut dyn Write) -> Result<(), String> {
    writeln!(out, "static void solve_dense(int n, double *J, double *b) {{").map_err(|e| e.to_string())?;
    writeln!(out, "  double tmp; int i, j, k;").map_err(|e| e.to_string())?;
    writeln!(out, "  for (k = 0; k < n; k++) {{").map_err(|e| e.to_string())?;
    writeln!(out, "    int p = k; double max = fabs(J[k*n+k]);").map_err(|e| e.to_string())?;
    writeln!(out, "    for (i = k+1; i < n; i++) {{ double a = fabs(J[i*n+k]); if (a > max) {{ max = a; p = i; }} }}").map_err(|e| e.to_string())?;
    writeln!(out, "    if (max < 1e-12) return;").map_err(|e| e.to_string())?;
    writeln!(out, "    if (p != k) {{ for (j = 0; j < n; j++) {{ tmp = J[k*n+j]; J[k*n+j] = J[p*n+j]; J[p*n+j] = tmp; }} tmp = b[k]; b[k] = b[p]; b[p] = tmp; }}").map_err(|e| e.to_string())?;
    writeln!(out, "    tmp = 1.0 / J[k*n+k]; for (j = k; j < n; j++) J[k*n+j] *= tmp; b[k] *= tmp;").map_err(|e| e.to_string())?;
    writeln!(out, "    for (i = 0; i < n; i++) {{ if (i == k) continue; double f = J[i*n+k]; for (j = k; j < n; j++) J[i*n+j] -= f * J[k*n+j]; b[i] -= f * b[k]; }}").map_err(|e| e.to_string())?;
    writeln!(out, "  }}").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    Ok(())
}

/// Emit C residual function: void residual(double t, const double* x, double* xdot, const double* p, double* y).
/// Supports Simple equations and SolvableBlock with 1 to 32 residuals (Newton in C; IR4-1 aligned with JIT).
pub fn emit_residual(
    state_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    sorted_eqs: &[Equation],
    out: &mut dyn Write,
) -> Result<(), String> {
    let ctx = CCodegenContext::new(state_vars, param_vars, output_vars);

    let need_solve_dense = sorted_eqs.iter().any(|eq| {
        if let Equation::SolvableBlock { residuals, unknowns, .. } = eq {
            residuals.len() >= 1 && unknowns.len() >= 1 && unknowns.len() <= 32 && residuals.len() >= unknowns.len()
        } else {
            false
        }
    });

    writeln!(out, "/* Generated by rustmodlica CG1-1. Do not edit. */").map_err(|e| e.to_string())?;
    writeln!(out, "#include <math.h>").map_err(|e| e.to_string())?;
    if need_solve_dense {
        emit_solve_dense_helper(out)?;
    }
    writeln!(out, "void residual(double t, const double* x, double* xdot, const double* p, double* y) {{").map_err(|e| e.to_string())?;

    for eq in sorted_eqs {
        match eq {
            Equation::Simple(lhs, rhs) => {
                if !matches!(lhs, Expression::Variable(_)) {
                    return Err("C codegen: equation LHS must be a variable (residual-form equations not supported as standalone; use JIT backend)".to_string());
                }
                let rhs_c = expr_to_c(rhs, &ctx)?;
                emit_one_equation(lhs, &rhs_c, &ctx, out)?;
            }
            Equation::SolvableBlock {
                unknowns,
                tearing_var: Some(_),
                equations: inner,
                residuals,
            } if residuals.len() == 1 => {
                let t_var = unknowns.first().ok_or("C codegen: SolvableBlock empty unknowns")?;
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
            Equation::SolvableBlock {
                unknowns,
                equations: inner,
                residuals,
                ..
            } if residuals.len() == 2 && unknowns.len() >= 2 => {
                let u0 = &unknowns[0];
                let u1 = &unknowns[1];
                let i0 = *ctx.output_index.get(u0).ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u0))?;
                let i1 = *ctx.output_index.get(u1).ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u1))?;
                let ov: Vec<(String, String)> = vec![(u0.clone(), "local_0".to_string()), (u1.clone(), "local_1".to_string())];
                let ctx_inner = ctx.clone().with_overrides(&ov);
                writeln!(out, "  {{").map_err(|e| e.to_string())?;
                writeln!(out, "    double local_0 = y[{}], local_1 = y[{}];", i0, i1).map_err(|e| e.to_string())?;
                writeln!(out, "    double eps = 1e-6; int iter;").map_err(|e| e.to_string())?;
                writeln!(out, "    for (iter = 0; iter < 50; iter++) {{").map_err(|e| e.to_string())?;
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                        emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                    }
                }
                let r0_c = expr_to_c(&residuals[0], &ctx_inner)?;
                let r1_c = expr_to_c(&residuals[1], &ctx_inner)?;
                writeln!(out, "      double r0 = {}, r1 = {};", r0_c, r1_c).map_err(|e| e.to_string())?;
                writeln!(out, "      if (fabs(r0) < 1e-8 && fabs(r1) < 1e-8) break;").map_err(|e| e.to_string())?;
                writeln!(out, "      local_0 += eps;").map_err(|e| e.to_string())?;
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                        emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                    }
                }
                let r0p0 = expr_to_c(&residuals[0], &ctx_inner)?;
                let r1p0 = expr_to_c(&residuals[1], &ctx_inner)?;
                writeln!(out, "      double dr0_0 = ({} - r0) / eps, dr1_0 = ({} - r1) / eps;", r0p0, r1p0).map_err(|e| e.to_string())?;
                writeln!(out, "      local_0 -= eps; local_1 += eps;").map_err(|e| e.to_string())?;
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                        emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                    }
                }
                let r0p1 = expr_to_c(&residuals[0], &ctx_inner)?;
                let r1p1 = expr_to_c(&residuals[1], &ctx_inner)?;
                writeln!(out, "      double dr0_1 = ({} - r0) / eps, dr1_1 = ({} - r1) / eps;", r0p1, r1p1).map_err(|e| e.to_string())?;
                writeln!(out, "      local_1 -= eps;").map_err(|e| e.to_string())?;
                writeln!(out, "      double det = dr0_0*dr1_1 - dr0_1*dr1_0; if (fabs(det) < 1e-12) break;").map_err(|e| e.to_string())?;
                writeln!(out, "      double dx0 = (-r0*dr1_1 + r1*dr0_1) / det, dx1 = (r0*dr1_0 - r1*dr0_0) / det;").map_err(|e| e.to_string())?;
                writeln!(out, "      local_0 += dx0; local_1 += dx1;").map_err(|e| e.to_string())?;
                writeln!(out, "    }}").map_err(|e| e.to_string())?;
                writeln!(out, "    y[{}] = local_0; y[{}] = local_1;", i0, i1).map_err(|e| e.to_string())?;
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        let rhs_c = expr_to_c(rhs, &ctx)?;
                        emit_one_equation(lhs, &rhs_c, &ctx, out)?;
                    }
                }
                writeln!(out, "  }}").map_err(|e| e.to_string())?;
            }
            Equation::SolvableBlock {
                unknowns,
                equations: inner,
                residuals,
                ..
            } if residuals.len() == 3 && unknowns.len() >= 3 => {
                let u0 = &unknowns[0];
                let u1 = &unknowns[1];
                let u2 = &unknowns[2];
                let i0 = *ctx.output_index.get(u0).ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u0))?;
                let i1 = *ctx.output_index.get(u1).ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u1))?;
                let i2 = *ctx.output_index.get(u2).ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u2))?;
                let ov: Vec<(String, String)> = vec![
                    (u0.clone(), "local_0".to_string()),
                    (u1.clone(), "local_1".to_string()),
                    (u2.clone(), "local_2".to_string()),
                ];
                let ctx_inner = ctx.clone().with_overrides(&ov);
                writeln!(out, "  {{").map_err(|e| e.to_string())?;
                writeln!(out, "    double local_0 = y[{}], local_1 = y[{}], local_2 = y[{}];", i0, i1, i2).map_err(|e| e.to_string())?;
                writeln!(out, "    double eps = 1e-6; int iter;").map_err(|e| e.to_string())?;
                writeln!(out, "    for (iter = 0; iter < 50; iter++) {{").map_err(|e| e.to_string())?;
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                        emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                    }
                }
                let r0_c = expr_to_c(&residuals[0], &ctx_inner)?;
                let r1_c = expr_to_c(&residuals[1], &ctx_inner)?;
                let r2_c = expr_to_c(&residuals[2], &ctx_inner)?;
                writeln!(out, "      double r0 = {}, r1 = {}, r2 = {};", r0_c, r1_c, r2_c).map_err(|e| e.to_string())?;
                writeln!(out, "      if (fabs(r0) < 1e-8 && fabs(r1) < 1e-8 && fabs(r2) < 1e-8) break;").map_err(|e| e.to_string())?;
                for (col, local) in ["local_0", "local_1", "local_2"].iter().enumerate() {
                    writeln!(out, "      {} += eps;", local).map_err(|e| e.to_string())?;
                    for ieq in inner {
                        if let Equation::Simple(lhs, rhs) = ieq {
                            let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                            emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                        }
                    }
                    for (row, res) in residuals.iter().enumerate() {
                        let rp = expr_to_c(res, &ctx_inner)?;
                        writeln!(out, "      double J_{}_{} = ({} - r{}) / eps;", row, col, rp, row).map_err(|e| e.to_string())?;
                    }
                    writeln!(out, "      {} -= eps;", local).map_err(|e| e.to_string())?;
                }
                writeln!(out, "      double J00 = J_0_0, J01 = J_0_1, J02 = J_0_2, J10 = J_1_0, J11 = J_1_1, J12 = J_1_2, J20 = J_2_0, J21 = J_2_1, J22 = J_2_2;").map_err(|e| e.to_string())?;
                writeln!(out, "      double c0 = J11*J22 - J12*J21, c1 = J12*J20 - J10*J22, c2 = J10*J21 - J11*J20;").map_err(|e| e.to_string())?;
                writeln!(out, "      double det = J00*c0 + J01*c1 + J02*c2; if (fabs(det) < 1e-12) break;").map_err(|e| e.to_string())?;
                writeln!(out, "      double dx0 = (-r0*c0 - r1*c1 - r2*c2) / det;").map_err(|e| e.to_string())?;
                writeln!(out, "      c0 = J01*J22 - J02*J21; c1 = J02*J20 - J00*J22; c2 = J00*J21 - J01*J20;").map_err(|e| e.to_string())?;
                writeln!(out, "      double dx1 = (-r0*c0 - r1*c1 - r2*c2) / det;").map_err(|e| e.to_string())?;
                writeln!(out, "      c0 = J01*J12 - J02*J11; c1 = J02*J10 - J00*J12; c2 = J00*J11 - J01*J10;").map_err(|e| e.to_string())?;
                writeln!(out, "      double dx2 = (-r0*c0 - r1*c1 - r2*c2) / det;").map_err(|e| e.to_string())?;
                writeln!(out, "      local_0 += dx0; local_1 += dx1; local_2 += dx2;").map_err(|e| e.to_string())?;
                writeln!(out, "    }}").map_err(|e| e.to_string())?;
                writeln!(out, "    y[{}] = local_0; y[{}] = local_1; y[{}] = local_2;", i0, i1, i2).map_err(|e| e.to_string())?;
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        let rhs_c = expr_to_c(rhs, &ctx)?;
                        emit_one_equation(lhs, &rhs_c, &ctx, out)?;
                    }
                }
                writeln!(out, "  }}").map_err(|e| e.to_string())?;
            }
            Equation::SolvableBlock {
                unknowns,
                equations: inner,
                residuals,
                ..
            } if residuals.len() >= 1 && unknowns.len() >= 1 && unknowns.len() <= 32 && residuals.len() >= unknowns.len() => {
                let n = unknowns.len();
                let indices: Vec<usize> = unknowns.iter().take(n).map(|u| {
                    ctx.output_index.get(u).ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u)).copied()
                }).collect::<Result<Vec<_>, _>>()?;
                let ov: Vec<(String, String)> = unknowns.iter().take(n).enumerate()
                    .map(|(i, u)| (u.clone(), format!("local_[{}]", i))).collect();
                let ctx_inner = ctx.clone().with_overrides(&ov);
                writeln!(out, "  {{").map_err(|e| e.to_string())?;
                writeln!(out, "    double local_[32], res[32], J[32*32], dx[32]; int n = {};", n).map_err(|e| e.to_string())?;
                for (i, &idx) in indices.iter().enumerate() {
                    writeln!(out, "    local_[{}] = y[{}];", i, idx).map_err(|e| e.to_string())?;
                }
                writeln!(out, "    int iter; double eps = 1e-6;").map_err(|e| e.to_string())?;
                writeln!(out, "    for (iter = 0; iter < 50; iter++) {{").map_err(|e| e.to_string())?;
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        if matches!(lhs, Expression::Variable(_)) {
                            let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                            emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                        }
                    }
                }
                for (row, res_expr) in residuals.iter().take(n).enumerate() {
                    let c = expr_to_c(res_expr, &ctx_inner)?;
                    writeln!(out, "      res[{}] = {};", row, c).map_err(|e| e.to_string())?;
                }
                writeln!(out, "      {{ double max = 0; int row; for (row = 0; row < n; row++) {{ double a = fabs(res[row]); if (a > max) max = a; }} if (max < 1e-8) break; }}").map_err(|e| e.to_string())?;
                for col in 0..n {
                    writeln!(out, "      local_[{}] += eps;", col).map_err(|e| e.to_string())?;
                    for ieq in inner {
                        if let Equation::Simple(lhs, rhs) = ieq {
                            if matches!(lhs, Expression::Variable(_)) {
                                let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                                emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                            }
                        }
                    }
                    for (row, res_expr) in residuals.iter().take(n).enumerate() {
                        let rp = expr_to_c(res_expr, &ctx_inner)?;
                        writeln!(out, "      J[{}*n + {}] = ({} - res[{}]) / eps;", row, col, rp, row).map_err(|e| e.to_string())?;
                    }
                    writeln!(out, "      local_[{}] -= eps;", col).map_err(|e| e.to_string())?;
                }
                writeln!(out, "      for (int i = 0; i < n; i++) dx[i] = -res[i];").map_err(|e| e.to_string())?;
                writeln!(out, "      solve_dense(n, J, dx);").map_err(|e| e.to_string())?;
                writeln!(out, "      for (int i = 0; i < n; i++) local_[i] += dx[i];").map_err(|e| e.to_string())?;
                writeln!(out, "    }}").map_err(|e| e.to_string())?;
                for (i, &idx) in indices.iter().enumerate() {
                    writeln!(out, "    y[{}] = local_[{}];", idx, i).map_err(|e| e.to_string())?;
                }
                for ieq in inner {
                    if let Equation::Simple(lhs, rhs) = ieq {
                        if matches!(lhs, Expression::Variable(_)) {
                            let rhs_c = expr_to_c(rhs, &ctx)?;
                            emit_one_equation(lhs, &rhs_c, &ctx, out)?;
                        }
                    }
                }
                writeln!(out, "  }}").map_err(|e| e.to_string())?;
            }
            Equation::SolvableBlock { residuals, .. } => {
                return Err(format!(
                    "C codegen: SolvableBlock with {} residuals not supported (1 to 32 allowed)",
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
