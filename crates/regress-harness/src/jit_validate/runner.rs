use crate::jit_validate::artifacts::{
    Case, CasePaths, GitInfo, HostInfo, PerfStats, RunManifest, ScenarioResolved,
    Summary, TraceFlags, ValidatePerfReport,
};
use crate::jit_validate::{ensure_cache_dir, normalize_model_list, CacheDirPolicy, RunSpec, Scenario};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn safe_slug(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else if ch == '.' {
            out.push('.');
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "case".to_string()
    } else {
        out
    }
}

fn write_json_pretty(path: &Path, v: &impl serde::Serialize) -> Result<()> {
    let text = serde_json::to_string_pretty(v)?;
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn json_get_u64(v: &serde_json::Value, key: &str) -> Option<u64> {
    v.get(key).and_then(|x| x.as_u64())
}

fn resolve_artifact_path(out_dir: &Path, p: &str) -> PathBuf {
    let raw = p.trim();
    if raw.is_empty() {
        return out_dir.to_path_buf();
    }
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() && candidate.exists() {
        return candidate;
    }
    if candidate.exists() {
        return candidate;
    }
    let joined = out_dir.join(&candidate);
    joined
}

struct ParsedCompilePerf {
    flatten_inline_ms: u64,
    flatten_wall_ms: u64,
    inline_wall_ms: u64,
    decl_expand_ms: u64,
    eq_expand_ms: u64,
    inline_substitute_ms: u64,
    inline_load_model_ms: u64,
    cache_deserialize_ms: u64,
}

fn parse_compile_perf_metrics(perf_json_path: &Path) -> Option<ParsedCompilePerf> {
    let text = std::fs::read_to_string(perf_json_path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let c = v.get("compile_perf")?;
    let u = |key: &str| json_get_u64(c, key).unwrap_or(0);
    Some(ParsedCompilePerf {
        flatten_inline_ms: json_get_u64(c, "flatten_inline_ms")?,
        flatten_wall_ms: u("flatten_wall_ms"),
        inline_wall_ms: u("inline_wall_ms"),
        decl_expand_ms: u("decl_expand_ms"),
        eq_expand_ms: u("eq_expand_ms"),
        inline_substitute_ms: u("inline_substitute_ms"),
        inline_load_model_ms: u("inline_load_model_ms"),
        cache_deserialize_ms: u("cache_deserialize_us") / 1000,
    })
}

fn update_min_max(opt_min: &mut Option<u64>, opt_max: &mut Option<u64>, v: u64) {
    *opt_min = Some(opt_min.map(|x| x.min(v)).unwrap_or(v));
    *opt_max = Some(opt_max.map(|x| x.max(v)).unwrap_or(v));
}

fn build_perf_stats(out_dir: &Path, cases: &[Case]) -> PerfStats {
    let mut stats = PerfStats::default();
    for c in cases {
        let by_model = stats
            .by_scenario
            .entry(c.scenario.clone())
            .or_default();
        let s = by_model.entry(c.model.clone()).or_default();
        s.runs += 1;
        update_min_max(&mut s.duration_ms_min, &mut s.duration_ms_max, c.duration_ms);
        if let Some(p) = c.perf_json.as_ref() {
            let path = resolve_artifact_path(out_dir, p);
            if let Some(m) = parse_compile_perf_metrics(&path) {
                update_min_max(
                    &mut s.flatten_inline_ms_min,
                    &mut s.flatten_inline_ms_max,
                    m.flatten_inline_ms,
                );
                update_min_max(
                    &mut s.flatten_wall_ms_min,
                    &mut s.flatten_wall_ms_max,
                    m.flatten_wall_ms,
                );
                update_min_max(
                    &mut s.inline_wall_ms_min,
                    &mut s.inline_wall_ms_max,
                    m.inline_wall_ms,
                );
                update_min_max(
                    &mut s.decl_expand_ms_min,
                    &mut s.decl_expand_ms_max,
                    m.decl_expand_ms,
                );
                update_min_max(&mut s.eq_expand_ms_min, &mut s.eq_expand_ms_max, m.eq_expand_ms);
                update_min_max(
                    &mut s.inline_substitute_ms_min,
                    &mut s.inline_substitute_ms_max,
                    m.inline_substitute_ms,
                );
                update_min_max(
                    &mut s.inline_load_model_ms_min,
                    &mut s.inline_load_model_ms_max,
                    m.inline_load_model_ms,
                );
                update_min_max(
                    &mut s.cache_deserialize_ms_min,
                    &mut s.cache_deserialize_ms_max,
                    m.cache_deserialize_ms,
                );
                if c.run_index == 1 {
                    s.run1_flatten_inline_ms = Some(m.flatten_inline_ms);
                    s.run1_decl_expand_ms = Some(m.decl_expand_ms);
                } else {
                    s.best_after_run1_flatten_inline_ms = Some(
                        s.best_after_run1_flatten_inline_ms
                            .map(|x| x.min(m.flatten_inline_ms))
                            .unwrap_or(m.flatten_inline_ms),
                    );
                    s.best_after_run1_decl_expand_ms = Some(
                        s.best_after_run1_decl_expand_ms
                            .map(|x| x.min(m.decl_expand_ms))
                            .unwrap_or(m.decl_expand_ms),
                    );
                }
            }
        }
    }
    stats
}

fn capture_output_to_files(
    mut cmd: Command,
    stdout_path: &Path,
    stderr_path: &Path,
) -> Result<(i32, String, String)> {
    let out = cmd.output()?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    std::fs::write(stdout_path, &stdout)?;
    std::fs::write(stderr_path, &stderr)?;
    Ok((out.status.code().unwrap_or(-1), stdout, stderr))
}

fn git_info(repo_root: &Path) -> GitInfo {
    let mut info = GitInfo::default();
    let head = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(repo_root)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    info.head = head;
    let branch = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(repo_root)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    info.branch = branch;
    let dirty = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(repo_root)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| !s.trim().is_empty());
    info.dirty = dirty;
    info
}

fn host_info() -> HostInfo {
    let mut h = HostInfo::default();
    h.os = Some(std::env::consts::OS.to_string());
    h.arch = Some(std::env::consts::ARCH.to_string());
    h.hostname = std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok());
    h
}

