//! User catalog packs: reusable `.mo` libraries under `{project}/userlib/<pack>/`.

use crate::diagram::CatalogEntry;
use crate::wrap_mo::model_to_catalog;
use rustmodlica::ast::ClassItem;
use rustmodlica::parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPackManifest {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Relative `.mo` file names inside the pack directory. Empty = scan all `.mo`.
    #[serde(default)]
    pub entries: Vec<CatalogPackEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPackEntry {
    pub mo_file: String,
}

fn default_schema() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPackResult {
    pub success: bool,
    pub error: Option<String>,
    pub pack_name: String,
    pub library_path: String,
    pub entries: Vec<CatalogEntry>,
    pub copied: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPackResult {
    pub success: bool,
    pub error: Option<String>,
    pub out_dir: String,
    pub copied: u32,
}

fn safe_pack_name(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "UserLib".into()
    } else {
        out
    }
}

fn read_pack_manifest(pack_dir: &Path) -> CatalogPackManifest {
    let path = pack_dir.join("catalog.json");
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(m) = serde_json::from_str::<CatalogPackManifest>(&text) {
            return m;
        }
    }
    let name = pack_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("UserLib")
        .to_string();
    CatalogPackManifest {
        schema_version: 1,
        name,
        version: "1".into(),
        description: String::new(),
        entries: Vec::new(),
    }
}

fn mo_files_in_pack(pack_dir: &Path, manifest: &CatalogPackManifest) -> Vec<PathBuf> {
    if !manifest.entries.is_empty() {
        return manifest
            .entries
            .iter()
            .map(|e| pack_dir.join(&e.mo_file))
            .filter(|p| p.is_file())
            .collect();
    }
    let Ok(rd) = fs::read_dir(pack_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) == Some("mo") {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn entry_from_mo(mo_path: &Path, pack_name: &str, rel_asset: &str) -> Option<CatalogEntry> {
    let content = fs::read_to_string(mo_path).ok()?;
    let item = parser::parse(&content).ok()?;
    let model = match item {
        ClassItem::Model(m) if !m.is_function => m,
        _ => return None,
    };
    let stem = mo_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("block");
    let mut entry = model_to_catalog(&model, stem);
    entry.id = format!("userlib.{}.{}", safe_pack_name(pack_name), crate::wrap_mo::safe_id_pub(&model.name));
    entry.category = "UserLib".into();
    entry.requires_library = pack_name.into();
    entry.display_key = format!("catalog.userlib.{}", entry.id);
    entry.tags.retain(|t| t != "wrapped");
    entry.tags.push("userlib".into());
    entry.tags.push("modelRef".into());
    entry.tags.push(format!("asset:{rel_asset}"));
    entry.tags.push(format!("pack:{pack_name}"));
    Some(entry)
}

/// List all userlib pack catalog entries under the project.
pub fn list_user_catalog(project_root: &Path) -> Vec<CatalogEntry> {
    let root = project_root.join("userlib");
    let Ok(rd) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let pack_dir = e.path();
        if !pack_dir.is_dir() {
            continue;
        }
        let manifest = read_pack_manifest(&pack_dir);
        let pack_name = safe_pack_name(&manifest.name);
        for mo in mo_files_in_pack(&pack_dir, &manifest) {
            let file = mo
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("block.mo");
            let rel = format!("userlib/{pack_name}/{file}");
            if let Some(entry) = entry_from_mo(&mo, &pack_name, &rel) {
                out.push(entry);
            }
        }
    }
    out
}

/// Copy an external pack folder into `{project}/userlib/<name>/`.
pub fn import_catalog_pack(pack_dir: &Path, project_root: &Path) -> ImportPackResult {
    let empty = |err: String| ImportPackResult {
        success: false,
        error: Some(err),
        pack_name: String::new(),
        library_path: String::new(),
        entries: Vec::new(),
        copied: 0,
    };
    if !pack_dir.is_dir() {
        return empty("pack directory not found".into());
    }
    let manifest = read_pack_manifest(pack_dir);
    let pack_name = safe_pack_name(&manifest.name);
    let dest = project_root.join("userlib").join(&pack_name);
    if let Err(e) = fs::create_dir_all(&dest) {
        return empty(format!("create userlib dir: {e}"));
    }
    let files = mo_files_in_pack(pack_dir, &manifest);
    if files.is_empty() {
        return empty("pack has no .mo files".into());
    }
    let mut copied = 0u32;
    let mut entries = Vec::new();
    for src in &files {
        let file = src
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("block.mo");
        let dst = dest.join(file);
        if let Err(e) = fs::copy(src, &dst) {
            return empty(format!("copy {file}: {e}"));
        }
        copied += 1;
        let rel = format!("userlib/{pack_name}/{file}");
        if let Some(entry) = entry_from_mo(&dst, &pack_name, &rel) {
            entries.push(entry);
        }
    }
    let pack_manifest = CatalogPackManifest {
        schema_version: 1,
        name: pack_name.clone(),
        version: manifest.version,
        description: manifest.description,
        entries: files
            .iter()
            .filter_map(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|f| CatalogPackEntry {
                        mo_file: f.to_string(),
                    })
            })
            .collect(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&pack_manifest) {
        let _ = fs::write(dest.join("catalog.json"), json);
    }
    ImportPackResult {
        success: true,
        error: None,
        pack_name: pack_name.clone(),
        library_path: dest.display().to_string(),
        entries,
        copied,
    }
}

