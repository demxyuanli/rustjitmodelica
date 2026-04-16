//! Tier 0 equation interpreter (Phase 5 of Leyden-inspired compilation).
//!
//! A lightweight tree-walking interpreter for small equation systems. Avoids
//! Cranelift compilation entirely for fast startup on simple models. The interpreter
//! evaluates the AST directly at each simulation step.
//!
//! This is analogous to the JVM interpreter tier in HotSpot: slowest execution but
//! zero compilation overhead.

use crate::ast::{Expression, Equation, Operator};
use crate::string_intern::resolve_id;
use std::collections::HashMap;

/// Interpreter state for evaluating equations.
pub struct EquationInterpreter {
    variables: HashMap<String, f64>,
    derivatives: HashMap<String, f64>,
    params: HashMap<String, f64>,
}

impl EquationInterpreter {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            derivatives: HashMap::new(),
            params: HashMap::new(),
        }
    }

    pub fn set_variable(&mut self, name: &str, value: f64) {
        self.variables.insert(name.to_string(), value);
    }

    pub fn get_variable(&self, name: &str) -> f64 {
        self.variables.get(name).copied().unwrap_or(0.0)
    }

    pub fn set_derivative(&mut self, name: &str, value: f64) {
        self.derivatives.insert(name.to_string(), value);
    }

    pub fn get_derivative(&self, name: &str) -> f64 {
        self.derivatives.get(name).copied().unwrap_or(0.0)
    }

    pub fn set_param(&mut self, name: &str, value: f64) {
        self.params.insert(name.to_string(), value);
    }

    /// Load state variables and parameters from indexed arrays (matching JIT layout).
    pub fn load_state(
        &mut self,
        state_vars: &[String],
        states: &[f64],
        param_vars: &[String],
        params: &[f64],
    ) {
        for (i, name) in state_vars.iter().enumerate() {
            if i < states.len() {
                self.set_variable(name, states[i]);
            }
        }
        for (i, name) in param_vars.iter().enumerate() {
            if i < params.len() {
                self.set_param(name, params[i]);
            }
        }
    }

    /// Write computed derivatives back to an indexed array.
    pub fn write_derivatives(&self, state_vars: &[String], derivs: &mut [f64]) {
        for (i, name) in state_vars.iter().enumerate() {
            if i < derivs.len() {
                derivs[i] = self.get_derivative(name);
            }
        }
    }

    /// Evaluate a single expression.
    pub fn eval_expr(&self, expr: &Expression) -> f64 {
        match expr {
            Expression::Number(n) => *n,
            Expression::Variable(var_id) => {
                let name = resolve_id(*var_id);
                if let Some(&v) = self.params.get(&name) {
                    v
                } else {
                    self.get_variable(&name)
                }
            }
            Expression::BinaryOp(lhs, op, rhs) => {
                let l = self.eval_expr(lhs);
                let r = self.eval_expr(rhs);
                match op {
                    Operator::Add => l + r,
                    Operator::Sub => l - r,
                    Operator::Mul => l * r,
                    Operator::Div => {
                        if r.abs() < 1e-300 { 0.0 } else { l / r }
                    }
                    Operator::Less => if l < r { 1.0 } else { 0.0 },
                    Operator::LessEq => if l <= r { 1.0 } else { 0.0 },
                    Operator::Greater => if l > r { 1.0 } else { 0.0 },
                    Operator::GreaterEq => if l >= r { 1.0 } else { 0.0 },
                    Operator::Equal => if (l - r).abs() < 1e-15 { 1.0 } else { 0.0 },
                    Operator::NotEqual => if (l - r).abs() >= 1e-15 { 1.0 } else { 0.0 },
                    Operator::And => if l != 0.0 && r != 0.0 { 1.0 } else { 0.0 },
                    Operator::Or => if l != 0.0 || r != 0.0 { 1.0 } else { 0.0 },
                }
            }
            Expression::Call(name, args) => {
                self.eval_builtin_call(name, args)
            }
            Expression::Der(inner) => {
                if let Expression::Variable(var_id) = inner.as_ref() {
                    let name = resolve_id(*var_id);
                    self.get_derivative(&name)
                } else {
                    0.0
                }
            }
            Expression::If(cond, then_expr, else_expr) => {
                if self.eval_expr(cond) != 0.0 {
                    self.eval_expr(then_expr)
                } else {
                    self.eval_expr(else_expr)
                }
            }
            Expression::ArrayAccess(arr, idx) => {
                let _ = (arr, idx);
                0.0
            }
            Expression::Dot(inner, field) => {
                if let Expression::Variable(var_id) = inner.as_ref() {
                    let name = format!("{}.{}", resolve_id(*var_id), field);
                    if let Some(&v) = self.params.get(&name) {
                        v
                    } else {
                        self.get_variable(&name)
                    }
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    fn eval_builtin_call(&self, name: &str, args: &[Expression]) -> f64 {
        let a0 = args.first().map(|e| self.eval_expr(e)).unwrap_or(0.0);
        let a1 = args.get(1).map(|e| self.eval_expr(e)).unwrap_or(0.0);
        match name {
            "sin" => a0.sin(),
            "cos" => a0.cos(),
            "tan" => a0.tan(),
            "asin" => a0.asin(),
            "acos" => a0.acos(),
            "atan" => a0.atan(),
            "atan2" => a0.atan2(a1),
            "exp" => a0.exp(),
            "log" => a0.ln(),
            "log10" => a0.log10(),
            "sqrt" => a0.sqrt(),
            "abs" => a0.abs(),
            "sign" => {
                if a0 > 0.0 { 1.0 }
                else if a0 < 0.0 { -1.0 }
                else { 0.0 }
            }
            "min" => a0.min(a1),
            "max" => a0.max(a1),
            "floor" => a0.floor(),
            "ceil" => a0.ceil(),
            "mod" | "rem" => {
                if a1.abs() < 1e-300 { 0.0 } else { a0 % a1 }
            }
            "div" => {
                if a1.abs() < 1e-300 { 0.0 } else { (a0 / a1).trunc() }
            }
            "sinh" => a0.sinh(),
            "cosh" => a0.cosh(),
            "tanh" => a0.tanh(),
            "smooth" => a1,
            "noEvent" => a0,
            "pre" => a0,
            _ => 0.0,
        }
    }

    /// Evaluate a differential equation of the form `der(x) = rhs`.
    pub fn eval_diff_equation(&mut self, eq: &Equation) {
        if let Equation::Simple(lhs, rhs) = eq {
            if let Expression::Der(inner) = lhs {
                if let Expression::Variable(var_id) = inner.as_ref() {
                    let var_name = resolve_id(*var_id);
                    let value = self.eval_expr(rhs);
                    self.set_derivative(&var_name, value);
                }
            }
        }
    }

    /// Evaluate all differential equations in the system.
    pub fn eval_all_diff_equations(&mut self, equations: &[Equation]) {
        for eq in equations {
            self.eval_diff_equation(eq);
        }
    }

    /// Check if the equation system is small enough to be interpreted.
    pub fn is_interpretable(
        alg_eq_count: usize,
        diff_eq_count: usize,
        state_count: usize,
    ) -> bool {
        alg_eq_count + diff_eq_count <= 10 && state_count <= 5
    }
}

use std::sync::{Mutex, OnceLock};

struct InterpreterContext {
    state_vars: Vec<String>,
    param_vars: Vec<String>,
    diff_equations: Vec<Equation>,
    state_count: usize,
}

static INTERPRETER_CTX: OnceLock<Mutex<InterpreterContext>> = OnceLock::new();

pub fn is_context_installed() -> bool {
    INTERPRETER_CTX.get().is_some()
}

pub fn install_interpreter_context(
    state_vars: Vec<String>,
    param_vars: Vec<String>,
    diff_equations: Vec<Equation>,
) {
    let state_count = state_vars.len();
    let ctx = InterpreterContext {
        state_vars,
        param_vars,
        diff_equations,
        state_count,
    };
    if let Some(existing) = INTERPRETER_CTX.get() {
        if let Ok(mut guard) = existing.lock() {
            *guard = ctx;
        }
    } else {
        let _ = INTERPRETER_CTX.set(Mutex::new(ctx));
    }
}

pub unsafe extern "C" fn interpreter_trampoline(
    _time: f64,
    states: *mut f64,
    _discrete: *mut f64,
    derivs: *mut f64,
    params: *const f64,
    _outputs: *mut f64,
    _when_states: *mut f64,
    _crossings: *mut f64,
    _pre_states: *const f64,
    _pre_discrete: *const f64,
    _t_end: f64,
    _diag_residual: *mut f64,
    _diag_x: *mut f64,
    _homotopy_lambda: *const f64,
) -> i32 {
    if states.is_null() || derivs.is_null() {
        return -1;
    }
    let Some(ctx_lock) = INTERPRETER_CTX.get() else {
        return -1;
    };
    let Ok(ctx) = ctx_lock.lock() else {
        return -1;
    };
    let n = ctx.state_count;
    let states_slice = std::slice::from_raw_parts(states, n);
    let params_slice = if params.is_null() {
        &[]
    } else {
        std::slice::from_raw_parts(params, ctx.param_vars.len())
    };

    let mut interp = EquationInterpreter::new();
    interp.load_state(&ctx.state_vars, states_slice, &ctx.param_vars, params_slice);
    interp.eval_all_diff_equations(&ctx.diff_equations);
    let derivs_slice = std::slice::from_raw_parts_mut(derivs, n);
    interp.write_derivatives(&ctx.state_vars, derivs_slice);
    0
}
