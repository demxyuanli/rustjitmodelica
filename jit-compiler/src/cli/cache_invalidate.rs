use super::RunError;
use rustmodlica::cache::cache_selective_invalidate;
use rustmodlica::cache::ir_epoch::CacheStage;

pub(crate) fn run_cache_invalidate(args: &[String]) -> Result<(), RunError> {
    let Some(dir) = rustmodlica::flatten::flatten_cache_dir() else {
        return Err(RunError::Message(
            "flatten cache disabled or unset (configure RUSTMODLICA_FLATTEN_CACHE_DIR)".into(),
        ));
    };
    if args.len() < 3 {
        return Err(RunError::Message(
            "usage: rustmodlica --cache-invalidate <soft|hard|model> [args...]\n\
             soft --stage <tag>   e.g. flat_full_v2 or flat_full\n\
             hard --scope project|user|std|all\n\
             model <Qualified.Name>"
                .into(),
        ));
    }
    let mode = args[2].to_ascii_lowercase();
    match mode.as_str() {
        "soft" => {
            let mut stage_tag: Option<String> = None;
            let mut i = 3usize;
            while i + 1 < args.len() {
                if args[i] == "--stage" {
                    stage_tag = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            let tag = stage_tag.ok_or_else(|| {
                RunError::Message("--cache-invalidate soft requires --stage <tag>".into())
            })?;
            let stage = resolve_stage_tag(&tag).ok_or_else(|| {
                RunError::Message(format!("unknown cache stage tag: {tag}"))
            })?;
            let n = cache_selective_invalidate::soft_invalidate_stage(dir.as_path(), stage)
                .map_err(|e| RunError::Message(e))?;
            eprintln!("[cache-invalidate] soft stage={:?} deleted_rows={}", stage, n);
        }
        "hard" => {
            let mut scope = "project".to_string();
            let mut i = 3usize;
            while i + 1 < args.len() {
                if args[i] == "--scope" {
                    scope = args[i + 1].to_ascii_lowercase();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            match scope.as_str() {
                "project" => {
                    let cfg = rustmodlica::flatten::sqlite_config_for_scope(
                        rustmodlica::cache::cache_scope::CacheScope::Project,
                        Some(dir.as_path()),
                    )
                    .ok_or_else(|| RunError::Message("no project sqlite".into()))?;
                    cache_selective_invalidate::hard_invalidate_scope_db(&cfg.path)?;
                }
                "user" => {
                    let cfg = rustmodlica::flatten::sqlite_config_for_scope(
                        rustmodlica::cache::cache_scope::CacheScope::UserExt,
                        Some(dir.as_path()),
                    )
                    .ok_or_else(|| RunError::Message("no user sqlite".into()))?;
                    cache_selective_invalidate::hard_invalidate_scope_db(&cfg.path)?;
                }
                "std" => {
                    let cfg = rustmodlica::flatten::sqlite_config_for_scope(
                        rustmodlica::cache::cache_scope::CacheScope::GlobalStd,
                        Some(dir.as_path()),
                    )
                    .ok_or_else(|| RunError::Message("no std sqlite".into()))?;
                    cache_selective_invalidate::hard_invalidate_scope_db(&cfg.path)?;
                }
                "all" => {
                    for sc in [
                        rustmodlica::cache::cache_scope::CacheScope::Project,
                        rustmodlica::cache::cache_scope::CacheScope::UserExt,
                        rustmodlica::cache::cache_scope::CacheScope::GlobalStd,
                    ] {
                        if let Some(cfg) =
                            rustmodlica::flatten::sqlite_config_for_scope(
                                sc.clone(),
                                Some(dir.as_path()),
                            )
                        {
                            let _ = cache_selective_invalidate::hard_invalidate_scope_db(&cfg.path);
                        }
                    }
                }
                _ => {
                    return Err(RunError::Message(format!("unknown --scope {scope}")));
                }
            }
            eprintln!("[cache-invalidate] hard scope={scope} ok");
        }
        "model" => {
            if args.len() < 4 {
                return Err(RunError::Message(
                    "--cache-invalidate model requires <Qualified.Model.Name>".into(),
                ));
            }
            let m = &args[3];
            let n = cache_selective_invalidate::invalidate_model_sqlite(dir.as_path(), m)
                .map_err(|e| RunError::Message(e))?;
            eprintln!("[cache-invalidate] model={} deleted_rows={}", m, n);
        }
        _ => {
            return Err(RunError::Message(format!(
                "unknown invalidate mode: {mode} (use soft|hard|model)"
            )));
        }
    }
    Ok(())
}

fn resolve_stage_tag(tag: &str) -> Option<CacheStage> {
    let t = tag.trim();
    if let Some(s) = CacheStage::from_tag(t) {
        return Some(s);
    }
    match t.to_ascii_lowercase().as_str() {
        "flat_full" | "flatfull" => CacheStage::from_tag("flat_full_v2"),
        "array_sizes" | "arraysizes" => CacheStage::from_tag("array_sizes_v3"),
        "parse" => CacheStage::from_tag("parse_v2"),
        _ => None,
    }
}