/// Export selected wrapped / userlib `.mo` files into a new pack directory with catalog.json.
pub fn export_catalog_pack(
    project_root: &Path,
    relative_assets: &[String],
    out_dir: &Path,
    pack_name: &str,
) -> ExportPackResult {
    let name = safe_pack_name(pack_name);
    if let Err(e) = fs::create_dir_all(out_dir) {
        return ExportPackResult {
            success: false,
            error: Some(format!("create out dir: {e}")),
            out_dir: out_dir.display().to_string(),
            copied: 0,
        };
    }
    let mut copied = 0u32;
    let mut entries = Vec::new();
    for rel in relative_assets {
        let rel = rel.trim().trim_start_matches("asset:");
        if rel.is_empty() {
            continue;
        }
        let src = project_root.join(rel);
        if !src.is_file() {
            // Also try wrapped/<file>
            let alt = project_root.join("wrapped").join(
                Path::new(rel)
                    .file_name()
                    .unwrap_or_default(),
            );
            if !alt.is_file() {
                continue;
            }
            let file = alt
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("block.mo");
            let dst = out_dir.join(file);
            if fs::copy(&alt, &dst).is_ok() {
                copied += 1;
                entries.push(CatalogPackEntry {
                    mo_file: file.to_string(),
                });
            }
            continue;
        }
        let file = src
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("block.mo");
        let dst = out_dir.join(file);
        if fs::copy(&src, &dst).is_ok() {
            copied += 1;
            entries.push(CatalogPackEntry {
                mo_file: file.to_string(),
            });
        }
    }
    if copied == 0 {
        // Fallback: export all wrapped/
        let wrapped = project_root.join("wrapped");
        if let Ok(rd) = fs::read_dir(&wrapped) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some("mo") {
                    continue;
                }
                let file = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("block.mo");
                if fs::copy(&p, out_dir.join(file)).is_ok() {
                    copied += 1;
                    entries.push(CatalogPackEntry {
                        mo_file: file.to_string(),
                    });
                }
            }
        }
    }
    let manifest = CatalogPackManifest {
        schema_version: 1,
        name: name.clone(),
        version: "1".into(),
        description: "Exported from Modai Studio".into(),
        entries,
    };
    if let Ok(json) = serde_json::to_string_pretty(&manifest) {
        let _ = fs::write(out_dir.join("catalog.json"), json);
    }
    ExportPackResult {
        success: copied > 0,
        error: if copied == 0 {
            Some("no .mo files exported".into())
        } else {
            None
        },
        out_dir: out_dir.display().to_string(),
        copied,
    }
}

/// Re-parse branding helper (kept for future list migrations).
#[allow(dead_code)]
pub fn brand_as_model_ref(mut entry: CatalogEntry, asset_rel: &str) -> CatalogEntry {
    entry.category = "ModelRef".into();
    entry.requires_library = "ModelRef".into();
    if !entry.id.starts_with("modelref.") {
        let name = entry.type_name.clone();
        entry.id = format!("modelref.{}", crate::wrap_mo::safe_id_pub(&name));
    }
    if !entry.tags.iter().any(|t| t == "modelRef") {
        entry.tags.push("modelRef".into());
    }
    if !entry.tags.iter().any(|t| t.starts_with("asset:")) {
        entry.tags.push(format!("asset:{asset_rel}"));
    }
    entry
}

/// Ensure wrapped/ and userlib/* dirs are on the library path list.
pub fn ensure_project_library_paths(project_root: &Path, paths: &[String]) -> Vec<String> {
    let mut out = paths.to_vec();
    let push_unique = |out: &mut Vec<String>, p: PathBuf| {
        if !p.is_dir() {
            return;
        }
        let s = p.display().to_string();
        if out.iter().any(|x| x == &s || PathBuf::from(x) == p) {
            return;
        }
        out.push(s);
    };
    push_unique(&mut out, project_root.join("wrapped"));
    let userlib = project_root.join("userlib");
    if let Ok(rd) = fs::read_dir(&userlib) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                push_unique(&mut out, p);
            }
        }
    }
    // Also add project root so relative loads work.
    push_unique(&mut out, project_root.to_path_buf());
    out
}

/// Load Modelica class sources for referenced types so emit is self-contained.
pub fn collect_referenced_sources(
    project_root: &Path,
    type_names: &[String],
) -> String {
    let mut out = String::new();
    let mut seen = std::collections::HashSet::new();
    for ty in type_names {
        let simple = ty.rsplit('.').next().unwrap_or(ty);
        if simple.is_empty() || !seen.insert(simple.to_string()) {
            continue;
        }
        if ty.starts_with("Modelica.") || ty.starts_with("ModelicaTest.") {
            continue;
        }
        let candidates = [
            project_root.join("wrapped").join(format!("{simple}.mo")),
            project_root.join("wrapped").join(format!("{ty}.mo")),
        ];
        let mut loaded = None;
        for c in &candidates {
            if c.is_file() {
                loaded = fs::read_to_string(c).ok();
                break;
            }
        }
        if loaded.is_none() {
            if let Ok(rd) = fs::read_dir(project_root.join("userlib")) {
                'packs: for e in rd.flatten() {
                    let pack = e.path();
                    if !pack.is_dir() {
                        continue;
                    }
                    let c = pack.join(format!("{simple}.mo"));
                    if c.is_file() {
                        loaded = fs::read_to_string(&c).ok();
                        break 'packs;
                    }
                }
            }
        }
        if let Some(src) = loaded {
            let trimmed = src.trim();
            if !trimmed.is_empty() {
                out.push_str(trimmed);
                if !trimmed.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
        }
    }
    out
}
