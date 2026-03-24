// INT-2: Script mode. Minimal grammar: load ModelName, setParameter name value, simulate, quit.
// SCRIPT-2: eval <expr> for expression evaluation.

use std::io::BufRead;
use std::io::Read;

use crate::ast::{Expression, Operator};
use crate::compiler::{CompileOutput, Compiler};
use crate::expr_eval::eval_expr;
use crate::parser::mos_parse::{parse_mos_script, MosExpr, MosStmt};
use crate::simulation::run_simulation;

type RunError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone)]
enum MosValue {
    Number(f64),
    String(String),
    Bool(bool),
    Array(Vec<MosValue>),
}

impl MosValue {
    fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(v) => Some(*v),
            _ => None,
        }
    }
    fn as_string(&self) -> Option<String> {
        match self {
            Self::String(v) => Some(v.clone()),
            _ => None,
        }
    }
    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            Self::Number(v) => Some(*v != 0.0),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ScriptCommand {
    Load(String),
    SetParameter(String, f64),
    SetStartValue(String, f64),
    SetStopTime(f64),
    GetParameter(String),
    GetVariable(String),
    Eval(String),
    /// SCRIPT-3: set result file path for next simulate.
    SetResultFile(String),
    /// SCRIPT-3: alias for SetResultFile (OMC-style saveResult).
    SaveResult(String),
    /// SCRIPT-4: set solver tolerances (atol, optional rtol).
    SetTolerance(f64, Option<f64>),
    /// SCRIPT-4: plot var1 var2 ... (stub: record vars for result; run simulate to get data).
    Plot(Vec<String>),
    /// OMC-style plotAll: all output variables (same stub as plot).
    PlotAll,
    /// OMC-style getErrorString: not tracked; prints empty line.
    GetErrorString,
    /// SCRIPT-5: switch current model to a previously loaded one by name.
    SwitchModel(String),
    /// OMC-style: compile-only check (prints true on success).
    InstantiateModel(String),
    /// OMC-style: load model, set t_end and dt, run simulation (model t_end dt).
    SimulateModel { model: String, t_end: f64, dt: f64 },
    Simulate,
    Quit,
    CommentOrEmpty,
}

/// SCRIPT-2: Minimal expression parser for eval: "var", "var + number", "var - number", "var * number", "var / number".
fn parse_simple_expr(s: &str) -> Option<Expression> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let tokens: Vec<&str> = s.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    if tokens.len() == 1 {
        let t = tokens[0];
        if let Ok(n) = t.parse::<f64>() {
            return Some(Expression::Number(n));
        }
        return Some(Expression::var(t));
    }
    if tokens.len() == 3 {
        let var = tokens[0];
        let op_str = tokens[1];
        let num_str = tokens[2];
        let num: f64 = num_str.parse().ok()?;
        let op = match op_str {
            "+" => Operator::Add,
            "-" => Operator::Sub,
            "*" => Operator::Mul,
            "/" => Operator::Div,
            _ => return None,
        };
        let lhs = if let Ok(n) = var.parse::<f64>() {
            Expression::Number(n)
        } else {
            Expression::var(var)
        };
        return Some(Expression::BinaryOp(
            Box::new(lhs),
            op,
            Box::new(Expression::Number(num)),
        ));
    }
    None
}

