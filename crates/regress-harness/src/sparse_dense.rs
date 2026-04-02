use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();
    format!("{secs}")
}

fn parse_csv_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn run_rustmodlica_with_env(
    repo_root: &Path,
    cargo_target_dir: &Path,
    args_after_dashdash: &[String],
    envs: &[(&str, &str)],
) -> Result<i32> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo_root);
    cmd.arg("run")
        .arg("-p")
        .arg("rustmodlica")
        .arg("--target-dir")
        .arg(cargo_target_dir);
    cmd.arg("--");
    for a in args_after_dashdash {
        cmd.arg(a);
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let st = cmd.status()?;
    Ok(st.code().unwrap_or(1))
}

fn read_compile_perf(perf_path: &Path) -> Option<Value> {
    let text = fs::read_to_string(perf_path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v.get("compile_perf").cloned()
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchRow {
    pub model: String,
    pub path_preference: String,
    pub exit_code: i32,
    pub status: String,
    pub wall_ms: i128,
    pub compile_jit_ms: Option<i64>,
    pub blt_guard_triggered: Option<bool>,
    pub perf_json: String,
    pub result_csv: String,
}

pub struct BenchOutput {
    pub csv_path: PathBuf,
    pub json_path: PathBuf,
    #[allow(dead_code)]
    pub rows: Vec<BenchRow>,
}

pub fn bench_sparse_dense(
    repo_root: &Path,
    models: &[String],
    t_end: f64,
    dt: f64,
    warnings: &str,
    out_dir: &Path,
    use_release: bool,
) -> Result<BenchOutput> {
    if models.is_empty() {
        bail!("models is empty");
    }
    fs::create_dir_all(out_dir)?;
    let stamp = now_stamp();
    let csv_path = out_dir.join(format!("sparse_dense_{stamp}.csv"));
    let json_path = out_dir.join(format!("sparse_dense_{stamp}.json"));

    let mut rows = Vec::<BenchRow>::new();

    for m in models {
        for pref in ["dense", "sparse"] {
            let safe = m
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
                .collect::<String>();
            let perf_path = out_dir.join(format!("perf_{safe}_{pref}_{stamp}.json"));
            let result_path = out_dir.join(format!("result_{safe}_{pref}_{stamp}.csv"));
            let cargo_target_dir = repo_root.join(format!("target_bench_{pref}"));

            let args = vec![
                format!("--warnings={warnings}"),
                format!("--perf-json={}", perf_path.display()),
                format!("--t-end={t_end}"),
                format!("--dt={dt}"),
                format!("--result-file={}", result_path.display()),
                m.to_string(),
            ];
            if use_release {
                // keep release behavior via env: actual cargo release flag is outside `--`.
                // For now, rely on target dir separation and caller using release build in CI.
            }

            let start = std::time::Instant::now();
            let exit_code = run_rustmodlica_with_env(
                repo_root,
                &cargo_target_dir,
                &args,
                &[("RUSTMODLICA_NEWTON_PATH", pref), ("RUSTMODLICA_NEWTON_PATH_TRACE", "1")],
            )?;
            let wall_ms = start.elapsed().as_millis() as i128;

            let status = if exit_code == 0 { "OK" } else { "BAD" }.to_string();
            let cp = read_compile_perf(&perf_path);
            let compile_jit_ms = cp
                .as_ref()
                .and_then(|v| v.get("jit_ms"))
                .and_then(|x| x.as_i64());
            let blt_guard_triggered = cp
                .as_ref()
                .and_then(|v| v.get("blt_degrade_guard_triggered"))
                .and_then(|x| x.as_bool());

            rows.push(BenchRow {
                model: m.to_string(),
                path_preference: pref.to_string(),
                exit_code,
                status,
                wall_ms,
                compile_jit_ms,
                blt_guard_triggered,
                perf_json: perf_path.to_string_lossy().to_string(),
                result_csv: result_path.to_string_lossy().to_string(),
            });
        }
    }

    let mut csv = String::new();
    csv.push_str("model,path_preference,exit_code,status,wall_ms,compile_jit_ms,blt_degrade_guard_triggered,perf_json,result_csv\n");
    for r in &rows {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            r.model,
            r.path_preference,
            r.exit_code,
            r.status,
            r.wall_ms,
            r.compile_jit_ms
                .map(|v| v.to_string())
                .unwrap_or_else(|| "".to_string()),
            r.blt_guard_triggered
                .map(|v| v.to_string())
                .unwrap_or_else(|| "".to_string()),
            r.perf_json,
            r.result_csv
        ));
    }
    fs::write(&csv_path, csv)?;
    fs::write(&json_path, serde_json::to_string_pretty(&rows)?)?;

    Ok(BenchOutput {
        csv_path,
        json_path,
        rows,
    })
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        Some(v[n / 2])
    } else {
        Some((v[n / 2 - 1] + v[n / 2]) / 2.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SummaryRow {
    pub model: String,
    pub path_preference: String,
    pub sample_count: usize,
    pub wall_ms_median: f64,
    pub compile_jit_ms_median: f64,
    pub blt_guard_filter: String,
}

pub struct SummaryOutput {
    pub csv_path: PathBuf,
    pub json_path: PathBuf,
    #[allow(dead_code)]
    pub rows: Vec<SummaryRow>,
}

pub fn summarize_sparse_dense(
    input_dir: &Path,
    output_dir: &Path,
    blt_guard_filter: &str,
    model_filter: &[String],
) -> Result<SummaryOutput> {
    if !input_dir.exists() {
        bail!("input dir not found: {}", input_dir.display());
    }
    fs::create_dir_all(output_dir)?;

    let mut samples: Vec<BenchRow> = Vec::new();
    for entry in fs::read_dir(input_dir)? {
        let p = entry?.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("sparse_dense_") || !name.ends_with(".csv") {
            continue;
        }
        let text = fs::read_to_string(&p)?;
        let mut it = text.lines();
        let header = it.next().unwrap_or("");
        if !header.contains("model") {
            continue;
        }
        for line in it {
            let cols = line.split(',').map(|s| s.trim()).collect::<Vec<_>>();
            if cols.len() < 9 {
                continue;
            }
            let model = cols[0].to_string();
            if !model_filter.is_empty() && !model_filter.contains(&model) {
                continue;
            }
            let status = cols[3].to_string();
            if status != "OK" {
                continue;
            }
            let path_preference = cols[1].to_string();
            let wall_ms = cols[4].parse::<i128>().unwrap_or(-1);
            let compile_jit_ms = cols[5].parse::<i64>().ok();
            let blt_guard_triggered = cols[6].parse::<bool>().ok();

            if blt_guard_filter == "non_triggered" && blt_guard_triggered.unwrap_or(true) {
                continue;
            }
            if blt_guard_filter == "triggered" && !blt_guard_triggered.unwrap_or(false) {
                continue;
            }

            samples.push(BenchRow {
                model,
                path_preference,
                exit_code: cols[2].parse::<i32>().unwrap_or(1),
                status,
                wall_ms,
                compile_jit_ms,
                blt_guard_triggered,
                perf_json: cols[7].to_string(),
                result_csv: cols[8].to_string(),
            });
        }
    }
    if samples.is_empty() {
        bail!("no samples found");
    }

    let mut grouped: HashMap<(String, String), Vec<BenchRow>> = HashMap::new();
    for s in samples {
        grouped
            .entry((s.model.clone(), s.path_preference.clone()))
            .or_default()
            .push(s);
    }

    let mut out_rows = Vec::<SummaryRow>::new();
    for ((model, pref), group) in grouped {
        let wall_vals = group
            .iter()
            .filter_map(|r| if r.wall_ms >= 0 { Some(r.wall_ms as f64) } else { None })
            .collect::<Vec<_>>();
        let jit_vals = group
            .iter()
            .filter_map(|r| r.compile_jit_ms.map(|v| v as f64))
            .collect::<Vec<_>>();
        let wall_med = median(wall_vals).unwrap_or(0.0);
        let jit_med = median(jit_vals).unwrap_or(0.0);
        out_rows.push(SummaryRow {
            model,
            path_preference: pref,
            sample_count: group.len(),
            wall_ms_median: (wall_med * 1000.0).round() / 1000.0,
            compile_jit_ms_median: (jit_med * 1000.0).round() / 1000.0,
            blt_guard_filter: blt_guard_filter.to_string(),
        });
    }
    out_rows.sort_by(|a, b| a.model.cmp(&b.model).then(a.path_preference.cmp(&b.path_preference)));

    let stamp = now_stamp();
    let csv_path = output_dir.join(format!("sparse_dense_summary_{stamp}.csv"));
    let json_path = output_dir.join(format!("sparse_dense_summary_{stamp}.json"));

    let mut csv = String::new();
    csv.push_str("model,path_preference,sample_count,wall_ms_median,compile_jit_ms_median,blt_guard_filter\n");
    for r in &out_rows {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            r.model,
            r.path_preference,
            r.sample_count,
            r.wall_ms_median,
            r.compile_jit_ms_median,
            r.blt_guard_filter
        ));
    }
    fs::write(&csv_path, csv)?;
    fs::write(&json_path, serde_json::to_string_pretty(&out_rows)?)?;

    Ok(SummaryOutput {
        csv_path,
        json_path,
        rows: out_rows,
    })
}

pub fn parse_models_arg(models_arg: &str) -> Vec<String> {
    if models_arg.trim().is_empty() {
        vec![
            "TestLib/SolvableBlock4Res".to_string(),
            "TestLib/ClockedPartitionTest".to_string(),
        ]
    } else {
        parse_csv_list(models_arg)
    }
}

