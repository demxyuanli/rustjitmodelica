// INT-2: Script mode. Minimal grammar: load ModelName, setParameter name value, simulate, quit.
// Script is read from file or stdin; each line one command.

use std::io::BufRead;
use std::io::Read;

use crate::compiler::{Compiler, CompileOutput};
use crate::simulation::run_simulation;

type RunError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub enum ScriptCommand {
    Load(String),
    SetParameter(String, f64),
    Simulate,
    Quit,
    CommentOrEmpty,
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
        return Some(ScriptCommand::Load(line["load ".len()..].trim().to_string()));
    }
    if let Some(rest) = lower.strip_prefix("setparameter ") {
        let rest = rest.trim();
        let mut tokens = rest.split_whitespace();
        let name = tokens.next()?.to_string();
        let value_str = tokens.next()?;
        let value: f64 = value_str.parse().ok()?;
        return Some(ScriptCommand::SetParameter(name, value));
    }
    None
}

pub struct ScriptRunner {
    pub compiler: Compiler,
    pub artifacts: Option<crate::compiler::Artifacts>,
}

impl ScriptRunner {
    pub fn new(compiler: Compiler) -> Self {
        ScriptRunner {
            compiler,
            artifacts: None,
        }
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
                        self.artifacts = Some(artifacts);
                        Ok(true)
                    }
                    CompileOutput::FunctionRun(v) => {
                        eprintln!("Script mode expects a simulation model, got function result: {}", v);
                        Err("load: model is a function, not a simulation model".into())
                    }
                }
            }
            ScriptCommand::SetParameter(name, value) => {
                let arts = self
                    .artifacts
                    .as_mut()
                    .ok_or("setParameter: no model loaded (run load first)")?;
                if let Some(&i) = arts.state_var_index.get(&name) {
                    if i < arts.states.len() {
                        arts.states[i] = value;
                        return Ok(true);
                    }
                }
                if let Some((i, _)) = arts.param_vars.iter().enumerate().find(|(_, s)| s.as_str() == name) {
                    if i < arts.params.len() {
                        arts.params[i] = value;
                        return Ok(true);
                    }
                }
                if let Some((i, _)) = arts.discrete_vars.iter().enumerate().find(|(_, s)| s.as_str() == name) {
                    if i < arts.discrete_vals.len() {
                        arts.discrete_vals[i] = value;
                        return Ok(true);
                    }
                }
                Err(format!("setParameter: unknown variable '{}'", name).into())
            }
            ScriptCommand::Simulate => {
                let arts = self
                    .artifacts
                    .as_ref()
                    .ok_or("simulate: no model loaded (run load first)")?;
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
                    return Err(format!("Script line {}: unknown command (expected load, setParameter, simulate, quit)", line_no).into());
                }
            }
        }
        Ok(())
    }
}