pub fn parse_script_line(line: &str) -> Option<ScriptCommand> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") {
        return Some(ScriptCommand::CommentOrEmpty);
    }
    let lower = line.to_lowercase();
    if lower == "simulate" {
        return Some(ScriptCommand::Simulate);
    }
    if lower == "quit" || lower == "exit" {
        return Some(ScriptCommand::Quit);
    }
    if let Some(rest) = lower.strip_prefix("load ") {
        let name = rest.trim();
        if name.is_empty() {
            return None;
        }
        return Some(ScriptCommand::Load(
            line["load ".len()..].trim().to_string(),
        ));
    }
    if let Some(rest) = lower.strip_prefix("buildmodel ") {
        let name = rest.trim();
        if !name.is_empty() {
            let start = line.len().saturating_sub(rest.len());
            return Some(ScriptCommand::Load(line[start..].trim().to_string()));
        }
    }
    if let Some(rest) = lower.strip_prefix("translatemodel ") {
        let name = rest.trim();
        if !name.is_empty() {
            let start = line.len().saturating_sub(rest.len());
            return Some(ScriptCommand::Load(line[start..].trim().to_string()));
        }
    }
    if let Some(rest) = lower.strip_prefix("loadclass ") {
        let name = rest.trim();
        if name.is_empty() {
            return None;
        }
        let start = line.len().saturating_sub(rest.len());
        return Some(ScriptCommand::Load(line[start..].trim().to_string()));
    }
    if let Some(rest) = lower.strip_prefix("setparameter ") {
        let rest = rest.trim();
        let mut tokens = rest.split_whitespace();
        let name = tokens.next()?.to_string();
        let value_str = tokens.next()?;
        let value: f64 = value_str.parse().ok()?;
        return Some(ScriptCommand::SetParameter(name, value));
    }
    if let Some(rest) = lower.strip_prefix("setstartvalue ") {
        let rest = rest.trim();
        let mut tokens = rest.split_whitespace();
        let name = tokens.next()?.to_string();
        let value_str = tokens.next()?;
        let value: f64 = value_str.parse().ok()?;
        return Some(ScriptCommand::SetStartValue(name, value));
    }
    if let Some(rest) = lower.strip_prefix("setstoptime ") {
        let value_str = rest.trim().split_whitespace().next()?;
        let value: f64 = value_str.parse().ok()?;
        return Some(ScriptCommand::SetStopTime(value));
    }
    if let Some(rest) = lower.strip_prefix("getparameter ") {
        let name = rest.trim().to_string();
        if name.is_empty() {
            return None;
        }
        return Some(ScriptCommand::GetParameter(name));
    }
    if let Some(rest) = lower.strip_prefix("getvariable ") {
        let name = rest.trim().to_string();
        if name.is_empty() {
            return None;
        }
        return Some(ScriptCommand::GetVariable(name));
    }
    if lower.strip_prefix("eval ").is_some() {
        let expr_str = line["eval ".len()..].trim();
        if !expr_str.is_empty() {
            return Some(ScriptCommand::Eval(expr_str.to_string()));
        }
    }
    if let Some(rest) = lower.strip_prefix("setresultfile ") {
        let path = rest.trim().to_string();
        if !path.is_empty() {
            return Some(ScriptCommand::SetResultFile(path));
        }
    }
    if let Some(rest) = lower.strip_prefix("saveresult ") {
        let path = rest.trim();
        let path = path
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(path)
            .trim();
        if !path.is_empty() {
            return Some(ScriptCommand::SaveResult(path.to_string()));
        }
    }
    if let Some(rest) = lower.strip_prefix("save ") {
        let path = rest.trim();
        let path = path
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(path)
            .trim();
        if !path.is_empty() {
            return Some(ScriptCommand::SaveResult(path.to_string()));
        }
    }
    if let Some(rest) = lower.strip_prefix("settolerance ") {
        let tokens: Vec<&str> = rest.trim().split_whitespace().collect();
        if tokens.len() >= 1 {
            if let Ok(atol) = tokens[0].parse::<f64>() {
                let rtol = tokens.get(1).and_then(|s| s.parse::<f64>().ok());
                return Some(ScriptCommand::SetTolerance(atol, rtol));
            }
        }
    }
    if lower == "plotall" {
        return Some(ScriptCommand::PlotAll);
    }
    if let Some(rest) = lower.strip_prefix("geterrorstring") {
        let tail = rest.trim();
        if tail.is_empty() || tail == "()" {
            return Some(ScriptCommand::GetErrorString);
        }
    }
    if let Some(rest) = lower.strip_prefix("plot ") {
        let vars: Vec<String> = rest
            .trim()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        if !vars.is_empty() {
            return Some(ScriptCommand::Plot(vars));
        }
    }
    if let Some(rest) = lower.strip_prefix("use ") {
        let rest = rest.trim();
        if !rest.is_empty() {
            let start = line.len().saturating_sub(rest.len());
            return Some(ScriptCommand::SwitchModel(line[start..].trim().to_string()));
        }
    }
    if let Some(rest) = lower.strip_prefix("switchmodel ") {
        let rest = rest.trim();
        if !rest.is_empty() {
            let start = line.len().saturating_sub(rest.len());
            return Some(ScriptCommand::SwitchModel(line[start..].trim().to_string()));
        }
    }
    if let Some(rest) = lower.strip_prefix("instantiatemodel ") {
        let name = rest.trim();
        if !name.is_empty() {
            let start = line.len().saturating_sub(rest.len());
            return Some(ScriptCommand::InstantiateModel(
                line[start..].trim().to_string(),
            ));
        }
    }
    if let Some(rest) = lower.strip_prefix("simulatemodel ") {
        let rest = rest.trim();
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 3 {
            let te: f64 = parts[parts.len() - 2].parse().ok()?;
            let dtt: f64 = parts[parts.len() - 1].parse().ok()?;
            let model = parts[..parts.len() - 2].join(" ");
            if !model.is_empty() {
                return Some(ScriptCommand::SimulateModel {
                    model,
                    t_end: te,
                    dt: dtt,
                });
            }
        }
    }
    None
}

pub struct ScriptRunner {
    pub compiler: Compiler,
    /// SCRIPT-5: multiple loaded models by name; current model key for setParameter/simulate etc.
    pub artifacts_map: std::collections::HashMap<String, crate::compiler::Artifacts>,
    pub current_model: Option<String>,
    mos_vars: std::collections::HashMap<String, MosValue>,
    last_error: String,
}

impl ScriptRunner {
    fn stmt_span(stmt: &MosStmt) -> (usize, usize) {
        match stmt {
            MosStmt::Expr { span, .. }
            | MosStmt::Assign { span, .. }
            | MosStmt::If { span, .. }
            | MosStmt::For { span, .. } => (span.line, span.col),
        }
    }

