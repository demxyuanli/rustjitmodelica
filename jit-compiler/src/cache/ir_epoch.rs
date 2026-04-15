//! Per-stage IR schema epochs for fine-grained cache invalidation.
//!
//! Each compilation stage has its own epoch version. When the IR schema for a specific stage
//! changes, only that stage's epoch needs to be incremented, preserving caches for other stages.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Compiler version from Cargo.toml.
pub const COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Cache stages corresponding to the compilation pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CacheStage {
    Parse,
    ModelAst,
    Inheritance,
    DeclExpand,
    EqExpand,
    ConstrainedBy,
    FlatModelQ,
    FlatFull,
    ArraySizes,
}

impl CacheStage {
    /// Returns the cache key tag for this stage (used in cache key generation).
    pub fn tag(&self) -> &'static str {
        match self {
            CacheStage::Parse => "parse_v2",
            CacheStage::ModelAst => "model_ast_v2",
            CacheStage::Inheritance => "inheritance_v2",
            CacheStage::DeclExpand => "decl_expand_v2",
            CacheStage::EqExpand => "eq_expand_v2",
            CacheStage::ConstrainedBy => "constrainedby_v2",
            CacheStage::FlatModelQ => "flat_model_q_v2",
            CacheStage::FlatFull => "flat_full_v2",
            CacheStage::ArraySizes => "array_sizes_v3",
        }
    }

    /// Parse a stage from its tag string.
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "parse_v2" => Some(CacheStage::Parse),
            "model_ast_v2" => Some(CacheStage::ModelAst),
            "inheritance_v2" => Some(CacheStage::Inheritance),
            "decl_expand_v2" => Some(CacheStage::DeclExpand),
            "eq_expand_v2" => Some(CacheStage::EqExpand),
            "constrainedby_v2" => Some(CacheStage::ConstrainedBy),
            "flat_model_q_v2" => Some(CacheStage::FlatModelQ),
            "flat_full_v2" => Some(CacheStage::FlatFull),
            "array_sizes_v3" => Some(CacheStage::ArraySizes),
            _ => None,
        }
    }

    /// Short name for stamp file entries (without _v2 suffix).
    fn stamp_key(&self) -> &'static str {
        match self {
            CacheStage::Parse => "parse",
            CacheStage::ModelAst => "model_ast",
            CacheStage::Inheritance => "inheritance",
            CacheStage::DeclExpand => "decl_expand",
            CacheStage::EqExpand => "eq_expand",
            CacheStage::ConstrainedBy => "constrainedby",
            CacheStage::FlatModelQ => "flat_model_q",
            CacheStage::FlatFull => "flat_full",
            CacheStage::ArraySizes => "array_sizes",
        }
    }
}

/// Per-stage epoch configuration.
#[derive(Debug, Clone, Copy)]
pub struct StageEpoch {
    pub stage: CacheStage,
    pub epoch: u32,
}

/// Per-stage epochs. Increment the epoch for a stage when its IR schema changes.
///
/// Example: If `DeclExpand` IR structure changes, increment its epoch to 2.
/// Only `DeclExpand` and downstream stages will have their caches invalidated.
pub const STAGE_EPOCHS: &[StageEpoch] = &[
    StageEpoch { stage: CacheStage::Parse, epoch: 1 },
    StageEpoch { stage: CacheStage::ModelAst, epoch: 1 },
    StageEpoch { stage: CacheStage::Inheritance, epoch: 1 },
    StageEpoch { stage: CacheStage::DeclExpand, epoch: 1 },
    StageEpoch { stage: CacheStage::EqExpand, epoch: 1 },
    StageEpoch { stage: CacheStage::ConstrainedBy, epoch: 1 },
    StageEpoch { stage: CacheStage::FlatModelQ, epoch: 1 },
    StageEpoch { stage: CacheStage::FlatFull, epoch: 1 },
    StageEpoch { stage: CacheStage::ArraySizes, epoch: 1 },
];

