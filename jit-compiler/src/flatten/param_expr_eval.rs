//! Constant folding with parameter `Expression` bindings (cross-references) and array sizes.

use crate::ast::{Expression, Operator};
use crate::flatten::expressions::expr_to_path;
use crate::flatten::real_fft_sample_points::msl_real_fft_sample_points;
use std::collections::{HashMap, HashSet};

const MAX_DEPTH: usize = 512;

/// MSL `Modelica.Electrical.Polyphase.Functions.numberOfSymmetricBaseSystems` (same algorithm in QS.Polyphase.Functions).
fn number_of_symmetric_base_systems_i64(m: i64) -> i64 {
    if m <= 0 {
        return 0;
    }
    if m % 2 == 0 {
        if m == 2 {
            1
        } else {
            2 * number_of_symmetric_base_systems_i64(m / 2)
        }
    } else {
        1
    }
}

fn lookup_array_size_dimension(
    array_sizes: &HashMap<String, usize>,
    path: &str,
    dim_1_based: i64,
) -> Option<f64> {
    if dim_1_based <= 0 {
        return None;
    }
    if dim_1_based > 1 {
        return Some(1.0);
    }
    if let Some(sz) = array_sizes.get(path) {
        return Some(*sz as f64);
    }
    let underscored = path.replace('.', "_");
    if underscored != path {
        if let Some(sz) = array_sizes.get(&underscored) {
            return Some(*sz as f64);
        }
    }
    if let Some((_prefix, leaf)) = path.rsplit_once('.') {
        if let Some(sz) = array_sizes.get(leaf) {
            return Some(*sz as f64);
        }
    }
    None
}

fn eval_size_call(
    args: &[Expression],
    bindings: &HashMap<String, Expression>,
    array_sizes: &HashMap<String, usize>,
    visiting: &mut HashSet<String>,
    depth: usize,
) -> Option<f64> {
    if args.is_empty() || args.len() > 2 {
        return None;
    }
    let dim_1_based = if args.len() == 1 {
        1_i64
    } else {
        let d = eval_pe_inner(&args[1], bindings, array_sizes, visiting, depth + 1)?;
        if !d.is_finite() {
            return None;
        }
        d.round() as i64
    };
    match &args[0] {
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            lookup_array_size_dimension(array_sizes, &name, dim_1_based)
        }
        Expression::Dot(_, _) => {
            let path = expr_to_path(&args[0])?;
            lookup_array_size_dimension(array_sizes, &path, dim_1_based)
        }
        Expression::ArrayLiteral(items) => {
            if dim_1_based <= 1 {
                Some(items.len() as f64)
            } else {
                Some(1.0)
            }
        }
        Expression::Number(_) => Some(1.0),
        _ => None,
    }
}

pub fn eval_const_expr_with_param_exprs(
    expr: &Expression,
    bindings: &HashMap<String, Expression>,
    array_sizes: &HashMap<String, usize>,
) -> Option<f64> {
    let mut visiting = HashSet::new();
    eval_pe_inner(expr, bindings, array_sizes, &mut visiting, 0)
}

pub fn eval_const_expr_with_array_sizes(
    expr: &Expression,
    array_sizes: &HashMap<String, usize>,
) -> Option<f64> {
    eval_const_expr_with_param_exprs(expr, &HashMap::new(), array_sizes)
}

