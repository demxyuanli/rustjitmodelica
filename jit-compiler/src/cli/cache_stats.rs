use super::RunError;

/// Read-only cache statistics (no eviction). Use `--cache-gc` to run budget enforcement.
pub(crate) fn run_cache_stats(args: &[String]) -> Result<(), RunError> {
    let miss_breakdown = args
        .iter()
        .any(|a| a == "--miss-breakdown" || a == "--cache-stats-miss");
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
    let mut out = serde_json::json!({
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
        "sqlite_hit_rates": hit_rates_from_layers(&layers),
        "warmup_last_run": rustmodlica::cache::warmup::peek_last_warmup_run(),
    });
    if miss_breakdown {
        let agg = rustmodlica::cache::cache_miss_agg::read_aggregate(dir.as_path());
        out["cache_miss_breakdown"] = serde_json::json!({
            "by_reason": agg.by_reason,
            "by_layer": agg.by_layer,
        });
    }
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    Ok(())
}

fn hit_rates_from_layers(layers: &[rustmodlica::flatten::CacheStatsLayerExport]) -> serde_json::Value {
    let mut per_scope = serde_json::Map::new();
    for layer in layers {
        let mut kinds = serde_json::Map::new();
        for row in &layer.rows {
            let gets = row.get_count.max(0) as f64;
            let hits = row.hit_count.max(0) as f64;
            let rate = if gets > 0.0 { hits / gets } else { 0.0 };
            kinds.insert(
                row.kind.clone(),
                serde_json::json!({
                    "get_count": row.get_count,
                    "hit_count": row.hit_count,
                    "put_count": row.put_count,
                    "hit_rate": rate,
                }),
            );
        }
        per_scope.insert(layer.tier.clone(), serde_json::Value::Object(kinds));
    }
    serde_json::Value::Object(per_scope)
}

/// Run global budget enforcement (may delete WAL sidecars and JIT files over budget/TTL).
pub(crate) fn run_cache_gc() -> Result<(), RunError> {
    let Some(dir) = rustmodlica::flatten::flatten_cache_dir() else {
        return Err(RunError::Message(
            "flatten cache disabled or unset (configure RUSTMODLICA_FLATTEN_CACHE_DIR)".into(),
        ));
    };
    let jit_dir = rustmodlica::jit::codegen_cache::codegen_cache_root();
    let budget = rustmodlica::cache::global_budget::enforce_global_budget(
        Some(dir.as_path()),
        jit_dir.as_deref(),
    );
    let out = serde_json::json!({
        "cache_root": dir.display().to_string(),
        "jit_codegen_cache_dir": jit_dir.as_ref().map(|p| p.display().to_string()),
        "global_budget": budget,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    Ok(())
}

#[cfg(test)]
mod cache_stats_shape_tests {
    use super::hit_rates_from_layers;

    #[test]
    fn hit_rates_empty_layers_object() {
        let v = hit_rates_from_layers(&[]);
        assert!(v.is_object());
    }
}
