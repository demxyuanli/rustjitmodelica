// Evaluate scalar Real expressions for function entry (F3-1). No der/time/state.

use std::collections::HashMap;

use crate::ast::{Expression, Operator};

fn eval_builtin(name: &str, args: &[f64]) -> Result<f64, String> {
    let n = name;
    let a = |i: usize| args.get(i).copied().unwrap_or(0.0);
    match n {
        "abs" => Ok(a(0).abs()),
        "sign" => Ok(if a(0) > 0.0 {
            1.0
        } else if a(0) < 0.0 {
            -1.0
        } else {
            0.0
        }),
        "sqrt" => {
            if a(0) < 0.0 {
                return Err("sqrt of negative".into());
            }
            Ok(a(0).sqrt())
        }
        "min" => Ok(a(0).min(a(1))),
        "max" => Ok(a(0).max(a(1))),
        "mod" => Ok({
            let (x, y) = (a(0), a(1));
            if y == 0.0 {
                return Err("mod by zero".into());
            }
            x - (x / y).floor() * y
        }),
        "rem" => Ok({
            let (x, y) = (a(0), a(1));
            if y == 0.0 {
                return Err("rem by zero".into());
            }
            let q = (x / y).trunc();
            x - q * y
        }),
        "div" => Ok(if a(1) == 0.0 {
            return Err("div by zero".into());
        } else {
            (a(0) / a(1)).trunc()
        }),
        "integer" => Ok(a(0).trunc()),
        "ceil" => Ok(a(0).ceil()),
        "floor" => Ok(a(0).floor()),
        "sin" => Ok(a(0).sin()),
        "cos" => Ok(a(0).cos()),
        "tan" => Ok(a(0).tan()),
        "asin" => Ok(a(0).asin()),
        "acos" => Ok(a(0).acos()),
        "atan" => Ok(a(0).atan()),
        "atan2" => Ok(a(0).atan2(a(1))),
        "sinh" => Ok(a(0).sinh()),
        "cosh" => Ok(a(0).cosh()),
        "tanh" => Ok(a(0).tanh()),
        "exp" => Ok(a(0).exp()),
        "log" => Ok({
            if a(0) <= 0.0 {
                return Err("log non-positive".into());
            }
            a(0).ln()
        }),
        "log10" => Ok({
            if a(0) <= 0.0 {
                return Err("log10 non-positive".into());
            }
            a(0).log10()
        }),
        _ if n.starts_with("Modelica.Math.") => {
            let suffix = n.strip_prefix("Modelica.Math.").unwrap_or(n);
            match suffix {
                "sin" => Ok(a(0).sin()),
                "cos" => Ok(a(0).cos()),
                "tan" => Ok(a(0).tan()),
                "exp" => Ok(a(0).exp()),
                "log" => Ok(if a(0) <= 0.0 {
                    return Err("log <= 0".into());
                } else {
                    a(0).ln()
                }),
                "sqrt" => Ok(if a(0) < 0.0 {
                    return Err("sqrt negative".into());
                } else {
                    a(0).sqrt()
                }),
                "abs" => Ok(a(0).abs()),
                _ => Err(format!("unknown built-in: {}", n)),
            }
        }
        _ => Err(format!("unknown built-in: {}", n)),
    }
}

/// Evaluate expression to a single Real with given variable bindings. Fails on der/time/array/when.
pub fn eval_expr(expr: &Expression, vars: &HashMap<String, f64>) -> Result<f64, String> {
    use Expression::*;
    match expr {
        Variable(name) => vars
            .get(name)
            .copied()
            .ok_or_else(|| format!("unknown variable: {}", name)),
        Number(x) => Ok(*x),
        BinaryOp(l, op, r) => {
            let lv = eval_expr(l, vars)?;
            let rv = eval_expr(r, vars)?;
            match op {
                Operator::Add => Ok(lv + rv),
                Operator::Sub => Ok(lv - rv),
                Operator::Mul => Ok(lv * rv),
                Operator::Div => {
                    if rv == 0.0 {
                        return Err("division by zero".into());
                    }
                    Ok(lv / rv)
                }
                Operator::Less => Ok(if lv < rv { 1.0 } else { 0.0 }),
                Operator::Greater => Ok(if lv > rv { 1.0 } else { 0.0 }),
                Operator::LessEq => Ok(if lv <= rv { 1.0 } else { 0.0 }),
                Operator::GreaterEq => Ok(if lv >= rv { 1.0 } else { 0.0 }),
                Operator::Equal => Ok(if lv == rv { 1.0 } else { 0.0 }),
                Operator::NotEqual => Ok(if lv != rv { 1.0 } else { 0.0 }),
                Operator::And => Ok(if lv != 0.0 && rv != 0.0 { 1.0 } else { 0.0 }),
                Operator::Or => Ok(if lv != 0.0 || rv != 0.0 { 1.0 } else { 0.0 }),
            }
        }
        Call(name, args) => {
            let evaled: Result<Vec<f64>, _> = args.iter().map(|a| eval_expr(a, vars)).collect();
            let args_f = evaled?;
            eval_builtin(name, &args_f)
        }
        Der(_) => Err("der() not supported in function entry eval".into()),
        ArrayAccess(..) | Dot(..) | Range(..) | ArrayLiteral(..) | ArrayComprehension { .. } => {
            Err("array/dot/range not supported in function entry eval".into())
        }
        StringLiteral(_) => Err("string literal not supported in function entry eval (use JIT or C)".into()),
        If(cond, t, f) => {
            let c = eval_expr(cond, vars)?;
            if c != 0.0 {
                eval_expr(t, vars)
            } else {
                eval_expr(f, vars)
            }
        }
        Sample(_) | Interval(_) | Hold(_) | Previous(_) | SubSample(_, _) | SuperSample(_, _) | ShiftSample(_, _) => Err("sample()/interval()/hold()/previous()/subSample/superSample/shiftSample not supported in eval (SYNC)".into()),
    }
}
