use crate::ast::Expression;

/// Element (row,col) 0-based of MSL `symmetricTransformationMatrix(m)` (Fourier / Fortescue form).
pub(super) fn symmetric_transformation_matrix_element_re_im(
    m: usize,
    row: usize,
    col: usize,
) -> (f64, f64) {
    let m64 = m as f64;
    let scale = 1.0 / m64.sqrt();
    let angle = -2.0 * std::f64::consts::PI * (row as f64) * (col as f64) / m64;
    (scale * angle.cos(), scale * angle.sin())
}

/// `Dot(ArrayAccess(Call(symmetricTransformationMatrix,...), idx), "re"|"im")` at compile time.
pub(super) fn fold_dot_symmetric_transformation_matrix(
    inner: &Expression,
    member: &str,
) -> Option<f64> {
    let want_re = match member {
        "re" => true,
        "im" => false,
        _ => return None,
    };
    let Expression::ArrayAccess(arr, idx_expr) = inner else {
        return None;
    };
    let Expression::Call(fname, args) = &**arr else {
        return None;
    };
    if !fname.ends_with("symmetricTransformationMatrix") {
        return None;
    }
    if args.len() != 1 {
        return None;
    }
    let Expression::Number(m_num) = &args[0] else {
        return None;
    };
    let m = m_num.round().clamp(1.0, 64.0) as usize;
    let Expression::Number(idx_num) = &**idx_expr else {
        return None;
    };
    let idx_1 = idx_num.round() as i64;
    if idx_1 < 1 {
        return None;
    }
    let i0 = (idx_1 as usize).saturating_sub(1);
    let (row, col) = if i0 < m {
        (0usize, i0)
    } else {
        (i0 / m, i0 % m)
    };
    if row >= m || col >= m {
        return None;
    }
    let (re, im) = symmetric_transformation_matrix_element_re_im(m, row, col);
    Some(if want_re { re } else { im })
}
