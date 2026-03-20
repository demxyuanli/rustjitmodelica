// INT-2: Script mode. Minimal grammar: load ModelName, setParameter name value, simulate, quit.
// SCRIPT-2: eval <expr> for expression evaluation.

use std::io::BufRead;
use std::io::Read;

use crate::ast::{Expression, Operator};
use crate::compiler::{CompileOutput, Compiler};
use crate::expr_eval::eval_expr;
use crate::simulation::run_simulation;

type RunError = Box<dyn std::error::Error + Send + Sync>;

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
        return Some(Expression::Variable(t.to_string()));
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
            Expression::Variable(var.to_string())
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
    None
}

pub struct ScriptRunner {
    pub compiler: Compiler,
    /// SCRIPT-5: multiple loaded models by name; current model key for setParameter/simulate etc.
    pub artifacts_map: std::collections::HashMap<String, crate::compiler::Artifacts>,
    pub current_model: Option<String>,
}

impl ScriptRunner {
    pub fn new(compiler: Compiler) -> Self {
        ScriptRunner {
            compiler,
            artifacts_map: std::collections::HashMap::new(),
            current_model: None,
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
                println!("");
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
                    &arts.state_var_index,
                    arts.t_end,
                    arts.dt,
                    arts.numeric_ode_jacobian,
                    arts.symbolic_ode_jacobian.as_ref(),
                    &arts.newton_tearing_var_names,
                    arts.atol,
                    arts.rtol,
                    &arts.solver,
                    arts.output_interval,
                    arts.result_file.as_deref(),
                    None,
                )?;
                Ok(true)
            }
        }
    }

    pub fn run_script<R: Read>(&mut self, reader: R) -> Result<(), RunError> {
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
                    return Err(format!("Script line {}: unknown command (supported: load/loadClass/buildModel/translateModel, use/switchModel, setParameter, setStartValue, setStopTime, setResultFile, saveResult, save, setTolerance, plot/plotAll, getParameter, getVariable, getErrorString, eval, simulate, quit)", line_no).into());
                }
            }
        }
        Ok(())
    }
}
