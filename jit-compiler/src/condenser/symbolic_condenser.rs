//! SymbolicCondenser: pre-computes Jacobian sparsity patterns and equation dependency
//! graphs at build time.
//!
//! Analogous to Leyden's sealed-interface optimization -- freezes structural invariants
//! of the equation system so that subsequent compilations can skip symbolic analysis.

use super::{Condenser, CondenserContext, CondenserError, CondenserOutput, CondenserPhase};
use crate::compiler::{CompileStopPhase, Compiler};
use crate::analysis::ProvenanceIndex;

pub struct SymbolicCondenser;

impl SymbolicCondenser {
    fn build_provenance(
        ctx: &CondenserContext,
    ) -> Result<Option<ProvenanceIndex>, CondenserError> {
        let mut compiler = Compiler::new();
        compiler.options_mut().compile_stop = CompileStopPhase::Analyze;
        compiler.options_mut().quiet = true;
        for p in &ctx.lib_paths {
            compiler.loader.add_path(p.clone());
        }
        compiler
            .compile(&ctx.model_name)
            .map_err(|e| CondenserError::CompilationFailed(e.to_string()))?;
        Ok(compiler.last_provenance_index.as_ref().map(|arc| (**arc).clone()))
    }
}

impl Condenser for SymbolicCondenser {
    fn name(&self) -> &str {
        "symbolic"
    }

    fn phase(&self) -> CondenserPhase {
        CondenserPhase::BuildTime
    }

    fn can_apply(&self, ctx: &CondenserContext) -> bool {
        !ctx.model_name.is_empty()
            && matches!(
                ctx.phase,
                CondenserPhase::BuildTime | CondenserPhase::InstallTime | CondenserPhase::FirstRun
            )
    }

    fn apply(&self, ctx: &mut CondenserContext) -> Result<CondenserOutput, CondenserError> {
        let provenance = Self::build_provenance(ctx)?;
        let eq_count = provenance
            .as_ref()
            .map(|p| p.equations.len())
            .unwrap_or(0);
        let var_count = provenance
            .as_ref()
            .map(|p| p.var_dependencies.len())
            .unwrap_or(0);

        if let Some(prov) = &provenance {
            if let Ok(bytes) = bincode::serialize(prov) {
                ctx.artifacts
                    .insert("symbolic_provenance".to_string(), bytes);
            }
        }

        Ok(CondenserOutput {
            condenser_name: self.name().to_string(),
            phase: ctx.phase,
            artifacts_written: if provenance.is_some() { 1 } else { 0 },
            cache_hits: 0,
            elapsed_us: 0,
            detail: Some(format!("equations={} vars={}", eq_count, var_count)),
        })
    }
}
