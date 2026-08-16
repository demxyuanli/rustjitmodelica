//! Scalarized MultiBody Frames / gravity calls: `f(full_args)[i]`.
//! Orientation is the flatten Real[4] quaternion approximation.

use crate::ast::{Expression, Operator};
use cranelift::prelude::*;

use crate::jit::context::TranslationContext;
use super::helpers::jit_scalar_name_bound;

type CompileRec = fn(
    &Expression,
    &mut TranslationContext,
    &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String>;

pub(super) fn compile_vector_call_component(
    func_name: &str,
    args: &[Expression],
    idx: usize,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Value, String> {
    if idx == 0 {
        return Ok(builder.ins().f64const(0.0));
    }
    let tail = func_name.rsplit('.').next().unwrap_or(func_name);
    match tail {
        "zeros" | "fill" => Ok(builder.ins().f64const(0.0)),
        "ones" => Ok(builder.ins().f64const(1.0)),
        "identity" => {
            // identity(n)[i] is not a matrix element; scalar lowering uses 0.
            Ok(builder.ins().f64const(if idx == 1 { 1.0 } else { 0.0 }))
        }
        "nullRotation" => {
            // Quaternions.nullRotation = {0,0,0,1}
            Ok(builder.ins().f64const(if idx == 4 { 1.0 } else { 0.0 }))
        }
        "from_Q" | "from_nxy" | "from_nxz" | "from_T" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            load_indexed_arg(&args[0], idx, ctx, builder, compile_rec)
        }
        "normalize" | "normalizeWithAssert" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            let v = load_vec3(&args[0], ctx, builder, compile_rec)?;
            let nrm = vec3_norm(builder, v);
            let eps = builder.ins().f64const(1e-12);
            let den = builder.ins().fmax(nrm, eps);
            let c = vec3_get(v, idx).unwrap_or_else(|| builder.ins().f64const(0.0));
            Ok(builder.ins().fdiv(c, den))
        }
        "cross" => {
            if args.len() < 2 {
                return Ok(builder.ins().f64const(0.0));
            }
            let a = load_vec3(&args[0], ctx, builder, compile_rec)?;
            let b = load_vec3(&args[1], ctx, builder, compile_rec)?;
            let c = vec3_cross(builder, a, b);
            Ok(vec3_get(c, idx).unwrap_or_else(|| builder.ins().f64const(0.0)))
        }
        "gravityAcceleration" | "GravityAcceleration" | "standardGravityAcceleration" => {
            compile_gravity_component(func_name, idx, ctx, builder, compile_rec)
        }
        "planarRotation" => compile_planar_rotation_component(args, idx, ctx, builder, compile_rec),
        "absoluteRotation" => {
            compile_absolute_rotation_component(args, idx, ctx, builder, compile_rec)
        }
        "resolve1" => compile_resolve_component(args, idx, true, ctx, builder, compile_rec),
        "resolve2" => compile_resolve_component(args, idx, false, ctx, builder, compile_rec),
        "angularVelocity1" | "angularVelocity2" => {
            compile_angular_velocity_component(func_name, args, idx, ctx, builder, compile_rec)
        }
        _ => {
            if args.is_empty() {
                Ok(builder.ins().f64const(0.0))
            } else {
                load_indexed_arg(&args[0], idx, ctx, builder, compile_rec)
            }
        }
    }
}

