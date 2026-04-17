//! L4-T06: Opportunistic warmup — after a successful full compilation, scan the
//! dependency closure for sibling models in the same directories and optionally
//! pre-populate L2 caches (flatten + analyze only, no JIT) in the background.
//!
//! Input sources:
//!   1. Directory scanning: `.mo` files in the same directories as loaded sources.
//!   2. JSON manifest: `RUSTMODLICA_WARMUP_MANIFEST=<path>` pointing to a JSON file
//!      with the same format as `--precompile` (`["A.B"]` or `{"models":["A.B"]}`).
//!   3. Optional dep graph JSON: `RUSTMODLICA_WARMUP_DEP_GRAPH=<path>` (baseline-style
//!      `{"entries":[{"file":"...","models":[...]}]}`) — files listed first get higher priority.
//!
//! Controls:
//!   - `RUSTMODLICA_WARMUP_ENABLED=1` (default on) — set to `0` to disable.
//!   - `RUSTMODLICA_WARMUP_MAX_CANDIDATES=<N>` (default 20) — cap on candidates per trigger.
//!   - `RUSTMODLICA_WARMUP_DELAY_MS=<N>` (default 2000) — delay before starting warmup.
//!   - `RUSTMODLICA_WARMUP_TIER=analyze|full|auto` — auto picks tier from host load (best-effort).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use fs2::FileExt;
use rayon::prelude::*;
use serde::Deserialize;
use xxhash_rust::xxh64::Xxh64;

use crate::cache::global_budget;
use crate::cache::model_hotness_record;
use crate::cache::warmup_control;

static WARMUP_LOCK: OnceLock<()> = OnceLock::new();
static HEAVY_WARMUP_SPAWNS: AtomicU32 = AtomicU32::new(0);

/// Last background warmup run (for `cache stats`).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct WarmupLastRun {
    pub candidates: u32,
    pub ok: u32,
    pub err: u32,
    pub elapsed_ms: u64,
    pub tier: String,
}

static LAST_WARMUP: Mutex<Option<WarmupLastRun>> = Mutex::new(None);

pub fn take_last_warmup_run() -> Option<WarmupLastRun> {
    LAST_WARMUP.lock().ok().and_then(|mut g| g.take())
}

pub fn peek_last_warmup_run() -> Option<WarmupLastRun> {
    LAST_WARMUP.lock().ok().and_then(|g| g.clone())
}

fn set_last_warmup(w: WarmupLastRun) {
    if let Ok(mut g) = LAST_WARMUP.lock() {
        *g = Some(w);
    }
}

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
        .unwrap_or(2000)
}

fn budget_check_every_n() -> usize {
    std::env::var("RUSTMODLICA_WARMUP_BUDGET_CHECK_EVERY")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(3)
}

fn max_cache_bytes_soft_stop() -> u64 {
    std::env::var("RUSTMODLICA_CACHE_MAX_BYTES")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= 1_048_576)
        .unwrap_or(1_073_741_824)
}

