//! L4-T06: Opportunistic warmup — after a successful full compilation, scan the
//! dependency closure for sibling models in the same directories and optionally
//! pre-populate L2 caches (flatten + analyze only, no JIT) in the background.
//!
//! Input sources:
//!   1. Directory scanning: `.mo` files in the same directories as loaded sources.
//!   2. JSON manifest: `RUSTMODLICA_WARMUP_MANIFEST=<path>` pointing to a JSON file
//!      with the same format as `--precompile` (`["A.B"]` or `{"models":["A.B"]}`).
//!
//! Controls:
//!   - `RUSTMODLICA_WARMUP_ENABLED=1` (default on) — set to `0` to disable.
//!   - `RUSTMODLICA_WARMUP_MAX_CANDIDATES=<N>` (default 20) — cap on candidates per trigger.
//!   - `RUSTMODLICA_WARMUP_DELAY_MS=<N>` (default 2000) — delay before starting warmup.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use xxhash_rust::xxh64::Xxh64;

static WARMUP_LOCK: OnceLock<()> = OnceLock::new();

pub fn warmup_enabled() -> bool {
    match std::env::var("RUSTMODLICA_WARMUP_ENABLED") {
        Ok(v) => {
            let t = v.trim();
            if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                return false;
            }
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") || t.is_empty()
        }
        Err(_) => true,
    }
}

fn max_candidates() -> usize {
    std::env::var("RUSTMODLICA_WARMUP_MAX_CANDIDATES")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(20)
}

fn delay_ms() -> u64 {
    std::env::var("RUSTMODLICA_WARMUP_DELAY_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= 0)
        .unwrap_or(2000)
}

/// Derive candidate model names from loaded source paths + optional manifest.
/// Returns deduplicated, sorted list.
pub fn derive_candidates(loaded_paths: &[PathBuf], manifest_path: Option<&Path>) -> Vec<String> {
    let mut candidates = Vec::new();
    let max = max_candidates();

    // Source 1: .mo files in same directories
    let mut dirs: Vec<PathBuf> = loaded_paths
        .iter()
        .filter_map(|p| p.parent().map(|d| d.to_path_buf()))
        .collect();
    dirs.sort();
    dirs.dedup();

    for dir in &dirs {
        if candidates.len() >= max {
            break;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut mo_files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext.eq_ignore_ascii_case("mo"))
                        .unwrap_or(false)
                })
                .map(|e| e.path())
                .collect();
            mo_files.sort();
            for mo in mo_files {
                if candidates.len() >= max {
                    break;
                }
                if let Some(name) = mo_to_model_name(&mo, loaded_paths) {
                    candidates.push(name);
                }
            }
        }
    }

    // Source 2: manifest JSON
    if let Some(mp) = manifest_path {
        if let Ok(text) = std::fs::read_to_string(mp) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let models: Vec<String> = if let Some(a) = v.as_array() {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                } else if let Some(a) = v.get("models").and_then(|x| x.as_array()) {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                } else {
                    Vec::new()
                };
                for m in models {
                    if candidates.len() >= max {
                        break;
                    }
                    if !candidates.contains(&m) {
                        candidates.push(m);
                    }
                }
            }
        }
    }

    candidates.sort();
    candidates.dedup();
    candidates
}

/// Attempt to derive a Modelica fully-qualified name from a `.mo` file path.
/// Uses a simple heuristic: strip common prefix dirs found in loaded_paths,
/// then replace `/` with `.` and remove the `.mo` extension.
fn mo_to_model_name(mo_path: &Path, loaded_paths: &[PathBuf]) -> Option<String> {
    let file_stem = mo_path.file_stem()?.to_str()?.to_string();
    // Try to find a package hierarchy by walking up from the .mo file.
    // Simple approach: just use the file stem as the short model name.
    // For packages, the file is typically `path/to/Packagename.mo` and
    // the model inside is `Packagename` or `Packagename.Model`.
    // We return the stem as a candidate; the loader will resolve it.
    if file_stem.is_empty() || file_stem.starts_with('_') {
        return None;
    }
    // Skip if this is one of the loaded files (already compiled)
    for lp in loaded_paths {
        if lp.file_stem().and_then(|s| s.to_str()) == Some(file_stem.as_str()) {
            return None;
        }
    }
    Some(file_stem)
}

/// Compute a fingerprint of the loaded paths to avoid re-scanning unchanged sets.
pub fn loaded_paths_fingerprint(paths: &[PathBuf]) -> String {
    let mut h = Xxh64::new(0);
    let mut sorted: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
    sorted.sort();
    for p in sorted {
        h.update(p.to_string_lossy().as_bytes());
        h.update(&[0]);
        if let Ok(meta) = std::fs::metadata(p) {
            if let Ok(mtime) = meta.modified() {
                let ns = mtime
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                h.update(&ns.to_le_bytes());
            }
        }
    }
    format!("{:016x}", h.digest())
}