fn compile_gravity_component(
    func_name: &str,
    idx: usize,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Value, String> {
    let inst = gravity_instance_prefix(func_name);
    let g_name = format!("{inst}_g");
    let n_base = format!("{inst}_n");
    let g = if jit_scalar_name_bound(ctx, &g_name) {
        load_scalar_named(&g_name, 9.80665, ctx, builder, compile_rec)?
    } else {
        load_scalar_named("world_g", 9.80665, ctx, builder, compile_rec)?
    };
    let n_expr = if (1..=3).all(|i| jit_scalar_name_bound(ctx, &format!("{n_base}_{i}"))) {
        Expression::var(&n_base)
    } else {
        Expression::var("world_n")
    };
    let n = load_vec3(&n_expr, ctx, builder, compile_rec)?;
    let nrm = vec3_norm(builder, n);
    let eps = builder.ins().f64const(1e-12);
    let den = builder.ins().fmax(nrm, eps);
    let ni = vec3_get(n, idx).unwrap_or_else(|| builder.ins().f64const(0.0));
    let unit = builder.ins().fdiv(ni, den);
    Ok(builder.ins().fmul(g, unit))
}

fn compile_planar_rotation_component(
    args: &[Expression],
    idx: usize,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Value, String> {
    // Q = {e*sin(angle/2), cos(angle/2)}
    if args.len() < 2 {
        return Ok(builder.ins().f64const(if idx == 4 { 1.0 } else { 0.0 }));
    }
    let e = load_vec3(&args[0], ctx, builder, compile_rec)?;
    let half = Expression::BinaryOp(
        Box::new(args[1].clone()),
        Operator::Mul,
        Box::new(Expression::Number(0.5)),
    );
    let s = compile_rec(
        &Expression::Call("sin".to_string(), vec![half.clone()]),
        ctx,
        builder,
    )?;
    let c = compile_rec(&Expression::Call("cos".to_string(), vec![half]), ctx, builder)?;
    if idx == 4 {
        return Ok(c);
    }
    let ei = vec3_get(e, idx).unwrap_or_else(|| builder.ins().f64const(0.0));
    Ok(builder.ins().fmul(ei, s))
}

fn compile_absolute_rotation_component(
    args: &[Expression],
    idx: usize,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Value, String> {
    if args.len() < 2 {
        return Ok(builder.ins().f64const(0.0));
    }
    let q1 = load_quat(&args[0], ctx, builder, compile_rec)?;
    let qr = load_quat(&args[1], ctx, builder, compile_rec)?;
    let q2 = quat_abs_rotate(builder, q1, qr);
    Ok(quat_get(q2, idx).unwrap_or_else(|| builder.ins().f64const(0.0)))
}

fn compile_resolve_component(
    args: &[Expression],
    idx: usize,
    resolve1: bool,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Value, String> {
    if args.len() < 2 {
        return Ok(builder.ins().f64const(0.0));
    }
    let q = load_quat(&args[0], ctx, builder, compile_rec)?;
    let v = load_vec3(&args[1], ctx, builder, compile_rec)?;
    let out = quat_resolve(builder, q, v, resolve1);
    Ok(vec3_get(out, idx).unwrap_or_else(|| builder.ins().f64const(0.0)))
}

fn compile_angular_velocity_component(
    func_name: &str,
    args: &[Expression],
    idx: usize,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Value, String> {
    if idx == 0 || args.is_empty() {
        return Ok(builder.ins().f64const(0.0));
    }
    let tail = func_name.rsplit('.').next().unwrap_or(func_name);
    let w2 = compile_angular_velocity2(&args[0], ctx, builder, compile_rec)?;
    if tail == "angularVelocity1" {
        let q = load_quat(&args[0], ctx, builder, compile_rec)?;
        let w1 = quat_resolve(builder, q, w2, true);
        return Ok(vec3_get(w1, idx).unwrap_or_else(|| builder.ins().f64const(0.0)));
    }
    Ok(vec3_get(w2, idx).unwrap_or_else(|| builder.ins().f64const(0.0)))
}

fn compile_angular_velocity2(
    r: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<[Value; 3], String> {
    match r {
        Expression::Call(fname, args) => match fname.rsplit('.').next().unwrap_or(fname.as_str()) {
            "nullRotation" => {
                let z = builder.ins().f64const(0.0);
                return Ok([z, z, z]);
            }
            "planarRotation" if args.len() >= 3 => {
                let e = load_vec3(&args[0], ctx, builder, compile_rec)?;
                let w = compile_rec(&args[2], ctx, builder)?;
                return Ok(vec3_scale(builder, w, e));
            }
            "absoluteRotation" if args.len() >= 2 => {
                let w1 = compile_angular_velocity2(&args[0], ctx, builder, compile_rec)?;
                let wrel = compile_angular_velocity2(&args[1], ctx, builder, compile_rec)?;
                let qrel = load_quat(&args[1], ctx, builder, compile_rec)?;
                let mapped = quat_resolve(builder, qrel, w1, false);
                return Ok(vec3_add(builder, mapped, wrel));
            }
            _ => {}
        },
        Expression::ArrayAccess(inner, _) => {
            return compile_angular_velocity2(inner, ctx, builder, compile_rec);
        }
        _ => {}
    }
    // MSL Quaternions.angularVelocity2(Q, der(Q)): w = 2 * M(Q) * der(Q)
    let q = load_quat(r, ctx, builder, compile_rec)?;
    let dq = load_quat_der(r, ctx, builder, compile_rec)?;
    Ok(quat_angular_velocity2(builder, q, dq))
}

fn load_quat_der(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<[Value; 4], String> {
    match expr {
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            let mut comps = [
                builder.ins().f64const(0.0),
                builder.ins().f64const(0.0),
                builder.ins().f64const(0.0),
                builder.ins().f64const(0.0),
            ];
            for i in 1..=4 {
                let di = format!("der_{name}_{i}");
                let si = format!("{name}_{i}");
                if jit_scalar_name_bound(ctx, &di)
                    || ctx.state_index(&si).is_some()
                    || ctx.output_index(&di).is_some()
                {
                    comps[i - 1] = compile_rec(&Expression::var(&di), ctx, builder)?;
                }
            }
            Ok(comps)
        }
        _ => {
            let z = builder.ins().f64const(0.0);
            Ok([z, z, z, z])
        }
    }
}

fn quat_angular_velocity2(
    builder: &mut FunctionBuilder<'_>,
    q: [Value; 4],
    dq: [Value; 4],
) -> [Value; 3] {
    // w = 2 * [[q4, q3, -q2, -q1], [-q3, q4, q1, -q2], [q2, -q1, q4, -q3]] * der_Q
    let two = builder.ins().f64const(2.0);
    let nq1 = builder.ins().fneg(q[0]);
    let nq2 = builder.ins().fneg(q[1]);
    let nq3 = builder.ins().fneg(q[2]);
    let t0 = builder.ins().fmul(q[3], dq[0]);
    let t1 = builder.ins().fmul(q[2], dq[1]);
    let t2 = builder.ins().fmul(nq2, dq[2]);
    let t3 = builder.ins().fmul(nq1, dq[3]);
    let s01 = builder.ins().fadd(t0, t1);
    let s23 = builder.ins().fadd(t2, t3);
    let w1 = builder.ins().fadd(s01, s23);
    let t0 = builder.ins().fmul(nq3, dq[0]);
    let t1 = builder.ins().fmul(q[3], dq[1]);
    let t2 = builder.ins().fmul(q[0], dq[2]);
    let t3 = builder.ins().fmul(nq2, dq[3]);
    let s01 = builder.ins().fadd(t0, t1);
    let s23 = builder.ins().fadd(t2, t3);
    let w2 = builder.ins().fadd(s01, s23);
    let t0 = builder.ins().fmul(q[1], dq[0]);
    let t1 = builder.ins().fmul(nq1, dq[1]);
    let t2 = builder.ins().fmul(q[3], dq[2]);
    let t3 = builder.ins().fmul(nq3, dq[3]);
    let s01 = builder.ins().fadd(t0, t1);
    let s23 = builder.ins().fadd(t2, t3);
    let w3 = builder.ins().fadd(s01, s23);
    [
        builder.ins().fmul(two, w1),
        builder.ins().fmul(two, w2),
        builder.ins().fmul(two, w3),
    ]
}

pub(super) fn load_indexed_arg(
    expr: &Expression,
    idx: usize,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Value, String> {
    match expr {
        Expression::ArrayLiteral(items) if idx >= 1 && idx <= items.len() => {
            compile_rec(&items[idx - 1], ctx, builder)
        }
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            let indexed = format!("{name}_{idx}");
            if jit_scalar_name_bound(ctx, &indexed) {
                compile_rec(&Expression::var(&indexed), ctx, builder)
            } else if jit_scalar_name_bound(ctx, &name) {
                compile_rec(&Expression::var(&name), ctx, builder)
            } else {
                compile_rec(&Expression::var(&indexed), ctx, builder)
            }
        }
        Expression::Call(fname, args) => {
            compile_vector_call_component(fname, args, idx, ctx, builder, compile_rec)
        }
        Expression::ArrayAccess(arr, _) => load_indexed_arg(arr, idx, ctx, builder, compile_rec),
        Expression::If(c, t, f) => {
            let cv = compile_rec(c, ctx, builder)?;
            let tv = load_indexed_arg(t, idx, ctx, builder, compile_rec)?;
            let fv = load_indexed_arg(f, idx, ctx, builder, compile_rec)?;
            let zero = builder.ins().f64const(0.0);
            let cond = builder.ins().fcmp(FloatCC::NotEqual, cv, zero);
            Ok(builder.ins().select(cond, tv, fv))
        }
        Expression::BinaryOp(lhs, op, rhs) => match op {
            Operator::Add => {
                let a = load_indexed_arg(lhs, idx, ctx, builder, compile_rec)?;
                let b = load_indexed_arg(rhs, idx, ctx, builder, compile_rec)?;
                Ok(builder.ins().fadd(a, b))
            }
            Operator::Sub => {
                if matches!(lhs.as_ref(), Expression::Number(n) if *n == 0.0) {
                    let b = load_indexed_arg(rhs, idx, ctx, builder, compile_rec)?;
                    return Ok(builder.ins().fneg(b));
                }
                let a = load_indexed_arg(lhs, idx, ctx, builder, compile_rec)?;
                let b = load_indexed_arg(rhs, idx, ctx, builder, compile_rec)?;
                Ok(builder.ins().fsub(a, b))
            }
            Operator::Mul => {
                if let Some(v) = compile_tensor3_times_vec3(lhs, rhs, idx, ctx, builder, compile_rec)?
                {
                    return Ok(v);
                }
                if expr_is_vec3(lhs, ctx) && expr_is_vec3(rhs, ctx) {
                    let a = load_vec3(lhs, ctx, builder, compile_rec)?;
                    let b = load_vec3(rhs, ctx, builder, compile_rec)?;
                    return Ok(vec3_dot(builder, a, b));
                }
                if expr_is_vec3(lhs, ctx) {
                    let s = compile_rec(rhs, ctx, builder)?;
                    let v = load_indexed_arg(lhs, idx, ctx, builder, compile_rec)?;
                    return Ok(builder.ins().fmul(s, v));
                }
                if expr_is_vec3(rhs, ctx) {
                    let s = compile_rec(lhs, ctx, builder)?;
                    let v = load_indexed_arg(rhs, idx, ctx, builder, compile_rec)?;
                    return Ok(builder.ins().fmul(s, v));
                }
                let a = compile_rec(lhs, ctx, builder)?;
                let b = compile_rec(rhs, ctx, builder)?;
                Ok(builder.ins().fmul(a, b))
            }
            Operator::Div => {
                let a = load_indexed_arg(lhs, idx, ctx, builder, compile_rec)?;
                let b = compile_rec(rhs, ctx, builder)?;
                Ok(builder.ins().fdiv(a, b))
            }
            _ => compile_rec(expr, ctx, builder),
        },
        _ => compile_rec(expr, ctx, builder),
    }
}

fn load_scalar_named(
    name: &str,
    fallback: f64,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Value, String> {
    if jit_scalar_name_bound(ctx, name) {
        compile_rec(&Expression::var(name), ctx, builder)
    } else {
        Ok(builder.ins().f64const(fallback))
    }
}

fn gravity_instance_prefix(func_name: &str) -> String {
    let flat = func_name.replace('.', "_");
    for suffix in [
        "_gravityAcceleration",
        "_GravityAcceleration",
        "_standardGravityAcceleration",
    ] {
        if let Some(p) = flat.strip_suffix(suffix) {
            if !p.is_empty() {
                return p.to_string();
            }
        }
    }
    if let Some((p, tail)) = func_name.rsplit_once('.') {
        if tail.eq_ignore_ascii_case("gravityAcceleration")
            || tail.eq_ignore_ascii_case("GravityAcceleration")
        {
            return p.replace('.', "_");
        }
    }
    "world".to_string()
}

pub(super) fn try_compile_inner_product(
    lhs: &Expression,
    rhs: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Option<Value>, String> {
    if expr_is_vec3(lhs, ctx) && expr_is_vec3(rhs, ctx) {
        let a = load_vec3(lhs, ctx, builder, compile_rec)?;
        let b = load_vec3(rhs, ctx, builder, compile_rec)?;
        return Ok(Some(vec3_dot(builder, a, b)));
    }
    Ok(None)
}

fn expr_is_tensor3(expr: &Expression, ctx: &TranslationContext) -> bool {
    match expr {
        Expression::ArrayLiteral(items) => items.len() == 9,
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            (1..=9).all(|i| {
                let n = format!("{name}_{i}");
                ctx.output_index(&n).is_some() || ctx.param_index(&n).is_some()
            })
        }
        _ => false,
    }
}

fn compile_tensor3_times_vec3(
    lhs: &Expression,
    rhs: &Expression,
    idx: usize,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<Option<Value>, String> {
    if idx == 0 || idx > 3 {
        return Ok(None);
    }
    let (tensor, vec) = if expr_is_tensor3(lhs, ctx) && expr_is_vec3(rhs, ctx) {
        (lhs, rhs)
    } else if expr_is_tensor3(rhs, ctx) && expr_is_vec3(lhs, ctx) {
        (rhs, lhs)
    } else {
        return Ok(None);
    };
    let row0 = 3 * (idx - 1);
    let t1 = load_indexed_arg(tensor, row0 + 1, ctx, builder, compile_rec)?;
    let v1 = load_indexed_arg(vec, 1, ctx, builder, compile_rec)?;
    let p1 = builder.ins().fmul(t1, v1);
    let t2 = load_indexed_arg(tensor, row0 + 2, ctx, builder, compile_rec)?;
    let v2 = load_indexed_arg(vec, 2, ctx, builder, compile_rec)?;
    let p2 = builder.ins().fmul(t2, v2);
    let t3 = load_indexed_arg(tensor, row0 + 3, ctx, builder, compile_rec)?;
    let v3 = load_indexed_arg(vec, 3, ctx, builder, compile_rec)?;
    let p3 = builder.ins().fmul(t3, v3);
    let s12 = builder.ins().fadd(p1, p2);
    Ok(Some(builder.ins().fadd(s12, p3)))
}

fn expr_is_vec3(expr: &Expression, ctx: &TranslationContext) -> bool {
    match expr {
        Expression::ArrayLiteral(items) => items.len() == 3,
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            (1..=3).all(|i| {
                let n = format!("{name}_{i}");
                ctx.output_index(&n).is_some()
                    || ctx.state_index(&n).is_some()
                    || ctx.param_index(&n).is_some()
                    || ctx.discrete_index(&n).is_some()
            })
        }
        Expression::Call(fname, _) => {
            let tail = fname.rsplit('.').next().unwrap_or(fname);
            matches!(
                tail,
                "normalize"
                    | "normalizeWithAssert"
                    | "cross"
                    | "resolve1"
                    | "resolve2"
                    | "angularVelocity1"
                    | "angularVelocity2"
                    | "zeros"
                    | "ones"
            )
        }
        Expression::BinaryOp(lhs, op, rhs) => match op {
            Operator::Add | Operator::Sub => expr_is_vec3(lhs, ctx) || expr_is_vec3(rhs, ctx),
            Operator::Mul => {
                (expr_is_vec3(lhs, ctx) && matches!(rhs.as_ref(), Expression::Number(_)))
                    || (expr_is_vec3(rhs, ctx) && matches!(lhs.as_ref(), Expression::Number(_)))
            }
            _ => false,
        },
        Expression::ArrayAccess(arr, _) => expr_is_vec3(arr, ctx),
        _ => false,
    }
}

fn load_vec3(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<[Value; 3], String> {
    Ok([
        load_indexed_arg(expr, 1, ctx, builder, compile_rec)?,
        load_indexed_arg(expr, 2, ctx, builder, compile_rec)?,
        load_indexed_arg(expr, 3, ctx, builder, compile_rec)?,
    ])
}

fn load_quat(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: CompileRec,
) -> Result<[Value; 4], String> {
    Ok([
        load_indexed_arg(expr, 1, ctx, builder, compile_rec)?,
        load_indexed_arg(expr, 2, ctx, builder, compile_rec)?,
        load_indexed_arg(expr, 3, ctx, builder, compile_rec)?,
        load_indexed_arg(expr, 4, ctx, builder, compile_rec)?,
    ])
}

fn vec3_get(v: [Value; 3], idx: usize) -> Option<Value> {
    match idx {
        1 => Some(v[0]),
        2 => Some(v[1]),
        3 => Some(v[2]),
        _ => None,
    }
}

fn quat_get(q: [Value; 4], idx: usize) -> Option<Value> {
    match idx {
        1 => Some(q[0]),
        2 => Some(q[1]),
        3 => Some(q[2]),
        4 => Some(q[3]),
        _ => None,
    }
}

fn vec3_norm(builder: &mut FunctionBuilder<'_>, v: [Value; 3]) -> Value {
    let a = builder.ins().fmul(v[0], v[0]);
    let b = builder.ins().fmul(v[1], v[1]);
    let c = builder.ins().fmul(v[2], v[2]);
    let s = builder.ins().fadd(a, b);
    let s = builder.ins().fadd(s, c);
    builder.ins().sqrt(s)
}

fn vec3_cross(builder: &mut FunctionBuilder<'_>, a: [Value; 3], b: [Value; 3]) -> [Value; 3] {
    let a1b2 = builder.ins().fmul(a[1], b[2]);
    let a2b1 = builder.ins().fmul(a[2], b[1]);
    let c1 = builder.ins().fsub(a1b2, a2b1);
    let a2b0 = builder.ins().fmul(a[2], b[0]);
    let a0b2 = builder.ins().fmul(a[0], b[2]);
    let c2 = builder.ins().fsub(a2b0, a0b2);
    let a0b1 = builder.ins().fmul(a[0], b[1]);
    let a1b0 = builder.ins().fmul(a[1], b[0]);
    let c3 = builder.ins().fsub(a0b1, a1b0);
    [c1, c2, c3]
}

fn vec3_dot(builder: &mut FunctionBuilder<'_>, a: [Value; 3], b: [Value; 3]) -> Value {
    let t0 = builder.ins().fmul(a[0], b[0]);
    let t1 = builder.ins().fmul(a[1], b[1]);
    let t2 = builder.ins().fmul(a[2], b[2]);
    let s = builder.ins().fadd(t0, t1);
    builder.ins().fadd(s, t2)
}

fn vec3_scale(builder: &mut FunctionBuilder<'_>, s: Value, v: [Value; 3]) -> [Value; 3] {
    [
        builder.ins().fmul(s, v[0]),
        builder.ins().fmul(s, v[1]),
        builder.ins().fmul(s, v[2]),
    ]
}

fn vec3_add(builder: &mut FunctionBuilder<'_>, a: [Value; 3], b: [Value; 3]) -> [Value; 3] {
    [
        builder.ins().fadd(a[0], b[0]),
        builder.ins().fadd(a[1], b[1]),
        builder.ins().fadd(a[2], b[2]),
    ]
}

/// Quaternions.resolve1 / resolve2.
fn quat_resolve(
    builder: &mut FunctionBuilder<'_>,
    q: [Value; 4],
    v: [Value; 3],
    resolve1: bool,
) -> [Value; 3] {
    let qv = [q[0], q[1], q[2]];
    let q4 = q[3];
    let half = builder.ins().f64const(0.5);
    let two = builder.ins().f64const(2.0);
    let q4sq = builder.ins().fmul(q4, q4);
    let scale = builder.ins().fsub(q4sq, half);
    let qv_dot_v = vec3_dot(builder, qv, v);
    let qv_term = vec3_scale(builder, qv_dot_v, qv);
    let cr = vec3_cross(builder, qv, v);
    let signed_cr = if resolve1 {
        vec3_scale(builder, q4, cr)
    } else {
        let nq4 = builder.ins().fneg(q4);
        vec3_scale(builder, nq4, cr)
    };
    let t0 = vec3_scale(builder, scale, v);
    let t1 = vec3_add(builder, t0, qv_term);
    let t2 = vec3_add(builder, t1, signed_cr);
    vec3_scale(builder, two, t2)
}

fn quat_abs_rotate(builder: &mut FunctionBuilder<'_>, q1: [Value; 4], qr: [Value; 4]) -> [Value; 4] {
    let (a, b, c, d) = (qr[0], qr[1], qr[2], qr[3]);
    let (w, x, y, z) = (q1[0], q1[1], q1[2], q1[3]);
    // Q2 = M(Q_rel) * Q1  (MSL Quaternions.absoluteRotation)
    let dw = builder.ins().fmul(d, w);
    let cx = builder.ins().fmul(c, x);
    let by = builder.ins().fmul(b, y);
    let az = builder.ins().fmul(a, z);
    let t = builder.ins().fadd(dw, cx);
    let t = builder.ins().fsub(t, by);
    let q21 = builder.ins().fadd(t, az);

    let nc = builder.ins().fneg(c);
    let ncw = builder.ins().fmul(nc, w);
    let dx = builder.ins().fmul(d, x);
    let ay = builder.ins().fmul(a, y);
    let bz = builder.ins().fmul(b, z);
    let t = builder.ins().fadd(ncw, dx);
    let t = builder.ins().fadd(t, ay);
    let q22 = builder.ins().fadd(t, bz);

    let bw = builder.ins().fmul(b, w);
    let ax = builder.ins().fmul(a, x);
    let dy = builder.ins().fmul(d, y);
    let cz = builder.ins().fmul(c, z);
    let t = builder.ins().fsub(bw, ax);
    let t = builder.ins().fadd(t, dy);
    let q23 = builder.ins().fadd(t, cz);

    let na = builder.ins().fneg(a);
    let naw = builder.ins().fmul(na, w);
    let bx = builder.ins().fmul(b, x);
    let cy = builder.ins().fmul(c, y);
    let dz = builder.ins().fmul(d, z);
    let t = builder.ins().fsub(naw, bx);
    let t = builder.ins().fsub(t, cy);
    let q24 = builder.ins().fadd(t, dz);
    [q21, q22, q23, q24]
}
