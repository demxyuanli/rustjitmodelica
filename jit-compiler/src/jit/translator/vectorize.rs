//! SIMD vectorization: equation clustering and Cranelift vector code emission.
//!
//! Scans the flattened equation list looking for consecutive subscript-indexed
//! equations with identical structure (same base variable names, same operator).
//! Groups of 2+ such equations are merged into [`VectorGroup`] for vectorized
//! codegen; smaller groups and non-matching equations remain as scalar
//! [`CompileUnit::Scalar`].

use cranelift::prelude::InstBuilder;
use cranelift_module::Module;

/// Supported vectorizable operations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum VectorOp {
    Add,
    Sub,
    Mul,
    Div,
    /// Fused multiply-add: `a * b + c`
    Fma,
}

/// A group of N consecutive equations that can be vectorized.
#[derive(Debug, Clone)]
pub(crate) struct VectorGroup {
    /// Target variable base name (e.g. "x" for x_1..x_N).
    pub dst_base: String,
    /// Source variable base names (binary op has 2, unary op has 1).
    pub src_bases: Vec<String>,
    /// Starting index (1-based).
    pub lo: usize,
    /// Ending index (1-based).
    pub hi: usize,
    /// Operation type.
    pub op: VectorOp,
}

/// Compilation unit: either a single scalar equation or a vector group.
pub(crate) enum CompileUnit {
    Scalar(crate::ast::Equation),
    Vector(VectorGroup),
}

/// Extract `(base_name, index)` from a variable expression like `x_3`.
/// Returns `None` if the expression is not a subscript-indexed variable.
fn extract_array_index(expr: &crate::ast::Expression) -> Option<(String, usize)> {
    match expr {
        crate::ast::Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            name.rsplit_once('_')
                .and_then(|(base, idx_str)| idx_str.parse::<usize>().ok().map(|i| (base.to_string(), i)))
        }
        _ => None,
    }
}

/// Try to classify a simple equality equation as a vectorizable pattern.
/// Returns `(dst_base, dst_index, op, src_bases)` on success.
fn try_extract_pattern(eq: &crate::ast::Equation) -> Option<(String, usize, VectorOp, Vec<String>)> {
    match eq {
        crate::ast::Equation::Simple(lhs, rhs) => {
            let (lhs_base, lhs_idx) = extract_array_index(lhs)?;
            let (op, srcs) = try_extract_binary_op(rhs)
                .or_else(|| try_extract_fma(rhs))?;
            Some((lhs_base, lhs_idx, op, srcs))
        }
        _ => None,
    }
}

/// Try to extract a binary vectorizable operation from an expression tree.
fn try_extract_binary_op(expr: &crate::ast::Expression) -> Option<(VectorOp, Vec<String>)> {
    match expr {
        crate::ast::Expression::BinaryOp(lhs, op, rhs) => {
            let lhs_base = extract_array_index(lhs)?.0;
            let rhs_base = extract_array_index(rhs)?.0;
            let vec_op = match op {
                crate::ast::Operator::Add => VectorOp::Add,
                crate::ast::Operator::Sub => VectorOp::Sub,
                crate::ast::Operator::Mul => VectorOp::Mul,
                crate::ast::Operator::Div => VectorOp::Div,
                _ => return None,
            };
            Some((vec_op, vec![lhs_base, rhs_base]))
        }
        _ => None,
    }
}

/// Try to extract a fused multiply-add pattern: `(a * b) + c` or `c + (a * b)`.
fn try_extract_fma(expr: &crate::ast::Expression) -> Option<(VectorOp, Vec<String>)> {
    match expr {
        crate::ast::Expression::BinaryOp(lhs, op, rhs) if *op == crate::ast::Operator::Add => {
            // Try (a * b) + c
            if let crate::ast::Expression::BinaryOp(a, mul_op, b) = lhs.as_ref() {
                if *mul_op == crate::ast::Operator::Mul {
                    let a_base = extract_array_index(a)?.0;
                    let b_base = extract_array_index(b)?.0;
                    let c_base = extract_array_index(rhs)?.0;
                    return Some((VectorOp::Fma, vec![a_base, b_base, c_base]));
                }
            }
            // Try c + (a * b)
            if let crate::ast::Expression::BinaryOp(a, mul_op, b) = rhs.as_ref() {
                if *mul_op == crate::ast::Operator::Mul {
                    let a_base = extract_array_index(a)?.0;
                    let b_base = extract_array_index(b)?.0;
                    let c_base = extract_array_index(lhs)?.0;
                    return Some((VectorOp::Fma, vec![a_base, b_base, c_base]));
                }
            }
            None
        }
        _ => None,
    }
}

