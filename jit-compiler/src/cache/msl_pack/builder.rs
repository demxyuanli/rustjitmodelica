//! Build an MSL pre-bake pack directory (manifest + `cache-std.sqlite`).

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::compiler::flatten_and_inline;
use crate::compiler::{CompileStopPhase, CompilerOptions};
use crate::flatten::ArraySizePolicy;
use crate::flatten::ValidationMode;
use crate::loader::ModelLoader;
use crate::query_db::{Database, QueryDb};

use super::context;
use super::hotness;
use super::leaves::{self, LeavesFile};
use super::manifest::{self, MslPackManifestV1, PackFileEntry, PACK_FORMAT_V1};
use super::tree_digest;
use super::version;

struct EnvRestore {
    key: &'static str,
    old: Option<String>,
}

impl EnvRestore {
    fn set(key: &'static str, val: &str) -> Self {
        let old = std::env::var(key).ok();
        std::env::set_var(key, val);
        Self { key, old }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        if let Some(ref v) = self.old {
            std::env::set_var(self.key, v);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

/// Populate `out_pack_dir` with `manifest.json` and `cache-std.sqlite` for `msl_root`.
pub fn bake_msl_pack(
    msl_root: &Path,
    out_pack_dir: &Path,
    curated_leaves: &LeavesFile,
    hot_json: Option<&Path>,
) -> Result<(), String> {
    fs::create_dir_all(out_pack_dir).map_err(|e| e.to_string())?;
    let tree = tree_digest::compute_msl_tree_digest(msl_root).map_err(|e| e.to_string())?;
    let ver = version::read_msl_version_label(msl_root).ok_or_else(|| {
        format!(
            "could not read MSL version from {}",
            msl_root.join("Modelica/package.mo").display()
        )
    })?;

    let std_root = out_pack_dir.join("_bake_std");
    let flat_root = out_pack_dir.join("_bake_flat");
    let _ = fs::remove_dir_all(&std_root);
    let _ = fs::remove_dir_all(&flat_root);
    fs::create_dir_all(&std_root).map_err(|e| e.to_string())?;
    fs::create_dir_all(&flat_root).map_err(|e| e.to_string())?;

    let _std_env = EnvRestore::set("RUSTMODLICA_STD_CACHE_ROOT", &std_root.to_string_lossy());
    let _flat_env = EnvRestore::set(
        "RUSTMODLICA_FLATTEN_CACHE_DIR",
        &flat_root.to_string_lossy(),
    );

    crate::flatten::cache_sqlite::sqlite_connection_pool_clear();

    context::session_activate(ver.as_str(), &tree);

    let mut loader = ModelLoader::new();
    loader.set_quiet(true);
    loader.add_path(msl_root.to_path_buf());
    loader
        .load_model("Modelica")
        .map_err(|e| format!("load Modelica: {e}"))?;

    let names: Vec<String> = loader
        .loaded_model_names()
        .into_iter()
        .filter(|n| n.starts_with("Modelica."))
        .collect();

    let mut db = Database::default();
    db.set_library_paths(Arc::new(vec![msl_root.to_path_buf()]));
    db.set_coarse_constrainedby_only(false);
    db.set_compile_stop(Arc::new("analyze".to_string()));
    let opts = CompilerOptions::default();
    let validation_mode = ValidationMode::parse(opts.validation_mode.as_str());
    db.set_validation_mode(Arc::new(format!("{validation_mode:?}")));

    for n in &names {
        let _ = db.parsed_items(n.clone());
        let _ = db.model_ast(n.clone());
    }

    let hot: Vec<String> = hot_json
        .map(|p| hotness::read_hot_models(p))
        .unwrap_or_default();
    let merged = leaves::merge_leaves(curated_leaves.clone(), &hot);

    let array_policy = ArraySizePolicy::parse(opts.array_size_policy.as_str());
    let array_sizes_path = opts.array_sizes_json.as_deref().map(std::path::Path::new);

    for leaf in &merged {
        let Ok(mut root) = loader.load_model(leaf.as_str()) else {
            continue;
        };
        let _ = flatten_and_inline(
            &mut root,
            leaf.as_str(),
            &mut loader,
            CompileStopPhase::Analyze,
            true,
            true,
            false,
            None,
            opts.coarse_constrainedby_only,
            validation_mode,
            array_policy,
            array_sizes_path,
            opts.warnings_level.as_str(),
        );
    }

    let baked_sqlite = std_root.join("std").join("cache-std.sqlite");
    if !baked_sqlite.is_file() {
        return Err(format!(
            "expected sqlite at {} after bake",
            baked_sqlite.display()
        ));
    }
    let dest_sqlite = out_pack_dir.join("cache-std.sqlite");
    fs::copy(&baked_sqlite, &dest_sqlite).map_err(|e| e.to_string())?;

    let (xxh, sz) = manifest::hash_file_xxh128(&dest_sqlite).map_err(|e| e.to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let man = MslPackManifestV1 {
        pack_format: PACK_FORMAT_V1,
        msl_version: ver.clone(),
        tree_digest: tree.clone(),
        created_unix_ms: now,
        cache_std_sqlite: PackFileEntry {
            relative_path: "cache-std.sqlite".to_string(),
            xxh128_hex: xxh,
            size_bytes: sz,
        },
    };
    manifest::write_manifest(&out_pack_dir.join("manifest.json"), &man)?;

    leaves::write_leaves_toml(&out_pack_dir.join("leaves-baked.toml"), &merged)?;

    context::session_deactivate();
    // Close handles before removing bake dirs (Windows keeps SQLite files locked).
    crate::flatten::cache_sqlite::sqlite_connection_pool_clear();
    let _ = fs::remove_dir_all(&std_root);
    let _ = fs::remove_dir_all(&flat_root);
    Ok(())
}
