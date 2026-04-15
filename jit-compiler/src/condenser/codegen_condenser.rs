//! CodegenCondenser: shifts Cranelift JIT compilation from runtime to first-run/warmup.
//!
//! Analogous to Leyden's AOT method compilation condenser -- generates native code for
//! the model's calc_derivs function and persists it in the codegen disk cache.

use super::{Condenser, CondenserContext, CondenserError, CondenserOutput, CondenserPhase};
use crate::compiler::{CompileStopPhase, Compiler};

pub struct CodegenCondenser;

impl Condenser for CodegenCondenser {
    fn name(&self) -> &str {
        "codegen"
    }

    fn phase(&self) -> CondenserPhase {
        CondenserPhase::Warmup
    }

    fn can_apply(&self, ctx: &CondenserContext) -> bool {
        !ctx.model_name.is_empty()
            && matches!(
                ctx.phase,
                CondenserPhase::FirstRun | CondenserPhase::Warmup
            )
    }

    fn apply(&self, ctx: &mut CondenserContext) -> Result<CondenserOutput, CondenserError> {
        let mut compiler = Compiler::new();
        compiler.options_mut().compile_stop = CompileStopPhase::Full;
        compiler.options_mut().quiet = ctx.quiet;
        for p in &ctx.lib_paths {
            compiler.loader.add_path(p.clone());
        }

        match compiler.compile(&ctx.model_name) {
            Ok(_) => {
                let perf = compiler.last_compile_perf.as_ref();
                let structural_hit = perf.map(|p| p.structural_cache_hit).unwrap_or(false);
                let jit_ok = perf.map(|p| p.jit_compile_ok).unwrap_or(false);
                Ok(CondenserOutput {
                    condenser_name: self.name().to_string(),
                    phase: ctx.phase,
                    artifacts_written: if structural_hit { 0 } else { 1 },
                    cache_hits: if structural_hit { 1 } else { 0 },
                    elapsed_us: 0,
                    detail: Some(format!(
                        "structural_hit={} jit_ok={}",
                        structural_hit, jit_ok
                    )),
                })
            }
            Err(e) => Err(CondenserError::CompilationFailed(format!(
                "codegen {}: {}",
                ctx.model_name, e
            ))),
        }
    }
}