fn eval_pe_inner(
    expr: &Expression,
    bindings: &HashMap<String, Expression>,
    array_sizes: &HashMap<String, usize>,
    visiting: &mut HashSet<String>,
    depth: usize,
) -> Option<f64> {
    if depth > MAX_DEPTH {
        return None;
    }
    match expr {
        Expression::Number(n) => Some(*n),
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            if visiting.contains(&name) {
                return None;
            }
            if let Some(b) = bindings.get(&name) {
                visiting.insert(name.clone());
                let r = eval_pe_inner(b, bindings, array_sizes, visiting, depth + 1);
                visiting.remove(&name);
                return r;
            }
            let underscore_name = name.replace('.', "_");
            if underscore_name != name {
                if let Some(b) = bindings.get(&underscore_name) {
                    visiting.insert(underscore_name.clone());
                    let r = eval_pe_inner(b, bindings, array_sizes, visiting, depth + 1);
                    visiting.remove(&underscore_name);
                    return r;
                }
            }
            match name.as_str() {
                "Modelica.Constants.pi" | "Modelica_Constants_pi" => Some(std::f64::consts::PI),
                "Modelica.Constants.eps" | "Modelica_Constants_eps" => Some(f64::EPSILON),
                "Modelica.Constants.small" | "Modelica_Constants_small" => Some(1.0e-60),
                "Modelica.Constants.inf" | "Modelica_Constants_inf" => Some(f64::INFINITY),
                "Modelica.Constants.g_n" | "Modelica_Constants_g_n" => Some(9.80665),
                "Modelica.Constants.T_zero" => Some(273.15),
                "Modelica.Constants.sigma" => Some(5.670374419e-8),
                "Modelica.Constants.R" => Some(8.314462618),
                "Modelica.Constants.N_A" => Some(6.02214076e23),
                _ => {
                    if let Some(sz) = array_sizes.get(&name) {
                        Some(*sz as f64)
                    } else {
                        None
                    }
                }
            }
        }
        Expression::Dot(base, member) => {
            if let Some(base_path) = expr_to_path(base) {
                let dot_name = format!("{}.{}", base_path, member);
                if let Some(b) = bindings.get(&dot_name) {
                    return eval_pe_inner(b, bindings, array_sizes, visiting, depth + 1);
                }
                let underscore_name = dot_name.replace('.', "_");
                if let Some(b) = bindings.get(&underscore_name) {
                    return eval_pe_inner(b, bindings, array_sizes, visiting, depth + 1);
                }
            }
            None
        }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = eval_pe_inner(lhs, bindings, array_sizes, visiting, depth + 1)?;
            let r = eval_pe_inner(rhs, bindings, array_sizes, visiting, depth + 1)?;
            match op {
                Operator::Add => Some(l + r),
                Operator::Sub => Some(l - r),
                Operator::Mul => Some(l * r),
                Operator::Div if r != 0.0 => Some(l / r),
                Operator::Less => Some(if l < r { 1.0 } else { 0.0 }),
                Operator::Greater => Some(if l > r { 1.0 } else { 0.0 }),
                Operator::LessEq => Some(if l <= r { 1.0 } else { 0.0 }),
                Operator::GreaterEq => Some(if l >= r { 1.0 } else { 0.0 }),
                Operator::Equal => Some(if (l - r).abs() < 1e-15 { 1.0 } else { 0.0 }),
                Operator::NotEqual => Some(if (l - r).abs() >= 1e-15 { 1.0 } else { 0.0 }),
                Operator::And => Some(if l != 0.0 && r != 0.0 { 1.0 } else { 0.0 }),
                Operator::Or => Some(if l != 0.0 || r != 0.0 { 1.0 } else { 0.0 }),
                _ => None,
            }
        }
        Expression::If(cond, t_expr, f_expr) => {
            let c = eval_pe_inner(cond, bindings, array_sizes, visiting, depth + 1)?;
            if c != 0.0 {
                eval_pe_inner(t_expr, bindings, array_sizes, visiting, depth + 1)
            } else {
                eval_pe_inner(f_expr, bindings, array_sizes, visiting, depth + 1)
            }
        }
        Expression::Call(func, args) => {
            let func_lower = func.to_lowercase();
            let func_tail = func_lower.rsplit('.').next().unwrap_or(&func_lower);
            if func_tail == "realfftsamplepoints" {
                return eval_real_fft_sample_points_call(args, bindings, array_sizes, visiting, depth);
            }
            if func == "size" || func_tail == "size" {
                return eval_size_call(args, bindings, array_sizes, visiting, depth);
            }
            if func_tail == "numberofsymmetricbasesystems" && args.len() == 1 {
                let mv = eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)?;
                if !mv.is_finite() {
                    return None;
                }
                let m = mv.round() as i64;
                let n = number_of_symmetric_base_systems_i64(m);
                return Some(n as f64);
            }
            match func_tail {
                "sin" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.sin())
                }
                "cos" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.cos())
                }
                "tan" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.tan())
                }
                "asin" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.asin())
                }
                "acos" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.acos())
                }
                "atan" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.atan())
                }
                "atan2" if args.len() == 2 => {
                    let y = eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)?;
                    let x = eval_pe_inner(&args[1], bindings, array_sizes, visiting, depth + 1)?;
                    Some(y.atan2(x))
                }
                "sqrt" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.sqrt())
                }
                "abs" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.abs())
                }
                "exp" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.exp())
                }
                "log" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.ln())
                }
                "log10" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.log10())
                }
                "pow" if args.len() == 2 => {
                    let base = eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)?;
                    let exp = eval_pe_inner(&args[1], bindings, array_sizes, visiting, depth + 1)?;
                    Some(base.powf(exp))
                }
                "max" if args.len() == 2 => {
                    let a = eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)?;
                    let b = eval_pe_inner(&args[1], bindings, array_sizes, visiting, depth + 1)?;
                    Some(a.max(b))
                }
                "min" if args.len() == 2 => {
                    let a = eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)?;
                    let b = eval_pe_inner(&args[1], bindings, array_sizes, visiting, depth + 1)?;
                    Some(a.min(b))
                }
                "sign" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.signum())
                }
                "floor" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.floor())
                }
                "ceil" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.ceil())
                }
                "integer" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.floor())
                }
                "mod" if args.len() == 2 => {
                    let a = eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)?;
                    let b = eval_pe_inner(&args[1], bindings, array_sizes, visiting, depth + 1)?;
                    if b != 0.0 { Some(a % b) } else { None }
                }
                "rem" if args.len() == 2 => {
                    let a = eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)?;
                    let b = eval_pe_inner(&args[1], bindings, array_sizes, visiting, depth + 1)?;
                    if b != 0.0 { Some(a % b) } else { None }
                }
                "div" if args.len() == 2 => {
                    let a = eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)?;
                    let b = eval_pe_inner(&args[1], bindings, array_sizes, visiting, depth + 1)?;
                    if b != 0.0 { Some((a / b).trunc()) } else { None }
                }
                "sinh" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.sinh())
                }
                "cosh" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.cosh())
                }
                "tanh" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1).map(|v| v.tanh())
                }
                "noevent" | "smooth" if !args.is_empty() => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)
                }
                "homotopy" if args.len() >= 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)
                }
                "fill" if args.len() >= 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)
                }
                "scalar" if args.len() == 1 => {
                    eval_pe_inner(&args[0], bindings, array_sizes, visiting, depth + 1)
                }
                _ => None,
            }
        }
        Expression::ArrayLiteral(items) => {
            if items.len() == 1 {
                eval_pe_inner(&items[0], bindings, array_sizes, visiting, depth + 1)
            } else if !items.is_empty() {
                eval_pe_inner(&items[0], bindings, array_sizes, visiting, depth + 1)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn eval_real_fft_sample_points_call(
    args: &[Expression],
    bindings: &HashMap<String, Expression>,
    array_sizes: &HashMap<String, usize>,
    visiting: &mut HashSet<String>,
    depth: usize,
) -> Option<f64> {
    let mut positional: Vec<&Expression> = Vec::new();
    let mut f_max_factor: i64 = 5;
    for arg in args {
        if let Expression::Call(fname, nargs) = arg {
            if fname == "named" && nargs.len() == 2 {
                if let Expression::StringLiteral(nm) = &nargs[0] {
                    if nm == "f_max_factor" {
                        if let Some(v) =
                            eval_pe_inner(&nargs[1], bindings, array_sizes, visiting, depth + 1)
                        {
                            f_max_factor = v as i64;
                        }
                    }
                }
                continue;
            }
        }
        positional.push(arg);
    }
    if positional.len() < 2 {
        return None;
    }
    let f_max = eval_pe_inner(positional[0], bindings, array_sizes, visiting, depth + 1)?;
    let f_res = eval_pe_inner(positional[1], bindings, array_sizes, visiting, depth + 1)?;
    let ns = msl_real_fft_sample_points(f_max, f_res, f_max_factor)?;
    Some(ns as f64)
}
