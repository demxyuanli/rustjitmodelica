//! Persistent per-flat-model-hash record for adaptive CONST_FOLD / EQ_DCE policy.

use crate::cache::cache_scope::CacheScope;
use crate::flatten::cache_sqlite;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

static TIERUP_SKIP_CONST_FOLD: AtomicBool = AtomicBool::new(false);

pub fn set_tierup_skip_const_fold(v: bool) {
    TIERUP_SKIP_CONST_FOLD.store(v, Ordering::Relaxed);
}

pub fn tierup_skip_const_fold() -> bool {
    TIERUP_SKIP_CONST_FOLD.load(Ordering::Relaxed)
}

const SCHEMA: &str = "fbV1";
const KIND: &str = "fold_benefit_v1";

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FoldBenefitRecord {
    pub version: u32,
    pub last_fold_count: u64,
    pub last_dce_removed: u64,
    pub last_eq_count: u64,
    pub cooldown_remaining: u32,
    pub prev_ext_resolve_ms: u64,
}

pub fn adaptive_fold_policy_enabled() -> bool {
    match std::env::var("RUSTMODLICA_ADAPTIVE_FOLD_POLICY") {
        Ok(v) => {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

pub fn fold_benefit_cache_key(flat_pipeline_hex: &str) -> String {
    format!("fold_benefit_v1:{flat_pipeline_hex}")
}

pub fn try_load(cache_root: &Path, flat_pipeline_hex: &str) -> Option<FoldBenefitRecord> {
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))?;
    let key = fold_benefit_cache_key(flat_pipeline_hex);
    let bytes = cache_sqlite::sqlite_get(&cfg.path, &key, KIND).ok()??;
    let mut r: FoldBenefitRecord = bincode::deserialize(&bytes).ok()?;
    if r.version == 0 {
        r.version = 1;
    }
    Some(r)
}

pub fn try_store(
    cache_root: &Path,
    flat_pipeline_hex: &str,
    record: &FoldBenefitRecord,
) -> Result<(), String> {
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))
        .ok_or_else(|| "no sqlite config for project scope".to_string())?;
    let key = fold_benefit_cache_key(flat_pipeline_hex);
    let bytes = bincode::serialize(record).map_err(|e| e.to_string())?;
    cache_sqlite::sqlite_put(&cfg.path, &key, SCHEMA, KIND, &bytes, None)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Update tier-up flags and optional SQLite record after a full compile.
pub fn persist_and_tierup_flags(
    adaptive: bool,
    cache_root: Option<&Path>,
    flat_hex: &str,
    prev: Option<FoldBenefitRecord>,
    started_cooldown: bool,
    skipped_policy: bool,
    cooldown_active: bool,
    const_fold_count: u64,
    eq_dce_removed: u64,
    alg_eq_count: usize,
    diff_eq_count: usize,
    external_resolve_ms: u64,
) {
    if skipped_policy || cooldown_active {
        set_tierup_skip_const_fold(true);
    }
    if !adaptive {
        return;
    }
    let Some(root) = cache_root else {
        return;
    };
    let ran = const_fold_count > 0 || eq_dce_removed > 0;
    let rec = next_record_after_compile(
        prev,
        const_fold_count,
        eq_dce_removed,
        (alg_eq_count + diff_eq_count) as u64,
        external_resolve_ms,
        started_cooldown,
        ran,
    );
    let _ = try_store(root, flat_hex, &rec);
}

/// Decide cooldown after a completed compile (external resolve wall known).
pub fn next_record_after_compile(
    prev: Option<FoldBenefitRecord>,
    fold_count: u64,
    dce_removed: u64,
    eq_count: u64,
    ext_resolve_ms: u64,
    started_in_cooldown: bool,
    const_fold_ran: bool,
) -> FoldBenefitRecord {
    let mut r = prev.unwrap_or_default();
    r.version = 1;
    r.last_fold_count = fold_count;
    r.last_dce_removed = dce_removed;
    r.last_eq_count = eq_count;

    if started_in_cooldown {
        r.cooldown_remaining = r.cooldown_remaining.saturating_sub(1);
    } else if const_fold_ran && fold_count > 0 && r.prev_ext_resolve_ms > 0 {
        let delta = ext_resolve_ms as i64 - r.prev_ext_resolve_ms as i64;
        if delta > 100 {
            r.cooldown_remaining = r.cooldown_remaining.max(5);
        }
    }

    r.prev_ext_resolve_ms = ext_resolve_ms;
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_record_cooldown_ticks() {
        let prev = Some(FoldBenefitRecord {
            version: 1,
            last_fold_count: 0,
            last_dce_removed: 0,
            last_eq_count: 10,
            cooldown_remaining: 2,
            prev_ext_resolve_ms: 1000,
        });
        let n = next_record_after_compile(prev, 0, 0, 10, 900, true, false);
        assert_eq!(n.cooldown_remaining, 1);
        assert_eq!(n.prev_ext_resolve_ms, 900);
    }

    #[test]
    fn next_record_slow_resolve_triggers_cooldown() {
        let prev = Some(FoldBenefitRecord {
            version: 1,
            last_fold_count: 5,
            last_dce_removed: 0,
            last_eq_count: 20,
            cooldown_remaining: 0,
            prev_ext_resolve_ms: 1000,
        });
        let n = next_record_after_compile(prev, 10, 0, 20, 5000, false, true);
        assert!(n.cooldown_remaining >= 5);
        assert_eq!(n.prev_ext_resolve_ms, 5000);
    }

    #[test]
    fn serde_roundtrip() {
        let r = FoldBenefitRecord {
            version: 1,
            last_fold_count: 3,
            last_dce_removed: 1,
            last_eq_count: 99,
            cooldown_remaining: 4,
            prev_ext_resolve_ms: 3333,
        };
        let b = bincode::serialize(&r).unwrap();
        let d: FoldBenefitRecord = bincode::deserialize(&b).unwrap();
        assert_eq!(r, d);
    }
}
