use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

#[derive(Debug, Serialize)]
pub struct MslCacheStatus {
    pub msl_version: Option<String>,
    pub tree_digest: Option<String>,
    pub msl_root: Option<String>,
    pub pack_dirs: String,
    pub matched_pack_dir: Option<String>,
    pub hotness_path: String,
}

#[derive(Debug, Deserialize)]
struct RemoteMslPackManifest {
    pub pack_format: u32,
    pub msl_version: String,
    pub tree_digest: String,
    #[serde(default)]
    pub cache_std_sqlite_url: String,
    #[serde(default)]
    pub cache_std_sqlite_sha256: String,
    #[serde(default)]
    pub pack_dir_name: String,
}

fn msl_root_from_settings() -> Result<Option<PathBuf>, String> {
    let s = crate::app_settings::load_settings()?;
    let raw = s.extensions.modelica_stdlib_path.trim().to_string();
    if raw.is_empty() {
        return Ok(None);
    }
    let p = PathBuf::from(&raw);
    let as_root = p.join("Modelica").join("package.mo");
    let as_modelica = p.join("package.mo");
    if as_root.is_file() {
        return Ok(Some(p));
    }
    if as_modelica.is_file() {
        return Ok(p.parent().map(PathBuf::from));
    }
    Ok(None)
}

#[tauri::command]
pub fn msl_cache_status() -> Result<MslCacheStatus, String> {
    let pack_dirs = std::env::var("RUSTMODLICA_MSL_PACK_DIRS").unwrap_or_default();
    let hotness_path = std::env::var("RUSTMODLICA_MSL_HOTNESS_JSON").unwrap_or_default();
    let msl_root = msl_root_from_settings()?;
    let (msl_version, tree_digest) = match &msl_root {
        Some(root) => {
            let v = rustmodlica::read_msl_version_label(root);
            let t = rustmodlica::compute_msl_tree_digest(root).ok();
            (v, t)
        }
        None => (None, None),
    };
    let matched_pack_dir = match (&msl_root, &tree_digest) {
        (Some(root), Some(td)) => find_pack_for_tree(root, td, &pack_dirs),
        _ => None,
    };
    Ok(MslCacheStatus {
        msl_version,
        tree_digest,
        msl_root: msl_root.map(|p| p.to_string_lossy().into_owned()),
        pack_dirs,
        matched_pack_dir,
        hotness_path,
    })
}

fn find_pack_for_tree(msl_root: &Path, tree: &str, pack_dirs: &str) -> Option<String> {
    for base in pack_dirs.split(';') {
        let t = base.trim();
        if t.is_empty() {
            continue;
        }
        let base = Path::new(t);
        if !base.is_dir() {
            continue;
        }
        let candidates: Vec<PathBuf> = if base.join("manifest.json").is_file() {
            vec![base.to_path_buf()]
        } else {
            let mut v = Vec::new();
            if let Ok(rd) = fs::read_dir(base) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() && p.join("manifest.json").is_file() {
                        v.push(p);
                    }
                }
            }
            v
        };
        for cand in candidates {
            let mf = cand.join("manifest.json");
            let Ok(m) = rustmodlica::read_msl_pack_manifest(&mf) else {
                continue;
            };
            if m.tree_digest == tree {
                let ver = rustmodlica::read_msl_version_label(msl_root);
                if ver.as_deref() == Some(m.msl_version.as_str()) || ver.is_none() {
                    return Some(cand.to_string_lossy().into_owned());
                }
            }
        }
    }
    None
}

#[tauri::command]
pub fn msl_cache_clear() -> Result<(), String> {
    let root = crate::app_data::app_data_root()?.join("msl-cache");
    if root.is_dir() {
        for e in fs::read_dir(&root).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let p = e.path();
            if p.file_name().and_then(|n| n.to_str()) == Some("msl-hotness.json") {
                continue;
            }
            if p.is_file() {
                let _ = fs::remove_file(&p);
            } else if p.is_dir() {
                let _ = fs::remove_dir_all(&p);
            }
        }
    }
    rustmodlica::sqlite_connection_pool_clear();
    Ok(())
}