/// Pre-expand simple for-loops with const bounds and small range into subscript-indexed
/// equations so the existing vectorizer can cluster them.
fn expand_small_for_loops(equations: &[crate::ast::Equation]) -> Vec<crate::ast::Equation> {
    let max_expand = std::env::var("RUSTMODLICA_VECTORIZE_FOR_EXPAND_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(1024);
    let mut out = Vec::with_capacity(equations.len());
    for eq in equations {
        match eq {
            crate::ast::Equation::For(loop_var, start_expr, end_expr, body) => {
                // Only expand if bounds are constant and range is manageable
                let start_val = match crate::analysis::blt::helpers::eval_const_expr(start_expr) {
                    Some(v) => v as i64,
                    None => {
                        out.push(eq.clone());
                        continue;
                    }
                };
                let end_val = match crate::analysis::blt::helpers::eval_const_expr(end_expr) {
                    Some(v) => v as i64,
                    None => {
                        out.push(eq.clone());
                        continue;
                    }
                };
                let count = end_val - start_val + 1;
                if count <= 0 || count as usize > max_expand {
                    out.push(eq.clone());
                    continue;
                }
                // Expand: substitute loop var with each index value
                for i in start_val..=end_val {
                    let idx_name = format!("{}_{}", loop_var, i);
                    for body_eq in body {
                        out.push(substitute_loop_index(body_eq, loop_var, &idx_name));
                    }
                }
            }
            _ => out.push(eq.clone()),
        }
    }
    out
}

/// Substitute loop variable with a specific index in an equation body.
/// Replaces `x[i]` → `x_3` etc. for each iteration.
fn substitute_loop_index(
    eq: &crate::ast::Equation,
    loop_var: &str,
    suffix: &str, // e.g. "i_3"
) -> crate::ast::Equation {
    // Extract the numeric part from suffix like "i_3" → 3
    let index_num: f64 = suffix.rsplit_once('_')
        .and_then(|(_, n)| n.parse::<f64>().ok())
        .unwrap_or(0.0);

    fn subst_expr(e: &crate::ast::Expression, loop_var: &str, index_num: f64) -> crate::ast::Expression {
        match e {
            crate::ast::Expression::ArrayAccess(arr, idx) => {
                let is_loop_idx = matches!(
                    idx.as_ref(),
                    crate::ast::Expression::Variable(id)
                        if crate::string_intern::resolve_id(*id) == loop_var
                );
                if is_loop_idx {
                    if let crate::ast::Expression::Variable(arr_id) = arr.as_ref() {
                        let base = crate::string_intern::resolve_id(*arr_id);
                        return crate::ast::Expression::Variable(
                            crate::string_intern::intern(
                                &format!("{}_{}", base, index_num as i64)
                            )
                        );
                    }
                }
                e.clone()
            }
            crate::ast::Expression::BinaryOp(l, op, r) => {
                crate::ast::Expression::BinaryOp(
                    Box::new(subst_expr(l, loop_var, index_num)),
                    *op,
                    Box::new(subst_expr(r, loop_var, index_num)),
                )
            }
            _ => e.clone(),
        }
    }
    match eq {
        crate::ast::Equation::Simple(lhs, rhs) => {
            crate::ast::Equation::Simple(
                subst_expr(lhs, loop_var, index_num),
                subst_expr(rhs, loop_var, index_num),
            )
        }
        _ => eq.clone(),
    }
}

/// Scan equations, grouping consecutive subscript-indexed equations with
/// matching structure into [`VectorGroup`]s. Minimum group size is 2.
/// For-loops with const bounds and ≤256 iterations are pre-expanded for
/// better vectorization coverage.
pub(crate) fn cluster_equations(equations: &[crate::ast::Equation]) -> Vec<CompileUnit> {
    // Pre-expand small for-loops for better vectorization coverage
    let expanded = expand_small_for_loops(equations);
    let equations = &expanded;
    if equations.is_empty() {
        return vec![];
    }

    let mut units = Vec::new();
    let mut i = 0;
    while i < equations.len() {
        if let Some((base, idx, op, srcs)) = try_extract_pattern(&equations[i]) {
            // Try to extend this vector group
            let mut j = i + 1;
            while j < equations.len() {
                if let Some((next_base, next_idx, next_op, next_srcs)) =
                    try_extract_pattern(&equations[j])
                {
                    if next_base == base
                        && next_op == op
                        && next_srcs == srcs
                        && next_idx == idx + (j - i)
                    {
                        j += 1;
                        continue;
                    }
                }
                break;
            }
            let count = j - i;
            if count >= 2 {
                units.push(CompileUnit::Vector(VectorGroup {
                    dst_base: base,
                    src_bases: srcs,
                    lo: idx,
                    hi: idx + count - 1,
                    op,
                }));
                i = j;
                continue;
            }
        }
        units.push(CompileUnit::Scalar(equations[i].clone()));
        i += 1;
    }
    units
}

/// Returns the SIMD width for the target platform. Defaults to 2 (F64X2)
/// which is safe on x86_64 with SSE2. Set `RUSTMODLICA_JIT_SIMD_WIDTH=sse2`
/// to force F64X2, or `avx2` for F64X4 (requires AVX2-capable CPU).
fn simd_width() -> (cranelift::codegen::ir::types::Type, usize) {
    let wide = std::env::var("RUSTMODLICA_JIT_SIMD_WIDTH")
        .ok()
        .map(|v| v == "avx2")
        .unwrap_or(false);
    if wide {
        (cranelift::codegen::ir::types::F64X4, 4)
    } else {
        (cranelift::codegen::ir::types::F64X2, 2)
    }
}

/// Emit vectorized computation for a [`VectorGroup`]. Resolves array pointers
/// through `ctx.array_storage()` and emits Cranelift vector load/compute/store
/// for contiguous chunks.
///
/// Falls back to scalar when array resolution fails or the group is too small
/// for the vector width.
pub(crate) fn emit_vector_loop(
    group: &VectorGroup,
    ctx: &crate::jit::context::TranslationContext<'_>,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let count = group.hi - group.lo + 1;

    // Resolve array pointer and start offset for the destination variable.
    let (dst_ptr, dst_start) = resolve_array_ptr(&group.dst_base, ctx)
        .ok_or_else(|| format!("SIMD: cannot resolve array '{}'", group.dst_base))?;

    let (src1_ptr, src1_start) = resolve_array_ptr(&group.src_bases[0], ctx)
        .ok_or_else(|| format!("SIMD: cannot resolve array '{}'", group.src_bases[0]))?;

    let src2_info = if group.src_bases.len() > 1 {
        Some(
            resolve_array_ptr(&group.src_bases[1], ctx)
                .ok_or_else(|| format!("SIMD: cannot resolve array '{}'", group.src_bases[1]))?,
        )
    } else {
        None
    };

    let src3_info = if group.src_bases.len() > 2 {
        Some(
            resolve_array_ptr(&group.src_bases[2], ctx)
                .ok_or_else(|| format!("SIMD: cannot resolve array '{}'", group.src_bases[2]))?,
        )
    } else {
        None
    };

    let (vec_type, vec_size) = simd_width();
    let full_chunks = count / vec_size;
    let remainder = count % vec_size;

    let _ptr_ty = ctx.module.target_config().pointer_type();
    let mem_flags = cranelift::codegen::ir::MemFlags::new();

    for chunk in 0..full_chunks {
        let elem = chunk * vec_size;
        // Per-array BYTE offsets that include each array's start_index. The
        // storage-class base pointer is shared across all arrays of that type,
        // so a plain chunk offset aliased every array to storage index 0.
        // Mirrors the remainder loop's (start + k) * 8 convention.
        let dst_off = ((dst_start + elem) * 8) as i32;
        let src1_off = ((src1_start + elem) * 8) as i32;

        let a = builder.ins().load(vec_type, mem_flags, src1_ptr, src1_off);
        let result = match (&src2_info, &src3_info) {
            (Some((src2_ptr, src2_start)), None) => {
                let src2_off = ((src2_start + elem) * 8) as i32;
                let b = builder.ins().load(vec_type, mem_flags, *src2_ptr, src2_off);
                match group.op {
                    VectorOp::Add => builder.ins().fadd(a, b),
                    VectorOp::Sub => builder.ins().fsub(a, b),
                    VectorOp::Mul => builder.ins().fmul(a, b),
                    VectorOp::Div => builder.ins().fdiv(a, b),
                    VectorOp::Fma => a, // unreachable for 2-source case
                }
            }
            (Some((src2_ptr, src2_start)), Some((src3_ptr, src3_start))) => {
                // FMA: a * b + c
                let src2_off = ((src2_start + elem) * 8) as i32;
                let src3_off = ((src3_start + elem) * 8) as i32;
                let b = builder.ins().load(vec_type, mem_flags, *src2_ptr, src2_off);
                let c = builder.ins().load(vec_type, mem_flags, *src3_ptr, src3_off);
                builder.ins().fma(a, b, c)
            }
            _ => a,
        };
        builder.ins().store(mem_flags, result, dst_ptr, dst_off);
    }

    // Remainder: emit scalar load/compute/store for each element.
    let rem_start = full_chunks * vec_size;
    let dst_off_base = (dst_start + rem_start) * 8;
    let src1_off_base = (src1_start + rem_start) * 8;
    let src2_off_base = src2_info
        .as_ref()
        .map(|(_, s2_start)| (s2_start + rem_start) * 8);
    let src3_off_base = src3_info
        .as_ref()
        .map(|(_, s3_start)| (s3_start + rem_start) * 8);

    for i in 0..remainder {
        let idx = i * 8;
        let a = builder
            .ins()
            .load(cranelift::codegen::ir::types::F64, mem_flags, src1_ptr, (src1_off_base + idx) as i32);
        let result = match (&src2_info, &src3_info) {
            (Some((src2_ptr, _)), None) => {
                let b = builder.ins().load(
                    cranelift::codegen::ir::types::F64,
                    mem_flags,
                    *src2_ptr,
                    (src2_off_base.unwrap_or(0) + idx) as i32,
                );
                match group.op {
                    VectorOp::Add => builder.ins().fadd(a, b),
                    VectorOp::Sub => builder.ins().fsub(a, b),
                    VectorOp::Mul => builder.ins().fmul(a, b),
                    VectorOp::Div => builder.ins().fdiv(a, b),
                    VectorOp::Fma => a,
                }
            }
            (Some((src2_ptr, _)), Some((src3_ptr, _))) => {
                let b = builder.ins().load(
                    cranelift::codegen::ir::types::F64,
                    mem_flags,
                    *src2_ptr,
                    (src2_off_base.unwrap_or(0) + idx) as i32,
                );
                let c = builder.ins().load(
                    cranelift::codegen::ir::types::F64,
                    mem_flags,
                    *src3_ptr,
                    (src3_off_base.unwrap_or(0) + idx) as i32,
                );
                builder.ins().fma(a, b, c)
            }
            _ => a,
        };
        builder
            .ins()
            .store(mem_flags, result, dst_ptr, (dst_off_base + idx) as i32);
    }

    Ok(())
}

/// Resolve a variable base name to its Cranelift IR pointer value and start index.
fn resolve_array_ptr(
    name: &str,
    ctx: &crate::jit::context::TranslationContext<'_>,
) -> Option<(cranelift::codegen::ir::Value, usize)> {
    let (array_type, start_index) = ctx.array_storage(name)?;
    let ptr = match array_type {
        crate::jit::types::ArrayType::State => ctx.states_ptr,
        crate::jit::types::ArrayType::Discrete => ctx.discrete_ptr,
        crate::jit::types::ArrayType::Parameter => ctx.params_ptr,
        crate::jit::types::ArrayType::Output => ctx.outputs_ptr,
        crate::jit::types::ArrayType::Derivative => ctx.derivs_ptr,
    };
    Some((ptr, start_index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Equation, Expression, Operator};
    use crate::string_intern::intern;

    fn var(name: &str) -> Expression {
        Expression::Variable(intern(name))
    }

    #[test]
    fn test_extract_array_index_simple() {
        let expr = var("x_3");
        let (base, idx) = extract_array_index(&expr).unwrap();
        assert_eq!(base, "x");
        assert_eq!(idx, 3);
    }

    #[test]
    fn test_extract_array_index_no_suffix() {
        let expr = var("plain");
        assert!(extract_array_index(&expr).is_none());
    }

    #[test]
    fn test_extract_array_index_multi_underscore() {
        let expr = var("my_var_7");
        let (base, idx) = extract_array_index(&expr).unwrap();
        assert_eq!(base, "my_var");
        assert_eq!(idx, 7);
    }

    #[test]
    fn test_cluster_empty() {
        let units = cluster_equations(&[]);
        assert!(units.is_empty());
    }

    #[test]
    fn test_cluster_insufficient_for_vector() {
        // 3 equations — below the threshold of 4
        let eqs: Vec<Equation> = (1..=3)
            .map(|i| {
                Equation::Simple(
                    var(&format!("y_{i}")),
                    Expression::BinaryOp(
                        Box::new(var(&format!("a_{i}"))),
                        Operator::Add,
                        Box::new(var(&format!("b_{i}"))),
                    ),
                )
            })
            .collect();
        let units = cluster_equations(&eqs);
        // count=3 ≥ 2 → one Vector group, remainder handled as scalar within emit
        assert_eq!(units.len(), 1);
        assert!(matches!(&units[0], CompileUnit::Vector(g) if g.lo == 1 && g.hi == 3));
    }

    #[test]
    fn test_cluster_minimal_vector_group() {
        // 2 consecutive matching equations — exact new threshold
        let eqs: Vec<Equation> = (1..=2)
            .map(|i| {
                Equation::Simple(
                    var(&format!("y_{i}")),
                    Expression::BinaryOp(
                        Box::new(var(&format!("a_{i}"))),
                        Operator::Add,
                        Box::new(var(&format!("b_{i}"))),
                    ),
                )
            })
            .collect();
        let units = cluster_equations(&eqs);
        assert_eq!(units.len(), 1);
        match &units[0] {
            CompileUnit::Vector(g) => {
                assert_eq!(g.dst_base, "y");
                assert_eq!(g.lo, 1);
                assert_eq!(g.hi, 2);
                assert_eq!(g.op, VectorOp::Add);
            }
            _ => panic!("expected Vector unit"),
        }
    }

    #[test]
    fn test_cluster_mixed_scalar_and_vector() {
        let eqs = vec![
            // Non-indexed scalar
            Equation::Simple(var("z"), Expression::Number(42.0)),
            // Vector group of 4
            Equation::Simple(
                var("x_1"),
                Expression::BinaryOp(
                    Box::new(var("p_1")),
                    Operator::Mul,
                    Box::new(var("q_1")),
                ),
            ),
            Equation::Simple(
                var("x_2"),
                Expression::BinaryOp(
                    Box::new(var("p_2")),
                    Operator::Mul,
                    Box::new(var("q_2")),
                ),
            ),
            Equation::Simple(
                var("x_3"),
                Expression::BinaryOp(
                    Box::new(var("p_3")),
                    Operator::Mul,
                    Box::new(var("q_3")),
                ),
            ),
            Equation::Simple(
                var("x_4"),
                Expression::BinaryOp(
                    Box::new(var("p_4")),
                    Operator::Mul,
                    Box::new(var("q_4")),
                ),
            ),
            // Another scalar
            Equation::Simple(var("w"), Expression::Number(1.0)),
        ];
        let units = cluster_equations(&eqs);
        assert_eq!(units.len(), 3); // scalar + vector + scalar
        assert!(matches!(units[0], CompileUnit::Scalar(_)));
        assert!(matches!(units[1], CompileUnit::Vector(_)));
        assert!(matches!(units[2], CompileUnit::Scalar(_)));
    }

    #[test]
    fn test_cluster_different_ops_split_groups() {
        // Add group then sub group — should NOT merge
        let eqs = vec![
            Equation::Simple(
                var("y_1"),
                Expression::BinaryOp(
                    Box::new(var("a_1")),
                    Operator::Add,
                    Box::new(var("b_1")),
                ),
            ),
            Equation::Simple(
                var("y_2"),
                Expression::BinaryOp(
                    Box::new(var("a_2")),
                    Operator::Add,
                    Box::new(var("b_2")),
                ),
            ),
            Equation::Simple(
                var("y_3"),
                Expression::BinaryOp(
                    Box::new(var("a_3")),
                    Operator::Sub,
                    Box::new(var("b_3")),
                ),
            ),
            Equation::Simple(
                var("y_4"),
                Expression::BinaryOp(
                    Box::new(var("a_4")),
                    Operator::Sub,
                    Box::new(var("b_4")),
                ),
            ),
        ];
        let units = cluster_equations(&eqs);
        // Two groups of 2 each — both meet threshold of 2
        assert_eq!(units.len(), 2);
        assert!(matches!(&units[0], CompileUnit::Vector(g) if g.op == VectorOp::Add && g.hi - g.lo + 1 == 2));
        assert!(matches!(&units[1], CompileUnit::Vector(g) if g.op == VectorOp::Sub && g.hi - g.lo + 1 == 2));
    }

    #[test]
    fn test_cluster_index_skip_breaks_group() {
        // y_1, y_2, y_4 — index skip at y_3 breaks continuity
        let eqs = vec![
            Equation::Simple(
                var("y_1"),
                Expression::BinaryOp(
                    Box::new(var("a_1")),
                    Operator::Add,
                    Box::new(var("b_1")),
                ),
            ),
            Equation::Simple(
                var("y_2"),
                Expression::BinaryOp(
                    Box::new(var("a_2")),
                    Operator::Add,
                    Box::new(var("b_2")),
                ),
            ),
            Equation::Simple(
                var("y_4"),
                Expression::BinaryOp(
                    Box::new(var("a_4")),
                    Operator::Add,
                    Box::new(var("b_4")),
                ),
            ),
        ];
        let units = cluster_equations(&eqs);
        // Group of 2 (y_1, y_2) meets threshold, y_4 is scalar
        assert_eq!(units.len(), 2);
        assert!(matches!(&units[0], CompileUnit::Vector(g) if g.lo == 1 && g.hi == 2));
        assert!(matches!(&units[1], CompileUnit::Scalar(_)));
    }

    #[test]
    fn test_cluster_div_vector_group() {
        let eqs: Vec<Equation> = (1..=4)
            .map(|i| {
                Equation::Simple(
                    var(&format!("y_{i}")),
                    Expression::BinaryOp(
                        Box::new(var(&format!("a_{i}"))),
                        Operator::Div,
                        Box::new(var(&format!("b_{i}"))),
                    ),
                )
            })
            .collect();
        let units = cluster_equations(&eqs);
        assert_eq!(units.len(), 1);
        match &units[0] {
            CompileUnit::Vector(g) => {
                assert_eq!(g.op, VectorOp::Div);
                assert_eq!(g.lo, 1);
                assert_eq!(g.hi, 4);
            }
            _ => panic!("expected Vector unit for Div"),
        }
    }

    #[test]
    fn test_cluster_fma_vector_group() {
        // (a_i * b_i) + c_i → Fma
        let eqs: Vec<Equation> = (1..=5)
            .map(|i| {
                Equation::Simple(
                    var(&format!("y_{i}")),
                    Expression::BinaryOp(
                        Box::new(Expression::BinaryOp(
                            Box::new(var(&format!("a_{i}"))),
                            Operator::Mul,
                            Box::new(var(&format!("b_{i}"))),
                        )),
                        Operator::Add,
                        Box::new(var(&format!("c_{i}"))),
                    ),
                )
            })
            .collect();
        let units = cluster_equations(&eqs);
        assert_eq!(units.len(), 1);
        match &units[0] {
            CompileUnit::Vector(g) => {
                assert_eq!(g.op, VectorOp::Fma);
                assert_eq!(g.src_bases.len(), 3);
                assert_eq!(g.src_bases[0], "a");
                assert_eq!(g.src_bases[1], "b");
                assert_eq!(g.src_bases[2], "c");
            }
            _ => panic!("expected Vector unit for Fma"),
        }
    }

    #[test]
    fn test_cluster_fma_reversed_order() {
        // c_i + (a_i * b_i) → also Fma
        let eqs: Vec<Equation> = (1..=4)
            .map(|i| {
                Equation::Simple(
                    var(&format!("y_{i}")),
                    Expression::BinaryOp(
                        Box::new(var(&format!("c_{i}"))),
                        Operator::Add,
                        Box::new(Expression::BinaryOp(
                            Box::new(var(&format!("a_{i}"))),
                            Operator::Mul,
                            Box::new(var(&format!("b_{i}"))),
                        )),
                    ),
                )
            })
            .collect();
        let units = cluster_equations(&eqs);
        assert_eq!(units.len(), 1);
        match &units[0] {
            CompileUnit::Vector(g) => {
                assert_eq!(g.op, VectorOp::Fma);
                assert_eq!(g.src_bases[0], "a");
                assert_eq!(g.src_bases[1], "b");
                assert_eq!(g.src_bases[2], "c");
            }
            _ => panic!("expected Vector unit for Fma (reversed)"),
        }
    }
}
