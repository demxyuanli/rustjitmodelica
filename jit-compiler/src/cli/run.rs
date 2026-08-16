use rustmodlica::{CompileStopPhase, Compiler};
use rustmodlica::script;

use super::cache_invalidate::run_cache_invalidate;
use super::cache_ops::run_cache_command;
use super::cache_stats::{run_cache_gc, run_cache_stats};
use super::event_scan::run_event_scan;
use super::precompile::{run_precompile, run_msl_precompile_if_needed};
use super::validate_json::parse_validate_tier;
use super::validate_stdio::run_validate_stdio;

use super::RunError;

#[path = "run_model.rs"]
mod run_model;
use run_model::run_model;

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
    let mut index_reduction_method = "dummyDerivative".to_string();
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
    let mut emit_coder_package = false;
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
    let mut validate_stdio = false;
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
        } else if let Some(v) = a.strip_prefix("--emit-coder=") {
            emit_c_dir = Some(v.to_string());
            emit_coder_package = true;
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
        } else if a == "--validate-stdio" {
            validate_only = true;
            validate_stdio = true;
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
        None if validate_stdio => String::new(),
        None => {
            let msg = format!(
                "Usage: {} [options] <model_name>\n  event-scan [scan-options] [model_name]\n\n  --lang=en|zh  message language\n  --validate  compile only, output JSON to stdout\n  --validate-tier=full|parse|flatten|analyze  with --validate: stop after tier (default full)\n  --validation-mode=full|quick|superfast  with --validate: speed/accuracy trade-off (default full)\n  env RUSTMODLICA_SALSA=0|1  query-based flatten: default on with --validate when unset; use 0 for legacy flatten path; use 1 to force on for simulation\n  --output-format=json  simulation: output time series as JSON to stdout\n  --perf-json=<path>  write structured compile perf report JSON\n  --solver=rk4|rk45|implicit|radau|qss|cvode|ida  (cvode/ida need --features sundials; default: rk45)\n  --warnings=all|none|error  (default: all)\n  --backend-dae-info  print backend DAE statistics\n  --index-reduction-method=<none|dummyDerivative|pantelides|pantelidesDummy|debugPrint>\n  --t-end=<float>  --dt=<float>  --atol=<float>  --rtol=<float>\n  --output-interval=<float>  (default 0.05)\n  --result-file=<path>  write CSV time series to file\n  --emit-flat-snapshot=<path>  Tier S flat JSON after flatten (before inline)\n  --flat-snapshot-only  stop after snapshot (requires --emit-flat-snapshot)\n  --coarse-constrainedby  legacy constrainedby check instead of extends-closure\n  --array-size-policy=legacy|strict  flatten: unevaluated array dims (default legacy: warn+scalar fallback; strict: error unless --array-sizes-json)\n  --array-sizes-json=<path>  JSON object with \"array_sizes\" map: flat base names to positive integer sizes\n  --emit-c=<dir>  emit C source (model.c, model.h) to directory\n  --repl  after compile, enter REPL (inspect vars, simulate, quit)\n  --script=<path>  run .mos script (AST parser + strict executor by default); use - for stdin\n  Env: RUSTMODLICA_SCRIPT_ENGINE=mos|legacy; RUSTMODLICA_NEWTON_SPARSE_POLICY=auto|dense|sparse; RUSTMODLICA_STRICT_NEWTON=1\n  --emit-fmu=<dir>  emit C + modelDescription.xml + fmi2_cs.c for FMI 2.0 CS\n  --emit-fmu-me=<dir>  emit C + modelDescription.xml + fmi2_me.c for FMI 2.0 ME\n  --fmi-model-id=<id>  override FMI modelIdentifier (sanitized; wins over env)\n  --fmi-guid=<uuid|token>  fixed guid attribute (UUID or ASCII alnum/-/_)\n  Env (FMI): RUSTMODLICA_FMI_MODEL_ID, RUSTMODLICA_FMI_MODEL_ID_PREFIX, RUSTMODLICA_FMI_GUID, RUSTMODLICA_FMI_GENERATION_TOOL\n  --external-lib=<path>  load shared library for external function symbols (EXT-1; repeatable)\n  --lib-path=<dir>  add a Modelica library root to the loader search path (repeatable)\n  --modelica-stdlib=<dir>  alias of --lib-path\n  --function-args=<f1,f2,...>  function input values\n\n  event-scan options:\n  --model=<name>  single target model (default: BouncingBall)\n  --models=<m1,m2,...>  multi-model batch scan\n  --lib-path=<dir>  repeatable model library roots\n  --t-end=<float> --dt=<float> --output-interval=<float>\n  --count-values=<v1,v2,...>  values for RUSTMODLICA_EVENT_COUNT_DEADBAND\n  --tail-velocity-values=<v1,v2,...>  values for RUSTMODLICA_TAIL_VELOCITY_DEADBAND\n  --aggregate-mode=sum|avg|max  aggregate sort strategy (default: sum)\n  --aggregate-report=full|compact  output detail level (default: full)\n  --output-file=<path>  write JSON result to file (stdout prints summary)\n  --quiet  alias of --quiet=all\n  --quiet=none|events|all  control scan logging granularity\n  --top-n=<N>  output top N combinations per model and aggregate\n\n  Use model_name '-' to read Modelica source from stdin.",
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
    compiler.options.emit_coder_package = emit_coder_package;
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

    if validate_stdio {
        if model_name != "-" && !model_name.is_empty() {
            return Err(
                "--validate-stdio does not accept a model argument on the command line".into(),
            );
        }
        return run_validate_stdio(&mut compiler);
    }
    run_model(
        &mut compiler,
        &model_name,
        overdet_tol,
        flat_snapshot_only,
        emit_flat_snapshot,
        training_run,
        use_profile,
        &lib_paths,
        json_mode,
        validate_only,
        condenser_stats,
        perf_json_path,
        fmi_model_id,
        fmi_guid,
        emit_fmu_dir,
        emit_fmu_me_dir,
        run_repl,
        output_format,
    )
}
