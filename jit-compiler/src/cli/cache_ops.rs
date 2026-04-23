//! `rustmodlica cache <subcommand>` — tiered warmup / bake / import / export.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::RunError;
use rustmodlica::cache::cache_scope::CacheScope;
use rustmodlica::cache::ir_epoch;
use rustmodlica::cache::warmup;
use rustmodlica::flatten::{flatten_cache_dir, std_cache_root, user_cache_root};

#[derive(Serialize)]
struct TierManifest {
    scope: String,
    compiler_version: String,
    ir_schema_epoch: u32,
    host_triple: String,
    model_count: usize,
    approx_bytes: u64,
}

fn parse_scope(s: &str) -> Result<CacheScope, RunError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "std" | "l0" => Ok(CacheScope::GlobalStd),
        "user" | "l1" => Ok(CacheScope::UserExt),
        "project" | "l2" => Ok(CacheScope::Project),
        _ => Err(RunError::Message(
            "unknown --scope (use std|user|project)".into(),
        )),
    }
}

fn apply_root_override(scope: CacheScope, root: &Path) -> Result<(), RunError> {
    let s = root.to_str().ok_or_else(|| RunError::Message("cache root not UTF-8".into()))?;
    match scope {
        CacheScope::GlobalStd => std::env::set_var("RUSTMODLICA_STD_CACHE_ROOT", s),
        CacheScope::UserExt => std::env::set_var("RUSTMODLICA_USER_CACHE_ROOT", s),
        CacheScope::Project => std::env::set_var("RUSTMODLICA_FLATTEN_CACHE_DIR", s),
    }
    Ok(())
}

fn resolve_tier_root(scope: CacheScope) -> Result<PathBuf, RunError> {
    match scope {
        CacheScope::GlobalStd => std_cache_root().ok_or_else(|| {
            RunError::Message("no std cache root (set RUSTMODLICA_STD_CACHE_ROOT)".into())
        }),
        CacheScope::UserExt => user_cache_root().ok_or_else(|| {
            RunError::Message("no user cache root (set RUSTMODLICA_USER_CACHE_ROOT)".into())
        }),
        CacheScope::Project => flatten_cache_dir().ok_or_else(|| {
            RunError::Message("no project cache (set RUSTMODLICA_FLATTEN_CACHE_DIR)".into())
        }),
    }
}

fn tier_lock_label(scope: CacheScope) -> &'static str {
    match scope {
        CacheScope::GlobalStd => "std",
        CacheScope::UserExt => "user",
        CacheScope::Project => "project",
    }
}

fn read_models_json(path: &str) -> Result<Vec<String>, RunError> {
    let text =
        fs::read_to_string(path).map_err(|e| RunError::Message(format!("read '{}': {}", path, e)))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| RunError::Message(format!("json: {}", e)))?;
    let models: Vec<String> = if let Some(a) = v.as_array() {
        a.iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect()
    } else if let Some(a) = v.get("models").and_then(|x| x.as_array()) {
        a.iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        return Err(RunError::Message(
            "models file must be [\"A.B\", ...] or {\"models\":[\"A.B\"]}".into(),
        ));
    };
    if models.is_empty() {
        return Err(RunError::Message("models list is empty".into()));
    }
    Ok(models)
}

