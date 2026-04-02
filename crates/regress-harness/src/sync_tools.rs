use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn sha256_file(path: &Path) -> Result<String> {
    let data = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut h = Sha256::new();
    h.update(&data);
    Ok(format!("{:x}", h.finalize()))
}

fn run_cargo_rustmodlica(
    repo_root: &Path,
    cargo_target_dir: &Path,
    args_after_dashdash: &[String],
    extra_env: &[(&str, &str)],
    capture_stdout: bool,
) -> Result<(i32, String)> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo_root);
    cmd.arg("run")
        .arg("-p")
        .arg("rustmodlica")
        .arg("--target-dir")
        .arg(cargo_target_dir)
        .arg("--");
    for a in args_after_dashdash {
        cmd.arg(a);
    }
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    if capture_stdout {
        let out = cmd.output()?;
        let code = out.status.code().unwrap_or(1);
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        Ok((code, text))
    } else {
        let status = cmd.status()?;
        Ok((status.code().unwrap_or(1), String::new()))
    }
}

pub struct DeterminismResult {
    pub ok: bool,
    pub exit_a: i32,
    pub exit_b: i32,
    pub csv_a: PathBuf,
    pub csv_b: PathBuf,
    pub hash_a: Option<String>,
    pub hash_b: Option<String>,
    pub wall_ms_a: u128,
    pub wall_ms_b: u128,
}

pub fn sync_determinism(
    repo_root: &Path,
    cargo_target_dir: &Path,
    model: &str,
    output_interval: f64,
    artifacts_dir: &Path,
) -> Result<DeterminismResult> {
    fs::create_dir_all(artifacts_dir)?;
    let safe_name = model.replace('/', "_").replace('.', "_");
    let csv_a = artifacts_dir.join(format!("clocked_{safe_name}_a.csv"));
    let csv_b = artifacts_dir.join(format!("clocked_{safe_name}_b.csv"));

    let args_a = vec![
        "--solver=rk4".to_string(),
        format!("--output-interval={output_interval}"),
        format!("--result-file={}", csv_a.display()),
        model.to_string(),
    ];
    let args_b = vec![
        "--solver=rk4".to_string(),
        format!("--output-interval={output_interval}"),
        format!("--result-file={}", csv_b.display()),
        model.to_string(),
    ];

    let sw_a = Instant::now();
    let (exit_a, _) = run_cargo_rustmodlica(repo_root, cargo_target_dir, &args_a, &[], false)?;
    let wall_ms_a = sw_a.elapsed().as_millis();
    let sw_b = Instant::now();
    let (exit_b, _) = run_cargo_rustmodlica(repo_root, cargo_target_dir, &args_b, &[], false)?;
    let wall_ms_b = sw_b.elapsed().as_millis();

    let mut ok = exit_a == 0 && exit_b == 0 && csv_a.exists() && csv_b.exists();
    let (hash_a, hash_b) = if ok {
        let ha = sha256_file(&csv_a).ok();
        let hb = sha256_file(&csv_b).ok();
        if let (Some(ref ha), Some(ref hb)) = (&ha, &hb) {
            ok = ha == hb;
        } else {
            ok = false;
        }
        (ha, hb)
    } else {
        (None, None)
    };

    Ok(DeterminismResult {
        ok,
        exit_a,
        exit_b,
        csv_a,
        csv_b,
        hash_a,
        hash_b,
        wall_ms_a,
        wall_ms_b,
    })
}

pub struct TraceAssertResult {
    pub ok: bool,
    pub exit_code: i32,
    pub trace_path: PathBuf,
    pub csv_path: PathBuf,
}

fn parse_csv_f64_list(s: &str) -> Result<Vec<f64>> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for part in t.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        out.push(p.parse::<f64>()?);
    }
    Ok(out)
}

pub fn sync_trace_assert(
    repo_root: &Path,
    cargo_target_dir: &Path,
    model: &str,
    expect_substr: &str,
    t_end: f64,
    expect_times: &str,
    disallow_times: &str,
    artifacts_dir: &Path,
) -> Result<TraceAssertResult> {
    if expect_substr.trim().is_empty() {
        bail!("expect_substr is empty");
    }
    fs::create_dir_all(artifacts_dir)?;
    let safe_name = model.replace('/', "_").replace('.', "_");
    let trace_path = artifacts_dir.join(format!("trace_clocked_{safe_name}.txt"));
    let csv_path = artifacts_dir.join(format!("trace_clocked_{safe_name}.csv"));

    let args = vec![
        "--solver=rk4".to_string(),
        "--dt=0.01".to_string(),
        format!("--t-end={t_end}"),
        "--output-interval=0.25".to_string(),
        format!("--result-file={}", csv_path.display()),
        model.to_string(),
    ];
    let (exit_code, out) = run_cargo_rustmodlica(
        repo_root,
        cargo_target_dir,
        &args,
        &[("RUSTMODLICA_EVENT_TRACE", "1")],
        true,
    )?;
    fs::write(&trace_path, &out)?;

    let mut ok = exit_code == 0;
    let expect_times = parse_csv_f64_list(expect_times)?;
    let disallow_times = parse_csv_f64_list(disallow_times)?;
    let substr_esc = regex::escape(expect_substr);
    let prefix = r"(?:\\[event[-_]trace\\]|event[-_]trace)";

    for t in expect_times {
        let t_str = format!("{t:.6}");
        let pat = format!(r"{prefix}\s+t={t_str}.*{substr_esc}");
        let re = regex::Regex::new(&pat)?;
        if !re.is_match(&out) {
            ok = false;
        }
    }
    for t in disallow_times {
        let t_str = format!("{t:.6}");
        let pat = format!(r"{prefix}\s+t={t_str}.*{substr_esc}");
        let re = regex::Regex::new(&pat)?;
        if re.is_match(&out) {
            ok = false;
        }
    }

    Ok(TraceAssertResult {
        ok,
        exit_code,
        trace_path,
        csv_path,
    })
}

