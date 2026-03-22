use std::sync::Arc;
use std::time::Instant;

use crate::ast::Model;
use crate::flatten::Flattener;
use crate::loader::ModelLoader;

use super::trace::log_stage_timing;
use super::types::{CompilerResult, FrontendStage};

pub(crate) fn flatten_and_inline(
    root_model: &mut Arc<Model>,
    model_name: &str,
    loader: &mut ModelLoader,
    quiet: bool,
    stage_trace: bool,
) -> CompilerResult<FrontendStage> {
    let started_at = Instant::now();
    if stage_trace {
        eprintln!("[stage] flatten");
    }
    let mut flattener = Flattener::new();
    for path in &loader.library_paths {
        flattener.loader.add_path(path.clone());
    }
    if let Some(p) = loader.get_path_for_model(model_name) {
        flattener.loader.register_path(model_name, p);
    }
    flattener.loader.set_quiet(quiet);
    let mut flat_model = flattener.flatten(root_model, model_name)?;
    log_stage_timing(stage_trace, "flatten", started_at);

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
