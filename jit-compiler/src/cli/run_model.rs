use rustmodlica::{
    run_simulation, run_simulation_collect, CompileOutput, Compiler,
};
use rustmodlica::jit::deopt::DeoptSimPerfSummary;
use rustmodlica::runtime_perf_counters;
use rustmodlica::fmi;
use rustmodlica::i18n;
use std::io::Read;
use std::time::Instant;

use super::super::args::parse_rustmodlica_overdet_tol;
use super::super::perf_json::{compile_export_sidebar_json, maybe_write_perf_json};
use super::super::repl::run_repl_loop;
use super::super::validate_json::emit_validate_json;
use super::super::RunError;

pub(super) fn run_model(
    compiler: &mut Compiler,
    model_name: &str,
    overdet_tol: Option<f64>,
    flat_snapshot_only: bool,
    emit_flat_snapshot: Option<String>,
    training_run: bool,
    use_profile: Option<String>,
    lib_paths: &[String],
    json_mode: bool,
    validate_only: bool,
    condenser_stats: bool,
    perf_json_path: Option<String>,
    fmi_model_id: Option<String>,
    fmi_guid: Option<String>,
    emit_fmu_dir: Option<String>,
    emit_fmu_me_dir: Option<String>,
    run_repl: bool,
    output_format: Option<String>,
) -> Result<(), RunError> {
    let effective_model = if model_name == "-" {
        let mut code = String::new();
        std::io::stdin()
            .read_to_string(&mut code)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;
        compiler
            .loader
            .load_model_from_source("<stdin>", &code)
            .map_err(|e| format!("{}", e))?;
        "<stdin>".to_string()
    } else {
        model_name.to_string()
    };

    if overdet_tol.is_none() && model_name != "-" {
        if let Ok(model) = compiler.loader.load_model_silent(&effective_model, true) {
            if let Some(ann) = model.annotation.as_ref() {
                if let Some(v) = parse_rustmodlica_overdet_tol(ann) {
                    std::env::set_var("RUSTMODLICA_OVERDET_RESIDUAL_TOL", v.to_string());
                }
            }
        }
    }

    if flat_snapshot_only && emit_flat_snapshot.is_none() {
        return Err("--flat-snapshot-only requires --emit-flat-snapshot=<path>".into());
    }

    if training_run {
        let profile_out = use_profile.as_ref().map(|p| std::path::PathBuf::from(p));
        let config = rustmodlica::condenser::training_run::TrainingRunConfig {
            model_name: effective_model.clone(),
            lib_paths: lib_paths.iter().map(|s| std::path::PathBuf::from(s)).collect(),
            output_profile_path: profile_out,
            quiet: false,
        };
        let result = rustmodlica::condenser::training_run::execute_training_run(&config)
            .map_err(|e| -> RunError { format!("training run: {}", e).into() })?;
        eprintln!(
            "[training-run] profile: steps={} hot_eqs={} wall={:.1}ms",
            result.profile.total_steps,
            result.profile.hot_equations.len(),
            result.wall_us as f64 / 1000.0,
        );
        if let Some(ref p) = result.profile_path {
            eprintln!("[training-run] saved to {}", p.display());
        }
        return Ok(());
    }

    if let Some(ref profile_path) = use_profile {
        if !training_run {
            match rustmodlica::condenser::training_run::load_profile_from_file(
                std::path::Path::new(profile_path),
            ) {
                Ok(profile) => {
                    rustmodlica::jit::speculation::init_global_registry(&profile);
                    if !json_mode {
                        eprintln!(
                            "[profile] loaded profile from {} (steps={} hot_eqs={})",
                            profile_path,
                            profile.total_steps,
                            profile.hot_equations.len(),
                        );
                    }
                }
                Err(e) => {
                    eprintln!("[profile] failed to load {}: {}", profile_path, e);
                }
            }
        }
    }

    if !json_mode {
        println!(
            "{}",
            i18n::msg("compiling", &[&effective_model as &dyn std::fmt::Display])
        );
    }
    let perf_enabled = std::env::var("RUSTMODLICA_PERF_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false);
    // Always time simulation when writing --perf-json (not only PERF_TRACE).
    let sim_perf_enabled = perf_enabled || perf_json_path.is_some();
    let compile_t0 = if perf_enabled { Some(Instant::now()) } else { None };
    let out = match compiler.compile(&effective_model) {
        Ok(o) => o,
        Err(e) => {
            let warnings = compiler.take_warnings();
            if validate_only {
                emit_validate_json(
                    false,
                    &warnings,
                    &[e.to_string()],
                    &[],
                    &[],
                    None,
                    false,
                    None,
                    None,
                );
                return Ok(());
            }
            return Err(e.into());
        }
    };
    if let Some(t0) = compile_t0 {
        eprintln!("[perf] compile_ms={}", t0.elapsed().as_millis());
    }
    let warnings = compiler.take_warnings();
    let compile_perf = serde_json::to_value(compiler.take_compile_perf_report())
        .map_err(|e| RunError::Message(format!("serialize compile perf failed: {}", e)))?;
    if perf_enabled {
        if let Some(s) = compile_perf
            .get("backend_dae_cache_status")
            .and_then(|v| v.as_str())
        {
            eprintln!("[perf] backend_dae_cache_status={}", s);
        }
        if let Some(b) = compile_perf
            .get("param_only_update")
            .and_then(|v| v.as_bool())
        {
            eprintln!("[perf] compile_param_only_update={}", b);
        }
    }
    if condenser_stats {
        if let Some(perf) = compiler.last_compile_perf.as_ref() {
            eprintln!("[condenser-stats] tier={} profile_guided={}", perf.compile_tier, perf.profile_guided);
            eprintln!(
                "[condenser-stats] condensers: elapsed={}us artifacts={} cache_hits={} errors={}",
                perf.condenser_total_elapsed_us,
                perf.condenser_artifacts_written,
                perf.condenser_cache_hits,
                perf.condenser_errors,
            );
            eprintln!(
                "[condenser-stats] speculation: guards={} invalidations={}",
                perf.speculation_guard_count,
                perf.speculation_invalidation_count,
            );
        }
    }

    let warn_level = compiler.options.warnings_level.as_str();
    if validate_only {
        match &out {
            CompileOutput::FunctionRun(_) => {
                maybe_write_perf_json(
                    &perf_json_path,
                    &effective_model,
                    warnings.len(),
                    Some(compile_perf.clone()),
                    None,
                )?;
                emit_validate_json(
                    true,
                    &warnings,
                    &[],
                    &[],
                    &[],
                    Some("full"),
                    false,
                    Some(compile_export_sidebar_json(&compile_perf, None)),
                    None,
                );
            }
            CompileOutput::Simulation(artifacts) => {
                maybe_write_perf_json(
                    &perf_json_path,
                    &effective_model,
                    warnings.len(),
                    Some(compile_perf.clone()),
                    None,
                )?;
                emit_validate_json(
                    true,
                    &warnings,
                    &[],
                    &artifacts.state_vars,
                    &artifacts.output_vars,
                    Some("full"),
                    false,
                    Some(compile_export_sidebar_json(&compile_perf, Some(artifacts))),
                    None,
                );
            }
            CompileOutput::FlatSnapshotDone => {
                maybe_write_perf_json(
                    &perf_json_path,
                    &effective_model,
                    warnings.len(),
                    Some(compile_perf.clone()),
                    None,
                )?;
                emit_validate_json(
                    true,
                    &warnings,
                    &[],
                    &[],
                    &[],
                    Some("full"),
                    false,
                    Some(compile_export_sidebar_json(&compile_perf, None)),
                    None,
                );
            }
            CompileOutput::ValidationParseOk => {
                maybe_write_perf_json(
                    &perf_json_path,
                    &effective_model,
                    warnings.len(),
                    Some(compile_perf.clone()),
                    None,
                )?;
                emit_validate_json(
                    true,
                    &warnings,
                    &[],
                    &[],
                    &[],
                    Some("parse"),
                    true,
                    Some(compile_export_sidebar_json(&compile_perf, None)),
                    None,
                );
            }
            CompileOutput::ValidationFlattenOk { .. } => {
                maybe_write_perf_json(
                    &perf_json_path,
                    &effective_model,
                    warnings.len(),
                    Some(compile_perf.clone()),
                    None,
                )?;
                emit_validate_json(
                    true,
                    &warnings,
                    &[],
                    &[],
                    &[],
                    Some("flatten"),
                    true,
                    Some(compile_export_sidebar_json(&compile_perf, None)),
                    None,
                );
            }
            CompileOutput::ValidationAnalyzed(s) => {
                maybe_write_perf_json(
                    &perf_json_path,
                    &effective_model,
                    warnings.len(),
                    Some(compile_perf.clone()),
                    None,
                )?;
                emit_validate_json(
                    true,
                    &warnings,
                    &[],
                    &s.state_vars,
                    &s.output_vars,
                    Some("analyze"),
                    true,
                    Some(compile_export_sidebar_json(&compile_perf, None)),
                    None,
                );
            }
        }
        return Ok(());
    }
    if warn_level != "none" {
        for w in &warnings {
            if warn_level == "error" {
                return Err(w.to_string().into());
            }
            eprintln!("{}", w);
        }
        if !warnings.is_empty() && warn_level == "all" {
            eprintln!("{}", i18n::msg("warnings_generated", &[&warnings.len()]));
        }
    }
    match out {
        CompileOutput::ValidationParseOk | CompileOutput::ValidationFlattenOk { .. } => {
            maybe_write_perf_json(
                &perf_json_path,
                &effective_model,
                warnings.len(),
                Some(compile_perf.clone()),
                None,
            )?;
            if !json_mode {
                println!("Compile stopped early (--validate); no simulation artifacts.");
            }
            return Ok(());
        }
        CompileOutput::ValidationAnalyzed(_) => {
            maybe_write_perf_json(
                &perf_json_path,
                &effective_model,
                warnings.len(),
                Some(compile_perf.clone()),
                None,
            )?;
            if !json_mode {
                println!("Compile stopped after analysis (--validate-tier=analyze); no simulation artifacts.");
            }
            return Ok(());
        }
        CompileOutput::FlatSnapshotDone => {
            maybe_write_perf_json(
                &perf_json_path,
                &effective_model,
                warnings.len(),
                Some(compile_perf.clone()),
                None,
            )?;
            if !json_mode {
                println!("Tier S flat snapshot written.");
            }
            return Ok(());
        }
        CompileOutput::FunctionRun(value) => {
            maybe_write_perf_json(
                &perf_json_path,
                &effective_model,
                warnings.len(),
                Some(compile_perf.clone()),
                None,
            )?;
            if !json_mode {
                println!("{}", i18n::msg("result", &[&value]));
            }
            return Ok(());
        }
        CompileOutput::Simulation(artifacts) => {
            if let Some(generic_fn) = artifacts.dual_compile_generic {
                rustmodlica::jit::deopt::set_precompiled_generic(generic_fn);
            }
            println!("{}", i18n::msg0("compilation_successful"));
            println!("{}", i18n::msg("states", &[&artifacts.states.len()]));
            println!(
                "{}",
                i18n::msg("discrete_vars", &[&artifacts.discrete_vars.len()])
            );
            println!("{}", i18n::msg("parameters", &[&artifacts.params.len()]));
            println!("{}", i18n::msg("outputs", &[&artifacts.output_vars.len()]));
            println!("{}", i18n::msg("when_statements", &[&artifacts.when_count]));
            println!(
                "{}",
                i18n::msg("zero_crossings", &[&artifacts.crossings_count])
            );
            let fmi_opts = fmi::FmiExportOptions {
                model_identifier_override: fmi_model_id.clone(),
                guid_override: fmi_guid.clone(),
            };
            if let Some(ref dir) = emit_fmu_dir {
                let path = std::path::Path::new(dir);
                match fmi::emit_fmu_artifacts_with_options(
                    path,
                    &effective_model,
                    &artifacts.state_vars,
                    &artifacts.param_vars,
                    &artifacts.output_vars,
                    0.0,
                    artifacts.t_end,
                    artifacts.dt,
                    &fmi_opts,
                ) {
                    Ok(files) => {
                        let paths: Vec<String> =
                            files.iter().map(|p| p.display().to_string()).collect();
                        println!("FMI CS: emitted {}", paths.join(", "));
                        // Package into .fmu
                        let fmu_path = path.join(format!("{}.fmu", fmi::resolve_model_identifier(&effective_model, fmi_model_id.as_deref())));
                        match fmi::package_fmu(path, &fmu_path) {
                            Ok(()) => println!("FMI CS: packaged {}", fmu_path.display()),
                            Err(e) => eprintln!("FMI CS: package warning: {}", e),
                        }
                    }
                    Err(e) => return Err(format!("FMI CS emit failed: {}", e).into()),
                }
            }
            if let Some(ref dir) = emit_fmu_me_dir {
                let path = std::path::Path::new(dir);
                match fmi::emit_fmu_me_artifacts_with_options(
                    path,
                    &effective_model,
                    &artifacts.state_vars,
                    &artifacts.param_vars,
                    &artifacts.output_vars,
                    0.0,
                    artifacts.t_end,
                    artifacts.dt,
                    &fmi_opts,
                ) {
                    Ok(files) => {
                        let paths: Vec<String> =
                            files.iter().map(|p| p.display().to_string()).collect();
                        println!("FMI ME: emitted {}", paths.join(", "));
                        // Package into .fmu
                        let fmu_path = path.join(format!("{}.fmu", fmi::resolve_model_identifier(&effective_model, fmi_model_id.as_deref())));
                        match fmi::package_fmu(path, &fmu_path) {
                            Ok(()) => println!("FMI ME: packaged {}", fmu_path.display()),
                            Err(e) => eprintln!("FMI ME: package warning: {}", e),
                        }
                    }
                    Err(e) => return Err(format!("FMI ME emit failed: {}", e).into()),
                }
            }
            if run_repl {
                run_repl_loop(artifacts, &effective_model, lib_paths.iter().map(|s| std::path::PathBuf::from(s)).collect())?;
                return Ok(());
            }
            if emit_fmu_dir.is_some() || emit_fmu_me_dir.is_some() {
                return Ok(());
            }
            if output_format.as_deref() == Some("json") {
                let sim_t0 = if sim_perf_enabled { Some(Instant::now()) } else { None };
                let sim_compile_sidebar =
                    compile_export_sidebar_json(&compile_perf, Some(&artifacts));
                let mut deopt_perf_summary = DeoptSimPerfSummary::default();
                let result = run_simulation_collect(
                    artifacts.calc_derivs,
                    artifacts.when_count,
                    artifacts.crossings_count,
                    artifacts.states,
                    artifacts.discrete_vals,
                    artifacts.params,
                    &artifacts.state_vars,
                    &artifacts.discrete_vars,
                    &artifacts.output_vars,
                    &artifacts.output_start_vals,
                    &artifacts.state_var_index,
                    artifacts.t_end,
                    artifacts.dt,
                    artifacts.numeric_ode_jacobian,
                    artifacts.symbolic_ode_jacobian.as_ref(),
                    &artifacts.newton_tearing_var_names,
                    artifacts.atol,
                    artifacts.rtol,
                    artifacts.differential_index,
                    artifacts.ida_component_id.as_slice(),
                    &artifacts.solver,
                    artifacts.output_interval,
                    &artifacts.clock_partition_schedule,
                    Some(&mut deopt_perf_summary),
                    &effective_model,
                    lib_paths.iter().map(|s| std::path::PathBuf::from(s)).collect(),
                    None,
                )?;
                let (sim_ms, sim_us) = sim_t0
                    .as_ref()
                    .map(|t0| {
                        let d = t0.elapsed();
                        (d.as_millis() as u64, d.as_micros() as u64)
                    })
                    .unwrap_or((0, 0));
                if sim_us > 0 {
                    eprintln!("[perf] sim_ms={} sim_us={}", sim_ms, sim_us);
                }
                let (event_iter_total, clock_dispatch_total) = runtime_perf_counters();
                let deopt_json = serde_json::to_value(&deopt_perf_summary)
                    .unwrap_or(serde_json::Value::Null);
                let tiered_json =
                    serde_json::to_value(rustmodlica::jit::tiered::tiered_events_snapshot())
                        .unwrap_or(serde_json::Value::Null);
                maybe_write_perf_json(
                    &perf_json_path,
                    &effective_model,
                    warnings.len(),
                    Some(compile_perf.clone()),
                    Some(serde_json::json!({
                        "sim_ms": sim_ms,
                        "sim_us": sim_us,
                        "event_iter_total": event_iter_total,
                        "clock_dispatch_total": clock_dispatch_total,
                        "deopt": deopt_json,
                        "tiered": tiered_json
                    })),
                )?;
                let mut sim_json = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
                if let serde_json::Value::Object(ref mut sim_obj) = sim_json {
                    if let serde_json::Value::Object(side) = sim_compile_sidebar {
                        for (k, v) in side {
                            sim_obj.insert(k, v);
                        }
                    }
                }
                println!("{}", serde_json::to_string(&sim_json).unwrap_or_default());
                return Ok(());
            }
            if !json_mode {
                println!("{}", i18n::msg0("starting_simulation"));
            }
            let sim_t0 = if sim_perf_enabled { Some(Instant::now()) } else { None };
            let mut deopt_perf_summary = DeoptSimPerfSummary::default();
            run_simulation(
                artifacts.calc_derivs,
                artifacts.when_count,
                artifacts.crossings_count,
                artifacts.states,
                artifacts.discrete_vals,
                artifacts.params,
                &artifacts.state_vars,
                &artifacts.discrete_vars,
                &artifacts.output_vars,
                &artifacts.output_start_vals,
                &artifacts.state_var_index,
                artifacts.t_end,
                artifacts.dt,
                artifacts.numeric_ode_jacobian,
                artifacts.symbolic_ode_jacobian.as_ref(),
                &artifacts.newton_tearing_var_names,
                artifacts.atol,
                artifacts.rtol,
                artifacts.differential_index,
                artifacts.ida_component_id.as_slice(),
                &artifacts.solver,
                artifacts.output_interval,
                artifacts.result_file.as_deref(),
                &artifacts.clock_partition_schedule,
                None,
                Some(&mut deopt_perf_summary),
                &effective_model,
                lib_paths.iter().map(|s| std::path::PathBuf::from(s)).collect(),
                None,
            )?;
            let (sim_ms, sim_us) = sim_t0
                .as_ref()
                .map(|t0| {
                    let d = t0.elapsed();
                    (d.as_millis() as u64, d.as_micros() as u64)
                })
                .unwrap_or((0, 0));
            if sim_us > 0 {
                eprintln!("[perf] sim_ms={} sim_us={}", sim_ms, sim_us);
            }
            let (event_iter_total, clock_dispatch_total) = runtime_perf_counters();
            let deopt_json = serde_json::to_value(&deopt_perf_summary)
                .unwrap_or(serde_json::Value::Null);
            let tiered_json =
                serde_json::to_value(rustmodlica::jit::tiered::tiered_events_snapshot())
                    .unwrap_or(serde_json::Value::Null);
            maybe_write_perf_json(
                &perf_json_path,
                &effective_model,
                warnings.len(),
                Some(compile_perf),
                Some(serde_json::json!({
                    "sim_ms": sim_ms,
                    "sim_us": sim_us,
                    "event_iter_total": event_iter_total,
                    "clock_dispatch_total": clock_dispatch_total,
                    "deopt": deopt_json,
                    "tiered": tiered_json
                })),
            )?;
            if !json_mode {
                println!("{}", i18n::msg0("simulation_completed"));
            }
        }
    }
    Ok(())
}