/// Derive candidate model names from loaded source paths + optional manifest + optional dep graph.
/// Returns deduplicated list sorted by descending priority score then name.
pub fn derive_candidates(loaded_paths: &[PathBuf], manifest_path: Option<&Path>) -> Vec<String> {
    let max = max_candidates();
    let cache_root = crate::flatten::flatten_cache_dir();
    let mut scored: Vec<(String, f64)> = Vec::new();

    if let Ok(p) = std::env::var("RUSTMODLICA_WARMUP_DEP_GRAPH") {
        let dp = PathBuf::from(p.trim());
        if dp.is_file() {
            if let Ok(names) = dep_graph_priority_names(&dp, loaded_paths) {
                for n in names {
                    if scored.iter().any(|(s, _)| s == &n) {
                        continue;
                    }
                    let mo_size = guess_mo_size_for_name(&n, loaded_paths);
                    let hot = model_hotness_record::score_for_model(cache_root.as_deref(), &n);
                    let score = hot + (mo_size as f64) / 1024.0 + 1000.0;
                    scored.push((n, score));
                    if scored.len() >= max {
                        break;
                    }
                }
            }
        }
    }

    let mut dirs: Vec<PathBuf> = loaded_paths
        .iter()
        .filter_map(|p| p.parent().map(|d| d.to_path_buf()))
        .collect();
    dirs.sort();
    dirs.dedup();

    for dir in &dirs {
        if scored.len() >= max {
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
                if scored.len() >= max {
                    break;
                }
                if let Some(name) = mo_to_model_name(&mo, loaded_paths) {
                    if scored.iter().any(|(s, _)| s == &name) {
                        continue;
                    }
                    let mo_size = std::fs::metadata(&mo).map(|m| m.len()).unwrap_or(0);
                    let hot = model_hotness_record::score_for_model(cache_root.as_deref(), &name);
                    let score = hot + (mo_size as f64) / 1024.0;
                    scored.push((name, score));
                }
            }
        }
    }

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
                    if scored.len() >= max {
                        break;
                    }
                    if scored.iter().any(|(s, _)| s == &m) {
                        continue;
                    }
                    let hot = model_hotness_record::score_for_model(cache_root.as_deref(), &m);
                    scored.push((m, hot + 500.0));
                }
            }
        }
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.cmp(&b.0)));
    scored.dedup_by(|a, b| a.0 == b.0);
    scored.into_iter().map(|(n, _)| n).collect()
}

#[derive(Deserialize)]
struct DepGraphFile {
    entries: Vec<DepGraphEntry>,
}

#[derive(Deserialize)]
struct DepGraphEntry {
    file: String,
}

fn dep_graph_priority_names(path: &Path, loaded_paths: &[PathBuf]) -> std::io::Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    let dg: DepGraphFile = serde_json::from_str(&text).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
    })?;
    let mut out = Vec::new();
    for e in &dg.entries {
        let p = PathBuf::from(e.file.trim());
        if p.extension()
            .and_then(|s| s.to_str())
            .map(|x| x.eq_ignore_ascii_case("mo"))
            != Some(true)
        {
            continue;
        }
        if let Some(n) = mo_to_model_name(&p, loaded_paths) {
            out.push(n);
        }
    }
    Ok(out)
}

fn guess_mo_size_for_name(model_name: &str, loaded_paths: &[PathBuf]) -> u64 {
    let rel = format!("{}.mo", model_name.replace('.', "/"));
    for root in loaded_paths {
        if let Some(parent) = root.parent() {
            let candidate = parent.join(Path::new(&rel));
            if let Ok(m) = std::fs::metadata(&candidate) {
                return m.len();
            }
        }
    }
    0
}

fn parse_within_prefix(mo_path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(mo_path).ok()?;
    for line in text.lines().take(64) {
        let t = line.trim();
        if let Some(rest) = t
            .strip_prefix("within ")
            .or_else(|| t.strip_prefix("within\t"))
        {
            let name = rest.split(';').next()?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Attempt to derive a Modelica fully-qualified name from a `.mo` file path.
fn mo_to_model_name(mo_path: &Path, loaded_paths: &[PathBuf]) -> Option<String> {
    let file_stem = mo_path.file_stem()?.to_str()?.to_string();
    if file_stem.is_empty() || file_stem.starts_with('_') {
        return None;
    }
    for lp in loaded_paths {
        if lp.file_stem().and_then(|s| s.to_str()) == Some(file_stem.as_str()) {
            return None;
        }
    }
    let within = parse_within_prefix(mo_path);
    let candidate = match within {
        Some(w) if !w.is_empty() => format!("{w}.{file_stem}"),
        _ => file_stem,
    };
    Some(candidate)
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
    Analyze,
    Full,
}

fn warmup_tier_resolved() -> WarmupTier {
    match std::env::var("RUSTMODLICA_WARMUP_TIER") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            if t == "full" || t == "jit" || t == "codegen" {
                WarmupTier::Full
            } else if t == "auto" {
                adaptive_warmup_tier()
            } else {
                WarmupTier::Analyze
            }
        }
        Err(_) => WarmupTier::Analyze,
    }
}

fn adaptive_warmup_tier() -> WarmupTier {
    let idle = host_cpu_idle_ratio_best_effort();
    if idle < 0.20 {
        return WarmupTier::Analyze;
    }
    if let Some(root) = crate::flatten::flatten_cache_dir() {
        let layers = crate::flatten::export_sqlite_kind_stats_layers(root.as_path());
        let (hits, gets) = layers.iter().fold((0_i64, 0_i64), |acc, layer| {
            let h: i64 = layer.rows.iter().map(|r| r.hit_count).sum();
            let g: i64 = layer.rows.iter().map(|r| r.get_count).sum();
            (acc.0 + h, acc.1 + g)
        });
        if gets > 0 {
            let ratio = hits as f64 / gets as f64;
            if ratio > 0.85 {
                return WarmupTier::Analyze;
            }
        }
    }
    WarmupTier::Full
}

#[cfg(windows)]
fn host_cpu_idle_ratio_best_effort() -> f64 {
    // windows-sys does not expose GetSystemTimes in all feature sets; use neutral default.
    0.5
}

#[cfg(not(windows))]
fn host_cpu_idle_ratio_best_effort() -> f64 {
    if let Ok(content) = std::fs::read_to_string("/proc/stat") {
        if let Some(line) = content.lines().next() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 && parts[0] == "cpu" {
                let idle = parts.get(4).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                let sum: u64 = parts
                    .iter()
                    .skip(1)
                    .take(10)
                    .filter_map(|s| s.parse::<u64>().ok())
                    .sum();
                if sum > 0 {
                    return (idle as f64 / sum as f64).clamp(0.0, 1.0);
                }
            }
        }
    }
    0.5
}