/// Get the epoch for a specific cache stage.
pub fn epoch_for_stage(stage: CacheStage) -> u32 {
    STAGE_EPOCHS
        .iter()
        .find(|e| e.stage == stage)
        .map(|e| e.epoch)
        .unwrap_or(1)
}

/// Legacy single epoch for backwards compatibility with older cache formats.
/// This is equivalent to the maximum epoch across all stages.
pub const IR_SCHEMA_EPOCH: u32 = 1;

/// Stamp file name for per-stage epoch tracking in cache directory.
pub const EPOCH_STAMP_FILE: &str = "stage_epochs.txt";

/// Write current stage epochs to a stamp file in the cache directory.
pub fn write_stage_epochs_stamp(cache_root: &Path) -> std::io::Result<()> {
    let stamp_path = cache_root.join(EPOCH_STAMP_FILE);
    let mut content = String::new();
    content.push_str(&format!("compiler_version={}\n", COMPILER_VERSION));
    for se in STAGE_EPOCHS {
        content.push_str(&format!("{}={}\n", se.stage.stamp_key(), se.epoch));
    }
    std::fs::write(&stamp_path, content)
}

/// Check if any stage epoch has changed compared to the stamp file.
/// Returns a list of stages that need cache invalidation.
pub fn check_stage_epochs_stamp(cache_root: &Path) -> Vec<CacheStage> {
    let stamp_path = cache_root.join(EPOCH_STAMP_FILE);
    let Ok(content) = std::fs::read_to_string(&stamp_path) else {
        // No stamp file = first run, no invalidation needed
        return Vec::new();
    };

    let mut stamped: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("compiler_version=") {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if let Ok(epoch) = value.parse::<u32>() {
                stamped.insert(key.to_string(), epoch);
            }
        }
    }

    let mut invalidate: Vec<CacheStage> = Vec::new();
    for se in STAGE_EPOCHS {
        let key = se.stage.stamp_key();
        if let Some(&stamped_epoch) = stamped.get(key) {
            if stamped_epoch != se.epoch {
                invalidate.push(se.stage);
            }
        }
    }
    invalidate
}

/// When `stage_epochs.txt` disagrees with [`STAGE_EPOCHS`], delete matching rows from all
/// scope SQLite caches, evict overlapping entries from the in-memory hot flatten cache, clear
/// the connection pool, then rewrite the stamp file.
pub fn apply_stage_epoch_drift(cache_root: &Path) -> std::io::Result<()> {
    use crate::cache::cache_scope::CacheScope;

    let drift = check_stage_epochs_stamp(cache_root);
    let stamp_path = cache_root.join(EPOCH_STAMP_FILE);
    if drift.is_empty() {
        if !stamp_path.exists() {
            write_stage_epochs_stamp(cache_root)?;
        }
        return Ok(());
    }

    let needles: Vec<String> = drift.iter().map(|st| format!(":{}:", st.tag())).collect();

    for scope in [
        CacheScope::GlobalStd,
        CacheScope::UserExt,
        CacheScope::Project,
    ] {
        if let Some(cfg) =
            crate::flatten::cache_sqlite::sqlite_config_for_scope(scope, Some(cache_root))
        {
            if let Ok(conn) = rusqlite::Connection::open(&cfg.path) {
                for needle in &needles {
                    let pattern = format!("%{}%", needle);
                    let _ = conn.execute(
                        "DELETE FROM cache_entries WHERE key LIKE ?1",
                        rusqlite::params![pattern],
                    );
                }
            }
        }
    }

    crate::flatten::flatten_cache::hot_full_cache_evict_matching_needles(&needles);
    crate::flatten::cache_sqlite::sqlite_connection_pool_clear();
    write_stage_epochs_stamp(cache_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_for_stage() {
        assert_eq!(epoch_for_stage(CacheStage::Parse), 1);
        assert_eq!(epoch_for_stage(CacheStage::FlatModelQ), 1);
    }

    #[test]
    fn test_stage_tag_roundtrip() {
        for se in STAGE_EPOCHS {
            let tag = se.stage.tag();
            let parsed = CacheStage::from_tag(tag);
            assert_eq!(parsed, Some(se.stage));
        }
    }
}
