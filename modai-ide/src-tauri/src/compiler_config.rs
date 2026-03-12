// Compiler command configuration: persisted exe path + args, or auto-detect.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = "compiler.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerConfig {
    #[serde(default)]
    pub exe: String,
    #[serde(default)]
    pub args: Vec<String>,
}

fn config_path() -> Result<PathBuf, String> {
    Ok(crate::app_data::app_data_root()?.join(CONFIG_FILENAME))
}

pub fn load_config() -> Result<Option<CompilerConfig>, String> {
    let path = config_path()?;
    if !path.is_file() {
        return Ok(None);
    }
    let s = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let config: CompilerConfig = serde_json::from_str(&s).unwrap_or_default();
    Ok(Some(config))
}

pub fn save_config(config: &CompilerConfig) -> Result<(), String> {
    let path = config_path()?;
    let s = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, s).map_err(|e| e.to_string())?;
    Ok(())
}

fn find_exe(jit_compiler_root: &Path) -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = vec![
        jit_compiler_root.join("target/release/rustmodlica"),
        jit_compiler_root.join("target/debug/rustmodlica"),
    ];
    if let Some(ws) = jit_compiler_root.parent() {
        candidates.push(ws.join("target/release/rustmodlica"));
        candidates.push(ws.join("target/debug/rustmodlica"));
    }
    for p in &candidates {
        let exe = p.with_extension(std::env::consts::EXE_EXTENSION);
        if exe.exists() {
            return Ok(exe);
        }
    }
    Err("Compiler executable not found. Set path in Settings or run cargo build from repo root or jit-compiler/.".to_string())
}

/// Resolve compiler exe path and default args. If config has non-empty exe, use it; else auto-detect.
pub fn resolve_compiler_exe(jit_compiler_root: &Path) -> Result<(PathBuf, Vec<String>), String> {
    if let Ok(Some(config)) = load_config() {
        let exe_trim = config.exe.trim();
        if !exe_trim.is_empty() {
            return Ok((PathBuf::from(exe_trim), config.args.clone()));
        }
    }
    let exe = find_exe(jit_compiler_root)?;
    Ok((exe, Vec::new()))
}