fn resolve_scenario_cache_dir(out_dir: &Path, sc: &Scenario) -> PathBuf {
    if let Some(p) = &sc.cache_dir {
        return p.clone();
    }
    out_dir.join(format!("cache_{}", safe_slug(&sc.id)))
}

fn case_paths(out_dir: &Path, sc_id: &str, model: &str, run_index: usize) -> CasePaths {
    let sc = safe_slug(sc_id);
    let m = safe_slug(model);
    let base = format!("{}_{}_{}", sc, m, run_index);
    CasePaths {
        perf_json: out_dir.join(format!("perf_{}.json", base)),
        cache_stats_json: out_dir.join(format!("cache_stats_{}.json", base)),
        stdout_txt: out_dir.join(format!("stdout_{}.txt", base)),
        stderr_txt: out_dir.join(format!("stderr_{}.txt", base)),
    }
}

fn build_repro_command(
    spec: &RunSpec,
    sc: &Scenario,
    cache_dir: &Path,
    paths: &CasePaths,
    model: &str,
) -> Vec<String> {
    let mut cmd: Vec<String> = Vec::new();
    cmd.push(spec.exe_path.display().to_string());
    cmd.push("--validate".to_string());
    cmd.push(format!("--validate-tier={}", spec.validate.validate_tier));
    cmd.push(format!("--validation-mode={}", spec.validate.validation_mode));
    cmd.push(format!("--perf-json={}", paths.perf_json.display()));
    for lp in &spec.lib_paths {
        cmd.push(format!("--lib-path={}", lp.display()));
    }
    cmd.push(model.to_string());
    // env overlay description is stored separately; caller can reconstruct a PowerShell wrapper.
    let _ = (sc, cache_dir);
    cmd
}

fn parse_validate_success(stdout: &str, stderr: &str) -> bool {
    let s = format!("{stdout}\n{stderr}");
    s.contains("\"success\"") && s.contains("true")
}

fn build_manifest(spec: &RunSpec, scenarios: &[ScenarioResolved]) -> RunManifest {
    RunManifest {
        schema_version: 1,
        generated_at: now_rfc3339(),
        repo_root: spec.repo_root.display().to_string(),
        git: git_info(&spec.repo_root),
        host: host_info(),
        exe_path: spec.exe_path.display().to_string(),
        lib_paths: spec
            .lib_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        models: spec.models.clone(),
        validate_tier: spec.validate.validate_tier.clone(),
        validation_mode: spec.validate.validation_mode.clone(),
        trace: TraceFlags {
            stage_trace: spec.stage_trace,
            perf_trace: spec.perf_trace,
        },
        scenarios: scenarios.to_vec(),
    }
}

