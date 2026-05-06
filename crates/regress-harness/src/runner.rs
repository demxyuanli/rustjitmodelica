//! Process execution for each case kind.

use crate::compare::compare_csv_last_row_max_abs_diff;
use crate::config::{
    CaseDef, CaseKind, Defaults, ExpectKind, MosRunMode, OmcCompareDef,
};
use crate::report::{Artifacts, CaseResult, CaseStatus, OmcCompareResult};
use crate::runtime::events::{LogLevel, PhaseKind};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

const TAIL_MAX: usize = 4096;
const LOCK_PATTERNS: [&str; 3] = ["os error 5", "failed to remove file", "blocking waiting for file lock"];

fn discover_rustmodlica_exe(repo_root: &Path) -> Option<PathBuf> {
    // Prefer JIT regression build, then workspace build. Do not use mtime across trees:
    // a freshly built workspace `target/release` would beat an older
    // `jit-compiler/target_regression` binary and can pick an incompatible or stale
    // artifact. First existing candidate wins.
    let name = if cfg!(windows) {
        "rustmodlica.exe"
    } else {
        "rustmodlica"
    };
    let rel_dirs = [
        "jit-compiler/target_regression/release",
        "jit-compiler/target_regression/debug",
        "jit-compiler/target/release",
        "jit-compiler/target/debug",
        "target/release",
        "target/debug",
    ];
    for d in rel_dirs {
        let p = repo_root.join(d).join(name);
        if let Ok(md) = std::fs::metadata(&p) {
            if md.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn resolve_rustmodlica_exe(repo_root: &Path, defaults: &Defaults) -> Result<PathBuf, String> {
    let raw = defaults.rustmodlica_exe.trim();
    if raw.eq_ignore_ascii_case("auto") {
        if let Some(p) = discover_rustmodlica_exe(repo_root) {
            return Ok(p);
        }
        return Err("rustmodlica exe auto-discovery failed (no known candidate paths exist)".to_string());
    }
    Ok(resolve_path(repo_root, raw))
}

fn auto_cargo_target_dir(repo_root: &Path, work: &Path) -> PathBuf {
    // Try to keep parity with existing JIT workflow: prefer target_regression under jit-compiler.
    // Use absolute paths to avoid cwd surprises.
    let jit_td = repo_root.join("jit-compiler").join("target_regression");
    if jit_td.is_dir() || work.ends_with("jit-compiler") {
        return jit_td;
    }
    let root_td = repo_root.join("target_regression");
    if root_td.is_dir() {
        return root_td;
    }
    // Default: keep stable name under repo root.
    root_td
}

fn resolve_cargo_target_dirs(
    repo_root: &Path,
    work: &Path,
    defaults: &Defaults,
) -> (PathBuf, PathBuf) {
    let primary = defaults
        .cargo_target_dir_primary
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter(|s| !s.eq_ignore_ascii_case("auto"))
        .map(|s| resolve_path(repo_root, s))
        .unwrap_or_else(|| auto_cargo_target_dir(repo_root, work));
    let fallback = defaults
        .cargo_target_dir_fallback
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter(|s| !s.eq_ignore_ascii_case("auto"))
        .map(|s| resolve_path(repo_root, s))
        .unwrap_or_else(|| {
            let mut s = primary.to_string_lossy().to_string();
            s.push_str("_fallback");
            PathBuf::from(s)
        });
    (primary, fallback)
}

pub struct RunContext<'a> {
    pub repo_root: &'a Path,
    pub defaults: &'a Defaults,
    pub out_dir: &'a Path,
}

#[derive(Debug, Clone)]
pub struct CasePhaseTrace {
    pub phase: PhaseKind,
}

#[derive(Debug, Clone)]
pub struct CaseLogTrace {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CaseRunTrace {
    pub result: CaseResult,
    pub phases: Vec<CasePhaseTrace>,
    pub logs: Vec<CaseLogTrace>,
}

fn strip_warmup_lines(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("[warmup]"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn has_non_warmup_model_not_found(text: &str) -> bool {
    strip_warmup_lines(text)
        .to_lowercase()
        .contains("model not found")
}

fn extract_warmup_failed_models(text: &str) -> Vec<String> {
    const PREFIX: &str = "[warmup] failed ";
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with(PREFIX) {
                return None;
            }
            let rest = &trimmed[PREFIX.len()..];
            let model = rest.split(':').next().map(str::trim).unwrap_or_default();
            if model.is_empty() {
                None
            } else {
                Some(model.to_string())
            }
        })
        .collect()
}

pub fn classify_failure(stdout: &str, stderr: &str, exit_code: i32) -> String {
    let text = format!("{stdout}\n{stderr}");
    let lowered = text.to_lowercase();
    if has_non_warmup_model_not_found(&text) {
        "model_not_found".to_string()
    } else if lowered.contains("codetoolarge") || lowered.contains("code too large") {
        "jit_codegen_too_large".to_string()
    } else if lowered.contains("newton") {
        "newton_nonconverged".to_string()
    } else if lowered.contains("parse") {
        "parse_error".to_string()
    } else if lowered.contains("timeout") {
        "timeout".to_string()
    } else if exit_code == -1 {
        "process_error".to_string()
    } else {
        "runtime_error".to_string()
    }
}

fn tail_string(s: &str) -> String {
    if s.len() <= TAIL_MAX {
        s.to_string()
    } else {
        s[s.len() - TAIL_MAX..].to_string()
    }
}

pub fn hash_case_inputs(repo_root: &Path, defaults: &Defaults, case: &CaseDef) -> String {
    let mut h = Sha256::new();
    let wd = case
        .working_dir
        .as_ref()
        .unwrap_or(&defaults.working_dir);
    let base = repo_root.join(wd);
    let target = base.join(&case.target);
    if target.exists() {
        if let Ok(bytes) = fs::read(&target) {
            h.update(&bytes);
        }
    }
    h.update(case.id.as_bytes());
    h.update(format!("{:?}", case.expect.kind).as_bytes());
    format!("{:x}", h.finalize())
}

pub fn run_case(ctx: &RunContext, case: &CaseDef) -> CaseResult {
    run_case_with_trace(ctx, case).result
}

pub fn run_case_with_trace(ctx: &RunContext, case: &CaseDef) -> CaseRunTrace {
    let started = Instant::now();
    let timeout = Duration::from_millis(case.timeout_ms.unwrap_or(600_000));
    let input_hash = hash_case_inputs(ctx.repo_root, ctx.defaults, case);
    let mut phases = vec![CasePhaseTrace {
        phase: PhaseKind::Prepare,
    }];
    let mut logs = vec![CaseLogTrace {
        level: LogLevel::Info,
        message: format!("case {} kind {:?}", case.id, case.kind),
    }];

    let res = match case.kind {
        CaseKind::Model => {
            phases.push(CasePhaseTrace {
                phase: PhaseKind::Compile,
            });
            phases.push(CasePhaseTrace {
                phase: PhaseKind::Simulate,
            });
            if case.omc_compare.is_some() {
                phases.push(CasePhaseTrace {
                    phase: PhaseKind::Compare,
                });
            }
            run_model(ctx, case, timeout)
        }
        CaseKind::Mos => {
            phases.push(CasePhaseTrace {
                phase: PhaseKind::Simulate,
            });
            run_mos(ctx, case, timeout)
        }
        CaseKind::CustomCommand => {
            phases.push(CasePhaseTrace {
                phase: PhaseKind::Simulate,
            });
            run_custom(ctx, case, timeout)
        }
    };
    phases.push(CasePhaseTrace {
        phase: PhaseKind::Finalize,
    });

    let duration_ms = started.elapsed().as_millis() as u64;

    let result = match res {
        Ok(mut cr) => {
            cr.duration_ms = duration_ms;
            cr.input_hash = Some(input_hash);
            cr
        }
        Err(e) => CaseResult {
            case_id: case.id.clone(),
            tags: case.tags.clone(),
            tier: case.tier.clone(),
            status: CaseStatus::Fail,
            duration_ms,
            exit_code: Some(-1),
            classification: Some("spawn_error".to_string()),
            detail: None,
            stderr_tail: tail_string(&e),
            stdout_tail: String::new(),
            stdout_len: 0,
            stderr_len: e.len(),
            omc_compare: None,
            input_hash: Some(input_hash),
            warmup_failed_count: 0,
            warmup_failed_models: Vec::new(),
            artifacts: Artifacts::default(),
        },
    };
    logs.push(CaseLogTrace {
        level: if result.status == CaseStatus::Pass {
            LogLevel::Info
        } else {
            LogLevel::Warn
        },
        message: format!(
            "finished status={:?} duration_ms={} class={}",
            result.status,
            result.duration_ms,
            result.classification.clone().unwrap_or_else(|| "-".to_string())
        ),
    });
    CaseRunTrace {
        result,
        phases,
        logs,
    }
}

fn run_model(ctx: &RunContext, case: &CaseDef, timeout: Duration) -> Result<CaseResult, String> {
    let defaults = ctx.defaults;
    let work = ctx
        .repo_root
        .join(case.working_dir.as_ref().unwrap_or(&defaults.working_dir));
    let csv_name = format!("regress_{}.csv", sanitize_filename(&case.id));
    let csv_path = ctx.out_dir.join(&csv_name);
    let csv_str = csv_path.to_string_lossy().to_string();

    let mut args: Vec<String> = Vec::new();
    args.push(format!("--solver={}", defaults.solver));
    args.push(format!("--dt={}", defaults.dt));
    args.push(format!("--t-end={}", defaults.t_end));
    args.push(format!("--result-file={}", csv_str));
    for a in &case.extra_rust_args {
        args.push(a.clone());
    }
    args.push(case.target.clone());

    let (code, stdout, stderr, repro_bundle, detail_hint) = if defaults.cargo_run_models {
        run_rustmodlica_cargo_run_with_retry(
            ctx,
            &work,
            &args,
            timeout,
            &case.id,
            &case.env,
        )
    } else {
        let exe = resolve_rustmodlica_exe(ctx.repo_root, defaults)?;
        if !exe.exists() {
            return Err(format!("rustmodlica exe not found: {}", exe.display()));
        }
        let (code, stdout, stderr) =
            run_command_with_env(&exe, &args, &work, defaults, &case.env, timeout)?;
        (code, stdout, stderr, None, None)
    };

    let expect_ok = matches_expect(code, &case.expect);
    let mut classification = if expect_ok {
        None
    } else {
        let text = format!("{stdout}\n{stderr}");
        if has_non_warmup_model_not_found(&text) {
            Some("model_not_found".to_string())
        } else if is_release_binary_locked(&text) {
            Some("release_binary_locked".to_string())
        } else {
            Some(classify_failure(&stdout, &stderr, code))
        }
    };
    // Prefer the explicit locked classification if we also have lock metadata.
    if detail_hint
        .as_deref()
        .map(|d| d.contains("locked=true"))
        .unwrap_or(false)
    {
        if classification.as_deref() != Some("model_not_found") {
            classification = Some("release_binary_locked".to_string());
        }
    }

    let mut omc_res = None;
    let mut status = if expect_ok {
        CaseStatus::Pass
    } else {
        CaseStatus::Fail
    };
    let warmup_failed_models = extract_warmup_failed_models(&stderr);
    let warmup_failed_count = warmup_failed_models.len() as u32;

    if expect_ok && code == 0 {
        if let Some(omc) = &case.omc_compare {
            omc_res = Some(apply_omc_compare(ctx.repo_root, &csv_path, omc));
            if let Some(ref r) = omc_res {
                if r.status == "mismatch" {
                    status = CaseStatus::Fail;
                }
            }
        }
    }

    Ok(CaseResult {
        case_id: case.id.clone(),
        tags: case.tags.clone(),
        tier: case.tier.clone(),
        status,
        duration_ms: 0,
        exit_code: Some(code),
        classification,
        detail: detail_hint,
        stderr_tail: tail_string(&stderr),
        stdout_tail: tail_string(&stdout),
        stdout_len: stdout.len(),
        stderr_len: stderr.len(),
        omc_compare: omc_res,
        input_hash: None,
        warmup_failed_count,
        warmup_failed_models,
        artifacts: Artifacts {
            rust_csv: Some(csv_str),
            working_dir: Some(work.to_string_lossy().to_string()),
            repro_bundle,
        },
    })
}

fn is_release_binary_locked(text: &str) -> bool {
    let t = text.to_lowercase();
    LOCK_PATTERNS.iter().any(|p| t.contains(p))
}

#[cfg(windows)]
fn kill_windows_processes(names: &[&str]) {
    for n in names {
        let _ = Command::new("taskkill")
            .args(["/IM", n, "/F", "/T"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(not(windows))]
fn kill_windows_processes(_names: &[&str]) {}

fn write_repro_bundle(
    out_dir: &Path,
    case_id: &str,
    cmd: &str,
    env_text: &str,
    detail: &str,
) -> Option<String> {
    let safe = sanitize_filename(case_id);
    let dir = out_dir.join("artifacts");
    let _ = fs::create_dir_all(&dir);
    let p = dir.join(format!("repro_{safe}.txt"));
    let body = format!(
        "case_id={case_id}\ncommand={cmd}\nenv={env_text}\ndetail={detail}\n"
    );
    if fs::write(&p, body.as_bytes()).is_ok() {
        Some(p.to_string_lossy().to_string())
    } else {
        None
    }
}

fn run_rustmodlica_cargo_run_with_retry(
    ctx: &RunContext,
    work: &Path,
    run_args: &[String],
    timeout: Duration,
    case_id: &str,
    case_env: &std::collections::HashMap<String, String>,
) -> (i32, String, String, Option<String>, Option<String>) {
    let defaults = ctx.defaults;
    let cargo_path = PathBuf::from(&defaults.cargo_exe);
    let (primary, fallback) = resolve_cargo_target_dirs(ctx.repo_root, work, defaults);

    let mut attempt = 0usize;
    let mut used_fallback = false;
    let mut last = (1, String::new(), String::new());
    let mut locked = false;
    let mut target_dir_used = primary.to_string_lossy().to_string();
    let max_attempts = defaults.cargo_run_max_attempts.max(1);

    while attempt < max_attempts {
        attempt += 1;
        let target_dir = if used_fallback { &fallback } else { &primary };
        target_dir_used = target_dir.to_string_lossy().to_string();
        let mut args: Vec<String> = Vec::new();
        args.extend(defaults.cargo_run_prefix.clone());
        args.push("run".to_string());
        args.push("--target-dir".to_string());
        args.push(target_dir.to_string_lossy().to_string());
        args.extend(defaults.cargo_run_args.clone());
        args.push("-p".to_string());
        args.push("rustmodlica".to_string());
        args.push("--bin".to_string());
        args.push("rustmodlica".to_string());
        args.push("--release".to_string());
        args.push("--".to_string());
        args.extend(run_args.iter().cloned());

        match run_command_with_env(&cargo_path, &args, work, defaults, case_env, timeout) {
            Ok((code, stdout, stderr)) => {
                let text = format!("{stdout}\n{stderr}");
                locked = code != 0 && is_release_binary_locked(&text);
                last = (code, stdout, stderr);
                if code == 0 {
                    break;
                }
                if locked && !used_fallback {
                    let _ = fs::create_dir_all(&fallback);
                    used_fallback = true;
                    kill_windows_processes(&["rustmodlica.exe", "cargo.exe"]);
                    thread::sleep(Duration::from_millis(900));
                    continue;
                }
                break;
            }
            Err(e) => {
                last = (-1, String::new(), e);
                break;
            }
        }
    }

    let (code, stdout, stderr) = last;
    let mut repro = None;
    if code != 0 {
        let cmd = format!(
            "cargo run --target-dir {target_dir_used} -p rustmodlica --bin rustmodlica --release -- {}",
            run_args.join(" ")
        );
        let env_text = format!(
            "RUSTMODLICA_EVENT_TRACE={}",
            std::env::var("RUSTMODLICA_EVENT_TRACE").unwrap_or_default()
        );
        let detail = format!(
            "target_dir={};attempts={};locked={};fallback_used={}",
            target_dir_used, attempt, locked, used_fallback
        );
        repro = write_repro_bundle(ctx.out_dir, case_id, &cmd, &env_text, &detail);
    }
    let detail_hint = Some(format!(
        "target_dir={};attempts={};locked={};fallback_used={}",
        target_dir_used, attempt, locked, used_fallback
    ));
    (code, stdout, stderr, repro, detail_hint)
}

fn apply_omc_compare(
    repo_root: &Path,
    rust_csv: &Path,
    def: &OmcCompareDef,
) -> OmcCompareResult {
    let ref_path = resolve_path(repo_root, &def.reference_csv);
    if !rust_csv.exists() {
        return OmcCompareResult {
            status: "skipped".to_string(),
            max_abs_diff: None,
            max_column_index: None,
            threshold: Some(def.max_abs_diff),
            note: Some("rust csv missing".to_string()),
        };
    }
    if !ref_path.exists() {
        return OmcCompareResult {
            status: "skipped".to_string(),
            max_abs_diff: None,
            max_column_index: None,
            threshold: Some(def.max_abs_diff),
            note: Some(format!("reference missing: {}", ref_path.display())),
        };
    }
    match compare_csv_last_row_max_abs_diff(rust_csv, &ref_path) {
        Ok(out) => {
            let ok = out.max_abs_diff <= def.max_abs_diff;
            OmcCompareResult {
                status: if ok { "ok".to_string() } else { "mismatch".to_string() },
                max_abs_diff: Some(out.max_abs_diff),
                max_column_index: Some(out.max_column_index),
                threshold: Some(def.max_abs_diff),
                note: None,
            }
        }
        Err(e) => OmcCompareResult {
            status: "error".to_string(),
            max_abs_diff: None,
            max_column_index: None,
            threshold: Some(def.max_abs_diff),
            note: Some(e.to_string()),
        },
    }
}

fn run_mos(ctx: &RunContext, case: &CaseDef, timeout: Duration) -> Result<CaseResult, String> {
    let defaults = ctx.defaults;
    let work = ctx
        .repo_root
        .join(case.working_dir.as_ref().unwrap_or(&defaults.working_dir));
    match case.mos_mode {
        MosRunMode::CargoRunScript => {
            let cargo = &defaults.cargo_exe;
            let mut args: Vec<String> = Vec::new();
            args.extend(defaults.cargo_run_prefix.clone());
            args.push("run".to_string());
            let (primary, _) = resolve_cargo_target_dirs(ctx.repo_root, &work, defaults);
            args.push("--target-dir".to_string());
            args.push(primary.to_string_lossy().to_string());
            args.extend(defaults.cargo_run_args.clone());
            args.push("--release".to_string());
            args.push("--".to_string());
            args.push(format!("--script={}", case.target));

            let cargo_path = PathBuf::from(cargo);

            let (code, stdout, stderr) = run_command_with_env(
                &cargo_path,
                &args,
                &work,
                defaults,
                &case.env,
                timeout,
            )?;
            let expect_ok = matches_expect(code, &case.expect);
            let warmup_failed_models = extract_warmup_failed_models(&stderr);
            let warmup_failed_count = warmup_failed_models.len() as u32;
            Ok(CaseResult {
                case_id: case.id.clone(),
                tags: case.tags.clone(),
                tier: case.tier.clone(),
                status: if expect_ok {
                    CaseStatus::Pass
                } else {
                    CaseStatus::Fail
                },
                duration_ms: 0,
                exit_code: Some(code),
                classification: if expect_ok {
                    None
                } else {
                    Some(classify_failure(&stdout, &stderr, code))
                },
                detail: None,
                stderr_tail: tail_string(&stderr),
                stdout_tail: tail_string(&stdout),
                stdout_len: stdout.len(),
                stderr_len: stderr.len(),
                omc_compare: None,
                input_hash: None,
                warmup_failed_count,
                warmup_failed_models,
                artifacts: Artifacts {
                    rust_csv: None,
                    working_dir: Some(work.to_string_lossy().to_string()),
                    repro_bundle: None,
                },
            })
        }
    }
}

fn run_custom(ctx: &RunContext, case: &CaseDef, timeout: Duration) -> Result<CaseResult, String> {
    let prog = case
        .program
        .as_ref()
        .ok_or_else(|| "custom_command requires program".to_string())?;
    let defaults = ctx.defaults;
    let work = ctx
        .repo_root
        .join(case.working_dir.as_ref().unwrap_or(&defaults.working_dir));
    let exe = resolve_custom_program(ctx.repo_root, prog)?;
    let (code, stdout, stderr) =
        run_command_with_env(&exe, &case.args, &work, defaults, &case.env, timeout)?;
    let expect_ok = matches_expect(code, &case.expect);
    let warmup_failed_models = extract_warmup_failed_models(&stderr);
    let warmup_failed_count = warmup_failed_models.len() as u32;
    Ok(CaseResult {
        case_id: case.id.clone(),
        tags: case.tags.clone(),
        tier: case.tier.clone(),
        status: if expect_ok {
            CaseStatus::Pass
        } else {
            CaseStatus::Fail
        },
        duration_ms: 0,
        exit_code: Some(code),
        classification: if expect_ok {
            None
        } else {
            Some(classify_failure(&stdout, &stderr, code))
        },
        detail: None,
        stderr_tail: tail_string(&stderr),
        stdout_tail: tail_string(&stdout),
        stdout_len: stdout.len(),
        stderr_len: stderr.len(),
        omc_compare: None,
        input_hash: None,
        warmup_failed_count,
        warmup_failed_models,
        artifacts: Artifacts {
            rust_csv: None,
            working_dir: Some(work.to_string_lossy().to_string()),
            repro_bundle: None,
        },
    })
}

fn discover_regress_harness_exe(repo_root: &Path) -> Option<PathBuf> {
    let candidates = [
        repo_root.join("target/release/regress-harness.exe"),
        repo_root.join("target/debug/regress-harness.exe"),
        repo_root.join("target/release/regress-harness"),
        repo_root.join("target/debug/regress-harness"),
    ];
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for p in candidates {
        if let Ok(md) = std::fs::metadata(&p) {
            if !md.is_file() {
                continue;
            }
            let mt = md.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match best {
                None => best = Some((mt, p)),
                Some((best_mt, _)) => {
                    if mt >= best_mt {
                        best = Some((mt, p));
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

fn resolve_custom_program(repo_root: &Path, prog: &str) -> Result<PathBuf, String> {
    let p = Path::new(prog);
    if p.is_absolute() {
        return Ok(p.to_path_buf());
    }
    if prog.eq_ignore_ascii_case("regress-harness")
        || prog.eq_ignore_ascii_case("regress-harness.exe")
    {
        if let Some(x) = discover_regress_harness_exe(repo_root) {
            return Ok(x);
        }
        return Err("regress-harness exe not found (auto discovery failed)".to_string());
    }
    Ok(repo_root.join(p))
}

fn matches_expect(code: i32, expect: &crate::config::ExpectDef) -> bool {
    match expect.kind {
        ExpectKind::ExitZero => code == 0,
        ExpectKind::NonZero => code != 0,
        ExpectKind::ExitCode => code == expect.code.unwrap_or(0),
    }
}

fn run_command_with_env(
    program: &Path,
    args: &[String],
    cwd: &Path,
    defaults: &Defaults,
    extra_env: &std::collections::HashMap<String, String>,
    timeout: Duration,
) -> Result<(i32, String, String), String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.current_dir(cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    for (k, v) in &defaults.env {
        cmd.env(k, v);
    }
    for (k, v) in extra_env {
        cmd.env(k, v);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", program.display()))?;

    let stdout_pipe = child.stdout.take().ok_or_else(|| "stdout pipe".to_string())?;
    let stderr_pipe = child.stderr.take().ok_or_else(|| "stderr pipe".to_string())?;

    let (tx_out, rx_out) = mpsc::channel();
    let t1 = thread::spawn(move || {
        let mut s = String::new();
        let mut r = stdout_pipe;
        let _ = r.read_to_string(&mut s);
        let _ = tx_out.send(s);
    });
    let (tx_err, rx_err) = mpsc::channel();
    let t2 = thread::spawn(move || {
        let mut s = String::new();
        let mut r = stderr_pipe;
        let _ = r.read_to_string(&mut s);
        let _ = tx_err.send(s);
    });

    let start = Instant::now();
    let status = loop {
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = t1.join();
            let _ = t2.join();
            return Err("timeout".to_string());
        }
        if let Some(s) = child
            .wait_timeout(Duration::from_millis(50))
            .map_err(|e| e.to_string())?
        {
            break s;
        }
    };

    let _ = t1.join();
    let _ = t2.join();
    let stdout = rx_out.recv().unwrap_or_default();
    let stderr = rx_err.recv().unwrap_or_default();
    let code = status.code().unwrap_or(-1);
    Ok((code, stdout, stderr))
}

fn resolve_path(repo_root: &Path, p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
