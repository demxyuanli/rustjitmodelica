use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::ast::Model;
use crate::flatten::flat_snapshot;
use crate::flatten::flatten_cache;
use crate::flatten::{ArraySizePolicy, Flattener, ValidationMode, load_array_sizes_json_optional};
use crate::loader::ModelLoader;

use crate::compiler::CompileStopPhase;

use super::trace::log_stage_timing;
use super::types::{CompilerResult, FrontendStage};

pub(crate) fn flatten_and_inline(
    root_model: &mut Arc<Model>,
    model_name: &str,
    loader: &mut ModelLoader,
    compile_stop: CompileStopPhase,
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
        if let Some(mut flat_model) =
            flatten_cache::try_read_flat_cache_v1(&cache_root, &full_key, loader)
        {
            if stage_trace {
                eprintln!("[stage] flatten_cache_hit");
            }
            log_stage_timing(stage_trace, "flatten", started_at);
            if stage_trace {
                eprintln!("[stage] inline");
            }
            let inline_started_at = Instant::now();
            crate::compiler::inline::inline_function_calls(&mut flat_model, loader);
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
    let mut flat_model = if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
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
        flatten_cache::get_or_compute_flattened_model_v1(&cache_root, &full_key, loader, || {
            flattener.flatten_with_mode(root_model, model_name)
        })?
    } else {
        flattener.flatten_with_mode(root_model, model_name)?
    };
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
        flat_snapshot::write_flat_snapshot(path, &flat_model).map_err(|e| {
            let io_err = std::io::Error::new(std::io::ErrorKind::Other, e);
            Box::new(io_err) as Box<dyn std::error::Error + Send + Sync>
        })?;
    }

    if stage_trace {
        eprintln!("[stage] inline");
    }
    let inline_started_at = Instant::now();
    crate::compiler::inline::inline_function_calls(&mut flat_model, loader);
    log_stage_timing(stage_trace, "inline", inline_started_at);

    Ok(FrontendStage {
        total_equations: flat_model.equations.len(),
        total_declarations: flat_model.declarations.len(),
        flat_model,
    })
}
