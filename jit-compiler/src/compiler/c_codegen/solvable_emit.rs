//! C emission for `Equation::SolvableBlock` (Newton-style residuals in generated `residual()`).

use super::context::CCodegenContext;
use super::equation_emit::emit_one_equation;
use super::expr_emit::expr_to_c;
use crate::ast::{Equation, Expression};
use std::io::Write;

pub(super) fn sorted_eqs_need_solve_dense(sorted_eqs: &[Equation]) -> bool {
    sorted_eqs.iter().any(|eq| {
        if let Equation::SolvableBlock {
            residuals,
            unknowns,
            ..
        } = eq
        {
            residuals.len() >= 1
                && unknowns.len() >= 1
                && unknowns.len() <= 32
                && residuals.len() >= unknowns.len()
        } else {
            false
        }
    })
}

pub(super) fn emit_solve_dense_helper(out: &mut dyn Write) -> Result<(), String> {
    writeln!(
        out,
        "static void solve_dense(int n, double *J, double *b) {{"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "  double tmp; int i, j, k;").map_err(|e| e.to_string())?;
    writeln!(out, "  for (k = 0; k < n; k++) {{").map_err(|e| e.to_string())?;
    writeln!(out, "    int p = k; double max = fabs(J[k*n+k]);").map_err(|e| e.to_string())?;
    writeln!(out, "    for (i = k+1; i < n; i++) {{ double a = fabs(J[i*n+k]); if (a > max) {{ max = a; p = i; }} }}").map_err(|e| e.to_string())?;
    writeln!(out, "    if (max < 1e-12) return;").map_err(|e| e.to_string())?;
    writeln!(out, "    if (p != k) {{ for (j = 0; j < n; j++) {{ tmp = J[k*n+j]; J[k*n+j] = J[p*n+j]; J[p*n+j] = tmp; }} tmp = b[k]; b[k] = b[p]; b[p] = tmp; }}").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "    tmp = 1.0 / J[k*n+k]; for (j = k; j < n; j++) J[k*n+j] *= tmp; b[k] *= tmp;"
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "    for (i = 0; i < n; i++) {{ if (i == k) continue; double f = J[i*n+k]; for (j = k; j < n; j++) J[i*n+j] -= f * J[k*n+j]; b[i] -= f * b[k]; }}").map_err(|e| e.to_string())?;
    writeln!(out, "  }}").map_err(|e| e.to_string())?;
    writeln!(out, "}}").map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn emit_solvable_block_residual(
    eq: &Equation,
    ctx: &CCodegenContext<'_>,
    out: &mut dyn Write,
) -> Result<(), String> {
    match eq {
        Equation::SolvableBlock {
            unknowns,
            tearing_var: Some(_),
            equations: inner,
            residuals,
        } if residuals.len() == 1 => {
            let t_var = unknowns
                .first()
                .ok_or("C codegen: SolvableBlock empty unknowns")?;
            let tear_idx = ctx.output_index.get(t_var).ok_or_else(|| {
                format!("C codegen: tearing var '{}' not in output list", t_var)
            })?;
            writeln!(out, "  {{").map_err(|e| e.to_string())?;
            writeln!(out, "    double local_tear = y[{}];", tear_idx)
                .map_err(|e| e.to_string())?;
            writeln!(out, "    for (int iter = 0; iter < 50; iter++) {{")
                .map_err(|e| e.to_string())?;
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
            writeln!(out, "      double eps = 1e-6, old_tear = local_tear;")
                .map_err(|e| e.to_string())?;
            writeln!(out, "      local_tear += eps;").map_err(|e| e.to_string())?;
            for ieq in inner {
                if let Equation::Simple(lhs, rhs) = ieq {
                    let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                    emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                }
            }
            let res_pert_c = expr_to_c(&residuals[0], &ctx_inner)?;
            writeln!(out, "      double res_pert = {};", res_pert_c)
                .map_err(|e| e.to_string())?;
            writeln!(out, "      double J = (res_pert - res) / eps;")
                .map_err(|e| e.to_string())?;
            writeln!(out, "      if (fabs(J) < 1e-12) break;").map_err(|e| e.to_string())?;
            writeln!(out, "      local_tear = old_tear - res / J;")
                .map_err(|e| e.to_string())?;
            writeln!(out, "    }}").map_err(|e| e.to_string())?;
            writeln!(out, "    y[{}] = local_tear;", tear_idx).map_err(|e| e.to_string())?;
            for ieq in inner {
                if let Equation::Simple(lhs, rhs) = ieq {
                    let rhs_c = expr_to_c(rhs, &ctx)?;
                    emit_one_equation(lhs, &rhs_c, &ctx, out)?;
                }
            }
            writeln!(out, "  }}").map_err(|e| e.to_string())?;
            Ok(())
        }
        Equation::SolvableBlock {
            unknowns,
            equations: inner,
            residuals,
            ..
        } if residuals.len() == 2 && unknowns.len() >= 2 => {
            let u0 = &unknowns[0];
            let u1 = &unknowns[1];
            let i0 = *ctx
                .output_index
                .get(u0)
                .ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u0))?;
            let i1 = *ctx
                .output_index
                .get(u1)
                .ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u1))?;
            let ov: Vec<(String, String)> = vec![
                (u0.clone(), "local_0".to_string()),
                (u1.clone(), "local_1".to_string()),
            ];
            let ctx_inner = ctx.clone().with_overrides(&ov);
            writeln!(out, "  {{").map_err(|e| e.to_string())?;
            writeln!(out, "    double local_0 = y[{}], local_1 = y[{}];", i0, i1)
                .map_err(|e| e.to_string())?;
            writeln!(out, "    double eps = 1e-6; int iter;").map_err(|e| e.to_string())?;
            writeln!(out, "    for (iter = 0; iter < 50; iter++) {{")
                .map_err(|e| e.to_string())?;
            for ieq in inner {
                if let Equation::Simple(lhs, rhs) = ieq {
                    let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                    emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                }
            }
            let r0_c = expr_to_c(&residuals[0], &ctx_inner)?;
            let r1_c = expr_to_c(&residuals[1], &ctx_inner)?;
            writeln!(out, "      double r0 = {}, r1 = {};", r0_c, r1_c)
                .map_err(|e| e.to_string())?;
            writeln!(out, "      if (fabs(r0) < 1e-8 && fabs(r1) < 1e-8) break;")
                .map_err(|e| e.to_string())?;
            writeln!(out, "      local_0 += eps;").map_err(|e| e.to_string())?;
            for ieq in inner {
                if let Equation::Simple(lhs, rhs) = ieq {
                    let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                    emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                }
            }
            let r0p0 = expr_to_c(&residuals[0], &ctx_inner)?;
            let r1p0 = expr_to_c(&residuals[1], &ctx_inner)?;
            writeln!(
                out,
                "      double dr0_0 = ({} - r0) / eps, dr1_0 = ({} - r1) / eps;",
                r0p0, r1p0
            )
            .map_err(|e| e.to_string())?;
            writeln!(out, "      local_0 -= eps; local_1 += eps;")
                .map_err(|e| e.to_string())?;
            for ieq in inner {
                if let Equation::Simple(lhs, rhs) = ieq {
                    let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                    emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                }
            }
            let r0p1 = expr_to_c(&residuals[0], &ctx_inner)?;
            let r1p1 = expr_to_c(&residuals[1], &ctx_inner)?;
            writeln!(
                out,
                "      double dr0_1 = ({} - r0) / eps, dr1_1 = ({} - r1) / eps;",
                r0p1, r1p1
            )
            .map_err(|e| e.to_string())?;
            writeln!(out, "      local_1 -= eps;").map_err(|e| e.to_string())?;
            writeln!(
                out,
                "      double det = dr0_0*dr1_1 - dr0_1*dr1_0; if (fabs(det) < 1e-12) break;"
            )
            .map_err(|e| e.to_string())?;
            writeln!(out, "      double dx0 = (-r0*dr1_1 + r1*dr0_1) / det, dx1 = (r0*dr1_0 - r1*dr0_0) / det;").map_err(|e| e.to_string())?;
            writeln!(out, "      local_0 += dx0; local_1 += dx1;")
                .map_err(|e| e.to_string())?;
            writeln!(out, "    }}").map_err(|e| e.to_string())?;
            writeln!(out, "    y[{}] = local_0; y[{}] = local_1;", i0, i1)
                .map_err(|e| e.to_string())?;
            for ieq in inner {
                if let Equation::Simple(lhs, rhs) = ieq {
                    let rhs_c = expr_to_c(rhs, &ctx)?;
                    emit_one_equation(lhs, &rhs_c, &ctx, out)?;
                }
            }
            writeln!(out, "  }}").map_err(|e| e.to_string())?;
            Ok(())
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
            let i0 = *ctx
                .output_index
                .get(u0)
                .ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u0))?;
            let i1 = *ctx
                .output_index
                .get(u1)
                .ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u1))?;
            let i2 = *ctx
                .output_index
                .get(u2)
                .ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u2))?;
            let ov: Vec<(String, String)> = vec![
                (u0.clone(), "local_0".to_string()),
                (u1.clone(), "local_1".to_string()),
                (u2.clone(), "local_2".to_string()),
            ];
            let ctx_inner = ctx.clone().with_overrides(&ov);
            writeln!(out, "  {{").map_err(|e| e.to_string())?;
            writeln!(
                out,
                "    double local_0 = y[{}], local_1 = y[{}], local_2 = y[{}];",
                i0, i1, i2
            )
            .map_err(|e| e.to_string())?;
            writeln!(out, "    double eps = 1e-6; int iter;").map_err(|e| e.to_string())?;
            writeln!(out, "    for (iter = 0; iter < 50; iter++) {{")
                .map_err(|e| e.to_string())?;
            for ieq in inner {
                if let Equation::Simple(lhs, rhs) = ieq {
                    let rhs_c = expr_to_c(rhs, &ctx_inner)?;
                    emit_one_equation(lhs, &rhs_c, &ctx_inner, out)?;
                }
            }
            let r0_c = expr_to_c(&residuals[0], &ctx_inner)?;
            let r1_c = expr_to_c(&residuals[1], &ctx_inner)?;
            let r2_c = expr_to_c(&residuals[2], &ctx_inner)?;
            writeln!(
                out,
                "      double r0 = {}, r1 = {}, r2 = {};",
                r0_c, r1_c, r2_c
            )
            .map_err(|e| e.to_string())?;
            writeln!(
                out,
                "      if (fabs(r0) < 1e-8 && fabs(r1) < 1e-8 && fabs(r2) < 1e-8) break;"
            )
            .map_err(|e| e.to_string())?;
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
                    writeln!(
                        out,
                        "      double J_{}_{} = ({} - r{}) / eps;",
                        row, col, rp, row
                    )
                    .map_err(|e| e.to_string())?;
                }
                writeln!(out, "      {} -= eps;", local).map_err(|e| e.to_string())?;
            }
            writeln!(out, "      double J00 = J_0_0, J01 = J_0_1, J02 = J_0_2, J10 = J_1_0, J11 = J_1_1, J12 = J_1_2, J20 = J_2_0, J21 = J_2_1, J22 = J_2_2;").map_err(|e| e.to_string())?;
            writeln!(out, "      double c0 = J11*J22 - J12*J21, c1 = J12*J20 - J10*J22, c2 = J10*J21 - J11*J20;").map_err(|e| e.to_string())?;
            writeln!(
                out,
                "      double det = J00*c0 + J01*c1 + J02*c2; if (fabs(det) < 1e-12) break;"
            )
            .map_err(|e| e.to_string())?;
            writeln!(out, "      double dx0 = (-r0*c0 - r1*c1 - r2*c2) / det;")
                .map_err(|e| e.to_string())?;
            writeln!(
                out,
                "      c0 = J01*J22 - J02*J21; c1 = J02*J20 - J00*J22; c2 = J00*J21 - J01*J20;"
            )
            .map_err(|e| e.to_string())?;
            writeln!(out, "      double dx1 = (-r0*c0 - r1*c1 - r2*c2) / det;")
                .map_err(|e| e.to_string())?;
            writeln!(
                out,
                "      c0 = J01*J12 - J02*J11; c1 = J02*J10 - J00*J12; c2 = J00*J11 - J01*J10;"
            )
            .map_err(|e| e.to_string())?;
            writeln!(out, "      double dx2 = (-r0*c0 - r1*c1 - r2*c2) / det;")
                .map_err(|e| e.to_string())?;
            writeln!(out, "      local_0 += dx0; local_1 += dx1; local_2 += dx2;")
                .map_err(|e| e.to_string())?;
            writeln!(out, "    }}").map_err(|e| e.to_string())?;
            writeln!(
                out,
                "    y[{}] = local_0; y[{}] = local_1; y[{}] = local_2;",
                i0, i1, i2
            )
            .map_err(|e| e.to_string())?;
            for ieq in inner {
                if let Equation::Simple(lhs, rhs) = ieq {
                    let rhs_c = expr_to_c(rhs, &ctx)?;
                    emit_one_equation(lhs, &rhs_c, &ctx, out)?;
                }
            }
            writeln!(out, "  }}").map_err(|e| e.to_string())?;
            Ok(())
        }
        Equation::SolvableBlock {
            unknowns,
            equations: inner,
            residuals,
            ..
        } if residuals.len() >= 1
            && unknowns.len() >= 1
            && unknowns.len() <= 32
            && residuals.len() >= unknowns.len() =>
        {
            let n = unknowns.len();
            let indices: Vec<usize> = unknowns
                .iter()
                .take(n)
                .map(|u| {
                    ctx.output_index
                        .get(u)
                        .ok_or_else(|| format!("C codegen: unknown '{}' not in output list", u))
                        .copied()
                })
                .collect::<Result<Vec<_>, _>>()?;
            let ov: Vec<(String, String)> = unknowns
                .iter()
                .take(n)
                .enumerate()
                .map(|(i, u)| (u.clone(), format!("local_[{}]", i)))
                .collect();
            let ctx_inner = ctx.clone().with_overrides(&ov);
            writeln!(out, "  {{").map_err(|e| e.to_string())?;
            writeln!(
                out,
                "    double local_[32], res[32], J[32*32], dx[32]; int n = {};",
                n
            )
            .map_err(|e| e.to_string())?;
            for (i, &idx) in indices.iter().enumerate() {
                writeln!(out, "    local_[{}] = y[{}];", i, idx).map_err(|e| e.to_string())?;
            }
            writeln!(out, "    int iter; double eps = 1e-6;").map_err(|e| e.to_string())?;
            writeln!(out, "    for (iter = 0; iter < 50; iter++) {{")
                .map_err(|e| e.to_string())?;
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
                    writeln!(
                        out,
                        "      J[{}*n + {}] = ({} - res[{}]) / eps;",
                        row, col, rp, row
                    )
                    .map_err(|e| e.to_string())?;
                }
                writeln!(out, "      local_[{}] -= eps;", col).map_err(|e| e.to_string())?;
            }
            writeln!(out, "      for (int i = 0; i < n; i++) dx[i] = -res[i];")
                .map_err(|e| e.to_string())?;
            writeln!(out, "      solve_dense(n, J, dx);").map_err(|e| e.to_string())?;
            writeln!(out, "      for (int i = 0; i < n; i++) local_[i] += dx[i];")
                .map_err(|e| e.to_string())?;
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
            Ok(())
        }
        Equation::SolvableBlock { residuals, .. } => Err(format!(
            "C codegen: SolvableBlock with {} residuals not supported (1 to 32 allowed)",
            residuals.len()
        )),
        _ => Err("internal: emit_solvable_block_residual expects SolvableBlock".to_string()),
    }
}
