use rustmodlica::{
    run_simulation, run_simulation_collect, CompileOutput, CompileStopPhase, Compiler,
};
use rustmodlica::jit::deopt::DeoptSimPerfSummary;
use rustmodlica::runtime_perf_counters;
use rustmodlica::fmi;
use rustmodlica::i18n;
use rustmodlica::script;
use std::io::Read;
use std::time::Instant;

use super::args::parse_rustmodlica_overdet_tol;
use super::cache_invalidate::run_cache_invalidate;
use super::cache_ops::run_cache_command;
use super::cache_stats::{run_cache_gc, run_cache_stats};
use super::event_scan::run_event_scan;
use super::perf_json::{compile_export_sidebar_json, maybe_write_perf_json};
use super::precompile::{run_precompile, run_msl_precompile_if_needed};
use super::repl::run_repl_loop;
use super::validate_json::{emit_validate_json, parse_validate_tier};

use super::RunError;

pub fn run(args: Vec<String>) -> Result<(), RunError> {
    if args.len() >= 2 && args[1] == "cache" {
        return run_cache_command(&args);
    }
    if args.len() >= 2 && args[1] == "event-scan" {
        return run_event_scan(&args);
    }
    if args.len() >= 2 && args[1] == "--cache-stats" {
        return run_cache_stats(&args);
    }
    if args.len() >= 2 && args[1] == "--cache-gc" {
        return run_cache_gc();
    }
    if args.len() >= 2 && args[1] == "--cache-invalidate" {
        return run_cache_invalidate(&args);
    }
    if args.len() >= 2 && args[1] == "--msl-precompile" {
        let mut lib_paths = Vec::new();
        let mut i = 2usize;
        while i < args.len() {
            let a = &args[i];
            if let Some(v) = a.strip_prefix("--lib-path=") {
                lib_paths.push(v.to_string());
                i += 1;
            } else if a == "--lib-path" && i + 1 < args.len() {
                lib_paths.push(args[i + 1].to_string());
                i += 2;
            } else {
                i += 1;
            }
        }
        return run_msl_precompile_if_needed(&lib_paths);
    }
    if args.len() >= 3 && args[1] == "--precompile" {
        let mut precompile_lib_paths = Vec::new();
        let mut i = 3usize;
        while i < args.len() {
            let a = &args[i];
            if let Some(v) = a.strip_prefix("--lib-path=") {
                precompile_lib_paths.push(v.to_string());
                i += 1;
            } else if a == "--lib-path" && i + 1 < args.len() {
                precompile_lib_paths.push(args[i + 1].to_string());
                i += 2;
            } else {
                i += 1;
            }
        }
        return run_precompile(&args[2], &precompile_lib_paths);
    }
    let mut backend_dae_info = false;
    let mut index_reduction_method = "none".to_string();
    let mut tearing_method = "first".to_string();
    let mut generate_dynamic_jacobian = "none".to_string();
    let mut t_end = 10.0_f64;
    let mut dt = 0.01_f64;
    let mut atol = 1e-6_f64;
    let mut rtol = 1e-3_f64;
    let mut function_args: Option<Vec<f64>> = None;
    let mut solver = "rk45".to_string();
    let mut output_interval = 0.05_f64;
    let mut result_file: Option<String> = None;
    let mut warnings_level = "all".to_string();
    let mut emit_c_dir: Option<String> = None;
    let mut emit_fmu_dir: Option<String> = None;
    let mut emit_fmu_me_dir: Option<String> = None;
    let mut fmi_model_id: Option<String> = None;
    let mut fmi_guid: Option<String> = None;
    let mut external_libs: Vec<String> = Vec::new();
    let mut lib_paths: Vec<String> = Vec::new();
    let mut repl = false;
    let mut script_path: Option<String> = None;
    let mut model_name = None;
    let mut validate_only = false;
    let mut validate_tier: Option<CompileStopPhase> = None;
    let mut output_format: Option<String> = None;
    let mut perf_json_path: Option<String> = None;
    let mut emit_flat_snapshot: Option<String> = None;
    let mut flat_snapshot_only = false;
    let mut coarse_constrainedby_only = false;
    let mut validation_mode: Option<String> = None;
    let mut array_size_policy = "legacy".to_string();
    let mut array_sizes_json: Option<String> = None;
    let mut training_run = false;
    let mut use_profile: Option<String> = None;
    let mut condenser_stats = false;
    let mut dual_compile = false;
    // P8-1: CLI parameters for overdet check configuration
    let mut overdet_check: Option<bool> = None;
    let mut overdet_tol: Option<f64> = None;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if let Some(lang) = a.strip_prefix("--lang=") {
            let _ = std::env::set_var("RUSTMODLICA_LANG", lang);
            i += 1;
        } else if a == "--backend-dae-info" {
            backend_dae_info = true;
            i += 1;
        } else if a == "-d" && i + 1 < args.len() {
            if args[i + 1] == "backenddaeinfo" {
                backend_dae_info = true;
            }
            i += 2;
        } else if a.starts_with("-d=")
            && a.strip_prefix("-d=")
                .map(|s| s == "backenddaeinfo")
                .unwrap_or(false)
        {
            backend_dae_info = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--index-reduction-method=") {
            index_reduction_method = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--tearing-method=") {
            tearing_method = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--generate-dynamic-jacobian=") {
            generate_dynamic_jacobian = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--t-end=") {
            t_end = v.parse().unwrap_or(10.0);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--dt=") {
            dt = v.parse().unwrap_or(0.01);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--atol=") {
            atol = v.parse().unwrap_or(1e-6);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--rtol=") {
            rtol = v.parse().unwrap_or(1e-3);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--function-args=") {
            let parsed: Result<Vec<f64>, _> =
                v.split(',').map(|s| s.trim().parse::<f64>()).collect();
            function_args = parsed.ok();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--solver=") {
            solver = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--output-interval=") {
            output_interval = v.parse().unwrap_or(0.05);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--result-file=") {
            result_file = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-flat-snapshot=") {
            emit_flat_snapshot = Some(v.to_string());
            i += 1;
        } else if a == "--flat-snapshot-only" {
            flat_snapshot_only = true;
            i += 1;
        } else if a == "--coarse-constrainedby" {
            coarse_constrainedby_only = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--array-size-policy=") {
            array_size_policy = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--array-sizes-json=") {
            array_sizes_json = Some(v.to_string());
            i += 1;
        // P8-1: CLI parameters for overdet check configuration
        } else if let Some(v) = a.strip_prefix("--overdet-check=") {
            overdet_check = Some(v == "true" || v == "1" || v == "on");
            i += 1;
        } else if let Some(v) = a.strip_prefix("--overdet-tol=") {
            overdet_tol = v.parse::<f64>().ok().filter(|x| x.is_finite() && *x > 0.0);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--warnings=") {
            warnings_level = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-c=") {
            emit_c_dir = Some(v.to_string());
            i += 1;
        } else if a == "--repl" {
            repl = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--script=") {
            script_path = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-fmu=") {
            emit_fmu_dir = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-fmu-me=") {
            emit_fmu_me_dir = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--fmi-model-id=") {
            fmi_model_id = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--fmi-guid=") {
            fmi_guid = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--external-lib=") {
            external_libs.push(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--lib-path=") {
            if !v.trim().is_empty() {
                lib_paths.push(v.to_string());
            }
            i += 1;
        } else if let Some(v) = a.strip_prefix("--modelica-stdlib=") {
            if !v.trim().is_empty() {
                lib_paths.push(v.to_string());
            }
            i += 1;
        } else if a == "--validate" {
            validate_only = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--validate-tier=") {
            validate_tier = Some(parse_validate_tier(v)?);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--validation-mode=") {
            validation_mode = Some(v.to_string());
            i += 1;
        } else if a == "-" {
            model_name = Some("-".to_string());
            i += 1;
            break;
        } else if let Some(v) = a.strip_prefix("--output-format=") {
            output_format = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--perf-json=") {
            perf_json_path = Some(v.to_string());
            i += 1;
        } else if a == "--training-run" {
            training_run = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--use-profile=") {
            use_profile = Some(v.to_string());
            i += 1;
        } else if a == "--condenser-stats" {
            condenser_stats = true;
            i += 1;
        } else if a == "--dual-compile" {
            dual_compile = true;
            i += 1;
        } else if !a.starts_with('-') {
            model_name = Some(a.clone());
            i += 1;
            break;
        } else {
            return Err(format!("unknown argument: {}", a).into());
        }
    }
    if i < args.len() {
        return Err("Unknown or extra arguments after model name.".into());
    }
    
    // P8-1: Set environment variables for overdet check configuration
    if let Some(v) = overdet_check {
        std::env::set_var("RUSTMODLICA_OVERDET_CHECK", if v { "1" } else { "0" });
    }
    if let Some(v) = overdet_tol {
        std::env::set_var("RUSTMODLICA_OVERDET_RESIDUAL_TOL", v.to_string());
    }
    
    if let Some(ref path) = script_path {
        let mut compiler = Compiler::new();
        compiler.options.backend_dae_info = backend_dae_info;
        compiler.options.index_reduction_method = index_reduction_method;
        compiler.options.tearing_method = tearing_method;
        compiler.options.generate_dynamic_jacobian = generate_dynamic_jacobian;
        compiler.options.t_end = t_end;
        compiler.options.dt = dt;
        compiler.options.atol = atol;
        compiler.options.rtol = rtol;
        compiler.options.function_args = function_args;
        compiler.options.solver = solver;
        compiler.options.output_interval = output_interval;
        compiler.options.result_file = result_file;
        compiler.options.warnings_level = warnings_level;
        compiler.options.array_size_policy = array_size_policy.clone();
        compiler.options.array_sizes_json = array_sizes_json.clone();
        if let Some(vm) = validation_mode.clone() {
            compiler.options.validation_mode = vm;
        }
        compiler.options.emit_c_dir = emit_c_dir.clone();
        compiler.options.external_libs = external_libs;
        compiler.options.compile_stop = CompileStopPhase::Full;
        for p in &lib_paths {
            compiler.loader.add_path(p.into());
        }
        compiler.loader.add_path(".".into());
        compiler.loader.add_path("StandardLib".into());
        compiler.loader.add_path("TestLib".into());
        compiler.loader.add_path("ModelicaTest".into());
        let mut runner = script::ScriptRunner::new(compiler);
        if path == "-" {
            runner.run_script_named(std::io::stdin(), "<stdin>")?;
        } else {
            let f = std::fs::File::open(path)
                .map_err(|e| format!("Cannot open script file {}: {}", path, e))?;
            runner.run_script_named(f, path)?;
        }
        return Ok(());
    }
    let model_name = match model_name {
        Some(n) => n,
        None => {
            let msg = format!(
                "Usage: {} [options] <model_name>\n  event-scan [scan-options] [model_name]\n\n  --lang=en|zh  message language\n  --validate  compile only, output JSON to stdout\n  --validate-tier=full|parse|flatten|analyze  with --validate: stop after tier (default full)\n  --validation-mode=full|quick|superfast  with --validate: speed/accuracy trade-off (default full)\n  env RUSTMODLICA_SALSA=0|1  query-based flatten: default on with --validate when unset; use 0 for legacy flatten path; use 1 to force on for simulation\n  --output-format=json  simulation: output time series as JSON to stdout\n  --perf-json=<path>  write structured compile perf report JSON\n  --solver=rk4|rk45|implicit|cvode|ida  (cvode/ida need --features sundials; default: rk45)\n  --warnings=all|none|error  (default: all)\n  --backend-dae-info  print backend DAE statistics\n  --index-reduction-method=<none|dummyDerivative|pantelides|pantelidesDummy|debugPrint>\n  --t-end=<float>  --dt=<float>  --atol=<float>  --rtol=<float>\n  --output-interval=<float>  (default 0.05)\n  --result-file=<path>  write CSV time series to file\n  --emit-flat-snapshot=<path>  Tier S flat JSON after flatten (before inline)\n  --flat-snapshot-only  stop after snapshot (requires --emit-flat-snapshot)\n  --coarse-constrainedby  legacy constrainedby check instead of extends-closure\n  --array-size-policy=legacy|strict  flatten: unevaluated array dims (default legacy: warn+scalar fallback; strict: error unless --array-sizes-json)\n  --array-sizes-json=<path>  JSON object with \"array_sizes\" map: flat base names to positive integer sizes\n  --emit-c=<dir>  emit C source (model.c, model.h) to directory\n  --repl  after compile, enter REPL (inspect vars, simulate, quit)\n  --script=<path>  run .mos script (AST parser + strict executor by default); use - for stdin\n  Env: RUSTMODLICA_SCRIPT_ENGINE=mos|legacy; RUSTMODLICA_NEWTON_SPARSE_POLICY=auto|dense|sparse; RUSTMODLICA_STRICT_NEWTON=1\n  --emit-fmu=<dir>  emit C + modelDescription.xml + fmi2_cs.c for FMI 2.0 CS\n  --emit-fmu-me=<dir>  emit C + modelDescription.xml + fmi2_me.c for FMI 2.0 ME\n  --fmi-model-id=<id>  override FMI modelIdentifier (sanitized; wins over env)\n  --fmi-guid=<uuid|token>  fixed guid attribute (UUID or ASCII alnum/-/_)\n  Env (FMI): RUSTMODLICA_FMI_MODEL_ID, RUSTMODLICA_FMI_MODEL_ID_PREFIX, RUSTMODLICA_FMI_GUID, RUSTMODLICA_FMI_GENERATION_TOOL\n  --external-lib=<path>  load shared library for external function symbols (EXT-1; repeatable)\n  --lib-path=<dir>  add a Modelica library root to the loader search path (repeatable)\n  --modelica-stdlib=<dir>  alias of --lib-path\n  --function-args=<f1,f2,...>  function input values\n\n  event-scan options:\n  --model=<name>  single target model (default: BouncingBall)\n  --models=<m1,m2,...>  multi-model batch scan\n  --lib-path=<dir>  repeatable model library roots\n  --t-end=<float> --dt=<float> --output-interval=<float>\n  --count-values=<v1,v2,...>  values for RUSTMODLICA_EVENT_COUNT_DEADBAND\n  --tail-velocity-values=<v1,v2,...>  values for RUSTMODLICA_TAIL_VELOCITY_DEADBAND\n  --aggregate-mode=sum|avg|max  aggregate sort strategy (default: sum)\n  --aggregate-report=full|compact  output detail level (default: full)\n  --output-file=<path>  write JSON result to file (stdout prints summary)\n  --quiet  alias of --quiet=all\n  --quiet=none|events|all  control scan logging granularity\n  --top-n=<N>  output top N combinations per model and aggregate\n\n  Use model_name '-' to read Modelica source from stdin.",
                args[0]
            );
            return Err(msg.into());
        }
    };
    let mut compiler = Compiler::new();
    compiler.options.backend_dae_info = backend_dae_info;
    compiler.options.index_reduction_method = index_reduction_method;
    compiler.options.tearing_method = tearing_method;
    compiler.options.generate_dynamic_jacobian = generate_dynamic_jacobian;
    compiler.options.t_end = t_end;
    compiler.options.dt = dt;
    compiler.options.atol = atol;
    compiler.options.rtol = rtol;
    compiler.options.function_args = function_args;
    compiler.options.solver = solver;
    compiler.options.output_interval = output_interval;
    compiler.options.result_file = result_file;
    compiler.options.warnings_level = warnings_level;
    compiler.options.emit_flat_snapshot = emit_flat_snapshot.clone();
    compiler.options.flat_snapshot_only = flat_snapshot_only;
    compiler.options.coarse_constrainedby_only = coarse_constrainedby_only;
    compiler.options.array_size_policy = array_size_policy;
    compiler.options.array_sizes_json = array_sizes_json;
    if let Some(vm) = validation_mode {
        compiler.options.validation_mode = vm;
    }
    if emit_fmu_dir.is_some() && emit_c_dir.is_none() {
        emit_c_dir = emit_fmu_dir.clone();
    }
    if emit_fmu_me_dir.is_some() && emit_c_dir.is_none() {
        emit_c_dir = emit_fmu_me_dir.clone();
    }
    compiler.options.emit_c_dir = emit_c_dir;
    compiler.options.external_libs = external_libs;
    compiler.options.compile_stop = if validate_only {
        validate_tier.unwrap_or(CompileStopPhase::Full)
    } else {
        CompileStopPhase::Full
    };
    compiler.options.dual_compile = dual_compile;
    let run_repl = repl;
    let json_mode = validate_only || output_format.as_deref() == Some("json");
    if validate_only {
        compiler.options.quiet = true;
        compiler.options.validate_only = true;
    }
    for p in &lib_paths {
        compiler.loader.add_path(p.into());
    }
    compiler.loader.add_path(".".into());
    compiler.loader.add_path("StandardLib".into());
    compiler.loader.add_path("TestLib".into());
    compiler.loader.add_path("ModelicaTest".into());

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
        model_name.clone()
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
                let sim_t0 = if perf_enabled { Some(Instant::now()) } else { None };
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
                )?;
                let sim_ms = sim_t0
                    .as_ref()
                    .map(|t0| t0.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                if sim_ms > 0 {
                    eprintln!("[perf] sim_ms={}", sim_ms);
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
            let sim_t0 = if perf_enabled { Some(Instant::now()) } else { None };
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
            )?;
            let sim_ms = sim_t0
                .as_ref()
                .map(|t0| t0.elapsed().as_millis() as u64)
                .unwrap_or(0);
            if sim_ms > 0 {
                eprintln!("[perf] sim_ms={}", sim_ms);
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
