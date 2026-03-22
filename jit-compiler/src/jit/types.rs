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
// fn calc_derivs(..., t_end, diag_residual, diag_x, homotopy_lambda) -> i32
pub type CalcDerivsFunc = unsafe extern "C" fn(
    f64,        // time
    *mut f64,   // states
    *mut f64,   // discrete
    *mut f64,   // derivs
    *const f64, // params
    *mut f64,   // outputs
    *mut f64,   // when_states
    *mut f64,   // crossings
    *const f64, // pre_states
    *const f64, // pre_discrete
    f64,        // t_end
    *mut f64,   // diag_residual
    *mut f64,   // diag_x
    *const f64, // homotopy_lambda
) -> i32;

#[allow(dead_code)]
pub struct ArrayAccessInfo<'a> {
    pub info: &'a ArrayInfo,
    pub index_expr: &'a crate::ast::Expression,
}