fn try_cross_process_warmup_lock(cache_root: &Path) -> Option<std::fs::File> {
    let _ = std::fs::create_dir_all(cache_root);
    let path = cache_root.join(".warmup.lock");
    let f = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .ok()?;
    if f.try_lock_exclusive().is_err() {
        return None;
    }
    Some(f)
}

fn heavy_warmup_threshold_ms() -> u64 {
    std::env::var("RUSTMODLICA_WARMUP_HEAVY_THRESHOLD_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(30_000)
}

fn heavy_warmup_max_spawns() -> u32 {
    std::env::var("RUSTMODLICA_WARMUP_HEAVY_MAX_SPAWNS")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(3)
}

/// Heavy flatten path: background L2 warmup without taking the global one-shot warmup lock.
/// Returns true if a thread was spawned.
pub fn trigger_warmup_if_heavy(
    flatten_inline_ms: u64,
    just_compiled: &str,
    loaded_paths: &[PathBuf],
    lib_paths: &[PathBuf],
) -> bool {
    if !warmup_enabled() {
        return false;
    }
    if flatten_inline_ms < heavy_warmup_threshold_ms() {
        return false;
    }
    let Some(cache_root) = crate::flatten::flatten_cache_dir() else {
        return false;
    };
    let Some(_lockfile) = try_cross_process_warmup_lock(cache_root.as_path()) else {
        return false;
    };
    let max_spawns = heavy_warmup_max_spawns();
    let prev = HEAVY_WARMUP_SPAWNS.fetch_add(1, Ordering::Relaxed);
    if prev >= max_spawns {
        let _ = HEAVY_WARMUP_SPAWNS.fetch_sub(1, Ordering::Relaxed);
        return false;
    }

    let manifest = std::env::var("RUSTMODLICA_WARMUP_MANIFEST").ok();
    let loaded = loaded_paths.to_vec();
    let libs = lib_paths.to_vec();
    let compiled = just_compiled.to_string();
    let delay = delay_ms().min(500);
    let epoch_at_spawn = warmup_control::compile_epoch_snapshot();

    match std::thread::Builder::new()
        .name("warmup-heavy".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let _hold = _lockfile;
            if delay > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            if warmup_control::compile_epoch_changed_since(epoch_at_spawn) {
                let _ = HEAVY_WARMUP_SPAWNS.fetch_sub(1, Ordering::Relaxed);
                return;
            }
            let manifest_ref = manifest.as_deref().map(Path::new);
            let candidates = derive_candidates(&loaded, manifest_ref);
            let filtered: Vec<String> = candidates
                .into_iter()
                .filter(|c| c != &compiled)
                .collect();
            if filtered.is_empty() {
                let _ = HEAVY_WARMUP_SPAWNS.fetch_sub(1, Ordering::Relaxed);
                return;
            }
            eprintln!(
                "[warmup-heavy] flatten_inline_ms={} starting L2 warmup for {} candidates",
                flatten_inline_ms,
                filtered.len()
            );
            let t0 = Instant::now();
            let mut ok = 0usize;
            let mut err = 0usize;
            let every = budget_check_every_n();
            let max_b = max_cache_bytes_soft_stop();
            for (i, model) in filtered.iter().enumerate() {
                if warmup_control::compile_epoch_changed_since(epoch_at_spawn) {
                    break;
                }
                if i > 0 && i % every == 0 {
                    let flat = crate::flatten::flatten_cache_dir();
                    let jit = crate::jit::codegen_cache::codegen_cache_root();
                    let snap = global_budget::enforce_global_budget(flat.as_deref(), jit.as_deref());
                    if snap.total_bytes > max_b.saturating_mul(110) / 100 {
                        eprintln!(
                            "[warmup-heavy] stopping: cache over budget (bytes={} max~{})",
                            snap.total_bytes, max_b
                        );
                        break;
                    }
                }
                let mut compiler = crate::Compiler::new();
                for p in &libs {
                    compiler.loader.add_path(p.clone());
                }
                compiler.options_mut().compile_stop = crate::compiler::CompileStopPhase::Analyze;
                compiler.options_mut().quiet = true;
                compiler.options_mut().warm_background = true;
                match compiler.compile(model) {
                    Ok(_) => ok += 1,
                    Err(e) => {
                        err += 1;
                        eprintln!("[warmup-heavy] failed {}: {}", model, e);
                    }
                }
            }
            eprintln!(
                "[warmup-heavy] done: {} ok, {} err out of {}",
                ok,
                err,
                filtered.len()
            );
            set_last_warmup(WarmupLastRun {
                candidates: filtered.len() as u32,
                ok: ok as u32,
                err: err as u32,
                elapsed_ms: t0.elapsed().as_millis() as u64,
                tier: "analyze".into(),
            });
            let _ = HEAVY_WARMUP_SPAWNS.fetch_sub(1, Ordering::Relaxed);
        }) {
        Ok(_) => true,
        Err(e) => {
            eprintln!("[warmup-heavy] thread spawn failed: {}", e);
            let _ = HEAVY_WARMUP_SPAWNS.fetch_sub(1, Ordering::Relaxed);
            false
        }
    }
}

