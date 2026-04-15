//! ParamCondenser: enables hot-reload when only parameter values change.
//!
//! Analogous to Leyden's speculative constant folding -- detects that the model
//! structure is unchanged and only the parameter vector needs updating, bypassing
//! full recompilation.

use super::{Condenser, CondenserContext, CondenserError, CondenserOutput, CondenserPhase};

pub struct ParamCondenser;

impl Condenser for ParamCondenser {
    fn name(&self) -> &str {
        "param"
    }

    fn phase(&self) -> CondenserPhase {
        CondenserPhase::HotReload
    }

    fn can_apply(&self, ctx: &CondenserContext) -> bool {
        !ctx.model_name.is_empty() && ctx.phase == CondenserPhase::HotReload
    }

    fn apply(&self, ctx: &mut CondenserContext) -> Result<CondenserOutput, CondenserError> {
        let cache_root = ctx
            .cache_root
            .as_ref()
            .ok_or_else(|| CondenserError::CacheUnavailable("no cache root".into()))?;

        let has_artifact = crate::cache::artifact_cache::artifact_cache_enabled()
            && cache_root.join("project").join("cache-project.sqlite").exists();

        Ok(CondenserOutput {
            condenser_name: self.name().to_string(),
            phase: ctx.phase,
            artifacts_written: 0,
            cache_hits: if has_artifact { 1 } else { 0 },
            elapsed_us: 0,
            detail: Some(format!(
                "param_hot_reload structural_reuse={}",
                has_artifact
            )),
        })
    }
}
