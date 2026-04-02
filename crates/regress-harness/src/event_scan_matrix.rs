use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::io::Write;

fn sha256_file(path: &Path) -> Result<String> {
    let data = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut h = Sha256::new();
    h.update(&data);
    Ok(format!("{:x}", h.finalize()))
}

fn features_from_env() -> Vec<String> {
    let raw = std::env::var("RUSTMODLICA_CARGO_FEATURES").unwrap_or_else(|_| "sundials".to_string());
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn run_event_scan_once(
    repo_root: &Path,
    manifest: &Path,
    features: &[String],
    model: &str,
    count_deadband: &str,
    tail_deadband: &str,
    top_n: i32,
    out_file: &Path,
    lib_paths: &[String],
) -> Result<i32> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo_root);
    cmd.arg("run")
        .arg("-p")
        .arg("rustmodlica")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--release");
    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
    }
    cmd.arg("--");
    cmd.arg("event-scan")
        .arg(format!("--model={model}"))
        .arg(format!("--count-values={count_deadband}"))
        .arg(format!("--tail-velocity-values={tail_deadband}"))
        .arg(format!("--top-n={top_n}"))
        .arg("--aggregate-report=full")
        .arg(format!("--output-file={}", out_file.display()));
    for lp in lib_paths {
        cmd.arg(format!("--lib-path={lp}"));
    }
    let st = cmd.status()?;
    Ok(st.code().unwrap_or(1))
}

#[derive(Debug, Clone)]
pub struct EventScanMatrixArgs {
    pub out_dir: PathBuf,
    pub models: Vec<String>,
    pub count_values: Vec<String>,
    pub tail_velocity_values: Vec<String>,
    pub lib_paths: Vec<String>,
    pub top_n: i32,
    pub allow_unsupported: bool,
}

pub fn run_event_scan_matrix(repo_root: &Path, args: &EventScanMatrixArgs) -> Result<bool> {
    if args.lib_paths.is_empty() {
        bail!("lib_paths is empty");
    }
    let manifest = repo_root.join("jit-compiler").join("Cargo.toml");
    if !manifest.exists() {
        bail!("missing manifest: {}", manifest.display());
    }
    fs::create_dir_all(&args.out_dir)?;
    let out_path = repo_root.join(&args.out_dir);
    fs::create_dir_all(&out_path)?;

    let features = features_from_env();

    let csv_path = out_path.join("deadband_matrix_stability.csv");
    fs::write(
        &csv_path,
        "model,count_deadband,tail_deadband,run1_hash,run2_hash,status,reason\n",
    )?;

    let mut unsupported: Vec<String> = Vec::new();
    let mut config_errors: Vec<String> = Vec::new();

    for m in &args.models {
        for c in &args.count_values {
            for tv in &args.tail_velocity_values {
                let safe = m
                    .chars()
                    .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-' { ch } else { '_' })
                    .collect::<String>();
                let a = out_path.join(format!("event_{safe}_{c}_{tv}_a.json"));
                let b = out_path.join(format!("event_{safe}_{c}_{tv}_b.json"));

                let e1 = run_event_scan_once(
                    repo_root,
                    &manifest,
                    &features,
                    m,
                    c,
                    tv,
                    args.top_n,
                    &a,
                    &args.lib_paths,
                )?;
                let e2 = run_event_scan_once(
                    repo_root,
                    &manifest,
                    &features,
                    m,
                    c,
                    tv,
                    args.top_n,
                    &b,
                    &args.lib_paths,
                )?;

                if e1 != 0 || e2 != 0 || !a.exists() || !b.exists() {
                    fs::OpenOptions::new()
                        .append(true)
                        .open(&csv_path)?
                        .write_all(format!("{m},{c},{tv},,,error,process_failed\n").as_bytes())?;
                    config_errors.push(format!("{m} c={c} tv={tv} reason=process_failed"));
                    continue;
                }

                let json_a: serde_json::Value = serde_json::from_str(&fs::read_to_string(&a)?)?;
                let json_b: serde_json::Value = serde_json::from_str(&fs::read_to_string(&b)?)?;
                let model_a = json_a.get("models").and_then(|x| x.as_array()).and_then(|a| a.first());
                let model_b = json_b.get("models").and_then(|x| x.as_array()).and_then(|a| a.first());
                if model_a.is_none() || model_b.is_none() {
                    fs::OpenOptions::new()
                        .append(true)
                        .open(&csv_path)?
                        .write_all(format!("{m},{c},{tv},,,error,missing_model_output\n").as_bytes())?;
                    config_errors.push(format!("{m} c={c} tv={tv} reason=missing_model_output"));
                    continue;
                }
                let model_a = model_a.unwrap();
                let model_b = model_b.unwrap();
                let status_a = model_a.get("status").and_then(|x| x.as_str()).unwrap_or("");
                let status_b = model_b.get("status").and_then(|x| x.as_str()).unwrap_or("");
                if status_a == "unsupported" || status_b == "unsupported" {
                    let reason = model_a
                        .get("unsupported_reason")
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    fs::OpenOptions::new()
                        .append(true)
                        .open(&csv_path)?
                        .write_all(format!("{m},{c},{tv},,,unsupported,{reason}\n").as_bytes())?;
                    unsupported.push(format!("{m} c={c} tv={tv} reason={reason}"));
                    continue;
                }
                if status_a == "config_error" || status_b == "config_error" {
                    let reason = model_a
                        .get("config_error")
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    fs::OpenOptions::new()
                        .append(true)
                        .open(&csv_path)?
                        .write_all(format!("{m},{c},{tv},,,config_error,{reason}\n").as_bytes())?;
                    config_errors.push(format!("{m} c={c} tv={tv} reason={reason}"));
                    continue;
                }

                let ha = sha256_file(&a)?;
                let hb = sha256_file(&b)?;
                let st = if ha == hb { "stable" } else { "nondeterministic" };
                fs::OpenOptions::new()
                    .append(true)
                    .open(&csv_path)?
                    .write_all(format!("{m},{c},{tv},{ha},{hb},{st},\n").as_bytes())?;
            }
        }
    }

    let csv_text = fs::read_to_string(&csv_path)?;
    let mut nondet = 0usize;
    let mut config_err = 0usize;
    let mut unsupported_count = 0usize;
    for line in csv_text.lines().skip(1) {
        let cols = line.split(',').collect::<Vec<_>>();
        if cols.len() < 6 {
            continue;
        }
        match cols[5] {
            "nondeterministic" => nondet += 1,
            "unsupported" => unsupported_count += 1,
            "config_error" | "error" => config_err += 1,
            _ => {}
        }
    }

    let unsupported_path = out_path.join("unsupported_models.txt");
    if unsupported.is_empty() {
        fs::write(&unsupported_path, "none\n")?;
    } else {
        fs::write(&unsupported_path, unsupported.join("\n") + "\n")?;
    }

    let report_path = out_path.join("consistency_report.txt");
    let stable = (args.models.len()
        * args.count_values.len()
        * args.tail_velocity_values.len())
        .saturating_sub(nondet + config_err + unsupported_count);
    let report = format!(
        "stable={stable}\n\
         nondeterministic={nondet}\n\
         unsupported={unsupported_count}\n\
         config_error={config_err}\n\
         csv={}\n\
         unsupported_models={}\n",
        csv_path.display(),
        unsupported_path.display()
    );
    fs::write(&report_path, report)?;

    let mut ok = nondet == 0 && config_err == 0;
    if !args.allow_unsupported && unsupported_count > 0 {
        ok = false;
    }
    Ok(ok)
}

