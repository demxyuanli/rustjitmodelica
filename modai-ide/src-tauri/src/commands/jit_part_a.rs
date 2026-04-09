fn jit_validate_sync(request: JitValidateRequest) -> Result<JitValidateResult, String> {
    let _timer = ScopedTimer::new("jit_validate");
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let _eq_expand_mode_env =
        ScopedEnvVar::set("RUSTMODLICA_EQ_EXPAND_PARALLEL_MODE", normalize_eq_expand_parallel_mode(request.options.as_ref()));
    let param_impact_probe: Option<Vec<String>> = request
        .options
        .as_ref()
        .and_then(|o| o.param_change_impact_probe.clone());
    let instance_impact_probe: Option<String> = request
        .options
        .as_ref()
        .and_then(|o| o.instance_change_impact_probe.clone());
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
    compiler.options.validate_only = true;
    with_loader_paths(
        &mut compiler,
        request.project_dir.as_ref(),
        request.resolver_context.as_ref(),
    );
    let result = compiler.compile_from_source(&model_name, &request.code);
    let warnings = compiler.take_warnings();
    let perf = compiler.take_compile_perf_report();
    let provenance = build_provenance_report(
        &compiler,
        param_impact_probe.as_ref(),
        instance_impact_probe.as_deref(),
    );
    match result {
        Ok(rustmodlica::CompileOutput::FunctionRun(_)) => {
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                vec![],
                vec![],
                compile_trace,
                Some("full".to_string()),
                false,
                provenance,
            ))
        }
        Ok(rustmodlica::CompileOutput::Simulation(artifacts)) => {
            let state_vars = artifacts.state_vars;
            let output_vars = artifacts.output_vars;
            let compile_trace = build_compile_trace(
                perf.as_ref(),
                state_vars.len(),
                output_vars.len(),
            );
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                state_vars,
                output_vars,
                compile_trace,
                Some("full".to_string()),
                false,
                provenance,
            ))
        }
        Ok(rustmodlica::CompileOutput::FlatSnapshotDone) => {
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                vec![],
                vec![],
                compile_trace,
                Some("full".to_string()),
                false,
                provenance,
            ))
        }
        Ok(rustmodlica::CompileOutput::ValidationParseOk) => {
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                vec![],
                vec![],
                compile_trace,
                Some("parse".to_string()),
                true,
                provenance,
            ))
        }
        Ok(rustmodlica::CompileOutput::ValidationFlattenOk { .. }) => {
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                vec![],
                vec![],
                compile_trace,
                Some("flatten".to_string()),
                true,
                provenance,
            ))
        }
        Ok(rustmodlica::CompileOutput::ValidationAnalyzed(s)) => {
            let state_vars = s.state_vars;
            let output_vars = s.output_vars;
            let compile_trace = build_compile_trace(
                perf.as_ref(),
                state_vars.len(),
                output_vars.len(),
            );
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                state_vars,
                output_vars,
                compile_trace,
                Some("analyze".to_string()),
                true,
                provenance,
            ))
        }
        Err(err) => {
            let message = err.to_string();
            let diagnostics = vec![diagnostics_from_error_message(&message)];
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                false,
                warnings,
                vec![message],
                diagnostics,
                vec![],
                vec![],
                compile_trace,
                None,
                false,
                provenance,
            ))
        }
    }
}

#[tauri::command]
pub async fn jit_validate(request: JitValidateRequest) -> Result<JitValidateResult, String> {
    tokio::task::spawn_blocking(move || jit_validate_sync(request))
        .await
        .map_err(|e| format!("blocking task join error: {e}"))?
}

#[tauri::command]
pub async fn jit_validate_v2(
    app: AppHandle,
    request: JitValidateRequest,
) -> Result<JitApiEnvelope<JitValidateResult>, String> {
    let task_name = "validate";
    emit_jit_progress(&app, task_name, "started", 0, "Validation started");
    let started = Instant::now();
    let mut task = tokio::task::spawn_blocking(move || jit_validate_sync(request));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let result = loop {
        tokio::select! {
            join_res = &mut task => {
                break join_res;
            }
            _ = heartbeat.tick() => {
                let elapsed = started.elapsed().as_secs().max(1);
                emit_jit_progress(
                    &app,
                    task_name,
                    "running",
                    elapsed,
                    format!("Validation running ({}s)...", elapsed),
                );
            }
        }
    }
    .map_err(|e| {
        emit_jit_error(&app, task_name, "failed", started.elapsed().as_secs(), format!("Validation join error: {e}"), Some("join"));
        format!("blocking task join error: {e}")
    })??;
    emit_jit_progress(
        &app,
        task_name,
        if result.success { "completed" } else { "failed" },
        started.elapsed().as_secs().max(1),
        if result.success { "Validation completed" } else { "Validation completed with errors" },
    );
    if result.success {
        Ok(envelope_ok("validate", result))
    } else {
        let errors = if !result.diagnostics.is_empty() {
            to_api_errors(&result.diagnostics)
        } else {
            result
                .errors
                .iter()
                .map(|message| {
                    let d = diagnostics_from_error_message(message);
                    JitApiError {
                        code: d.code,
                        message: d.message,
                        path: d.path,
                        line: d.line,
                        column: d.column,
                    }
                })
                .collect()
        };
        Ok(envelope_err_with_data("validate", result, errors))
    }
}