#[tauri::command]
pub fn msl_cache_check_update() -> Result<serde_json::Value, String> {
    let s = crate::app_settings::load_settings()?;
    let url = s.extensions.msl_pack_manifest_url.trim().to_string();
    if url.is_empty() {
        return Ok(json!({ "ok": false, "reason": "no_manifest_url" }));
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let text = client.get(&url).send().map_err(|e| e.to_string())?.text().map_err(|e| e.to_string())?;
    let remote: RemoteMslPackManifest = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if remote.pack_format != 1 {
        return Err("unsupported remote pack_format".to_string());
    }
    if remote.cache_std_sqlite_url.trim().is_empty() {
        return Ok(json!({ "ok": false, "reason": "remote_missing_sqlite_url" }));
    }
    Ok(json!({
        "ok": true,
        "mslVersion": remote.msl_version,
        "treeDigest": remote.tree_digest,
        "sqliteUrl": remote.cache_std_sqlite_url,
        "sha256": remote.cache_std_sqlite_sha256,
        "packDirName": remote.pack_dir_name,
    }))
}

#[tauri::command]
pub fn msl_cache_download_update() -> Result<serde_json::Value, String> {
    let s = crate::app_settings::load_settings()?;
    let url = s.extensions.msl_pack_manifest_url.trim().to_string();
    if url.is_empty() {
        return Err("manifest URL is empty".to_string());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let text = client.get(&url).send().map_err(|e| e.to_string())?.text().map_err(|e| e.to_string())?;
    let remote: RemoteMslPackManifest = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if remote.pack_format != 1 {
        return Err("unsupported remote pack_format".to_string());
    }
    let sqlite_url = remote.cache_std_sqlite_url.trim();
    if sqlite_url.is_empty() {
        return Err("remote manifest missing cache_std_sqlite_url".to_string());
    }
    let bytes = client
        .get(sqlite_url)
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    h.update(&bytes);
    let digest = format!("{:x}", h.finalize());
    let expect = remote.cache_std_sqlite_sha256.trim();
    if !expect.is_empty() && !expect.eq_ignore_ascii_case(&digest) {
        return Err(format!("sha256 mismatch: expected {expect} got {digest}"));
    }
    let out_root = crate::app_data::app_data_root()?.join("msl-cache");
    fs::create_dir_all(&out_root).map_err(|e| e.to_string())?;
    let dir_name = if !remote.pack_dir_name.trim().is_empty() {
        remote.pack_dir_name.trim().to_string()
    } else {
        format!(
            "{}-{}",
            remote.msl_version.replace(['/', '\\'], "_"),
            &remote.tree_digest[..12.min(remote.tree_digest.len())]
        )
    };
    let pack_dir = out_root.join(&dir_name);
    let _ = fs::remove_dir_all(&pack_dir);
    fs::create_dir_all(&pack_dir).map_err(|e| e.to_string())?;
    let sqlite_path = pack_dir.join("cache-std.sqlite");
    let mut f = fs::File::create(&sqlite_path).map_err(|e| e.to_string())?;
    f.write_all(&bytes).map_err(|e| e.to_string())?;
    let manifest_path = pack_dir.join("manifest.json");
    fs::write(&manifest_path, text.as_bytes()).map_err(|e| e.to_string())?;
    rustmodlica::sqlite_connection_pool_clear();
    Ok(json!({ "ok": true, "packDir": pack_dir.to_string_lossy() }))
}

#[tauri::command]
pub fn msl_cache_rebuild_local(msl_root: String, out_name: Option<String>) -> Result<String, String> {
    let root = PathBuf::from(msl_root.trim());
    if !root.join("Modelica").join("package.mo").is_file() {
        return Err("invalid MSL root (expected Modelica/package.mo)".to_string());
    }
    let out_root = crate::app_data::app_data_root()?.join("msl-cache");
    fs::create_dir_all(&out_root).map_err(|e| e.to_string())?;
    let tree = rustmodlica::compute_msl_tree_digest(&root).map_err(|e| e.to_string())?;
    let ver = rustmodlica::read_msl_version_label(&root).ok_or_else(|| "MSL version not found".to_string())?;
    let short = tree.chars().take(12).collect::<String>();
    let dir_name = out_name.unwrap_or_else(|| {
        format!(
            "{}-{}",
            ver.replace(['/', '\\'], "_").replace(' ', "_"),
            short
        )
    });
    let pack_dir = out_root.join(dir_name);
    let _ = fs::remove_dir_all(&pack_dir);
    fs::create_dir_all(&pack_dir).map_err(|e| e.to_string())?;
    let leaves = include_str!("../../../../jit-compiler/msl-pack-bake/leaves-default.toml");
    let curated = rustmodlica::cache::msl_pack::leaves::parse_leaves_toml(leaves)
        .map_err(|e| format!("leaves: {e}"))?;
    let hot = crate::app_data::app_data_root()?.join("msl-cache").join("msl-hotness.json");
    let hot_p = if hot.is_file() {
        Some(hot.as_path())
    } else {
        None
    };
    rustmodlica::cache::msl_pack::bake_msl_pack(&root, &pack_dir, &curated, hot_p)?;
    Ok(pack_dir.to_string_lossy().into_owned())
}
