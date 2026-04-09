use std::io::BufRead;
use std::io::Read;

use crate::compiler::{CompileOutput, Compiler};
use crate::expr_eval::eval_expr;
use crate::parser::mos_parse::{parse_mos_script, MosExpr, MosStmt};
use crate::simulation::run_simulation;

use super::parse::{parse_script_line, parse_simple_expr, ScriptCommand};
use super::runner::{MosValue, ScriptRunner};

type RunError = Box<dyn std::error::Error + Send + Sync>;

impl ScriptRunner {
    pub(crate) fn stmt_span(stmt: &MosStmt) -> (usize, usize) {
        match stmt {
            MosStmt::Expr { span, .. }
            | MosStmt::Assign { span, .. }
            | MosStmt::If { span, .. }
            | MosStmt::For { span, .. } => (span.line, span.col),
        }
    }

    pub(crate) fn stmt_kind(stmt: &MosStmt) -> &'static str {
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
                    "abs" => {
                        if args.len() != 1 {
                            return Err("abs(x) expects one argument".into());
                        }
                        let v = self
                            .eval_mos_expr(&args[0].value)?
                            .as_f64()
                            .ok_or_else(|| "abs(x): x must be numeric".to_string())?;
                        Ok(MosValue::Number(v.abs()))
                    }
                    "sqrt" => {
                        if args.len() != 1 {
                            return Err("sqrt(x) expects one argument".into());
                        }
                        let v = self
                            .eval_mos_expr(&args[0].value)?
                            .as_f64()
                            .ok_or_else(|| "sqrt(x): x must be numeric".to_string())?;
                        if v < 0.0 {
                            return Err("sqrt(x): x must be >= 0".into());
                        }
                        Ok(MosValue::Number(v.sqrt()))
                    }
                    "min" | "max" => {
                        if args.len() != 2 {
                            return Err(format!("{}(a, b) expects two arguments", name).into());
                        }
                        let a = self
                            .eval_mos_expr(&args[0].value)?
                            .as_f64()
                            .ok_or_else(|| format!("{}(a, b): a must be numeric", name))?;
                        let b = self
                            .eval_mos_expr(&args[1].value)?
                            .as_f64()
                            .ok_or_else(|| format!("{}(a, b): b must be numeric", name))?;
                        Ok(MosValue::Number(if lower == "min" { a.min(b) } else { a.max(b) }))
                    }
                    "string" => {
                        if args.len() != 1 {
                            return Err("String(x) expects one argument".into());
                        }
                        let v = self.eval_mos_expr(&args[0].value)?;
                        let s = match v {
                            MosValue::String(s) => s,
                            MosValue::Number(n) => n.to_string(),
                            MosValue::Bool(b) => {
                                if b { "true".to_string() } else { "false".to_string() }
                            }
                            MosValue::Array(_) => return Err("String(array) is not supported".into()),
                        };
                        Ok(MosValue::String(s))
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

}
