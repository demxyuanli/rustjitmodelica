//! FlattenCondenser: shifts Modelica model flattening from runtime to install/first-run.
//!
//! Analogous to Leyden's AppCDS class loading condenser -- pre-resolves the model
//! inheritance/equation hierarchy and stores the FlatCacheV2 result in SQLite.

use super::{Condenser, CondenserContext, CondenserError, CondenserOutput, CondenserPhase};
use crate::compiler::{CompileStopPhase, Compiler};

pub struct FlattenCondenser;

impl Condenser for FlattenCondenser {
    fn name(&self) -> &str {
        "flatten"
    }

    fn phase(&self) -> CondenserPhase {
        CondenserPhase::FirstRun
    }

    fn can_apply(&self, ctx: &CondenserContext) -> bool {
        !ctx.model_name.is_empty()
            && matches!(
                ctx.phase,
                CondenserPhase::InstallTime | CondenserPhase::FirstRun | CondenserPhase::Warmup
            )
    }

    fn apply(&self, ctx: &mut CondenserContext) -> Result<CondenserOutput, CondenserError> {
        let mut compiler = Compiler::new();
        compiler.options_mut().compile_stop = CompileStopPhase::Flatten;
        compiler.options_mut().quiet = ctx.quiet;
        for p in &ctx.lib_paths {
            compiler.loader.add_path(p.clone());
        }

        match compiler.compile(&ctx.model_name) {
            Ok(_) => {
                let cache_hit = compiler
                    .last_compile_perf
                    .as_ref()
                    .map(|p| p.flat_full_cache_hits > 0)
                    .unwrap_or(false);
                Ok(CondenserOutput {
                    condenser_name: self.name().to_string(),
                    phase: ctx.phase,
                    artifacts_written: if cache_hit { 0 } else { 1 },
                    cache_hits: if cache_hit { 1 } else { 0 },
                    elapsed_us: 0,
                    detail: None,
                })
            }
            Err(e) => Err(CondenserError::CompilationFailed(format!(
                "flatten {}: {}",
                ctx.model_name, e
            ))),
        }
    }
}
