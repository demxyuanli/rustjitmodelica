use crate::diagram::{CatalogEntry, CatalogParam, PortDef};
use regex::Regex;
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FmuScalar {
    pub name: String,
    pub value_reference: u32,
    pub causality: String,
    pub variability: String,
    pub start: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FmuImportResult {
    pub success: bool,
    pub error: Option<String>,
    pub catalog_entry: Option<CatalogEntry>,
    pub asset_dir: String,
    pub model_name: String,
    pub guid: String,
    pub model_identifier: String,
    pub fmi_version: String,
    pub has_co_simulation: bool,
    pub has_binary: bool,
    pub scalars: Vec<FmuScalar>,
}

#[derive(Debug, Clone)]
pub struct FmuDescription {
    pub model_name: String,
    pub guid: String,
    pub model_identifier: String,
    pub fmi_version: String,
    pub has_co_simulation: bool,
    pub scalars: Vec<FmuScalar>,
}

pub fn safe_id(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "fmu".into()
    } else if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("f_{out}")
    } else {
        out
    }
}

pub fn port_id_from_name(name: &str) -> String {
    safe_id(&name.replace('.', "_"))
}

pub fn extract_fmu_zip(fmu_path: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let file = fs::File::open(fmu_path).map_err(|e| format!("open fmu: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry
            .enclosed_name()
            .ok_or_else(|| format!("unsafe zip path: {}", entry.name()))?
            .to_path_buf();
        let out_path = dest.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut outfile = fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn parse_model_description(xml: &str) -> Result<FmuDescription, String> {
    let fmi_version = attr(xml, "fmiVersion").unwrap_or_else(|| "2.0".into());
    let model_name = attr(xml, "modelName").unwrap_or_else(|| "ImportedFmu".into());
    let guid = attr(xml, "guid").unwrap_or_default();
    let has_co_simulation = xml.contains("<CoSimulation");
    let model_identifier = {
        let re = Regex::new(r#"<CoSimulation[^>]*\bmodelIdentifier="([^"]+)""#).unwrap();
        re.captures(xml)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            .or_else(|| {
                let re = Regex::new(r#"<ModelExchange[^>]*\bmodelIdentifier="([^"]+)""#).unwrap();
                re.captures(xml)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            })
            .unwrap_or_else(|| safe_id(&model_name))
    };

    let var_re = Regex::new(
        r#"(?s)<ScalarVariable\b([^>]*)>(.*?)</ScalarVariable>"#,
    )
    .unwrap();
    let mut scalars = Vec::new();
    for cap in var_re.captures_iter(xml) {
        let attrs = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let body = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if !body.contains("<Real") {
            continue;
        }
        let name = attr_from(attrs, "name").unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let causality = attr_from(attrs, "causality").unwrap_or_else(|| "local".into());
        let variability = attr_from(attrs, "variability").unwrap_or_else(|| "continuous".into());
        let vr = attr_from(attrs, "valueReference")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let start = attr_from(attrs, "start")
            .or_else(|| {
                let re = Regex::new(r#"<Real[^>]*\bstart="([^"]+)""#).ok()?;
                re.captures(body)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            })
            .and_then(|s| s.parse().ok());
        scalars.push(FmuScalar {
            name,
            value_reference: vr,
            causality,
            variability,
            start,
        });
    }

    Ok(FmuDescription {
        model_name,
        guid,
        model_identifier,
        fmi_version,
        has_co_simulation,
        scalars,
    })
}

fn attr(xml: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"{name}="([^"]*)""#)).ok()?;
    re.captures(xml)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

fn attr_from(attrs: &str, name: &str) -> Option<String> {
    attr(attrs, name)
}

pub fn find_binary(asset_dir: &Path, model_identifier: &str) -> Option<PathBuf> {
    let (ext, platforms) = match std::env::consts::OS {
        "windows" => ("dll", vec!["win64", "win32"]),
        "linux" => ("so", vec!["linux64", "linux32"]),
        "macos" => ("dylib", vec!["darwin64", "darwin64"]),
        _ => return None,
    };
    for plat in platforms {
        let cand = asset_dir
            .join("binaries")
            .join(plat)
            .join(format!("{model_identifier}.{ext}"));
        if cand.exists() {
            return Some(cand);
        }
        let dir = asset_dir.join("binaries").join(plat);
        if let Ok(entries) = fs::read_dir(&dir) {
            for ent in entries.flatten() {
                let p = ent.path();
                if p.extension().and_then(|e| e.to_str()) == Some(ext) {
                    return Some(p);
                }
            }
        }
    }
    None
}

pub fn ensure_binary(asset_dir: &Path, model_identifier: &str) -> Result<PathBuf, String> {
    if let Some(p) = find_binary(asset_dir, model_identifier) {
        return Ok(p);
    }
    // Try compile from sources (Studio-exported FMUs).
    let has_model_c = asset_dir.join("model.c").exists()
        || asset_dir.join("sources").join("model.c").exists();
    if !has_model_c {
        return Err(
            "FMU has no platform binary and no model.c sources to compile".into(),
        );
    }
    let compile_dir = if asset_dir.join("model.c").exists() {
        asset_dir.to_path_buf()
    } else {
        // Flatten sources/ into a temp work dir beside asset
        let work = asset_dir.join("_studio_compile");
        if work.exists() {
            let _ = fs::remove_dir_all(&work);
        }
        fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        copy_dir_flat(&asset_dir.join("sources"), &work)?;
        for name in ["modelDescription.xml", "fmi2_cs.c", "fmi2_me.c", "model.c", "model.h"] {
            let src = asset_dir.join(name);
            if src.exists() {
                let _ = fs::copy(&src, work.join(name));
            }
        }
        // also copy nested sources files up
        work
    };
    rustmodlica::fmi::compile_fmu_shared_lib(&compile_dir, model_identifier)
}

fn copy_dir_flat(src: &Path, dest: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    for ent in fs::read_dir(src).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let p = ent.path();
        if p.is_file() {
            let name = p.file_name().unwrap();
            fs::copy(&p, dest.join(name)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn description_to_catalog(desc: &FmuDescription, asset_dir: &Path) -> CatalogEntry {
    let prefix = safe_id(&desc.model_identifier).to_lowercase();
    let mut ports = Vec::new();
    let mut params = Vec::new();
    let mut plot_hints = Vec::new();
    for s in &desc.scalars {
        match s.causality.as_str() {
            "input" => {
                ports.push(PortDef {
                    id: port_id_from_name(&s.name),
                    kind: "signal".into(),
                    direction: "in".into(),
                    type_name: "RealInput".into(),
                });
            }
            "output" => {
                let id = port_id_from_name(&s.name);
                ports.push(PortDef {
                    id: id.clone(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "RealOutput".into(),
                });
                plot_hints.push(id);
            }
            "parameter" => {
                let start = s.start.map(|v| v.to_string()).unwrap_or_else(|| "0".into());
                params.push(CatalogParam {
                    name: port_id_from_name(&s.name),
                    value: start,
                    value_type: Some("Real".into()),
                });
            }
            _ => {}
        }
    }
    CatalogEntry {
        id: format!("fmu.{}", safe_id(&desc.model_identifier)),
        type_name: format!("FMI2.CS.{}", desc.model_identifier),
        domain: "signal".into(),
        category: "Imported FMU".into(),
        display_key: format!("catalog.fmu.{}", desc.model_identifier),
        default_instance_prefix: if prefix.is_empty() {
            "fmu".into()
        } else {
            prefix
        },
        requires_library: "".into(),
        default_params: params,
        ports,
        tags: vec!["fmu".into(), "cosim".into(), "signal".into()],
        plot_hints,
        fmu_asset: Some(asset_dir.display().to_string()),
        fmu_guid: Some(desc.guid.clone()),
        fmu_model_identifier: Some(desc.model_identifier.clone()),
    }
}

pub fn import_fmu(fmu_path: &Path, project_root: Option<&Path>) -> FmuImportResult {
    let fail = |msg: String| FmuImportResult {
        success: false,
        error: Some(msg),
        catalog_entry: None,
        asset_dir: String::new(),
        model_name: String::new(),
        guid: String::new(),
        model_identifier: String::new(),
        fmi_version: String::new(),
        has_co_simulation: false,
        has_binary: false,
        scalars: Vec::new(),
    };

    if !fmu_path.exists() {
        return fail(format!("FMU not found: {}", fmu_path.display()));
    }

    let stem = fmu_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported");
    let asset_dir = if let Some(root) = project_root {
        let dir = root.join("imported_fmus").join(safe_id(stem));
        if let Err(e) = extract_fmu_zip(fmu_path, &dir) {
            return fail(e);
        }
        let _ = fs::copy(fmu_path, dir.join(format!("{stem}.fmu")));
        dir
    } else {
        let dir = std::env::temp_dir().join("modai_studio_fmu").join(safe_id(stem));
        let _ = fs::remove_dir_all(&dir);
        if let Err(e) = extract_fmu_zip(fmu_path, &dir) {
            return fail(e);
        }
        dir
    };

    let xml_path = asset_dir.join("modelDescription.xml");
    let xml = match fs::read_to_string(&xml_path) {
        Ok(t) => t,
        Err(e) => return fail(format!("missing modelDescription.xml: {e}")),
    };
    let desc = match parse_model_description(&xml) {
        Ok(d) => d,
        Err(e) => return fail(e),
    };
    if !desc.has_co_simulation {
        return fail("FMU has no CoSimulation interface (ME-only not supported yet)".into());
    }

    let has_binary = find_binary(&asset_dir, &desc.model_identifier).is_some();
    let catalog_asset = if let Some(root) = project_root {
        asset_dir
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| asset_dir.display().to_string())
    } else {
        asset_dir.display().to_string()
    };
    let mut catalog_entry = description_to_catalog(&desc, &asset_dir);
    catalog_entry.fmu_asset = Some(catalog_asset);

    FmuImportResult {
        success: true,
        error: None,
        catalog_entry: Some(catalog_entry),
        asset_dir: asset_dir.display().to_string(),
        model_name: desc.model_name,
        guid: desc.guid,
        model_identifier: desc.model_identifier,
        fmi_version: desc.fmi_version,
        has_co_simulation: true,
        has_binary,
        scalars: desc.scalars,
    }
}

pub fn list_imported_fmus(project_root: &Path) -> Vec<CatalogEntry> {
    let dir = project_root.join("imported_fmus");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ent in entries.flatten() {
        let path = ent.path();
        if !path.is_dir() {
            continue;
        }
        let xml_path = path.join("modelDescription.xml");
        let Ok(xml) = fs::read_to_string(&xml_path) else {
            continue;
        };
        let Ok(desc) = parse_model_description(&xml) else {
            continue;
        };
        if !desc.has_co_simulation {
            continue;
        }
        out.push(description_to_catalog(&desc, &path));
    }
    out
}

#[allow(dead_code)]
pub fn read_description_from_asset(asset_dir: &Path) -> Result<FmuDescription, String> {
    let xml = fs::read_to_string(asset_dir.join("modelDescription.xml"))
        .map_err(|e| format!("read modelDescription: {e}"))?;
    parse_model_description(&xml)
}

/// Load modelDescription + optional raw bytes helper for zip entry peek.
#[allow(dead_code)]
pub fn peek_fmu_xml(fmu_path: &Path) -> Result<String, String> {
    let file = fs::File::open(fmu_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut entry = archive
        .by_name("modelDescription.xml")
        .map_err(|_| "modelDescription.xml not found in FMU".to_string())?;
    let mut xml = String::new();
    entry.read_to_string(&mut xml).map_err(|e| e.to_string())?;
    Ok(xml)
}
