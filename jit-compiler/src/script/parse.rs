// INT-2: Script mode. Minimal grammar: load ModelName, setParameter name value, simulate, quit.
// SCRIPT-2: eval <expr> for expression evaluation.

use crate::ast::{Expression, Operator};

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
pub(super) fn parse_simple_expr(s: &str) -> Option<Expression> {
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
