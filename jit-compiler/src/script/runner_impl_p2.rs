impl ScriptRunner {
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
                    CompileOutput::ValidationParseOk
                    | CompileOutput::ValidationFlattenOk { .. }
                    | CompileOutput::ValidationAnalyzed(_) => Err(
                        "load: tiered compile stop produced no simulation artifacts (use full compile)"
                            .into(),
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
                    CompileOutput::ValidationParseOk
                    | CompileOutput::ValidationFlattenOk { .. }
                    | CompileOutput::ValidationAnalyzed(_) => Err(
                        "instantiateModel: tiered compile stop produced no simulation model".into(),
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
                    CompileOutput::ValidationParseOk
                    | CompileOutput::ValidationFlattenOk { .. }
                    | CompileOutput::ValidationAnalyzed(_) => Err(
                        "simulateModel: tiered compile stop produced no simulation artifacts".into(),
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
            let (line, col) = ScriptRunner::stmt_span(stmt);
            let kind = ScriptRunner::stmt_kind(stmt);
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