    fn stmt_kind(stmt: &MosStmt) -> &'static str {
        match stmt {
            MosStmt::Expr { .. } => "expr",
            MosStmt::Assign { .. } => "assign",
            MosStmt::If { .. } => "if",
            MosStmt::For { .. } => "for",
        }
    }

    pub fn new(compiler: Compiler) -> Self {
        ScriptRunner {
            compiler,
            artifacts_map: std::collections::HashMap::new(),
            current_model: None,
            mos_vars: std::collections::HashMap::new(),
            last_error: String::new(),
        }
    }

    fn eval_mos_expr(&self, expr: &MosExpr) -> Result<MosValue, RunError> {
        match expr {
            MosExpr::Number(v) => Ok(MosValue::Number(*v)),
            MosExpr::String(v) => Ok(MosValue::String(v.clone())),
            MosExpr::Bool(v) => Ok(MosValue::Bool(*v)),
            MosExpr::Ident(name) => Ok(self
                .mos_vars
                .get(name)
                .cloned()
                .unwrap_or(MosValue::String(name.clone()))),
            MosExpr::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for it in items {
                    out.push(self.eval_mos_expr(it)?);
                }
                Ok(MosValue::Array(out))
            }
            MosExpr::Record(fields) => {
                let mut rendered = String::from("record(");
                for (idx, (k, v)) in fields.iter().enumerate() {
                    let vv = self.eval_mos_expr(v)?;
                    let text = match vv {
                        MosValue::Number(n) => n.to_string(),
                        MosValue::String(s) => format!("\"{}\"", s),
                        MosValue::Bool(b) => b.to_string(),
                        MosValue::Array(_) => {
                            return Err("record field array values are not supported in strict mode".into())
                        }
                    };
                    if idx > 0 {
                        rendered.push_str(", ");
                    }
                    rendered.push_str(k);
                    rendered.push('=');
                    rendered.push_str(&text);
                }
                rendered.push(')');
                Ok(MosValue::String(rendered))
            }
            MosExpr::Unary { op, expr } => {
                let v = self.eval_mos_expr(expr)?;
                match (op.as_str(), v) {
                    ("+", MosValue::Number(n)) => Ok(MosValue::Number(n)),
                    ("-", MosValue::Number(n)) => Ok(MosValue::Number(-n)),
                    ("not", MosValue::Bool(b)) => Ok(MosValue::Bool(!b)),
                    _ => Err(format!("unsupported unary op '{}' value type", op).into()),
                }
            }
            MosExpr::Binary { op, left, right } => {
                let l = self.eval_mos_expr(left)?;
                let r = self.eval_mos_expr(right)?;
                match (op.as_str(), l, r) {
                    ("+", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Number(a + b)),
                    ("-", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Number(a - b)),
                    ("*", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Number(a * b)),
                    ("/", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Number(a / b)),
                    ("<", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Bool(a < b)),
                    (">", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Bool(a > b)),
                    ("<=", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Bool(a <= b)),
                    (">=", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Bool(a >= b)),
                    ("==", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Bool((a - b).abs() < 1e-12)),
                    ("<>", MosValue::Number(a), MosValue::Number(b)) => Ok(MosValue::Bool((a - b).abs() >= 1e-12)),
                    _ => Err(format!("unsupported binary op '{}' value types", op).into()),
                }
            }
            MosExpr::Range { start, step, stop } => {
                let s = self
                    .eval_mos_expr(start)?
                    .as_f64()
                    .ok_or_else(|| "range start must be numeric".to_string())?;
                let e = self
                    .eval_mos_expr(stop)?
                    .as_f64()
                    .ok_or_else(|| "range stop must be numeric".to_string())?;
                let step_v = if let Some(st) = step {
                    self.eval_mos_expr(st)?
                        .as_f64()
                        .ok_or_else(|| "range step must be numeric".to_string())?
                } else if e >= s {
                    1.0
                } else {
                    -1.0
                };
                if step_v == 0.0 {
                    return Err("range step cannot be 0".into());
                }
                let mut arr = Vec::new();
                let mut cur = s;
                if step_v > 0.0 {
                    while cur <= e + 1e-12 {
                        arr.push(MosValue::Number(cur));
                        cur += step_v;
                    }
                } else {
                    while cur >= e - 1e-12 {
                        arr.push(MosValue::Number(cur));
                        cur += step_v;
                    }
                }
                Ok(MosValue::Array(arr))
            }
            MosExpr::Call { name, args } => {
                let lower = name.to_ascii_lowercase();
                match lower.as_str() {
                    "int" => {
                        if args.len() != 1 {
                            return Err("int(...) expects exactly one argument".into());
                        }
                        let v = self.eval_mos_expr(&args[0].value)?;
                        let n = v
                            .as_f64()
                            .ok_or_else(|| "int(...) argument must be numeric".to_string())?;
                        Ok(MosValue::Number(n.trunc()))
                    }
                    "sample" => {
                        if args.len() != 1 && args.len() != 2 {
                            return Err("sample(...) expects 1 or 2 arguments".into());
                        }
                        let time = self
                            .mos_vars
                            .get("time")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        let (start, period) = if args.len() == 1 {
                            (
                                0.0,
                                self.eval_mos_expr(&args[0].value)?
                                    .as_f64()
                                    .ok_or_else(|| "sample(period) expects numeric period".to_string())?,
                            )
                        } else {
                            (
                                self.eval_mos_expr(&args[0].value)?
                                    .as_f64()
                                    .ok_or_else(|| "sample(start, period) start must be numeric".to_string())?,
                                self.eval_mos_expr(&args[1].value)?
                                    .as_f64()
                                    .ok_or_else(|| "sample(start, period) period must be numeric".to_string())?,
                            )
                        };
                        if period <= 0.0 {
                            return Err("sample(...) period must be > 0".into());
                        }
                        let phase = (time - start) / period;
                        let k = phase.floor();
                        let frac = phase - k;
                        let active =
                            frac.abs() < 1e-12 || (1.0 - frac).abs() < 1e-12 || (time - start).abs() < 1e-12;
                        Ok(MosValue::Bool(active))
                    }
                    "interval" | "hold" | "previous" | "clock" => {
                        if args.len() != 1 {
                            return Err(format!("{}(...) expects exactly one argument", lower).into());
                        }
                        self.eval_mos_expr(&args[0].value)
                    }
                    "subsample" => {
                        if args.len() != 2 {
                            return Err("subSample(clock, n) expects exactly two arguments".into());
                        }
                        let c = self
                            .eval_mos_expr(&args[0].value)?
                            .as_f64()
                            .ok_or_else(|| "subSample clock must be numeric".to_string())?;
                        let n = self
                            .eval_mos_expr(&args[1].value)?
                            .as_f64()
                            .ok_or_else(|| "subSample n must be numeric".to_string())?;
                        Ok(MosValue::Number(c * n))
                    }
                    "supersample" => {
                        if args.len() != 2 {
                            return Err("superSample(clock, n) expects exactly two arguments".into());
                        }
                        let c = self
                            .eval_mos_expr(&args[0].value)?
                            .as_f64()
                            .ok_or_else(|| "superSample clock must be numeric".to_string())?;
                        let n = self
                            .eval_mos_expr(&args[1].value)?
                            .as_f64()
                            .ok_or_else(|| "superSample n must be numeric".to_string())?;
                        if n == 0.0 {
                            return Err("superSample n cannot be 0".into());
                        }
                        Ok(MosValue::Number(c / n))
                    }
                    "shiftsample" => {
                        if args.len() != 2 {
                            return Err("shiftSample(clock, n) expects exactly two arguments".into());
                        }
                        let c = self
                            .eval_mos_expr(&args[0].value)?
                            .as_f64()
                            .ok_or_else(|| "shiftSample clock must be numeric".to_string())?;
                        let n = self
                            .eval_mos_expr(&args[1].value)?
                            .as_f64()
                            .ok_or_else(|| "shiftSample n must be numeric".to_string())?;
                        Ok(MosValue::Number(c + n))
                    }
                    _ => Err("nested function call expression is not supported in strict mode".into()),
                }
            }
        }
    }

    fn apply_named_simulate_args(
        &mut self,
        args: &[crate::parser::mos_parse::MosArg],
    ) -> Result<(), RunError> {
        for a in args {
            let Some(key) = a.name.as_deref() else {
                continue;
            };
            let key_l = key.to_ascii_lowercase();
            let value = self.eval_mos_expr(&a.value)?;
            match key_l.as_str() {
                "model" => {
                    let m = value
                        .as_string()
                        .ok_or_else(|| "simulate model must be string".to_string())?;
                    if self.artifacts_map.contains_key(&m) {
                        self.current_model = Some(m);
                    } else {
                        // Keep OMC-compatible behavior: simulate(model="X") can trigger load.
                        let cont = self.run_command(ScriptCommand::Load(m.clone()))?;
                        if !cont {
                            return Err("simulate model load requested quit unexpectedly".into());
                        }
                    }
                }
                "stoptime" => {
                    let v = value
                        .as_f64()
                        .ok_or_else(|| "simulate stopTime must be number".to_string())?;
                    self.compiler.options.t_end = v;
                    if let Ok(arts) = self.current_artifacts() {
                        arts.t_end = v;
                    }
                }
                "stepsize" => {
                    let v = value
                        .as_f64()
                        .ok_or_else(|| "simulate stepSize must be number".to_string())?;
                    self.compiler.options.dt = v;
                    if let Ok(arts) = self.current_artifacts() {
                        arts.dt = v;
                    }
                }
                "numberofintervals" => {
                    let n = value
                        .as_f64()
                        .ok_or_else(|| "simulate numberOfIntervals must be number".to_string())?;
                    if n <= 0.0 {
                        return Err("simulate numberOfIntervals must be > 0".into());
                    }
                    let t_end = self
                        .current_artifacts_ref()
                        .map(|a| a.t_end)
                        .unwrap_or(self.compiler.options.t_end);
                    let dt = t_end / n;
                    self.compiler.options.dt = dt;
                    if let Ok(arts) = self.current_artifacts() {
                        arts.dt = dt;
                    }
                }
                "tolerance" => {
                    let v = value
                        .as_f64()
                        .ok_or_else(|| "simulate tolerance must be number".to_string())?;
                    self.compiler.options.atol = v;
                    self.compiler.options.rtol = v;
                    if let Ok(arts) = self.current_artifacts() {
                        arts.atol = v;
                        arts.rtol = v;
                    }
                }
                "method" => {
                    let m = value
                        .as_string()
                        .ok_or_else(|| "simulate method must be string".to_string())?
                        .to_ascii_lowercase();
                    let mapped = match m.as_str() {
                        "dassl" => "ida",
                        "cvode" => "cvode",
                        "euler" => "rk4",
                        "rungekutta" | "rk45" => "rk45",
                        "implicit" => "implicit",
                        _ => {
                            return Err(format!("simulate method '{}' is not supported", m).into());
                        }
                    };
                    self.compiler.options.solver = mapped.to_string();
                    if let Ok(arts) = self.current_artifacts() {
                        arts.solver = mapped.to_string();
                    }
                }
                "resultfile" => {
                    let rf = value
                        .as_string()
                        .ok_or_else(|| "simulate resultFile must be string".to_string())?;
                    self.compiler.options.result_file = Some(rf.clone());
                    if let Ok(arts) = self.current_artifacts() {
                        arts.result_file = Some(rf);
                    }
                }
                "starttime" | "outputformat" => {}
                other => {
                    return Err(format!("simulate named argument '{}' is not supported in strict mode", other).into());
                }
            }
        }
        Ok(())
    }

    fn mos_call_to_command(
        &self,
        name: &str,
        args: &[crate::parser::mos_parse::MosArg],
    ) -> Result<ScriptCommand, RunError> {
        let lower = name.to_ascii_lowercase();
        let eval_arg = |idx: usize| -> Result<MosValue, RunError> {
            let arg = args.get(idx).ok_or_else(|| format!("missing argument {} for {}", idx, name))?;
            self.eval_mos_expr(&arg.value)
        };
        let named_or_pos = |key: &str, pos: usize| -> Option<&crate::parser::mos_parse::MosArg> {
            args.iter()
                .find(|a| a.name.as_deref().map(|k| k.eq_ignore_ascii_case(key)).unwrap_or(false))
                .or_else(|| args.get(pos))
        };
        match lower.as_str() {
            "loadmodel" | "loadclass" | "buildmodel" | "translatemodel" => {
                let model = eval_arg(0)?
                    .as_string()
                    .ok_or_else(|| format!("{} arg0 must be model name string", name))?;
                Ok(ScriptCommand::Load(model))
            }
            "instantiatemodel" => {
                let model = eval_arg(0)?
                    .as_string()
                    .ok_or_else(|| "instantiateModel arg0 must be model name string".to_string())?;
                Ok(ScriptCommand::InstantiateModel(model))
            }
            "simulatemodel" => {
                let model = eval_arg(0)?
                    .as_string()
                    .ok_or_else(|| "simulateModel arg0 must be model name string".to_string())?;
                let t_end = eval_arg(1)?
                    .as_f64()
                    .ok_or_else(|| "simulateModel arg1 must be stop time number".to_string())?;
                let dt = eval_arg(2)?
                    .as_f64()
                    .ok_or_else(|| "simulateModel arg2 must be step size number".to_string())?;
                Ok(ScriptCommand::SimulateModel { model, t_end, dt })
            }
            "simulate" => {
                if args.iter().any(|a| a.name.is_some()) {
                    return Ok(ScriptCommand::Simulate);
                }
                let model_named = named_or_pos("model", 0);
                if let Some(arg0) = model_named {
                    if !matches!(arg0.name.as_deref(), Some("stopTime" | "startTime" | "stepSize" | "method" | "tolerance" | "resultFile" | "outputFormat")) {
                        let model = self
                            .eval_mos_expr(&arg0.value)?
                            .as_string()
                            .ok_or_else(|| "simulate model must be string".to_string())?;
                        let stop_time = named_or_pos("stopTime", 1)
                            .map(|a| self.eval_mos_expr(&a.value))
                            .transpose()?
                            .and_then(|v| v.as_f64())
                            .unwrap_or(1.0);
                        let step_size = named_or_pos("stepSize", 2)
                            .map(|a| self.eval_mos_expr(&a.value))
                            .transpose()?
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.01);
                        return Ok(ScriptCommand::SimulateModel {
                            model,
                            t_end: stop_time,
                            dt: step_size,
                        });
                    }
                }
                Ok(ScriptCommand::Simulate)
            }
            "setparameter" => {
                let name_v = eval_arg(0)?
                    .as_string()
                    .ok_or_else(|| "setParameter arg0 must be variable name string".to_string())?;
                let value = eval_arg(1)?
                    .as_f64()
                    .ok_or_else(|| "setParameter arg1 must be numeric".to_string())?;
                Ok(ScriptCommand::SetParameter(name_v, value))
            }
            "setstartvalue" => {
                let name_v = eval_arg(0)?
                    .as_string()
                    .ok_or_else(|| "setStartValue arg0 must be variable name string".to_string())?;
                let value = eval_arg(1)?
                    .as_f64()
                    .ok_or_else(|| "setStartValue arg1 must be numeric".to_string())?;
                Ok(ScriptCommand::SetStartValue(name_v, value))
            }
            "setstoptime" => {
                let value = eval_arg(0)?
                    .as_f64()
                    .ok_or_else(|| "setStopTime arg0 must be numeric".to_string())?;
                Ok(ScriptCommand::SetStopTime(value))
            }
            "setresultfile" | "saveresult" | "save" => {
                let path = eval_arg(0)?
                    .as_string()
                    .ok_or_else(|| format!("{} arg0 must be path string", name))?;
                Ok(ScriptCommand::SetResultFile(path))
            }
            "settolerance" => {
                let atol = eval_arg(0)?
                    .as_f64()
                    .ok_or_else(|| "setTolerance arg0 must be number".to_string())?;
                let rtol = if args.len() > 1 {
                    Some(
                        eval_arg(1)?
                            .as_f64()
                            .ok_or_else(|| "setTolerance arg1 must be number".to_string())?,
                    )
                } else {
                    None
                };
                Ok(ScriptCommand::SetTolerance(atol, rtol))
            }
            "getparametervalue" | "getparameter" => {
                let n = eval_arg(0)?
                    .as_string()
                    .ok_or_else(|| "getParameterValue arg0 must be variable name string".to_string())?;
                Ok(ScriptCommand::GetParameter(n))
            }
            "getvariablevalue" | "getvariable" => {
                let n = eval_arg(0)?
                    .as_string()
                    .ok_or_else(|| "getVariableValue arg0 must be variable name string".to_string())?;
                Ok(ScriptCommand::GetVariable(n))
            }
            "plot" => {
                let mut vars = Vec::new();
                for a in args {
                    let v = self.eval_mos_expr(&a.value)?;
                    match v {
                        MosValue::String(s) => vars.push(s),
                        MosValue::Array(arr) => {
                            for vv in arr {
                                if let MosValue::String(s) = vv {
                                    vars.push(s);
                                } else {
                                    return Err("plot array argument must contain string variable names".into());
                                }
                            }
                        }
                        _ => return Err("plot argument must be string or string array".into()),
                    }
                }
                Ok(ScriptCommand::Plot(vars))
            }
            "plotall" => Ok(ScriptCommand::PlotAll),
            "geterrorstring" => Ok(ScriptCommand::GetErrorString),
            "quit" | "exit" => Ok(ScriptCommand::Quit),
            other => Err(format!("unsupported .mos function '{}' in strict mode", other).into()),
        }
    }

    fn run_mos_stmt(&mut self, stmt: &MosStmt) -> Result<bool, RunError> {
        match stmt {
            MosStmt::Assign { name, value, .. } => {
                let v = self.eval_mos_expr(value)?;
                self.mos_vars.insert(name.clone(), v);
                Ok(true)
            }
            MosStmt::Expr { expr, .. } => {
                if let MosExpr::Call { name, args } = expr {
                    if name.eq_ignore_ascii_case("simulate") {
                        self.apply_named_simulate_args(args)?;
                    }
                    let cmd = self.mos_call_to_command(name, args)?;
                    return self.run_command(cmd);
                }
                // Allow bare literal expressions as no-op; others are strict errors.
                match expr {
                    MosExpr::Number(_) | MosExpr::String(_) | MosExpr::Bool(_) | MosExpr::Ident(_) => Ok(true),
                    _ => Err("unsupported expression statement in strict .mos mode".into()),
                }
            }
            MosStmt::If {
                cond,
                then_body,
                elseif,
                else_body,
                span,
            } => {
                let c = self.eval_mos_expr(cond)?.as_bool().ok_or_else(|| {
                    format!("if condition is not boolean/numeric at line {}, col {}", span.line, span.col)
                })?;
                if c {
                    for s in then_body {
                        if !self.run_mos_stmt(s)? {
                            return Ok(false);
                        }
                    }
                    return Ok(true);
                }
                for (ec, body) in elseif {
                    let b = self.eval_mos_expr(ec)?.as_bool().ok_or_else(|| {
                        format!("elseif condition is not boolean/numeric at line {}, col {}", span.line, span.col)
                    })?;
                    if b {
                        for s in body {
                            if !self.run_mos_stmt(s)? {
                                return Ok(false);
                            }
                        }
                        return Ok(true);
                    }
                }
                for s in else_body {
                    if !self.run_mos_stmt(s)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            MosStmt::For { var, iter, body, span } => {
                let it = self.eval_mos_expr(iter)?;
                let items = match it {
                    MosValue::Array(v) => v,
                    MosValue::Number(n) => {
                        if n < 1.0 {
                            return Ok(true);
                        }
                        let mut out = Vec::new();
                        let mut i = 1_i64;
                        while (i as f64) <= n {
                            out.push(MosValue::Number(i as f64));
                            i += 1;
                        }
                        out
                    }
                    _ => {
                        return Err(format!(
                            "for iterator must be array or numeric range-upper at line {}, col {}",
                            span.line, span.col
                        )
                        .into())
                    }
                };
                for v in items {
                    self.mos_vars.insert(var.clone(), v);
                    for s in body {
                        if !self.run_mos_stmt(s)? {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
        }
    }

    fn current_artifacts(&mut self) -> Result<&mut crate::compiler::Artifacts, RunError> {
        let name = self
            .current_model
            .as_deref()
            .ok_or("no model loaded (run load first)")?;
        self.artifacts_map.get_mut(name).ok_or_else(|| {
            format!(
                "model '{}' not loaded (use 'use {}' after loading)",
                name, name
            )
            .into()
        })
    }

    fn current_artifacts_ref(&self) -> Result<&crate::compiler::Artifacts, RunError> {
        let name = self
            .current_model
            .as_deref()
            .ok_or("no model loaded (run load first)")?;
        self.artifacts_map
            .get(name)
            .ok_or_else(|| format!("model '{}' not loaded", name).into())
    }

    pub fn run_command(&mut self, cmd: ScriptCommand) -> Result<bool, RunError> {
        match cmd {
            ScriptCommand::CommentOrEmpty => Ok(true),
            ScriptCommand::Quit => Ok(false),
            ScriptCommand::Load(model_name) => {
                println!("Loading model: {}", model_name);
                let out = self.compiler.compile(&model_name)?;
                let warnings = self.compiler.take_warnings();
                let warn_level = self.compiler.options.warnings_level.as_str();
                if warn_level != "none" {
                    for w in &warnings {
                        if warn_level == "error" {
                            return Err(w.to_string().into());
                        }
                        eprintln!("{}", w);
                    }
                }
                match out {
                    CompileOutput::Simulation(artifacts) => {
                        self.artifacts_map.insert(model_name.clone(), artifacts);
                        self.current_model = Some(model_name);
                        Ok(true)
                    }
                    CompileOutput::FunctionRun(v) => {
                        eprintln!(
                            "Script mode expects a simulation model, got function result: {}",
                            v
                        );
                        Err("load: model is a function, not a simulation model".into())
                    }
                    CompileOutput::FlatSnapshotDone => Err(
                        "load: flat-snapshot-only compile produced no simulation artifacts".into(),
                    ),
                }
            }
            ScriptCommand::SwitchModel(name) => {
                if self.artifacts_map.contains_key(&name) {
                    self.current_model = Some(name);
                    Ok(true)
                } else {
                    Err(format!("switchModel: model '{}' not loaded (load it first)", name).into())
                }
            }
            ScriptCommand::InstantiateModel(model_name) => {
                let warn_level = self.compiler.options.warnings_level.clone();
                let out = self.compiler.compile(&model_name)?;
                let warnings = self.compiler.take_warnings();
                if warn_level != "none" {
                    for w in &warnings {
                        if warn_level == "error" {
                            return Err(w.to_string().into());
                        }
                        eprintln!("{}", w);
                    }
                }
                match out {
                    CompileOutput::Simulation(_) => {
                        println!("true");
                        Ok(true)
                    }
                    CompileOutput::FunctionRun(_) => {
                        Err("instantiateModel: expected simulation model".into())
                    }
                    CompileOutput::FlatSnapshotDone => Err(
                        "instantiateModel: flat-snapshot-only produced no simulation model".into(),
                    ),
                }
            }
            ScriptCommand::SimulateModel { model, t_end, dt } => {
                println!("Loading model: {}", model);
                let warn_level = self.compiler.options.warnings_level.clone();
                let out = self.compiler.compile(&model)?;
                let warnings = self.compiler.take_warnings();
                if warn_level != "none" {
                    for w in &warnings {
                        if warn_level == "error" {
                            return Err(w.to_string().into());
                        }
                        eprintln!("{}", w);
                    }
                }
                match out {
                    CompileOutput::Simulation(mut artifacts) => {
                        artifacts.t_end = t_end;
                        artifacts.dt = dt;
                        self.artifacts_map.insert(model.clone(), artifacts);
                        self.current_model = Some(model);
                        let arts = self.current_artifacts_ref()?;
                        run_simulation(
                            arts.calc_derivs,
                            arts.when_count,
                            arts.crossings_count,
                            arts.states.clone(),
                            arts.discrete_vals.clone(),
                            arts.params.clone(),
                            &arts.state_vars,
                            &arts.discrete_vars,
                            &arts.output_vars,
                            &arts.output_start_vals,
                            &arts.state_var_index,
                            arts.t_end,
                            arts.dt,
                            arts.numeric_ode_jacobian,
                            arts.symbolic_ode_jacobian.as_ref(),
                            &arts.newton_tearing_var_names,
                            arts.atol,
                            arts.rtol,
                            arts.differential_index,
                            arts.ida_component_id.as_slice(),
                            &arts.solver,
                            arts.output_interval,
                            arts.result_file.as_deref(),
                            &arts.clock_partition_schedule,
                            None,
                        )?;
                        Ok(true)
                    }
                    CompileOutput::FunctionRun(v) => Err(format!(
                        "simulateModel: expected simulation model, got function result {}",
                        v
                    )
                    .into()),
                    CompileOutput::FlatSnapshotDone => Err(
                        "simulateModel: flat-snapshot-only produced no simulation artifacts".into(),
                    ),
                }
            }
            ScriptCommand::SetParameter(name, value) => {
                let arts = self.current_artifacts()?;
                if let Some(&i) = arts.state_var_index.get(&name) {
                    if i < arts.states.len() {
                        arts.states[i] = value;
                        return Ok(true);
                    }
                }
                if let Some((i, _)) = arts
                    .param_vars
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.as_str() == name)
                {
                    if i < arts.params.len() {
                        arts.params[i] = value;
                        return Ok(true);
                    }
                }
                if let Some((i, _)) = arts
                    .discrete_vars
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.as_str() == name)
                {
                    if i < arts.discrete_vals.len() {
                        arts.discrete_vals[i] = value;
                        return Ok(true);
                    }
                }
                Err(format!("setParameter: unknown variable '{}'", name).into())
            }
            ScriptCommand::SetStartValue(name, value) => {
                let arts = self.current_artifacts()?;
                if let Some(&i) = arts.state_var_index.get(&name) {
                    if i < arts.states.len() {
                        arts.states[i] = value;
                        return Ok(true);
                    }
                }
                if let Some((i, _)) = arts
                    .param_vars
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.as_str() == name)
                {
                    if i < arts.params.len() {
                        arts.params[i] = value;
                        return Ok(true);
                    }
                }
                if let Some((i, _)) = arts
                    .discrete_vars
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.as_str() == name)
                {
                    if i < arts.discrete_vals.len() {
                        arts.discrete_vals[i] = value;
                        return Ok(true);
                    }
                }
                Err(format!("setStartValue: unknown variable '{}'", name).into())
            }
            ScriptCommand::SetStopTime(value) => {
                let arts = self.current_artifacts()?;
                arts.t_end = value;
                Ok(true)
            }
            ScriptCommand::GetParameter(name) => {
                let arts = self.current_artifacts_ref()?;
                if let Some((i, _)) = arts
                    .param_vars
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.as_str() == name)
                {
                    if i < arts.params.len() {
                        println!("{}", arts.params[i]);
                        return Ok(true);
                    }
                }
                Err(format!("getParameter: unknown parameter '{}'", name).into())
            }
            ScriptCommand::GetVariable(name) => {
                let arts = self.current_artifacts_ref()?;
                if let Some(&i) = arts.state_var_index.get(&name) {
                    if i < arts.states.len() {
                        println!("{}", arts.states[i]);
                        return Ok(true);
                    }
                }
                if let Some((i, _)) = arts
                    .param_vars
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.as_str() == name)
                {
                    if i < arts.params.len() {
                        println!("{}", arts.params[i]);
                        return Ok(true);
                    }
                }
                if let Some((i, _)) = arts
                    .discrete_vars
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.as_str() == name)
                {
                    if i < arts.discrete_vals.len() {
                        println!("{}", arts.discrete_vals[i]);
                        return Ok(true);
                    }
                }
                Err(format!("getVariable: unknown variable '{}'", name).into())
            }
            ScriptCommand::Eval(expr_str) => {
                let arts = self.current_artifacts_ref()?;
                let mut vars: std::collections::HashMap<String, f64> =
                    std::collections::HashMap::new();
                for (i, name) in arts.state_vars.iter().enumerate() {
                    if i < arts.states.len() {
                        vars.insert(name.clone(), arts.states[i]);
                    }
                }
                for (i, name) in arts.param_vars.iter().enumerate() {
                    if i < arts.params.len() {
                        vars.insert(name.clone(), arts.params[i]);
                    }
                }
                for (i, name) in arts.discrete_vars.iter().enumerate() {
                    if i < arts.discrete_vals.len() {
                        vars.insert(name.clone(), arts.discrete_vals[i]);
                    }
                }
                vars.insert("time".to_string(), 0.0);
                let expr = parse_simple_expr(&expr_str)
                    .ok_or_else(|| format!("eval: could not parse expression '{}' (use: var, number, or var op number)", expr_str))?;
                let val = eval_expr(&expr, &vars)
                    .map_err(|e| format!("eval: {} (expr '{}')", e, expr_str))?;
                println!("{}", val);
                Ok(true)
            }
            ScriptCommand::SetResultFile(path) | ScriptCommand::SaveResult(path) => {
                let arts = self.current_artifacts()?;
                arts.result_file = Some(path);
                Ok(true)
            }
            ScriptCommand::SetTolerance(atol, rtol_opt) => {
                let arts = self.current_artifacts()?;
                arts.atol = atol;
                if let Some(rtol) = rtol_opt {
                    arts.rtol = rtol;
                }
                Ok(true)
            }
            ScriptCommand::Plot(vars) => {
                let _ = self.current_artifacts_ref()?;
                if vars.is_empty() {
                    return Ok(true);
                }
                eprintln!(
                    "plot: variables {} (run simulate and use result file for data)",
                    vars.join(", ")
                );
                Ok(true)
            }
            ScriptCommand::PlotAll => {
                let arts = self.current_artifacts_ref()?;
                let vars = arts.output_vars.clone();
                if vars.is_empty() {
                    return Ok(true);
                }
                eprintln!(
                    "plotAll: variables {} (run simulate and use result file for data)",
                    vars.join(", ")
                );
                Ok(true)
            }
            ScriptCommand::GetErrorString => {
                println!("{}", self.last_error);
                Ok(true)
            }
            ScriptCommand::Simulate => {
                let arts = self.current_artifacts_ref()?;
                run_simulation(
                    arts.calc_derivs,
                    arts.when_count,
                    arts.crossings_count,
                    arts.states.clone(),
                    arts.discrete_vals.clone(),
                    arts.params.clone(),
                    &arts.state_vars,
                    &arts.discrete_vars,
                    &arts.output_vars,
                    &arts.output_start_vals,
                    &arts.state_var_index,
                    arts.t_end,
                    arts.dt,
                    arts.numeric_ode_jacobian,
                    arts.symbolic_ode_jacobian.as_ref(),
                    &arts.newton_tearing_var_names,
                    arts.atol,
                    arts.rtol,
                    arts.differential_index,
                    arts.ida_component_id.as_slice(),
                    &arts.solver,
                    arts.output_interval,
                    arts.result_file.as_deref(),
                    &arts.clock_partition_schedule,
                    None,
                )?;
                Ok(true)
            }
        }
    }

    pub fn run_script<R: Read>(&mut self, reader: R) -> Result<(), RunError> {
        self.run_script_named(reader, "<stdin>")
    }

    pub fn run_script_named<R: Read>(&mut self, reader: R, script_name: &str) -> Result<(), RunError> {
        let engine = std::env::var("RUSTMODLICA_SCRIPT_ENGINE")
            .ok()
            .unwrap_or_else(|| "mos".to_string())
            .to_ascii_lowercase();
        if engine == "legacy" {
            let mut buf = std::io::BufReader::new(reader);
            let mut line = String::new();
            let mut line_no = 0u32;
            loop {
                line.clear();
                let n = buf.read_line(&mut line).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                line_no += 1;
                match parse_script_line(&line) {
                    Some(cmd) => {
                        let cont = self.run_command(cmd)?;
                        if !cont {
                            break;
                        }
                    }
                    None => {
                        return Err(format!("Script '{}' line {}: unknown command (supported: load/loadClass/buildModel/translateModel, use/switchModel, instantiateModel, simulateModel, setParameter, setStartValue, setStopTime, setResultFile, saveResult, save, setTolerance, plot/plotAll, getParameter, getVariable, getErrorString, eval, simulate, quit)", script_name, line_no).into());
                    }
                }
            }
            return Ok(());
        }

        let mut source = String::new();
        let mut br = std::io::BufReader::new(reader);
        br.read_to_string(&mut source)
            .map_err(|e| format!("failed to read .mos script: {}", e))?;
        let stmts = parse_mos_script(&source)
            .map_err(|e| format!("Script '{}': {}", script_name, e))?;
        for (idx, stmt) in stmts.iter().enumerate() {
            let (line, col) = Self::stmt_span(stmt);
            let kind = Self::stmt_kind(stmt);
            let cont = self.run_mos_stmt(stmt).map_err(|e| {
                let detailed = format!(
                    "Script '{}' stmt#{} [{}] at line {}, col {}: {}",
                    script_name,
                    idx + 1,
                    kind,
                    line,
                    col,
                    e
                );
                self.last_error = detailed.clone();
                detailed
            })?;
            if !cont {
                break;
            }
        }
        Ok(())
    }
}
