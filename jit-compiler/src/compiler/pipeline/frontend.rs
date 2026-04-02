use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::ast::Model;
use crate::flatten::flat_snapshot;
use crate::flatten::flatten_cache;
use crate::flatten::{ArraySizePolicy, Flattener, ValidationMode, load_array_sizes_json_optional};
use crate::loader::ModelLoader;
use crate::query_db;
use crate::query_db::QueryDb;

use crate::compiler::CompileStopPhase;

use super::trace::log_stage_timing;
use super::types::{CompilerResult, FrontendStage};

fn salsa_query_path_enabled(validate_only: bool) -> bool {
    match std::env::var("RUSTMODLICA_SALSA") {
        Ok(v) => {
            let t = v.trim();
            if t.is_empty() {
                return validate_only;
            }
            if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                return false;
            }
            true
        }
        Err(_) => validate_only,
    }
}

pub(crate) fn flatten_and_inline(
    root_model: &mut Arc<Model>,
    model_name: &str,
    loader: &mut ModelLoader,
    compile_stop: CompileStopPhase,
    validate_only: bool,
    quiet: bool,
    stage_trace: bool,
    emit_flat_snapshot: Option<&Path>,
    coarse_constrainedby_only: bool,
    validation_mode: ValidationMode,
    array_size_policy: ArraySizePolicy,
    array_sizes_json_path: Option<&Path>,
    warnings_level: &str,
) -> CompilerResult<FrontendStage> {
    let compile_stop_s: &str = match compile_stop {
        CompileStopPhase::Full => "full",
        CompileStopPhase::Parse => "parse",
        CompileStopPhase::Flatten => "flatten",
        CompileStopPhase::Analyze => "analyze",
    };
    let started_at = Instant::now();
    if stage_trace {
        eprintln!("[stage] flatten");
    }
    let mut external_array_sizes = load_array_sizes_json_optional(array_sizes_json_path)
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            format!("array_sizes_json: {}", e).into()
        })?;
    if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
        let full_key = flatten_cache::flatten_full_cache_key(
            model_name,
            loader,
            validation_mode,
            compile_stop_s,
            coarse_constrainedby_only,
            array_sizes_json_path,
            array_size_policy,
            warnings_level,
        );
        if validate_only && matches!(compile_stop, CompileStopPhase::Analyze) {
            let mem_key = format!("analyze_input_v1:{}", full_key);
            if let Some(v) = flatten_cache::analyze_input_mem_get(mem_key.as_str()) {
                let mut flat_model = match Arc::try_unwrap(v) {
                    Ok(v) => v,
                    Err(a) => (*a).clone(),
                };
                if let Some((hits, misses, evictions)) =
                    flatten_cache::array_sizes_cache_counters_snapshot_reset()
                {
                    let ttl_ms = std::env::var("RUSTMODLICA_FLATTEN_CACHE_TTL_MS")
                        .ok()
                        .unwrap_or_default();
                    eprintln!(
                        "[perf] array_sizes_cache hits={} misses={} evictions={} ttl_ms={}",
                        hits, misses, evictions, ttl_ms
                    );
                }
                crate::query_db::perf_record_us(
                    "flatten_wall_us",
                    started_at.elapsed().as_micros() as u64,
                );
                log_stage_timing(stage_trace, "flatten", started_at);
                if stage_trace {
                    eprintln!("[stage] inline");
                }
                let inline_started_at = Instant::now();
                crate::compiler::inline::inline_function_calls(&mut flat_model, loader, stage_trace);
                crate::query_db::perf_record_us(
                    "inline_us",
                    inline_started_at.elapsed().as_micros() as u64,
                );
                crate::query_db::perf_record_us(
                    "inline_wall_us",
                    inline_started_at.elapsed().as_micros() as u64,
                );
                log_stage_timing(stage_trace, "inline", inline_started_at);
                return Ok(FrontendStage {
                    total_equations: flat_model.equations.len(),
                    total_declarations: flat_model.declarations.len(),
                    flat_model,
                });
            }
        }
        if let Some(mut flat_model) =
            flatten_cache::try_read_flat_cache_v1(&cache_root, &full_key, loader)
        {
            if stage_trace {
                eprintln!("[stage] flatten_cache_hit");
            }
            crate::query_db::perf_record_us(
                "flatten_wall_us",
                started_at.elapsed().as_micros() as u64,
            );
            log_stage_timing(stage_trace, "flatten", started_at);
            if stage_trace {
                eprintln!("[stage] inline");
            }
            let inline_started_at = Instant::now();
            crate::compiler::inline::inline_function_calls(&mut flat_model, loader, stage_trace);
            crate::query_db::perf_record_us(
                "inline_us",
                inline_started_at.elapsed().as_micros() as u64,
            );
            crate::query_db::perf_record_us(
                "inline_wall_us",
                inline_started_at.elapsed().as_micros() as u64,
            );
            log_stage_timing(stage_trace, "inline", inline_started_at);
            return Ok(FrontendStage {
                total_equations: flat_model.equations.len(),
                total_declarations: flat_model.declarations.len(),
                flat_model,
            });
        }
        let key = flatten_cache::flatten_array_sizes_cache_key(
            model_name,
            loader,
            array_sizes_json_path,
            array_size_policy,
            warnings_level,
        );
        flatten_cache::merge_cached_array_sizes(&cache_root, &key, &mut external_array_sizes)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
    }
    let mut flattener = Flattener::new();
    flattener.coarse_constrainedby_only = coarse_constrainedby_only;
    flattener.validation_mode = validation_mode;
    flattener.array_size_policy = array_size_policy;
    flattener.external_array_sizes = external_array_sizes;
    flattener.warnings_level = warnings_level.to_string();
    for path in &loader.library_paths {
        flattener.loader.add_path(path.clone());
    }
    if let Some(p) = loader.get_path_for_model(model_name) {
        flattener.loader.register_path(model_name, p);
    }
    flattener.loader.set_quiet(quiet);

    // Query-based flatten: default on for --validate; off for full simulation unless RUSTMODLICA_SALSA=1.
    let salsa_enabled = salsa_query_path_enabled(validate_only);
    if salsa_enabled {
        let mut db = query_db::Database::default();
        let libs: Vec<String> = loader
            .library_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        db.set_library_paths(std::sync::Arc::new(libs));
        db.set_coarse_constrainedby_only(coarse_constrainedby_only);
        db.set_compile_stop(std::sync::Arc::new(compile_stop_s.to_string()));
        db.set_validation_mode(std::sync::Arc::new(format!("{:?}", validation_mode)));
        let inherited = db.inheritance_flattened(model_name.replace('/', ".")).0;
        *root_model = inherited;
    }
    let mut flat_model = if salsa_enabled {
        // Prefer query-based composition when enabled.
        let mut db = query_db::Database::default();
        let libs: Vec<String> = loader
            .library_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        db.set_library_paths(std::sync::Arc::new(libs));
        db.set_coarse_constrainedby_only(coarse_constrainedby_only);
        db.set_compile_stop(std::sync::Arc::new(compile_stop_s.to_string()));
        db.set_validation_mode(std::sync::Arc::new(format!("{:?}", validation_mode)));
        let res = db.flattened_model_q(model_name.replace('/', ".")).0;
        if let Some(err) = &res.err {
            return Err(err.clone().into());
        }
        let Some(flat_arc) = &res.flat else {
            return Err("flattened_model_q returned empty result".into());
        };
        match Arc::try_unwrap(Arc::clone(flat_arc)) {
            Ok(v) => v,
            Err(a) => (*a).clone(),
        }
    } else if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
        let full_key = flatten_cache::flatten_full_cache_key(
            model_name,
            loader,
            validation_mode,
            compile_stop_s,
            coarse_constrainedby_only,
            array_sizes_json_path,
            array_size_policy,
            warnings_level,
        );
        let out = flatten_cache::get_or_compute_flattened_model_v1(&cache_root, &full_key, loader, || {
            // If salsa inheritance is enabled, `root_model` already includes inheritance.
            if salsa_enabled {
                flattener.flatten_with_mode_preinherited(root_model, model_name)
            } else {
                flattener.flatten_with_mode(root_model, model_name)
            }
        })?;
        if validate_only && matches!(compile_stop, CompileStopPhase::Analyze) {
            let mem_key = format!("analyze_input_v1:{}", full_key);
            flatten_cache::analyze_input_mem_put(mem_key.as_str(), Arc::new(out.clone()));
        }
        out
    } else {
        if salsa_enabled {
            flattener.flatten_with_mode_preinherited(root_model, model_name)?
        } else {
            flattener.flatten_with_mode(root_model, model_name)?
        }
    };
    if let Some((hits, misses, evictions)) = flatten_cache::array_sizes_cache_counters_snapshot_reset() {
        let ttl_ms = std::env::var("RUSTMODLICA_FLATTEN_CACHE_TTL_MS")
            .ok()
            .unwrap_or_default();
        eprintln!(
            "[perf] array_sizes_cache hits={} misses={} evictions={} ttl_ms={}",
            hits, misses, evictions, ttl_ms
        );
    }
    crate::query_db::perf_record_us(
        "flatten_wall_us",
        started_at.elapsed().as_micros() as u64,
    );
    log_stage_timing(stage_trace, "flatten", started_at);

    if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
        let full_key = flatten_cache::flatten_full_cache_key(
            model_name,
            loader,
            validation_mode,
            compile_stop_s,
            coarse_constrainedby_only,
            array_sizes_json_path,
            array_size_policy,
            warnings_level,
        );
        let key = flatten_cache::flatten_array_sizes_cache_key(
            model_name,
            loader,
            array_sizes_json_path,
            array_size_policy,
            warnings_level,
        );
        let _ = flatten_cache::write_array_sizes_cache(&cache_root, &key, &flat_model.array_sizes);
        let deps = flattener.loader.loaded_source_paths();
        let _ = flatten_cache::write_flat_cache_v1(&cache_root, &full_key, model_name, &flat_model, &deps);
    }

    if let Some(path) = emit_flat_snapshot {
        let snap_started_at = Instant::now();
        flat_snapshot::write_flat_snapshot(path, &flat_model).map_err(|e| {
            let io_err = std::io::Error::new(std::io::ErrorKind::Other, e);
            Box::new(io_err) as Box<dyn std::error::Error + Send + Sync>
        })?;
        crate::query_db::perf_record_us(
            "snapshot_write_us",
            snap_started_at.elapsed().as_micros() as u64,
        );
    }

    if stage_trace {
        eprintln!("[stage] inline");
    }
    let inline_started_at = Instant::now();
    crate::compiler::inline::inline_function_calls(&mut flat_model, loader, stage_trace);
    crate::query_db::perf_record_us(
        "inline_us",
        inline_started_at.elapsed().as_micros() as u64,
    );
    crate::query_db::perf_record_us(
        "inline_wall_us",
        inline_started_at.elapsed().as_micros() as u64,
    );
    log_stage_timing(stage_trace, "inline", inline_started_at);

    Ok(FrontendStage {
        total_equations: flat_model.equations.len(),
        total_declarations: flat_model.declarations.len(),
        flat_model,
    })
}
