use std::env;
use std::path::{Path, PathBuf};

/// Resolve a project directory to auto-open on Studio startup.
/// Priority: `MODAI_STUDIO_OPEN` env, then `--project=` / `--project <path>` CLI args.
pub fn resolve_startup_project() -> Option<PathBuf> {
    if let Ok(raw) = env::var("MODAI_STUDIO_OPEN") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(path) = arg.strip_prefix("--project=") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }
        if arg == "--project" {
            if let Some(path) = args.next() {
                let trimmed = path.trim();
                if !trimmed.is_empty() {
                    return Some(PathBuf::from(trimmed));
                }
            }
        }
    }
    None
}

/// Headless smoke target: open (+ validate + simulate), or emit-only.
#[derive(Debug, Clone)]
pub enum SmokeTarget {
    /// Project directory path.
    Path(PathBuf),
    /// Bundled example id (signal_feedback, rc_circuit, ...).
    Example(String),
    /// Run every bundled example that has studio-project.json.
    All,
    /// Headless Export Suite emit (Coder + FMU CS) for a bundled example.
    EmitExample(String),
}

/// `--smoke-open=<dir>`, `--smoke-example=<id>`, `--smoke-all`,
/// or `--smoke-emit-example=<id>` (Export Suite headless gate).
pub fn resolve_smoke_target() -> Option<SmokeTarget> {
    for arg in env::args().skip(1) {
        if arg == "--smoke-all" {
            return Some(SmokeTarget::All);
        }
        if let Some(path) = arg.strip_prefix("--smoke-open=") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return Some(SmokeTarget::Path(PathBuf::from(trimmed)));
            }
        }
        if let Some(id) = arg.strip_prefix("--smoke-example=") {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                return Some(SmokeTarget::Example(trimmed.to_string()));
            }
        }
        if let Some(id) = arg.strip_prefix("--smoke-emit-example=") {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                return Some(SmokeTarget::EmitExample(trimmed.to_string()));
            }
        }
    }
    None
}

pub fn example_signal_feedback_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/signal_feedback")
}

pub fn path_display(path: &Path) -> String {
    path.display().to_string()
}
