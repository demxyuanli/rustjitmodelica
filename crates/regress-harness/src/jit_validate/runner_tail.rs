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

fn resolve_scenario_cache_dir(out_dir: &Path, shared_root: Option<&Path>, sc: &Scenario) -> PathBuf {
    if let Some(p) = &sc.cache_dir {
        return p.clone();
    }
    if let Some(root) = shared_root {
        return root.join(safe_slug(&sc.id));
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
        dep_graph_json: out_dir.join(format!("dep_graph_{}.json", base)),
        stdout_txt: out_dir.join(format!("stdout_{}.txt", base)),
        stderr_txt: out_dir.join(format!("stderr_{}.txt", base)),
    }
}

fn file_hash_hex(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    data.hash(&mut h);
    Some(format!("{:016x}", h.finish()))
}

fn parse_previous_dep_graph(
    dep_graph_path: &Path,
    model: &str,
    file_to_models: &mut HashMap<String, HashSet<String>>,
    baseline_hashes: &mut HashMap<String, String>,
) {
    let Ok(text) = std::fs::read_to_string(dep_graph_path) else {
        return;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let Some(entries) = v.get("entries").and_then(|x| x.as_array()) else {
        return;
    };
    for entry in entries {
        let Some(file) = entry.get("file").and_then(|x| x.as_str()) else {
            continue;
        };
        let hash = entry
            .get("content_hash")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();
        baseline_hashes.entry(file.to_string()).or_insert(hash);
        let models = file_to_models
            .entry(file.to_string())
            .or_default();
        models.insert(model.to_string());
        if let Some(arr) = entry.get("models").and_then(|x| x.as_array()) {
            for m in arr.iter().filter_map(|x| x.as_str()) {
                models.insert(m.to_string());
            }
        }
    }
}

fn incremental_affected_models(out_dir: &Path, candidate_models: &[String]) -> Option<Vec<String>> {
    let report_path = out_dir.join("report.json");
    let text = std::fs::read_to_string(report_path).ok()?;
    let report: ValidatePerfReport = serde_json::from_str(&text).ok()?;
    let mut file_to_models: HashMap<String, HashSet<String>> = HashMap::new();
    let mut baseline_hashes: HashMap<String, String> = HashMap::new();
    for c in &report.cases {
        let Some(dep_graph) = &c.dep_graph_json else {
            continue;
        };
        let dep_path = resolve_artifact_path(out_dir, dep_graph);
        parse_previous_dep_graph(&dep_path, c.model.as_str(), &mut file_to_models, &mut baseline_hashes);
    }
    if baseline_hashes.is_empty() {
        return None;
    }
    let mut changed_files: Vec<String> = Vec::new();
    for (file, old_hash) in &baseline_hashes {
        let p = PathBuf::from(file);
        let Some(new_hash) = file_hash_hex(&p) else {
            changed_files.push(file.clone());
            continue;
        };
        if &new_hash != old_hash {
            changed_files.push(file.clone());
        }
    }
    if changed_files.is_empty() {
        return Some(candidate_models.to_vec());
    }
    let mut affected: HashSet<String> = HashSet::new();
    for f in changed_files {
        if let Some(models) = file_to_models.get(&f) {
            affected.extend(models.iter().cloned());
        }
    }
    if affected.is_empty() {
        return Some(candidate_models.to_vec());
    }
    let mut out: Vec<String> = candidate_models
        .iter()
        .filter(|m| affected.contains(m.as_str()))
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    Some(out)
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
        purge_scenario_caches: spec.purge_scenario_caches,
        shared_cache_dir: spec
            .shared_cache_dir
            .as_ref()
            .map(|p| p.display().to_string()),
        force_flatten_full_cache: spec.force_flatten_full_cache,
        worker_per_scenario: spec.worker_per_scenario,
    }
}

/// Remove cache subdirs under `base_dir` matching the expected layout.
fn purge_scenario_cache_subdirs(base_dir: &std::path::Path, use_prefixed_layout: bool) -> anyhow::Result<()> {
    if !base_dir.exists() {
        return Ok(());
    }
    let rd = std::fs::read_dir(base_dir)
        .with_context(|| format!("read cache base dir {}", base_dir.display()))?;
    for ent in rd {
        let ent = ent.with_context(|| format!("read entry in {}", base_dir.display()))?;
        let p = ent.path();
        if !p.is_dir() {
            continue;
        }
        let name = ent.file_name();
        let Some(ns) = name.to_str() else { continue };
        let should_remove = if use_prefixed_layout {
            ns.starts_with("cache_")
        } else {
            // Shared-cache layout stores per-scenario subdirs directly under root.
            !ns.is_empty()
        };
        if should_remove {
            std::fs::remove_dir_all(&p).with_context(|| format!("remove {}", p.display()))?;
        }
    }
    Ok(())
}

fn scenario_env_resolved(
    sc: &Scenario,
    cache_dir: &Path,
    trace: &TraceFlags,
    force_flatten_full_cache: bool,
) -> (BTreeMap<String, String>, Vec<String>) {
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
    if force_flatten_full_cache {
        set.insert("RUSTMODLICA_FLATTEN_FULL_CACHE".to_string(), "1".to_string());
        unset.retain(|k| k != "RUSTMODLICA_FLATTEN_FULL_CACHE");
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
                    ("RUSTMODLICA_FLATTEN_FULL_CACHE".to_string(), "1".to_string()),
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
        if spec.incremental {
            if let Some(filtered) = incremental_affected_models(&spec.out_dir, &spec.models) {
                spec.models = filtered;
            }
        }
        std::fs::create_dir_all(&spec.out_dir)
            .with_context(|| format!("create out dir {}", spec.out_dir.display()))?;
        if spec.purge_scenario_caches {
            if let Some(shared_root) = spec.shared_cache_dir.as_deref() {
                purge_scenario_cache_subdirs(shared_root, false)?;
                eprintln!(
                    "[jit-validate-perf] purge_scenario_caches: removed scenario cache dirs under shared root {}",
                    shared_root.display()
                );
            } else {
                purge_scenario_cache_subdirs(&spec.out_dir, true)?;
                eprintln!(
                    "[jit-validate-perf] purge_scenario_caches: removed cache_* under {}",
                    spec.out_dir.display()
                );
            }
        }

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
        let shared_root = spec.shared_cache_dir.as_deref();
        for sc in &selected_scenarios {
            let cache_dir = resolve_scenario_cache_dir(&spec.out_dir, shared_root, sc);
            let (env_set, env_unset) = scenario_env_resolved(
                sc,
                &cache_dir,
                &trace,
                spec.force_flatten_full_cache,
            );
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

        if spec.worker_per_scenario {
            eprintln!(
                "[jit-validate-perf] worker-per-scenario PoC enabled: keeping artifact parity with legacy case execution path"
            );
        }

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
                        "RUSTMODLICA_PERF_SALSA_STATS".to_string(),
                        "1".to_string(),
                    );
                    env_set.insert(
                        "RUSTMODLICA_CACHE_STATS_JSON".to_string(),
                        paths.cache_stats_json.display().to_string(),
                    );
                    env_set.insert(
                        "RUSTMODLICA_DEP_GRAPH_JSON".to_string(),
                        paths.dep_graph_json.display().to_string(),
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
                        dep_graph_json: Some(paths.dep_graph_json.display().to_string()),
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
        if let Some(line) = format_validate_perf_track_ab(&report.stats) {
            println!("{}", line);
        }
        if let Some(line) = format_validate_perf_cache_rollup(&report.stats) {
            println!("{}", line);
        }
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

