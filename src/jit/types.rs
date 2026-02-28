#[derive(Clone, Copy, Debug)]
pub enum ArrayType {
    State,
    Discrete,
    Parameter,
    Output,
    #[allow(dead_code)]
    Derivative,
}

#[derive(Clone, Debug)]
pub struct ArrayInfo {
    pub array_type: ArrayType,
    pub start_index: usize,
    pub size: usize,
}

// Define function signature for derivative calculation
// fn calc_derivs(..., t_end, diag_residual, diag_x) -> i32; pass null for diag when not used
pub type CalcDerivsFunc = unsafe extern "C" fn(f64, *mut f64, *mut f64, *mut f64, *const f64, *mut f64, *mut f64, *mut f64, *const f64, *const f64, f64, *mut f64, *mut f64) -> i32;

#[allow(dead_code)]
pub struct ArrayAccessInfo<'a> {
    pub info: &'a ArrayInfo,
    pub index_expr: &'a crate::ast::Expression,
}