/// Trigger opportunistic warmup after a successful compilation.
pub fn trigger_warmup(
    just_compiled: &str,
    loaded_paths: &[PathBuf],
    lib_paths: &[PathBuf],
) {
    trigger_warmup_with_tier(just_compiled, loaded_paths, lib_paths, warmup_tier_resolved());
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

    let Some(cache_root) = crate::flatten::flatten_cache_dir() else {
        return;
    };
    let Some(_lockfile) = try_cross_process_warmup_lock(cache_root.as_path()) else {
        return;
    };
    if WARMUP_LOCK.set(()).is_err() {
        return;
    }

    let manifest = std::env::var("RUSTMODLICA_WARMUP_MANIFEST").ok();
    let loaded = loaded_paths.to_vec();
    let libs = lib_paths.to_vec();
    let compiled = just_compiled.to_string();
    let delay = delay_ms();
    let epoch_at_spawn = warmup_control::compile_epoch_snapshot();

    if let Err(e) = std::thread::Builder::new()
        .name("warmup".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let _hold = _lockfile;
            if delay > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            if warmup_control::compile_epoch_changed_since(epoch_at_spawn) {
                return;
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

            let t0 = Instant::now();
            let stop_phase = match tier {
                WarmupTier::Analyze => crate::compiler::CompileStopPhase::Analyze,
                WarmupTier::Full => crate::compiler::CompileStopPhase::Full,
            };

            let mut ok = 0usize;
            let mut err = 0usize;
            let every = budget_check_every_n();
            let max_b = max_cache_bytes_soft_stop();
            for (i, model) in filtered.iter().enumerate() {
                if warmup_control::compile_epoch_changed_since(epoch_at_spawn) {
                    eprintln!("[warmup] cancelled: foreground compile started");
                    break;
                }
                if i > 0 && i % every == 0 {
                    let flat = crate::flatten::flatten_cache_dir();
                    let jit = crate::jit::codegen_cache::codegen_cache_root();
                    let snap = global_budget::enforce_global_budget(flat.as_deref(), jit.as_deref());
                    if snap.total_bytes > max_b.saturating_mul(110) / 100 {
                        eprintln!(
                            "[warmup] stopping: cache over budget (bytes={} max~{})",
                            snap.total_bytes, max_b
                        );
                        break;
                    }
                }
                let mut compiler = crate::Compiler::new();
                for p in &libs {
                    compiler.loader.add_path(p.clone());
                }
                compiler.options_mut().compile_stop = stop_phase.clone();
                compiler.options_mut().quiet = true;
                compiler.options_mut().warm_background = true;
                match compiler.compile(model) {
                    Ok(_) => ok += 1,
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
            set_last_warmup(WarmupLastRun {
                candidates: filtered.len() as u32,
                ok: ok as u32,
                err: err as u32,
                elapsed_ms: t0.elapsed().as_millis() as u64,
                tier: tier_label.to_string(),
            });
        })
    {
        eprintln!("[warmup] thread spawn failed: {}", e);
    }
}

/// Run full precompilation for a list of models (synchronous, parallel via rayon when len > 1).
pub fn precompile_models_parallel(
    models: &[String],
    lib_paths: &[PathBuf],
    quiet: bool,
) -> (usize, usize) {
    if models.is_empty() {
        return (0, 0);
    }
    if models.len() == 1 {
        let mut compiler = crate::Compiler::new();
        compiler.options_mut().quiet = quiet;
        compiler.options_mut().compile_stop = crate::compiler::CompileStopPhase::Full;
        for p in lib_paths {
            compiler.loader.add_path(p.clone());
        }
        return match compiler.compile(&models[0]) {
            Ok(_) => {
                if !quiet {
                    eprintln!("[precompile] OK {}", models[0]);
                }
                (1, 0)
            }
            Err(e) => {
                if !quiet {
                    eprintln!("[precompile] SKIP {} ({})", models[0], e);
                }
                (0, 1)
            }
        };
    }

    let libs = lib_paths.to_vec();
    let counts: Vec<(usize, usize)> = models
        .par_iter()
        .map(|model| {
            let mut compiler = crate::Compiler::new();
            compiler.options_mut().quiet = quiet;
            compiler.options_mut().compile_stop = crate::compiler::CompileStopPhase::Full;
            for p in &libs {
                compiler.loader.add_path(p.clone());
            }
            match compiler.compile(model) {
                Ok(_) => {
                    if !quiet {
                        eprintln!("[precompile] OK {}", model);
                    }
                    (1, 0)
                }
                Err(e) => {
                    if !quiet {
                        eprintln!("[precompile] SKIP {} ({})", model, e);
                    }
                    (0, 1)
                }
            }
        })
        .collect();
    let ok = counts.iter().map(|x| x.0).sum();
    let err = counts.iter().map(|x| x.1).sum();
    (ok, err)
}

#[cfg(test)]
mod precompile_parallel_tests {
    use super::*;

    #[test]
    fn precompile_parallel_matches_serial_counts_empty_libs() {
        let models: Vec<String> = (0..4)
            .map(|i| format!("NonexistentModelForParallelWarmup{}", i))
            .collect();
        let libs: Vec<PathBuf> = Vec::new();
        let (ok_p, err_p) = precompile_models_parallel(&models, &libs, true);
        let mut ok_s = 0usize;
        let mut err_s = 0usize;
        for m in &models {
            let mut c = crate::Compiler::new();
            c.options_mut().quiet = true;
            c.options_mut().compile_stop = crate::compiler::CompileStopPhase::Full;
            match c.compile(m) {
                Ok(_) => ok_s += 1,
                Err(_) => err_s += 1,
            }
        }
        assert_eq!((ok_p, err_p), (ok_s, err_s));
    }
}
