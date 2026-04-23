use crate::analysis::{provenance_index_from_flat_model, ImpactAnalysisResult, ProvenanceIndex};
use crate::compiler::{CompileOutput, CompileStopPhase, Compiler, CompilerOptions};
use crate::diag::WarningInfo;
use crate::flatten::FlattenedModel;
use crate::simulation::{run_simulation_collect, SimulationResult};
use std::path::PathBuf;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Thin facade for IDE / service callers to validate a Modelica model from source.
#[derive(Debug, Clone)]
pub struct ValidateRequest<'a> {
    pub code: &'a str,
    /// Logical model name, e.g. `BouncingBall` or `MyLib.Package.Model`.
    pub model_name: &'a str,
    /// Optional compiler options. When None, `CompilerOptions::default()` is used.
    pub options: Option<CompilerOptions>,
}

#[derive(Debug, Clone)]
pub struct ValidateResult {
    pub success: bool,
    pub warnings: Vec<WarningInfo>,
    pub errors: Vec<String>,
    pub state_vars: Vec<String>,
    pub output_vars: Vec<String>,
    /// When successful, which compile tier finished (`Full` after JIT or function eval).
    pub validation_stop_phase: Option<CompileStopPhase>,
    /// True when validation stopped before JIT (`Parse`, `Flatten`, or `Analyze`).
    pub validation_partial: bool,
}

fn build_compiler_with_options(options: Option<CompilerOptions>) -> Compiler {
    let mut compiler = Compiler::new();
    if let Some(opts) = options {
        compiler.options = opts;
    }
    compiler.loader.add_path(".".into());
    compiler.loader.add_path("StandardLib".into());
    compiler.loader.add_path("TestLib".into());
    compiler
}

fn handle_validate_output(
    mut compiler: Compiler,
    compile_output: Result<CompileOutput, BoxError>,
) -> ValidateResult {
    match compile_output {
        Ok(out) => {
            let warnings = compiler.take_warnings();
            match out {
                CompileOutput::FunctionRun(_) => ValidateResult {
                    success: true,
                    warnings,
                    errors: Vec::new(),
                    state_vars: Vec::new(),
                    output_vars: Vec::new(),
                    validation_stop_phase: Some(CompileStopPhase::Full),
                    validation_partial: false,
                },
                CompileOutput::Simulation(artifacts) => ValidateResult {
                    success: true,
                    warnings,
                    errors: Vec::new(),
                    state_vars: artifacts.state_vars,
                    output_vars: artifacts.output_vars,
                    validation_stop_phase: Some(CompileStopPhase::Full),
                    validation_partial: false,
                },
                CompileOutput::FlatSnapshotDone => ValidateResult {
                    success: true,
                    warnings,
                    errors: Vec::new(),
                    state_vars: Vec::new(),
                    output_vars: Vec::new(),
                    validation_stop_phase: Some(CompileStopPhase::Full),
                    validation_partial: false,
                },
                CompileOutput::ValidationParseOk => ValidateResult {
                    success: true,
                    warnings,
                    errors: Vec::new(),
                    state_vars: Vec::new(),
                    output_vars: Vec::new(),
                    validation_stop_phase: Some(CompileStopPhase::Parse),
                    validation_partial: true,
                },
                CompileOutput::ValidationFlattenOk { .. } => ValidateResult {
                    success: true,
                    warnings,
                    errors: Vec::new(),
                    state_vars: Vec::new(),
                    output_vars: Vec::new(),
                    validation_stop_phase: Some(CompileStopPhase::Flatten),
                    validation_partial: true,
                },
                CompileOutput::ValidationAnalyzed(s) => ValidateResult {
                    success: true,
                    warnings,
                    errors: Vec::new(),
                    state_vars: s.state_vars,
                    output_vars: s.output_vars,
                    validation_stop_phase: Some(CompileStopPhase::Analyze),
                    validation_partial: true,
                },
            }
        }
        Err(e) => {
            let warnings = compiler.take_warnings();
            ValidateResult {
                success: false,
                warnings,
                errors: vec![e.to_string()],
                state_vars: Vec::new(),
                output_vars: Vec::new(),
                validation_stop_phase: None,
                validation_partial: false,
            }
        }
    }
}

pub fn validate_from_source(request: ValidateRequest<'_>) -> ValidateResult {
    let mut compiler = build_compiler_with_options(request.options);
    compiler.options.validate_only = true;
    let result = compiler.compile_from_source(request.model_name, request.code);
    handle_validate_output(compiler, result)
}

pub fn simulate_from_source(
    code: &str,
    model_name: &str,
    options: Option<CompilerOptions>,
) -> Result<SimulationResult, BoxError> {
    let mut compiler = build_compiler_with_options(options);
    compiler.options.compile_stop = CompileStopPhase::Full;
    let out = compiler.compile_from_source(model_name, code)?;
    let artifacts = match out {
        CompileOutput::FunctionRun(_) => {
            return Err("simulation requested but entry is a function, not a model".into());
        }
        CompileOutput::FlatSnapshotDone => {
            return Err("simulation not available for flat-snapshot-only compile".into());
        }
        CompileOutput::ValidationParseOk
        | CompileOutput::ValidationFlattenOk { .. }
        | CompileOutput::ValidationAnalyzed(_) => {
            return Err("simulation requires full compile (tiered validation stopped early)".into());
        }
        CompileOutput::Simulation(artifacts) => artifacts,
    };
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
        None,
        model_name,
        compiler.loader.library_paths.clone(),
    )?;
    Ok(result)
}

/// Returns model names that may need re-validation when the given source paths change. Uses the
/// in-process reverse dependency index; see [`crate::query_db::affected_models`].
pub fn affected_models_for_changed_files(changed_files: &[PathBuf]) -> Vec<String> {
    crate::query_db::affected_models(changed_files)
}

/// Impact of changing flattened parameter names against a pre-built provenance index (analysis / IDE only).
pub fn analyze_change_impact(
    provenance: &ProvenanceIndex,
    changed_params: &[String],
) -> ImpactAnalysisResult {
    provenance.analyze_param_change_names(changed_params)
}

/// Impact of editing a component instance path (analysis / IDE only; not a codegen skip guarantee).
pub fn analyze_instance_change_impact(
    provenance: &ProvenanceIndex,
    instance_path: &str,
) -> ImpactAnalysisResult {
    provenance.compute_instance_change_impact(instance_path).into()
}

/// Heuristic for UI or logs only; see [`ProvenanceIndex::incremental_codegen_worthwhile_hint`].
pub fn incremental_codegen_worthwhile_hint(
    provenance: &ProvenanceIndex,
    impact: &ImpactAnalysisResult,
) -> bool {
    provenance.incremental_codegen_worthwhile_hint(impact)
}

/// Build [`ProvenanceIndex`] from a flattened model (optional root `.mo` path for `source_file` hints).
pub fn provenance_index_for_flat_model(
    flat: &FlattenedModel,
    root_source_file: Option<&str>,
) -> ProvenanceIndex {
    provenance_index_from_flat_model(flat, root_source_file)
}
