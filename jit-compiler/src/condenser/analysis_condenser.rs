//! AnalysisCondenser: shifts DAE analysis (variable classification, equation matching,
//! backend DAE construction) from runtime to install/first-run.
//!
//! Analogous to Leyden's constant-pool resolution condenser -- pre-resolves symbolic
//! analysis results and stores them in the pipeline_analysis / backend_dae SQLite caches.

use super::{Condenser, CondenserContext, CondenserError, CondenserOutput, CondenserPhase};
use crate::compiler::{CompileStopPhase, Compiler};

pub struct AnalysisCondenser;

impl Condenser for AnalysisCondenser {
    fn name(&self) -> &str {
        "analysis"
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
        compiler.options_mut().compile_stop = CompileStopPhase::Analyze;
        compiler.options_mut().quiet = ctx.quiet;
        for p in &ctx.lib_paths {
            compiler.loader.add_path(p.clone());
        }

        match compiler.compile(&ctx.model_name) {
            Ok(_) => {
                let perf = compiler.last_compile_perf.as_ref();
                let flat_hit = perf.map(|p| p.flat_full_cache_hits > 0).unwrap_or(false);
                let analysis_hit = perf
                    .map(|p| p.analysis_pipeline_cache_status == "disk_hit")
                    .unwrap_or(false);
                let dae_hit = perf
                    .map(|p| p.backend_dae_cache_status == "disk_hit")
                    .unwrap_or(false);
                let hits = [flat_hit, analysis_hit, dae_hit]
                    .iter()
                    .filter(|&&h| h)
                    .count() as u32;
                let writes = 3u32.saturating_sub(hits);
                Ok(CondenserOutput {
                    condenser_name: self.name().to_string(),
                    phase: ctx.phase,
                    artifacts_written: writes,
                    cache_hits: hits,
                    elapsed_us: 0,
                    detail: Some(format!(
                        "flat={} analysis={} dae={}",
                        if flat_hit { "hit" } else { "miss" },
                        if analysis_hit { "hit" } else { "miss" },
                        if dae_hit { "hit" } else { "miss" },
                    )),
                })
            }
            Err(e) => Err(CondenserError::CompilationFailed(format!(
                "analyze {}: {}",
                ctx.model_name, e
            ))),
        }
    }
}
