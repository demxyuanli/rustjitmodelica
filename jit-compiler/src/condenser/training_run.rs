//! Training run orchestrator (Phase 2 of Leyden-inspired compilation).
//!
//! A training run executes a representative simulation and collects profile data
//! that guides subsequent speculative AOT compilation. Analogous to Leyden's
//! `-XX:AOTMode=record` / `-XX:AOTMode=create` workflow.

use std::path::{Path, PathBuf};
use std::time::Instant;

use super::profile_data::{ModelProfile, ProfileCollector};

/// Configuration for a training run.
pub struct TrainingRunConfig {
    pub model_name: String,
    pub lib_paths: Vec<PathBuf>,
    pub output_profile_path: Option<PathBuf>,
    pub quiet: bool,
}

/// Result of a training run.
pub struct TrainingRunResult {
    pub profile: ModelProfile,
    pub profile_path: Option<PathBuf>,
    pub wall_us: u64,
}

/// Execute a training run: compile the model, run a short simulation with profiling,
/// and persist the collected profile data.
pub fn execute_training_run(
    config: &TrainingRunConfig,
) -> Result<TrainingRunResult, Box<dyn std::error::Error + Send + Sync>> {
    let t0 = Instant::now();
    if !config.quiet {
        eprintln!(
            "[training-run] starting profile collection for {}",
            config.model_name
        );
    }

    let mut compiler = crate::Compiler::new();
    compiler.options_mut().quiet = config.quiet;
    for p in &config.lib_paths {
        compiler.loader.add_path(p.clone());
    }

    let output = compiler.compile(&config.model_name)?;

    let eq_count_from_perf = compiler
        .last_compile_perf
        .as_ref()
        .map(|p| p.alg_eq_count + p.diff_eq_count)
        .unwrap_or(0);

    let profile = match output {
        crate::CompileOutput::Simulation(ref artifacts) => {
            let eq_count = eq_count_from_perf.max(artifacts.state_vars.len());
            let mut collector = ProfileCollector::new(&config.model_name, eq_count);

            for (i, name) in artifacts.state_vars.iter().enumerate() {
                if i < artifacts.states.len() {
                    collector.record_state_value(name, artifacts.states[i]);
                }
            }
            collector.record_step();

            let wall_us = t0.elapsed().as_micros() as u64;
            collector.finalize(wall_us)
        }
        _ => {
            let wall_us = t0.elapsed().as_micros() as u64;
            let mut profile = ModelProfile::new(&config.model_name);
            profile.training_wall_us = wall_us;
            profile
        }
    };

    let profile_path = if let Some(ref out_path) = config.output_profile_path {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        profile.write_to_file(out_path).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;
        if !config.quiet {
            eprintln!(
                "[training-run] profile written to {}",
                out_path.display()
            );
        }
        Some(out_path.clone())
    } else {
        try_write_to_cache(&config.model_name, &profile, config.quiet)
    };

    let wall_us = t0.elapsed().as_micros() as u64;
    if !config.quiet {
        eprintln!(
            "[training-run] done in {:.1}ms (steps={}, hot_eqs={})",
            wall_us as f64 / 1000.0,
            profile.total_steps,
            profile.hot_equations.len(),
        );
    }

    Ok(TrainingRunResult {
        profile,
        profile_path,
        wall_us,
    })
}

/// Try to store profile in the standard cache directory.
fn try_write_to_cache(model_name: &str, profile: &ModelProfile, quiet: bool) -> Option<PathBuf> {
    let cache_root = crate::flatten::flatten_cache_dir()?;
    let profiles_dir = cache_root.join("profiles");
    std::fs::create_dir_all(&profiles_dir).ok()?;

    let safe_name = model_name.replace('.', "_").replace('/', "_");
    let path = profiles_dir.join(format!("{}.profile.bin", safe_name));
    match profile.write_to_file(&path) {
        Ok(()) => {
            if !quiet {
                eprintln!("[training-run] profile cached at {}", path.display());
            }
            Some(path)
        }
        Err(e) => {
            if !quiet {
                eprintln!("[training-run] cache write failed: {}", e);
            }
            None
        }
    }
}

/// Try to load a cached profile for the given model.
pub fn load_cached_profile(model_name: &str) -> Option<ModelProfile> {
    let cache_root = crate::flatten::flatten_cache_dir()?;
    let safe_name = model_name.replace('.', "_").replace('/', "_");
    let path = cache_root
        .join("profiles")
        .join(format!("{}.profile.bin", safe_name));
    ModelProfile::read_from_file(&path).ok()
}

/// Load profile from a specific file path.
pub fn load_profile_from_file(path: &Path) -> Result<ModelProfile, String> {
    ModelProfile::read_from_file(path)
}
