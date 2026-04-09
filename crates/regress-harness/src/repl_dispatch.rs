use super::*;

fn execute_line(ctx: &mut ReplContext, line: &str) -> Result<bool> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(true);
    }
    if trimmed.starts_with("//") {
        return Ok(true);
    }
    let raw = strip_leading_slash(trimmed);
    if raw.is_empty() {
        return Ok(true);
    }
    let args = expand_with_prefix(ctx, tokenize(raw));
    let cmd = args[0].to_ascii_lowercase();
    let rest = args[1..].to_vec();

    match cmd.as_str() {
        "ls" => {
            let subs = list_subcommands(ctx);
            if subs.is_empty() {
                println!("<no subcommands>");
            } else {
                for s in subs {
                    println!("{s}");
                }
            }
            Ok(true)
        }
        "cd" => {
            let t = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            cd_prefix(ctx, t)?;
            println!("{}", prompt_label(ctx));
            Ok(true)
        }
        "tree" => {
            if rest.iter().any(|s| s == "--select") {
                match tree_select_interactive() {
                    Ok(p) => {
                        println!("selected: {}", p.join(" "));
                        Ok(true)
                    }
                    Err(e) => {
                        // Keep non-fatal; interactive cancel is common.
                        println!("[ERROR] {}", e);
                        Ok(true)
                    }
                }
            } else {
            let mut depth = 1usize;
            let mut path: Option<&str> = None;
            if rest.iter().any(|s| s == "--all") {
                depth = 64;
            } else if let Some(d) = take_flag(&rest, "--depth").and_then(|s| s.parse::<usize>().ok())
            {
                depth = d.max(1);
            }
            if let Some(p) = rest.get(0).map(|s| s.as_str()) {
                if !p.starts_with("--") {
                    path = Some(p);
                }
            }
            print_command_tree(path, depth)?;
            Ok(true)
            }
        }
        "help" | "h" => {
            print_help();
            Ok(true)
        }
        "clear" | "cls" => {
            crate::ui::clear_screen();
            Ok(true)
        }
        "scope" => {
            if rest.is_empty() {
                profile_list();
                return Ok(true);
            }
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "list" => {
                    profile_list();
                    Ok(true)
                }
                "use" => {
                    let name = rest.get(1).map(|s| s.as_str()).unwrap_or("");
                    if name.is_empty() {
                        let scopes = crate::scope::list_scopes();
                        let labels = scopes
                            .iter()
                            .map(|s| format!("{}  -  {}", s.name, s.desc))
                            .collect::<Vec<_>>();
                        let picked = inquire::Select::new("Select scope", labels)
                            .with_render_config(repl_render_config())
                            .prompt()?;
                        let picked_name = picked
                            .split("  -  ")
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if picked_name.is_empty() {
                            bail!("no scope selected");
                        }
                        profile_use(ctx, &picked_name)?;
                    } else {
                        profile_use(ctx, name)?;
                    }
                    Ok(true)
                }
                "gen-config" => {
                    let name = rest.get(1).map(|s| s.as_str()).unwrap_or("");
                    if name.is_empty() {
                        let scopes = crate::scope::list_scopes();
                        let labels = scopes
                            .iter()
                            .map(|s| format!("{}  -  {}", s.name, s.desc))
                            .collect::<Vec<_>>();
                        let picked = inquire::Select::new("Select scope", labels)
                            .with_render_config(repl_render_config())
                            .prompt()?;
                        let picked_name = picked
                            .split("  -  ")
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if picked_name.is_empty() {
                            bail!("no scope selected");
                        }
                        let repo_root = ctx_repo_root(ctx)?;
                        let resolved = crate::scope::resolve_scope(&repo_root, &picked_name)?;
                        println!(
                            "config_path={}",
                            absolutize_under(&repo_root, resolved.config_path).display()
                        );
                        return Ok(true);
                    }
                    let repo_root = ctx_repo_root(ctx)?;
                    let resolved = crate::scope::resolve_scope(&repo_root, name)?;
                    println!(
                        "config_path={}",
                        absolutize_under(&repo_root, resolved.config_path).display()
                    );
                    Ok(true)
                }
                _ => bail!("usage: scope list | scope use <name> | scope gen-config <name>"),
            }
        }
        "sync" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "determinism" => {
                    let model = take_flag(&rest, "--model").unwrap_or_default();
                    if model.is_empty() {
                        bail!("missing --model");
                    }
                    let cargo_target_dir = take_flag(&rest, "--cargo-target-dir")
                        .map(PathBuf::from)
                        .or_else(|| ctx.cargo_target_dir.clone())
                        .unwrap_or_else(|| PathBuf::from("target_regression"));
                    let output_interval = take_flag(&rest, "--output-interval")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.001);
                    let artifacts_dir = take_flag(&rest, "--artifacts-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build/regression_data_jit_phase1/artifacts"));
                    let repo_root = ctx_repo_root(ctx)?;
                    let r = crate::sync_tools::sync_determinism(
                        &repo_root,
                        &absolutize_under(&repo_root, cargo_target_dir),
                        &model,
                        output_interval,
                        &absolutize_under(&repo_root, artifacts_dir),
                    )?;
                    println!(
                        "[sync-det] model={} ok={} exit_a={} exit_b={} csv_a={} csv_b={} hash_a={} hash_b={} wall_ms_a={} wall_ms_b={}",
                        model,
                        r.ok,
                        r.exit_a,
                        r.exit_b,
                        r.csv_a.display(),
                        r.csv_b.display(),
                        r.hash_a.clone().unwrap_or_else(|| "-".to_string()),
                        r.hash_b.clone().unwrap_or_else(|| "-".to_string()),
                        r.wall_ms_a,
                        r.wall_ms_b
                    );
                    Ok(true)
                }
                "trace-assert" => {
                    let model = take_flag(&rest, "--model").unwrap_or_default();
                    let expect_substr = take_flag(&rest, "--expect-substr").unwrap_or_default();
                    let t_end = take_flag(&rest, "--t-end")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(-1.0);
                    if model.is_empty() || expect_substr.is_empty() || t_end < 0.0 {
                        bail!("missing --model/--expect-substr/--t-end");
                    }
                    let cargo_target_dir = take_flag(&rest, "--cargo-target-dir")
                        .map(PathBuf::from)
                        .or_else(|| ctx.cargo_target_dir.clone())
                        .unwrap_or_else(|| PathBuf::from("target_regression"));
                    let artifacts_dir = take_flag(&rest, "--artifacts-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build/regression_data_jit_phase1/artifacts"));
                    let expect_times = take_flag(&rest, "--expect-times").unwrap_or_default();
                    let disallow_times = take_flag(&rest, "--disallow-times").unwrap_or_default();
                    let repo_root = ctx_repo_root(ctx)?;
                    let r = crate::sync_tools::sync_trace_assert(
                        &repo_root,
                        &absolutize_under(&repo_root, cargo_target_dir),
                        &model,
                        &expect_substr,
                        t_end,
                        &expect_times,
                        &disallow_times,
                        &absolutize_under(&repo_root, artifacts_dir),
                    )?;
                    println!(
                        "[sync-trace-assert] model={} ok={} exit={} trace={} csv={}",
                        model,
                        r.ok,
                        r.exit_code,
                        r.trace_path.display(),
                        r.csv_path.display()
                    );
                    Ok(true)
                }
                _ => bail!("usage: sync determinism|trace-assert ..."),
            }
        }
        "fmi" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "emit-fmu" => {
                    let model = take_flag(&rest, "--model").unwrap_or_default();
                    if model.is_empty() {
                        bail!("missing --model");
                    }
                    let cargo_target_dir = take_flag(&rest, "--cargo-target-dir")
                        .map(PathBuf::from)
                        .or_else(|| ctx.cargo_target_dir.clone())
                        .unwrap_or_else(|| PathBuf::from("target_regression"));
                    let out_dir = take_flag(&rest, "--out-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build_regress_fmu"));
                    let repo_root = ctx_repo_root(ctx)?;
                    let r = crate::fmi_tools::fmi_emit_fmu(
                        &repo_root,
                        &absolutize_under(&repo_root, cargo_target_dir),
                        &absolutize_under(&repo_root, out_dir),
                        &model,
                    )?;
                    println!(
                        "[fmi] ok={} exit={} out_dir={} md={} c={} {}",
                        r.ok,
                        r.exit_code,
                        r.out_dir.display(),
                        r.model_description.display(),
                        r.c_file.display(),
                        r.flags
                    );
                    Ok(true)
                }
                "validate" => {
                    let dir = take_flag(&rest, "--dir").unwrap_or_default();
                    if dir.is_empty() {
                        bail!("missing --dir");
                    }
                    crate::fmi_tools::fmi_validate_dir(PathBuf::from(dir).as_path())?;
                    println!("ok");
                    Ok(true)
                }
                _ => bail!("usage: fmi emit-fmu|validate ..."),
            }
        }
        "perf" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            if sub != "sparse-dense" {
                bail!("usage: perf sparse-dense ...");
            }
            let action = rest.get(1).map(|s| s.as_str()).unwrap_or("");
            match action {
                "bench" => {
                    let models_arg = take_flag(&rest, "--models").unwrap_or_default();
                    let models = crate::sparse_dense::parse_models_arg(&models_arg);
                    let t_end = take_flag(&rest, "--t-end")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(1.0);
                    let dt = take_flag(&rest, "--dt")
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.01);
                    let warnings = take_flag(&rest, "--warnings").unwrap_or_else(|| "none".to_string());
                    let out_dir = take_flag(&rest, "--out-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build_sparse_dense_bench"));
                    let repo_root = ctx_repo_root(ctx)?;
                    let out = crate::sparse_dense::bench_sparse_dense(
                        &repo_root,
                        &models,
                        t_end,
                        dt,
                        &warnings,
                        &absolutize_under(&repo_root, out_dir),
                        false,
                    )?;
                    println!("bench_csv={}", out.csv_path.display());
                    println!("bench_json={}", out.json_path.display());
                    Ok(true)
                }
                "summarize" => {
                    let input_dir = take_flag(&rest, "--input-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("jit-compiler/build_sparse_dense_bench"));
                    let output_dir = take_flag(&rest, "--output-dir")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("build_sparse_dense_summary"));
                    let blt_guard_filter =
                        take_flag(&rest, "--blt-guard-filter").unwrap_or_else(|| "non_triggered".to_string());
                    let model_filter_arg = take_flag(&rest, "--model-filter").unwrap_or_default();
                    let model_filter = model_filter_arg
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    let out = crate::sparse_dense::summarize_sparse_dense(
                        &input_dir,
                        &output_dir,
                        &blt_guard_filter,
                        &model_filter,
                    )?;
                    println!("summary_csv={}", out.csv_path.display());
                    println!("summary_json={}", out.json_path.display());
                    Ok(true)
                }
                _ => bail!("usage: perf sparse-dense bench|summarize ..."),
            }
        }
        "stability" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            if sub != "event-scan-matrix" {
                bail!("usage: stability event-scan-matrix ...");
            }
            let lib_paths = take_flag(&rest, "--lib-path").unwrap_or_default();
            let lib_paths = lib_paths
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if lib_paths.is_empty() {
                bail!("missing --lib-path (comma-separated)");
            }
            let out_dir = take_flag(&rest, "--out-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("build_stability/event_scan_matrix_ci"));
            let models = take_flag(&rest, "--models")
                .unwrap_or_else(|| "TestLib/BouncingBall,TestLib/Pendulum,ModelicaTest.JitStress.SyncOmCompare".to_string());
            let models = models
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            let count_values = take_flag(&rest, "--count-values")
                .unwrap_or_else(|| "0.0004,0.0005,0.0006,0.0008".to_string());
            let count_values = count_values
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            let tail_values = take_flag(&rest, "--tail-velocity-values")
                .unwrap_or_else(|| "0.02,0.03,0.04,0.05".to_string());
            let tail_velocity_values = tail_values
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            let top_n = take_flag(&rest, "--top-n")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(3);
            let allow_unsupported = has_flag(&rest, "--allow-unsupported");
            let repo_root = ctx_repo_root(ctx)?;
            let ok = crate::event_scan_matrix::run_event_scan_matrix(
                &repo_root,
                &crate::event_scan_matrix::EventScanMatrixArgs {
                    out_dir: absolutize_under(&repo_root, out_dir),
                    models,
                    count_values,
                    tail_velocity_values,
                    lib_paths,
                    top_n,
                    allow_unsupported,
                },
            )?;
            println!("[event-scan] ok={ok}");
            Ok(true)
        }
        "coverage" => {
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "generate-status" => {
                    let repo_root = ctx_repo_root(ctx)?;
                    let p = crate::coverage_status::generate_coverage_status(&repo_root)?;
                    println!("coverage_status_json={}", p.display());
                    Ok(true)
                }
                "gate" => {
                    let repo_root = ctx_repo_root(ctx)?;
                    let status_json = take_flag(&rest, "--status-json")
                        .map(PathBuf::from)
                        .map(|p| absolutize_under(&repo_root, p))
                        .unwrap_or_else(|| repo_root.join("jit-compiler/scripts/coverage_status.json"));
                    let ok = crate::coverage_status::coverage_gate(&status_json)?;
                    println!(
                        "[coverage-gate] ok={} status_json={}",
                        ok,
                        status_json.display()
                    );
                    Ok(true)
                }
                _ => bail!("usage: coverage generate-status|gate ..."),
            }
        }
        "profile" => {
            if rest.is_empty() {
                profile_list();
                return Ok(true);
            }
            let sub = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            match sub {
                "list" => {
                    profile_list();
                    Ok(true)
                }
                "use" => {
                    let name = rest.get(1).map(|s| s.as_str()).unwrap_or("");
                    if name.is_empty() {
                        let scopes = crate::scope::list_scopes();
                        let labels = scopes
                            .iter()
                            .map(|s| format!("{}  -  {}", s.name, s.desc))
                            .collect::<Vec<_>>();
                        let picked = inquire::Select::new("Select scope", labels)
                            .with_render_config(repl_render_config())
                            .prompt()?;
                        let picked_name = picked
                            .split("  -  ")
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if picked_name.is_empty() {
                            bail!("no scope selected");
                        }
                        profile_use(ctx, &picked_name)?;
                    } else {
                        profile_use(ctx, name)?;
                    }
                    Ok(true)
                }
                _ => bail!("usage: profile list | profile use <name>"),
            }
        }
        "ctx" => {
            print_ctx(ctx);
            Ok(true)
        }
        "set" => {
            let key = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            let value = rest.get(1).map(|s| s.as_str()).unwrap_or("");
            if key.is_empty() {
                bail!("usage: set <key> <value>");
            }
            let base_for_paths = ctx_repo_root(ctx)?;
            let value_owned: String;
            let value = if value.is_empty()
                && matches!(
                    key,
                    "repo-root"
                        | "rustmodlica-exe"
                        | "cargo-target-dir"
                        | "config"
                        | "baseline"
                        | "manifest"
                        | "data-root"
                        | "out-dir"
                )
            {
                value_owned = prompt_path("path", base_for_paths.clone())?;
                value_owned.as_str()
            } else {
                value
            };
            if value.is_empty() {
                bail!("usage: set <key> <value>");
            }
            match key {
                "repo-root" => {
                    let base = discover_repo_root()?;
                    let p = absolutize_under(&base, PathBuf::from(value));
                    ctx.repo_root = Some(canonicalize_best_effort(p));
                }
                "rustmodlica-exe" => {
                    let base = ctx_repo_root(ctx)?;
                    let p = absolutize_under(&base, PathBuf::from(value));
                    ctx.rustmodlica_exe = Some(canonicalize_best_effort(p));
                }
                "cargo-target-dir" => {
                    let base = ctx_repo_root(ctx)?;
                    let p = absolutize_under(&base, PathBuf::from(value));
                    ctx.cargo_target_dir = Some(canonicalize_best_effort(p));
                }
                "config" => ctx.config = Some(PathBuf::from(value)),
                "tier" => ctx.tier = Some(value.to_string()),
                "tags" => {
                    let v = value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    ctx.tags = if v.is_empty() { None } else { Some(v) };
                }
                "workers" => ctx.workers = Some(value.parse::<usize>()?),
                "baseline" => ctx.baseline = Some(PathBuf::from(value)),
                "incremental" => ctx.incremental = Some(value.to_string()),
                "manifest" => ctx.manifest = Some(PathBuf::from(value)),
                "data-root" => ctx.data_root = PathBuf::from(value),
                "out-dir" => ctx.out_dir = Some(PathBuf::from(value)),
                "format" => ctx.format = OutputFormat::parse(value)?,
                _ => bail!("unknown key: {key}"),
            }
            Ok(true)
        }
        "unset" => {
            let key = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            if key.is_empty() {
                bail!("usage: unset <key>");
            }
            match key {
                "repo-root" => ctx.repo_root = None,
                "rustmodlica-exe" => ctx.rustmodlica_exe = None,
                "cargo-target-dir" => ctx.cargo_target_dir = None,
                "tier" => ctx.tier = None,
                "tags" => ctx.tags = None,
                "workers" => ctx.workers = None,
                "baseline" => ctx.baseline = None,
                "incremental" => ctx.incremental = None,
                "manifest" => ctx.manifest = None,
                "out-dir" => ctx.out_dir = None,
                _ => bail!("unknown/unsettable key: {key}"),
            }
            Ok(true)
        }
        "flags" => {
            let mode = rest.get(0).map(|s| s.as_str()).unwrap_or("");
            let name = rest.get(1).map(|s| s.as_str()).unwrap_or("");
            let on = match mode {
                "on" => true,
                "off" => false,
                _ => bail!("usage: flags <on|off> <ndjson|summary-compat|progress>"),
            };
            match name {
                "ndjson" => ctx.ndjson = on,
                "summary-compat" => ctx.summary_compat = on,
                "progress" => ctx.progress = on,
                _ => bail!("unknown flag: {name}"),
            }
            Ok(true)
        }
        "quit" | "exit" | "q" => Ok(false),
        "validate" => {
            let config = parse_path(&rest, "--config")
                .or_else(|| ctx.config.clone())
                .ok_or_else(|| anyhow::anyhow!("missing --config (or set config)"))?;
            let root = ctx_repo_root(ctx)?;
            cmd_validate_config(&absolutize_under(&root, config))?;
            println!("{}", tr("ok"));
            Ok(true)
        }
        "list" => {
            let config = parse_path(&rest, "--config")
                .or_else(|| ctx.config.clone())
                .ok_or_else(|| anyhow::anyhow!("missing --config (or set config)"))?;
            let tier = take_flag(&rest, "--tier").or_else(|| ctx.tier.clone());
            let tags = parse_tags(&rest).or_else(|| ctx.tags.clone());
            let fmt = if take_flag(&rest, "--format").is_some() {
                parse_format(&rest)?
            } else {
                ctx.format
            };
            let root = ctx_repo_root(ctx)?;
            cmd_list_cases(absolutize_under(&root, config), tier, tags, fmt)?;
            Ok(true)
        }
        "plan" => {
            let config = parse_path(&rest, "--config")
                .or_else(|| ctx.config.clone())
                .ok_or_else(|| anyhow::anyhow!("missing --config (or set config)"))?;
            let tier = take_flag(&rest, "--tier").or_else(|| ctx.tier.clone());
            let tags = parse_tags(&rest).or_else(|| ctx.tags.clone());
            let workers = parse_workers(&rest)?.or(ctx.workers);
            let baseline = parse_path(&rest, "--baseline").or_else(|| ctx.baseline.clone());
            let incremental = take_flag(&rest, "--incremental").or_else(|| ctx.incremental.clone());
            let manifest = parse_path(&rest, "--manifest").or_else(|| ctx.manifest.clone());
            let data_root = parse_path(&rest, "--data-root").unwrap_or_else(|| ctx.data_root.clone());
            let out_dir = parse_path(&rest, "--out-dir").or_else(|| ctx.out_dir.clone());
            let fmt = if take_flag(&rest, "--format").is_some() {
                parse_format(&rest)?
            } else {
                ctx.format
            };
            let root = ctx_repo_root(ctx)?;
            cmd_plan(
                absolutize_under(&root, config),
                tier,
                tags,
                workers,
                baseline.map(|p| absolutize_under(&root, p)),
                incremental,
                manifest.map(|p| absolutize_under(&root, p)),
                absolutize_under(&root, data_root),
                out_dir.map(|p| absolutize_under(&root, p)),
                fmt,
            )?;
            Ok(true)
        }
        "run" => {
            let config = parse_path(&rest, "--config")
                .or_else(|| ctx.config.clone())
                .ok_or_else(|| anyhow::anyhow!("missing --config (or set config)"))?;
            let tier = take_flag(&rest, "--tier").or_else(|| ctx.tier.clone());
            let tags = parse_tags(&rest).or_else(|| ctx.tags.clone());
            let workers = parse_workers(&rest)?.or(ctx.workers);
            let baseline = parse_path(&rest, "--baseline").or_else(|| ctx.baseline.clone());
            let incremental = take_flag(&rest, "--incremental").or_else(|| ctx.incremental.clone());
            let manifest = parse_path(&rest, "--manifest").or_else(|| ctx.manifest.clone());
            let data_root = parse_path(&rest, "--data-root").unwrap_or_else(|| ctx.data_root.clone());
            let out_dir = parse_path(&rest, "--out-dir").or_else(|| ctx.out_dir.clone());
            let ndjson = has_flag(&rest, "--ndjson") || ctx.ndjson;
            let summary_compat = has_flag(&rest, "--summary-compat") || ctx.summary_compat;
            let progress = has_flag(&rest, "--progress") || ctx.progress;
            let root = ctx_repo_root(ctx)?;
            crate::run_cmd(
                absolutize_under(&root, config),
                tier,
                tags,
                workers,
                baseline.map(|p| absolutize_under(&root, p)),
                incremental,
                manifest.map(|p| absolutize_under(&root, p)),
                absolutize_under(&root, data_root),
                out_dir.map(|p| absolutize_under(&root, p)),
                ndjson,
                summary_compat,
                progress,
            )?;
            Ok(true)
        }
        "status" => {
            let data_root =
                parse_path(&rest, "--data-root").unwrap_or_else(|| ctx.data_root.clone());
            let fmt = if take_flag(&rest, "--format").is_some() {
                parse_format(&rest)?
            } else {
                ctx.format
            };
            let root = ctx_repo_root(ctx)?;
            cmd_status(absolutize_under(&root, data_root), fmt)?;
            Ok(true)
        }
        "monitor" => {
            let data_root =
                parse_path(&rest, "--data-root").unwrap_or_else(|| ctx.data_root.clone());
            let tail = take_flag(&rest, "--tail")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(20);
            let follow = has_flag(&rest, "--follow");
            let source = take_flag(&rest, "--source").unwrap_or_else(|| "auto".to_string());
            let root = ctx_repo_root(ctx)?;
            cmd_monitor(
                absolutize_under(&root, data_root),
                tail,
                follow,
                parse_monitor_source(Some(&source))?,
            )?;
            Ok(true)
        }
        "agent-context" => {
            let data_root =
                parse_path(&rest, "--data-root").unwrap_or_else(|| PathBuf::from("build/regression_data"));
            let config = parse_path(&rest, "--config");
            let root = ctx_repo_root(ctx)?;
            cmd_agent_context(
                absolutize_under(&root, data_root),
                config.map(|p| absolutize_under(&root, p)),
            )?;
            Ok(true)
        }
        _ => {
            bail!("unknown command: {cmd}");
        }
    }
}

