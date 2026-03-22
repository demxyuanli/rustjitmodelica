use super::context::{get_array_layout_info, CCodegenContext};
use super::is_c_builtin;
use crate::ast::{Expression, Operator};

/// FUNC-7: Escape string for C literal (backslash and double-quote).
pub(super) fn escape_c_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Convert Expression to C source string. Uses t, x[], xdot[], p[], y[] from context.
pub fn expr_to_c(expr: &Expression, ctx: &CCodegenContext) -> Result<String, String> {
    use Expression::*;
    match expr {
        Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            ctx.var_to_c(&name)
        }
        StringLiteral(s) => Ok(escape_c_string(s)),
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
            if let Variable(id) = inner.as_ref() {
                let name = crate::string_intern::resolve_id(*id);
                if let Some(&i) = ctx.state_index.get(&name) {
                    return Ok(format!("xdot[{}]", i));
                }
            }
            Err(format!("C codegen: der() only for state, got {}", base))
        }
        Call(name, args) => {
            let args_c: Vec<String> = args
                .iter()
                .map(|a| expr_to_c(a, ctx))
                .collect::<Result<Vec<_>, _>>()?;
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
                _ => {
                    if ctx
                        .external_fns
                        .as_ref()
                        .map_or(false, |s| s.contains(name))
                    {
                        let mut args_c = Vec::new();
                        for a in args {
                            if let Variable(id) = a {
                                let var_name = crate::string_intern::resolve_id(*id);
                                if let Some((base, start, size)) =
                                    get_array_layout_info(ctx, &var_name)
                                {
                                    args_c.push(format!("&{}[{}]", base, start));
                                    args_c.push(format!("{}", size));
                                    continue;
                                }
                            }
                            if let StringLiteral(s) = a {
                                args_c.push(escape_c_string(s));
                                continue;
                            }
                            args_c.push(expr_to_c(a, ctx)?);
                        }
                        let c_name = ctx
                            .external_c_names
                            .as_ref()
                            .and_then(|m| m.get(name))
                            .map(String::as_str)
                            .unwrap_or_else(|| name.as_str())
                            .replace('.', "_");
                        Ok(format!("{}({})", c_name, args_c.join(", ")))
                    } else if is_c_builtin(name) {
                        Ok(format!("{}({})", name, args_str))
                    } else {
                        Err(format!("C codegen: unsupported function '{}'", name))
                    }
                }
            }
        }
        If(cond, then_e, else_e) => {
            let c = expr_to_c(cond, ctx)?;
            let th = expr_to_c(then_e, ctx)?;
            let el = expr_to_c(else_e, ctx)?;
            Ok(format!("(({}) ? ({}) : ({}))", c, th, el))
        }
        ArrayAccess(arr, idx) => {
            if let Variable(id) = arr.as_ref() {
                let arr_name = crate::string_intern::resolve_id(*id);
                let idx_c = expr_to_c(idx, ctx)?;
                if let Some(&i) = ctx.state_index.get(&arr_name) {
                    return Ok(format!("x[{} + (int)({})]", i, idx_c));
                }
                if let Some(&i) = ctx.output_index.get(&arr_name) {
                    return Ok(format!("y[{} + (int)({})]", i, idx_c));
                }
                if let Some(&i) = ctx.param_index.get(&arr_name) {
                    return Ok(format!("p[{} + (int)({})]", i, idx_c));
                }
            }
            Err("C codegen: array base must be known variable".to_string())
        }
        Dot(_, _) | Range(_, _, _) | ArrayLiteral(_) | ArrayComprehension { .. } => {
            Err("C codegen: Dot/Range/ArrayLiteral not supported (flatten first)".to_string())
        }
        Sample(_) | Interval(_) => Err(
            "C codegen: sample()/interval() not supported (SYNC-1); use when/zero-crossing"
                .to_string(),
        ),
        Hold(inner) => expr_to_c(inner, ctx),
        Previous(_inner) => {
            Err("C codegen: previous() not supported in C emission (use pre or JIT)".to_string())
        }
        SubSample(_c, _n) | SuperSample(_c, _n) | ShiftSample(_c, _n) => {
            Err("C codegen: subSample/superSample/shiftSample not supported (SYNC-5)".to_string())
        }
    }
}