fn scenario_env_resolved(sc: &Scenario, cache_dir: &Path, trace: &TraceFlags) -> (BTreeMap<String, String>, Vec<String>) {
    let mut set = sc.env.set.clone();
    let mut unset = sc.env.unset.clone();
    set.insert(
        "RUSTMODLICA_FLATTEN_CACHE_DIR".to_string(),
        cache_dir.display().to_string(),
    );
    if trace.stage_trace {
        set.insert("RUSTMODLICA_STAGE_TRACE".to_string(), "1".to_string());
    }
    if trace.perf_trace {
        set.insert("RUSTMODLICA_PERF_TRACE".to_string(), "1".to_string());
    }
    // Ensure uniqueness and stable ordering.
    unset.sort();
    unset.dedup();
    (set, unset)
}

fn apply_env_to_command(
    cmd: &mut Command,
    env_set: &BTreeMap<String, String>,
    env_unset: &[String],
) {
    for k in env_unset {
        cmd.env_remove(k);
    }
    for (k, v) in env_set {
        cmd.env(k, v);
    }
}

pub fn default_perf_scenarios(hot_runs: usize) -> Vec<Scenario> {
    vec![
        Scenario {
            id: "cold_empty_nsCOLD".to_string(),
            runs: 1,
            cache_dir_policy: CacheDirPolicy::PurgeAndCreate,
            cache_dir: None,
            env: crate::jit_validate::EnvOverlay {
                set: BTreeMap::from([
                    ("RUSTMODLICA_QUERY_CACHE_NAMESPACE".to_string(), "COLD".to_string()),
                    ("RUSTMODLICA_CACHE_SQLITE".to_string(), "1".to_string()),
                ]),
                unset: vec![
                    "RUSTMODLICA_QUERY_CACHE".to_string(),
                    "RUSTMODLICA_SALSA".to_string(),
                ],
            },
        },
        Scenario {
            id: "cold_qcache0".to_string(),
            runs: 1,
            cache_dir_policy: CacheDirPolicy::CreateIfMissing,
            cache_dir: None,
            env: crate::jit_validate::EnvOverlay {
                set: BTreeMap::from([
                    ("RUSTMODLICA_QUERY_CACHE".to_string(), "0".to_string()),
                    ("RUSTMODLICA_CACHE_SQLITE".to_string(), "1".to_string()),
                ]),
                unset: vec![
                    "RUSTMODLICA_QUERY_CACHE_NAMESPACE".to_string(),
                    "RUSTMODLICA_SALSA".to_string(),
                ],
            },
        },
        Scenario {
            id: "hot_nsA".to_string(),
            runs: hot_runs.max(1),
            cache_dir_policy: CacheDirPolicy::CreateIfMissing,
            cache_dir: None,
            env: crate::jit_validate::EnvOverlay {
                set: BTreeMap::from([
                    ("RUSTMODLICA_QUERY_CACHE_NAMESPACE".to_string(), "A".to_string()),
                    ("RUSTMODLICA_CACHE_SQLITE".to_string(), "1".to_string()),
                ]),
                unset: vec![
                    "RUSTMODLICA_QUERY_CACHE".to_string(),
                    "RUSTMODLICA_SALSA".to_string(),
                ],
            },
        },
        Scenario {
            id: "legacy_salsa0".to_string(),
            runs: 1,
            cache_dir_policy: CacheDirPolicy::CreateIfMissing,
            cache_dir: None,
            env: crate::jit_validate::EnvOverlay {
                set: BTreeMap::from([
                    ("RUSTMODLICA_SALSA".to_string(), "0".to_string()),
                    ("RUSTMODLICA_CACHE_SQLITE".to_string(), "1".to_string()),
                    ("RUSTMODLICA_FLATTEN_FULL_CACHE".to_string(), "1".to_string()),
                ]),
                unset: vec![
                    "RUSTMODLICA_QUERY_CACHE".to_string(),
                    "RUSTMODLICA_QUERY_CACHE_NAMESPACE".to_string(),
                ],
            },
        },
    ]
}

pub struct ValidatePerfRunner;