/// Run one REPL-syntax line (same rules as the interactive session) and return.
/// Used by the `repl-exec` CLI subcommand and external UIs (e.g. Ink).
pub fn run_single_command(line: &str) -> Result<()> {
    let mut ctx = ReplContext::default();
    let _keep = execute_line(&mut ctx, line)?;
    Ok(())
}


pub fn run_repl() -> Result<()> {
    crate::ui::print_session_intro(env!("CARGO_PKG_VERSION"));
    if std::env::var("HARNESS_VERBOSE").is_ok() {
        eprintln!("[harness] verbose enabled (HARNESS_VERBOSE=1)");
    }
    let mut ctx = ReplContext::default();
    loop {
        let line = Text::new(&prompt_label(&ctx))
            .with_autocomplete(command_suggestions)
            .with_help_message("Commands or /help · Tab completes · Ctrl+C cancels line")
            .with_render_config(repl_render_config())
            .prompt();
        let line = match line {
            Ok(v) => v,
            Err(inquire::error::InquireError::OperationCanceled) => {
                continue;
            }
            Err(inquire::error::InquireError::OperationInterrupted) => {
                break;
            }
            Err(e) => return Err(anyhow::anyhow!(e)),
        };
        match execute_line(&mut ctx, &line) {
            Ok(keep) => {
                if !keep {
                    break;
                }
            }
            Err(e) => {
                eprintln!("[ERROR] {e}");
            }
        }
    }
    Ok(())
}

