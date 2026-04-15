use super::RunError;

pub(crate) fn run_cache_stats() -> Result<(), RunError> {
    let Some(dir) = rustmodlica::flatten::flatten_cache_dir() else {
        return Err(RunError::Message(
            "flatten cache disabled or unset (configure RUSTMODLICA_FLATTEN_CACHE_DIR)".into(),
        ));
    };
    let layers = rustmodlica::flatten::export_sqlite_kind_stats_layers(dir.as_path());
    let jit_dir = rustmodlica::jit::codegen_cache::codegen_cache_root();
    let mut jit_object_count = 0_u64;
    let mut jit_total_bytes = 0_u64;
    let mut jit_raw_count = 0_u64;
    let mut jit_raw_bytes = 0_u64;
    if let Some(jd) = &jit_dir {
        if jd.exists() {
            if let Ok(entries) = std::fs::read_dir(jd) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().map(|ext| ext == "bin").unwrap_or(false) {
                        if let Ok(meta) = p.metadata() {
                            jit_object_count += 1;
                            jit_total_bytes += meta.len();
                        }
                    } else if p.extension().map(|ext| ext == "rawbin").unwrap_or(false) {
                        if let Ok(meta) = p.metadata() {
                            jit_raw_count += 1;
                            jit_raw_bytes += meta.len();
                        }
                    }
                }
            }
        }
    }
    let warmup_enabled = rustmodlica::cache::warmup::warmup_enabled();
    let artifact_enabled = rustmodlica::cache::artifact_cache::artifact_cache_enabled();
    let artifact_deferred = rustmodlica::cache::artifact_cache::artifact_deferred_write();
    let flat_full_on = {
        match std::env::var("RUSTMODLICA_FLATTEN_FULL_CACHE") {
            Ok(v) => {
                let t = v.trim();
                !(t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no"))
            }
            Err(_) => true,
        }
    };
    let sqlite_on = {
        match std::env::var("RUSTMODLICA_CACHE_SQLITE") {
            Ok(v) => {
                let t = v.trim();
                !(t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no"))
            }
            Err(_) => true,
        }
    };
    let backend_dae_disk_on = {
        match std::env::var("RUSTMODLICA_BACKEND_DAE_CACHE") {
            Ok(v) => {
                let t = v.trim();
                if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                    false
                } else {
                    t == "1"
                        || t.eq_ignore_ascii_case("true")
                        || t.eq_ignore_ascii_case("yes")
                        || t.is_empty()
                }
            }
            Err(_) => true,
        }
    };
    let pipeline_analysis_disk_on = {
        match std::env::var("RUSTMODLICA_PIPELINE_ANALYSIS_CACHE") {
            Ok(v) => {
                let t = v.trim();
                if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                    false
                } else {
                    t == "1"
                        || t.eq_ignore_ascii_case("true")
                        || t.eq_ignore_ascii_case("yes")
                        || t.is_empty()
                }
            }
            Err(_) => true,
        }
    };
    let budget = rustmodlica::cache::global_budget::enforce_global_budget(
        Some(dir.as_path()),
        jit_dir.as_deref(),
    );
    let out = serde_json::json!({
        "cache_root": dir.display().to_string(),
        "jit_codegen_cache_dir": jit_dir.as_ref().map(|p| p.display().to_string()),
        "config": {
            "flatten_full_cache_enabled": flat_full_on,
            "sqlite_cache_enabled": sqlite_on,
            "artifact_cache_enabled": artifact_enabled,
            "artifact_deferred_write": artifact_deferred,
            "warmup_enabled": warmup_enabled,
            "pipeline_analysis_disk_cache_enabled": pipeline_analysis_disk_on,
            "backend_dae_disk_cache_enabled": backend_dae_disk_on,
        },
        "sqlite_per_scope_kind": layers,
        "jit_codegen_cache": {
            "object_count": jit_object_count,
            "object_total_bytes": jit_total_bytes,
            "raw_count": jit_raw_count,
            "raw_total_bytes": jit_raw_bytes,
        },
        "summary": {
            "total_sqlite_kinds": layers.iter()
                .map(|layer| layer.rows.len())
                .sum::<usize>(),
            "total_sqlite_bytes": layers.iter()
                .map(|layer| {
                    layer
                        .rows
                        .iter()
                        .filter_map(|r| u64::try_from(r.bytes_put).ok())
                        .sum::<u64>()
                })
                .sum::<u64>(),
            "total_jit_files": jit_object_count + jit_raw_count,
            "total_jit_bytes": jit_total_bytes + jit_raw_bytes,
        },
        "global_budget": {
            "total_bytes": budget.total_bytes,
            "max_bytes": budget.max_bytes,
            "jit_file_bytes": budget.jit_file_bytes,
            "sqlite_file_bytes": budget.sqlite_file_bytes,
            "files_scanned": budget.files_scanned,
            "files_evicted": budget.files_evicted,
            "bytes_evicted": budget.bytes_evicted,
        }
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    Ok(())
}