impl ValidatePerfRunner {
    pub fn run(mut spec: RunSpec) -> Result<ValidatePerfReport> {
        spec.models = normalize_model_list(&spec.models);
        std::fs::create_dir_all(&spec.out_dir)
            .with_context(|| format!("create out dir {}", spec.out_dir.display()))?;

        let trace = TraceFlags {
            stage_trace: spec.stage_trace,
            perf_trace: spec.perf_trace,
        };

        let scenario_filter: Vec<String> = spec
            .scenario_filter
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let use_filter = !scenario_filter.is_empty();
        let selected_scenarios: Vec<Scenario> = spec
            .scenarios
            .iter()
            .cloned()
            .filter(|sc| !use_filter || scenario_filter.iter().any(|x| x == &sc.id))
            .collect();

        let mut scenarios_resolved: Vec<ScenarioResolved> = Vec::new();
        for sc in &selected_scenarios {
            let cache_dir = resolve_scenario_cache_dir(&spec.out_dir, sc);
            let (env_set, env_unset) = scenario_env_resolved(sc, &cache_dir, &trace);
            scenarios_resolved.push(ScenarioResolved {
                id: sc.id.clone(),
                runs: sc.runs,
                cache_dir: cache_dir.display().to_string(),
                env_set,
                env_unset,
            });
        }

        let manifest = build_manifest(&spec, &scenarios_resolved);
        write_json_pretty(&spec.out_dir.join("run_manifest.json"), &manifest)?;

        let mut cases: Vec<Case> = Vec::new();
        let mut passed = 0usize;
        let mut failed = 0usize;

        let lib_args: Vec<String> = spec
            .lib_paths
            .iter()
            .map(|p| format!("--lib-path={}", p.display()))
            .collect();

        for (sc_idx, sc) in selected_scenarios.iter().enumerate() {
            let resolved = &scenarios_resolved[sc_idx];
            let cache_dir = PathBuf::from(&resolved.cache_dir);
            ensure_cache_dir(&cache_dir, sc.cache_dir_policy.clone())?;

            for model in &spec.models {
                for run_index in 1..=sc.runs.max(1) {
                    let paths = case_paths(&spec.out_dir, &sc.id, model, run_index);

                    let mut cmd = Command::new(&spec.exe_path);
                    cmd.arg("--validate");
                    cmd.arg(format!("--validate-tier={}", spec.validate.validate_tier));
                    cmd.arg(format!(
                        "--validation-mode={}",
                        spec.validate.validation_mode
                    ));
                    cmd.arg(format!("--perf-json={}", paths.perf_json.display()));
                    for a in &lib_args {
                        cmd.arg(a);
                    }
                    cmd.arg(model);

                    let mut env_set: BTreeMap<String, String> = resolved.env_set.clone();
                    let env_unset: Vec<String> = resolved.env_unset.clone();
                    env_set.insert(
                        "RUSTMODLICA_CACHE_STATS_JSON".to_string(),
                        paths.cache_stats_json.display().to_string(),
                    );
                    apply_env_to_command(&mut cmd, &env_set, &env_unset);

                    let repro = build_repro_command(&spec, sc, &cache_dir, &paths, model);
                    let t0 = Instant::now();
                    let (exit_code, stdout, stderr) =
                        capture_output_to_files(cmd, &paths.stdout_txt, &paths.stderr_txt)
                            .with_context(|| format!("run validate for {}", model))?;
                    let duration_ms = t0.elapsed().as_millis() as u64;
                    let success = exit_code == 0 && parse_validate_success(&stdout, &stderr);

                    if success {
                        passed += 1;
                    } else {
                        failed += 1;
                    }

                    cases.push(Case {
                        scenario: sc.id.clone(),
                        model: model.clone(),
                        run_index,
                        success,
                        exit_code,
                        duration_ms,
                        perf_json: Some(paths.perf_json.display().to_string()),
                        cache_stats_json: Some(paths.cache_stats_json.display().to_string()),
                        stdout_path: Some(paths.stdout_txt.display().to_string()),
                        stderr_path: Some(paths.stderr_txt.display().to_string()),
                        repro,
                        env: env_set,
                        env_unset,
                        cache_dir: Some(cache_dir.display().to_string()),
                        note: None,
                    });
                }
            }
        }

        let out_dir_str = spec.out_dir.display().to_string();
        let stats = build_perf_stats(&spec.out_dir, &cases);
        let report = ValidatePerfReport {
            schema_version: 1,
            generated_at: now_rfc3339(),
            out_dir: out_dir_str,
            summary: Summary {
                total: cases.len(),
                passed,
                failed,
            },
            cases,
            stats,
        };

        // Keep the report stable even if caller wants to post-process.
        write_json_pretty(&spec.out_dir.join("report.json"), &report)?;
        if report.summary.failed > 0 {
            let mut f = std::fs::File::create(spec.out_dir.join("FAILURES.txt"))?;
            writeln!(
                f,
                "failed_cases={} total_cases={}",
                report.summary.failed, report.summary.total
            )?;
        }
        Ok(report)
    }
}

