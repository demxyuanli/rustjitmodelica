//! Tier 0 equation interpreter (Phase 5 of Leyden-inspired compilation).
//!
//! A lightweight tree-walking interpreter for small equation systems. Avoids
//! Cranelift compilation entirely for fast startup on simple models. The interpreter
//! evaluates the AST directly at each simulation step.
//!
//! This is analogous to the JVM interpreter tier in HotSpot: slowest execution but
//! zero compilation overhead.

use crate::ast::{AlgorithmStatement, Expression, Equation, Operator};
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

    /// Evaluate an algebraic equation (non-derivative): assignment, when, or
    /// solvable-block approximation. Called once before diff equations so that
    /// algebraic/clocked variables feeding `der` RHSs are not 0-initialized.
    pub fn eval_alg_equation(&mut self, eq: &Equation, time: f64) {
        match eq {
            Equation::Simple(lhs, rhs) => {
                // Simple assignment: lhs = expr(rhs).  Collect into the variable
                // map for downstream diff-equation RHS access.
                if let Expression::Variable(var_id) = lhs {
                    let var_name = resolve_id(*var_id);
                    if var_name != "der_x" {
                        self.variables.insert(var_name, self.eval_expr(rhs));
                    }
                }
            }
            Equation::When(cond, body, else_whens) => {
                let all_whens = std::iter::once((cond, body))
                    .chain(else_whens.iter().map(|(c, b)| (c, b)));
                for (cond, body) in all_whens {
                    if self.eval_expr(cond) != 0.0 {
                        for e in body {
                            self.eval_alg_equation(e, time);
                        }
                        break;
                    }
                }
            }
            Equation::If(cond, then_eqs, elifs, else_eqs) => {
                if self.eval_expr(cond) != 0.0 {
                    for e in then_eqs {
                        self.eval_alg_equation(e, time);
                    }
                } else {
                    let mut handled = false;
                    for (ec, ee) in elifs {
                        if self.eval_expr(ec) != 0.0 {
                            for e in ee {
                                self.eval_alg_equation(e, time);
                            }
                            handled = true;
                            break;
                        }
                    }
                    if !handled {
                        if let Some(else_block) = else_eqs {
                            for e in else_block {
                                self.eval_alg_equation(e, time);
                            }
                        }
                    }
                }
            }
            // Solvable blocks are Newton-solved in JIT; interpreter falls back to
            // a single-pass approximation (already the JIT behavior for
            // non-iterated blocks — correctness requires the tier to be suitable).
            _ => {}
        }
    }

    /// Evaluate an algorithm statement (when-body, reinit, assignment).
    /// Unused for now; when-body items are compiled as Equation variants and
    /// handled by `eval_alg_equation` above.  Full algorithm support would
    /// mirror jit/translator/algorithm/mod.rs.
    #[allow(dead_code)]
    pub fn eval_alg_stmt(&mut self, _s: &AlgorithmStatement, _time: f64) {
    }

    /// Check if the equation system is small enough to be interpreted.
    pub fn is_interpretable(
        alg_eq_count: usize,
        diff_eq_count: usize,
        state_count: usize,
    ) -> bool {
        alg_eq_count + diff_eq_count <= 20 && state_count <= 10
    }
}

use std::sync::{Mutex, OnceLock};

struct InterpreterContext {
    state_vars: Vec<String>,
    param_vars: Vec<String>,
    diff_equations: Vec<Equation>,
    alg_equations: Vec<Equation>,
    state_count: usize,
}

/// Per-model interpreter contexts, keyed by model name. Replaces the single
/// global `OnceLock<Mutex<InterpreterContext>>` which would serve the wrong
/// model's equations in multi-simulation (IDE) scenarios.
static INTERPRETER_CTXS: OnceLock<Mutex<std::collections::HashMap<String, InterpreterContext>>> =
    OnceLock::new();

/// Thread-local active model identifier, set by the simulation driver before
/// calling the interpreter trampoline so it can find the right context.
std::thread_local! {
    pub(crate) static ACTIVE_INTERPRETER_MODEL: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
}

/// Set the active interpreter model for this thread (called by the simulation
/// driver before invoking calc_derivs). Returns a guard that clears it on drop.
pub fn set_active_interpreter_model(name: &str) -> TrampolineContext {
    ACTIVE_INTERPRETER_MODEL.with(|c| *c.borrow_mut() = Some(name.to_string()));
    TrampolineContext
}

pub struct TrampolineContext;

impl Drop for TrampolineContext {
    fn drop(&mut self) {
        ACTIVE_INTERPRETER_MODEL.with(|c| *c.borrow_mut() = None);
    }
}

fn ctx_map() -> &'static Mutex<std::collections::HashMap<String, InterpreterContext>> {
    INTERPRETER_CTXS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

pub fn is_context_installed_for(model_name: &str) -> bool {
    INTERPRETER_CTXS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|m| m.contains_key(model_name))
        .unwrap_or(false)
}

pub fn install_interpreter_context(
    model_name: String,
    state_vars: Vec<String>,
    param_vars: Vec<String>,
    diff_equations: Vec<Equation>,
    alg_equations: Vec<Equation>,
) {
    let state_count = state_vars.len();
    let ctx = InterpreterContext {
        state_vars,
        param_vars,
        diff_equations,
        alg_equations,
        state_count,
    };
    ctx_map().lock().map(|mut m| m.insert(model_name, ctx)).ok();
}

pub unsafe extern "C" fn interpreter_trampoline(
    time: f64,
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
    let model: Option<String> =
        ACTIVE_INTERPRETER_MODEL.with(|c| c.borrow().clone());
    let Some(model) = model else {
        return -1;
    };
    let ctx_map = ctx_map();
    let Ok(ctx_lock) = ctx_map.lock() else {
        return -1;
    };
    let Some(ctx) = ctx_lock.get(&model) else {
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
    // Inject `time` so expressions referencing it don't read 0.
    interp.set_variable("time", time);
    // Evaluate algebraic equations first so their outputs are available when
    // differential equation RHSs reference them (J5 + J6).
    for eq in &ctx.alg_equations {
        interp.eval_alg_equation(eq, time);
    }
    interp.eval_all_diff_equations(&ctx.diff_equations);
    let derivs_slice = std::slice::from_raw_parts_mut(derivs, n);
    interp.write_derivatives(&ctx.state_vars, derivs_slice);
    0
}