/// Warmup tier: controls how deep the warmup pre-populates caches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmupTier {
    /// Only flatten + analyze (L2 caches, no JIT). Default for background warmup.
    Analyze,
    /// Full compilation including JIT (populates codegen cache + artifact bundle).
    Full,
}

fn warmup_tier() -> WarmupTier {
    match std::env::var("RUSTMODLICA_WARMUP_TIER") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            if t == "full" || t == "jit" || t == "codegen" {
                WarmupTier::Full
            } else {
                WarmupTier::Analyze
            }
        }
        Err(_) => WarmupTier::Analyze,
    }
}

/// Trigger opportunistic warmup after a successful compilation.
/// This runs in a background thread (fire-and-forget).
/// `lib_paths` are forwarded to the background compiler's loader.
pub fn trigger_warmup(
    just_compiled: &str,
    loaded_paths: &[PathBuf],
    lib_paths: &[PathBuf],
) {
    trigger_warmup_with_tier(just_compiled, loaded_paths, lib_paths, warmup_tier());
}

/// Trigger warmup with an explicit tier.
pub fn trigger_warmup_with_tier(
    just_compiled: &str,
    loaded_paths: &[PathBuf],
    lib_paths: &[PathBuf],
    tier: WarmupTier,
) {
    if !warmup_enabled() {
        return;
    }
    // Only trigger once per process (avoid warmup-of-warmup recursion)
    if WARMUP_LOCK.set(()).is_err() {
        return;
    }

    let manifest = std::env::var("RUSTMODLICA_WARMUP_MANIFEST").ok();
    let loaded = loaded_paths.to_vec();
    let libs = lib_paths.to_vec();
    let compiled = just_compiled.to_string();
    let delay = delay_ms();

    if let Err(e) = std::thread::Builder::new()
        .name("warmup".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            if delay > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            let manifest_ref = manifest.as_deref().map(Path::new);
            let candidates = derive_candidates(&loaded, manifest_ref);
            let filtered: Vec<String> = candidates
                .into_iter()
                .filter(|c| c != &compiled)
                .collect();

            if filtered.is_empty() {
                return;
            }

            let tier_label = match tier {
                WarmupTier::Analyze => "L2 (analyze)",
                WarmupTier::Full => "L3 (full JIT)",
            };
            eprintln!(
                "[warmup] starting {} warmup for {} candidates (delay={}ms)",
                tier_label,
                filtered.len(),
                delay
            );

            let mut compiler = crate::Compiler::new();
            for p in &libs {
                compiler.loader.add_path(p.clone());
            }

            let stop_phase = match tier {
                WarmupTier::Analyze => crate::compiler::CompileStopPhase::Analyze,
                WarmupTier::Full => crate::compiler::CompileStopPhase::Full,
            };

            let mut ok = 0usize;
            let mut err = 0usize;
            for model in &filtered {
                compiler.options_mut().compile_stop = stop_phase.clone();
                compiler.options_mut().quiet = true;
                match compiler.compile(model) {
                    Ok(_) => {
                        ok += 1;
                    }
                    Err(e) => {
                        err += 1;
                        eprintln!("[warmup] failed {}: {}", model, e);
                    }
                }
            }
            eprintln!(
                "[warmup] done: {} ok, {} err out of {} candidates",
                ok,
                err,
                filtered.len()
            );
        })
    {
        eprintln!("[warmup] thread spawn failed: {}", e);
    }
}

/// Run full precompilation for a list of models (synchronous, parallel via rayon).
/// Used by `--precompile-msl` and install-time condensers.
pub fn precompile_models_parallel(
    models: &[String],
    lib_paths: &[PathBuf],
    quiet: bool,
) -> (usize, usize) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let ok_count = AtomicUsize::new(0);
    let err_count = AtomicUsize::new(0);

    for model in models {
        let mut compiler = crate::Compiler::new();
        compiler.options_mut().quiet = quiet;
        compiler.options_mut().compile_stop = crate::compiler::CompileStopPhase::Full;
        for p in lib_paths {
            compiler.loader.add_path(p.clone());
        }
        match compiler.compile(model) {
            Ok(_) => {
                ok_count.fetch_add(1, Ordering::Relaxed);
                if !quiet {
                    eprintln!("[precompile] OK {}", model);
                }
            }
            Err(e) => {
                err_count.fetch_add(1, Ordering::Relaxed);
                if !quiet {
                    eprintln!("[precompile] SKIP {} ({})", model, e);
                }
            }
        }
    }

    (
        ok_count.load(Ordering::Relaxed),
        err_count.load(Ordering::Relaxed),
    )
}