fn parse_kv_args(argv: &[String]) -> Result<(CacheScope, Option<PathBuf>, String, Vec<String>), RunError> {
    let mut scope: Option<CacheScope> = None;
    let mut root: Option<PathBuf> = None;
    let mut input: Option<String> = None;
    let mut libs: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < argv.len() {
        let a = &argv[i];
        if let Some(v) = a.strip_prefix("--scope=") {
            scope = Some(parse_scope(v)?);
            i += 1;
        } else if a == "--scope" && i + 1 < argv.len() {
            scope = Some(parse_scope(&argv[i + 1])?);
            i += 2;
        } else if let Some(v) = a.strip_prefix("--root=") {
            root = Some(PathBuf::from(v));
            i += 1;
        } else if a == "--root" && i + 1 < argv.len() {
            root = Some(PathBuf::from(&argv[i + 1]));
            i += 2;
        } else if let Some(v) = a.strip_prefix("--input=") {
            input = Some(v.to_string());
            i += 1;
        } else if a == "--input" && i + 1 < argv.len() {
            input = Some(argv[i + 1].clone());
            i += 2;
        } else if let Some(v) = a.strip_prefix("--lib-path=") {
            libs.push(v.to_string());
            i += 1;
        } else if a == "--lib-path" && i + 1 < argv.len() {
            libs.push(argv[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }
    let scope = scope.ok_or_else(|| RunError::Message("missing --scope std|user|project".into()))?;
    let input = input.ok_or_else(|| RunError::Message("missing --input <models.json>".into()))?;
    Ok((scope, root, input, libs))
}

fn dir_size_bytes(root: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(md) = e.metadata() else {
                continue;
            };
            if md.is_file() {
                total = total.saturating_add(md.len());
            } else if md.is_dir() {
                stack.push(p);
            }
        }
    }
    total
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), RunError> {
    fs::create_dir_all(dst).map_err(|e| RunError::Message(format!("mkdir {}: {}", dst.display(), e)))?;
    let Ok(rd) = fs::read_dir(src) else {
        return Ok(());
    };
    for e in rd.flatten() {
        let sp = e.path();
        let name = sp.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let dp = dst.join(name);
        let md = e.metadata().map_err(|e| RunError::Message(e.to_string()))?;
        if md.is_dir() {
            copy_tree(&sp, &dp)?;
        } else if md.is_file() {
            if let Some(parent) = dp.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::copy(&sp, &dp).map_err(|e| {
                RunError::Message(format!("copy {} -> {}: {}", sp.display(), dp.display(), e))
            })?;
        }
    }
    Ok(())
}

fn emit_tier_manifest(scope: CacheScope, root: &Path, model_count: usize) -> Result<(), RunError> {
    let m = TierManifest {
        scope: format!("{:?}", scope),
        compiler_version: ir_epoch::COMPILER_VERSION.to_string(),
        ir_schema_epoch: ir_epoch::IR_SCHEMA_EPOCH,
        host_triple: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        model_count,
        approx_bytes: dir_size_bytes(root),
    };
    let path = root.join("manifest.json");
    let json = serde_json::to_string_pretty(&m)
        .map_err(|e| RunError::Message(format!("manifest json: {}", e)))?;
    fs::write(&path, json).map_err(|e| RunError::Message(format!("write {}: {}", path.display(), e)))?;
    Ok(())
}

fn run_warmup_or_bake(argv: &[String], write_manifest: bool) -> Result<(), RunError> {
    let (scope, root_override, input, lib_paths) = parse_kv_args(argv)?;
    if let Some(ref r) = root_override {
        apply_root_override(scope, r)?;
    }
    let tier_root = resolve_tier_root(scope)?;
    let _lock = warmup::try_warmup_exclusive_lock(&tier_root, tier_lock_label(scope)).ok_or_else(|| {
        RunError::Message(format!(
            "could not lock warmup for {} (another process holds .warmup-*.lock?)",
            tier_root.display()
        ))
    })?;
    let models = read_models_json(&input)?;
    let path_bufs: Vec<PathBuf> = lib_paths.iter().map(PathBuf::from).collect();
    let (ok, err) = warmup::precompile_models_parallel(&models, &path_bufs, false);
    eprintln!("[cache] precompile done: ok={} err={}", ok, err);
    if write_manifest {
        emit_tier_manifest(scope, &tier_root, models.len())?;
        eprintln!(
            "[cache] wrote {}",
            tier_root.join("manifest.json").display()
        );
    }
    Ok(())
}

fn run_import(argv: &[String]) -> Result<(), RunError> {
    let mut scope: Option<CacheScope> = None;
    let mut from: Option<PathBuf> = None;
    let mut i = 0usize;
    while i < argv.len() {
        let a = &argv[i];
        if let Some(v) = a.strip_prefix("--scope=") {
            scope = Some(parse_scope(v)?);
            i += 1;
        } else if a == "--scope" && i + 1 < argv.len() {
            scope = Some(parse_scope(&argv[i + 1])?);
            i += 2;
        } else if let Some(v) = a.strip_prefix("--from=") {
            from = Some(PathBuf::from(v));
            i += 1;
        } else if a == "--from" && i + 1 < argv.len() {
            from = Some(PathBuf::from(&argv[i + 1]));
            i += 2;
        } else {
            i += 1;
        }
    }
    let scope = scope.ok_or_else(|| RunError::Message("missing --scope".into()))?;
    let src = from.ok_or_else(|| RunError::Message("missing --from <dir>".into()))?;
    if !src.is_dir() {
        return Err(RunError::Message(format!(
            "--from not a directory: {}",
            src.display()
        )));
    }
    let dst = resolve_tier_root(scope)?;
    copy_tree(&src, &dst)?;
    eprintln!("[cache] import {} -> {}", src.display(), dst.display());
    Ok(())
}

fn run_export(argv: &[String]) -> Result<(), RunError> {
    let mut scope: Option<CacheScope> = None;
    let mut to: Option<PathBuf> = None;
    let mut i = 0usize;
    while i < argv.len() {
        let a = &argv[i];
        if let Some(v) = a.strip_prefix("--scope=") {
            scope = Some(parse_scope(v)?);
            i += 1;
        } else if a == "--scope" && i + 1 < argv.len() {
            scope = Some(parse_scope(&argv[i + 1])?);
            i += 2;
        } else if let Some(v) = a.strip_prefix("--to=") {
            to = Some(PathBuf::from(v));
            i += 1;
        } else if a == "--to" && i + 1 < argv.len() {
            to = Some(PathBuf::from(&argv[i + 1]));
            i += 2;
        } else {
            i += 1;
        }
    }
    let scope = scope.ok_or_else(|| RunError::Message("missing --scope".into()))?;
    let dst = to.ok_or_else(|| RunError::Message("missing --to <dir>".into()))?;
    let src = resolve_tier_root(scope)?;
    fs::create_dir_all(&dst).map_err(|e| RunError::Message(e.to_string()))?;
    copy_tree(&src, &dst)?;
    emit_tier_manifest(scope, &dst, 0)?;
    eprintln!("[cache] export {} -> {}", src.display(), dst.display());
    Ok(())
}

/// `args` is full argv; expects `args[1] == "cache"`.
pub(crate) fn run_cache_command(args: &[String]) -> Result<(), RunError> {
    let sub = args
        .get(2)
        .map(|s| s.as_str())
        .ok_or_else(|| {
            RunError::Message(
                "usage: rustmodlica cache warmup|bake --scope std|user|project --input <models.json> [--root <dir>] [--lib-path <dir>]\n       rustmodlica cache import --scope ... --from <dir>\n       rustmodlica cache export --scope ... --to <dir>"
                    .into(),
            )
        })?;
    let tail = args.get(3..).unwrap_or(&[]).to_vec();
    match sub {
        "warmup" => run_warmup_or_bake(&tail, false),
        "bake" => run_warmup_or_bake(&tail, true),
        "import" => run_import(&tail),
        "export" => run_export(&tail),
        _ => Err(RunError::Message(format!(
            "unknown cache subcommand '{}'",
            sub
        ))),
    }
}
