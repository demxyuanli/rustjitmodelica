use anyhow::{bail, Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_cargo_emit_fmu(
    repo_root: &Path,
    cargo_target_dir: &Path,
    out_dir: &Path,
    model: &str,
) -> Result<i32> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo_root);
    cmd.arg("run")
        .arg("-p")
        .arg("rustmodlica")
        .arg("--target-dir")
        .arg(cargo_target_dir)
        .arg("--")
        .arg(format!("--emit-fmu={}", out_dir.display()))
        .arg(model);
    let status = cmd.status()?;
    Ok(status.code().unwrap_or(1))
}

fn model_identifier(model: &str) -> String {
    model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .rsplit('.')
        .next()
        .unwrap_or(model)
        .to_string()
}

pub struct FmiEmitResult {
    pub ok: bool,
    pub exit_code: i32,
    pub out_dir: PathBuf,
    pub model_description: PathBuf,
    pub c_file: PathBuf,
    pub flags: String,
}

pub fn fmi_emit_fmu(
    repo_root: &Path,
    cargo_target_dir: &Path,
    out_dir: &Path,
    model: &str,
) -> Result<FmiEmitResult> {
    fs::create_dir_all(out_dir)?;
    let exit_code = run_cargo_emit_fmu(repo_root, cargo_target_dir, out_dir, model)?;
    let model_description = out_dir.join("modelDescription.xml");
    let c_file = out_dir.join("fmi2_cs.c");

    let mut ok = exit_code == 0 && model_description.exists() && c_file.exists();
    let mut flags = String::new();
    if ok {
        let text = fs::read_to_string(&model_description)
            .with_context(|| format!("read {}", model_description.display()))?;
        let has_fmi2 = text.contains(r#"fmiVersion="2.0""#);
        let has_guid = Regex::new(r#"<fmiModelDescription[^>]*\bguid="[^"]+""#)?
            .is_match(&text);
        let has_cs = text.contains("<CoSimulation");
        let mid = model_identifier(model);
        let has_model_id = text.contains(&format!(r#"modelIdentifier="{mid}""#));
        let has_real = Regex::new(r#"<Real\s*/>"#)?.is_match(&text);
        ok = ok && has_fmi2 && has_guid && has_cs && has_model_id && has_real;
        flags = format!(
            "md_fmi2={has_fmi2};md_guid={has_guid};md_cs={has_cs};md_modelId={has_model_id};md_real={has_real}"
        );
    }
    Ok(FmiEmitResult {
        ok,
        exit_code,
        out_dir: out_dir.to_path_buf(),
        model_description,
        c_file,
        flags,
    })
}

pub fn fmi_validate_dir(dir: &Path) -> Result<()> {
    let md = dir.join("modelDescription.xml");
    let c = dir.join("fmi2_cs.c");
    if !md.exists() {
        bail!("missing modelDescription.xml: {}", md.display());
    }
    if !c.exists() {
        bail!("missing fmi2_cs.c: {}", c.display());
    }
    let text = fs::read_to_string(&md)?;
    if !text.contains(r#"fmiVersion="2.0""#) {
        bail!("invalid fmiVersion");
    }
    if !Regex::new(r#"<fmiModelDescription[^>]*\bguid="[^"]+""#)?
        .is_match(&text)
    {
        bail!("missing guid");
    }
    if !text.contains("<CoSimulation") {
        bail!("missing CoSimulation");
    }
    Ok(())
}

