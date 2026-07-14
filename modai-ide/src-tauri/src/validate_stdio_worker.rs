//! Process-level singleton: one long-lived `rustmodlica --validate-stdio` worker per
//! compatible spawn configuration (library paths, validate tier, validation mode).
//! Reuses the same subprocess across IDE validate calls for warm loader + Salsa DB.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, SyncSender};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::compiler_config;

#[derive(Debug, Clone)]
pub struct ValidateWorkerRequest {
    pub model_name: String,
    pub code: String,
    pub lib_paths: Vec<PathBuf>,
    pub validate_tier: String,
    pub validation_mode: String,
    pub eq_expand_parallel_mode: String,
    pub coarse_constrainedby_only: bool,
}

#[derive(Debug, Clone)]
pub struct ValidateWorkerResponse {
    pub success: bool,
    pub warnings: Vec<WorkerWarning>,
    pub errors: Vec<String>,
    pub state_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub validation_stop_phase: Option<String>,
    pub validation_partial: bool,
    pub compile_perf: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerWarning {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerValidateJson {
    success: bool,
    warnings: Vec<WorkerWarning>,
    errors: Vec<String>,
    state_vars: Vec<String>,
    output_vars: Vec<String>,
    validation_stop_phase: Option<String>,
    validation_partial: bool,
    #[serde(default)]
    compile_perf: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
struct WorkerSpawnConfig {
    exe: PathBuf,
    extra_args: Vec<String>,
    lib_paths: Vec<PathBuf>,
    validate_tier: String,
    validation_mode: String,
    eq_expand_parallel_mode: String,
    coarse_constrainedby_only: bool,
}

impl WorkerSpawnConfig {
    fn fingerprint(&self) -> String {
        let mut paths: Vec<String> = self
            .lib_paths
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        paths.sort();
        format!(
            "tier={}|mode={}|eq={}|coarse={}|args={}|libs={}",
            self.validate_tier,
            self.validation_mode,
            self.eq_expand_parallel_mode,
            self.coarse_constrainedby_only,
            self.extra_args.join(","),
            paths.join("|"),
        )
    }
}

struct WorkerState {
    config_hash: String,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

enum ActorMessage {
    Validate {
        request: ValidateWorkerRequest,
        respond: SyncSender<Result<ValidateWorkerResponse, String>>,
    },
}

fn apply_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

fn spawn_worker(config: &WorkerSpawnConfig) -> Result<WorkerState, String> {
    let mut cmd = Command::new(&config.exe);
    cmd.args(&config.extra_args);
    cmd.arg("--validate-stdio");
    cmd.arg(format!("--validate-tier={}", config.validate_tier));
    cmd.arg(format!("--validation-mode={}", config.validation_mode));
    if config.coarse_constrainedby_only {
        cmd.arg("--coarse-constrainedby");
    }
    for p in &config.lib_paths {
        cmd.arg(format!("--lib-path={}", p.display()));
    }
    cmd.env("RUSTMODLICA_SALSA", "1");
    cmd.env("RUSTMODLICA_SALSA_PROCESS_DB", "1");
    cmd.env(
        "RUSTMODLICA_EQ_EXPAND_PARALLEL_MODE",
        config.eq_expand_parallel_mode.as_str(),
    );
    apply_no_window(&mut cmd);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("spawn validate-stdio worker: {e}"))?;
    let stdin = child.stdin.take().ok_or("worker stdin unavailable")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("worker stdout unavailable")?;
    let stderr = child.stderr.take();
    if let Some(err) = stderr {
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = BufReader::new(err).read_to_string(&mut buf);
        });
    }
    Ok(WorkerState {
        config_hash: config.fingerprint(),
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}

fn shutdown_worker(state: &mut Option<WorkerState>) {
    if let Some(mut ws) = state.take() {
        let _ = writeln!(ws.stdin, r#"{{"quit":true}}"#);
        let _ = ws.stdin.flush();
        let _ = ws.child.kill();
        let _ = ws.child.wait();
    }
}

fn ensure_worker<'a>(
    slot: &'a mut Option<WorkerState>,
    config: &WorkerSpawnConfig,
) -> Result<&'a mut WorkerState, String> {
    let want = config.fingerprint();
    let needs_respawn = match slot.as_ref() {
        None => true,
        Some(ws) => ws.config_hash != want,
    };
    if !needs_respawn {
        return Ok(slot.as_mut().expect("worker slot"));
    }
    shutdown_worker(slot);
    *slot = Some(spawn_worker(config)?);
    Ok(slot.as_mut().expect("worker slot"))
}

fn read_worker_json(stdout: &mut BufReader<std::process::ChildStdout>) -> Result<String, String> {
    let mut collected = String::new();
    let mut last_json = String::new();
    loop {
        collected.clear();
        let n = stdout
            .read_line(&mut collected)
            .map_err(|e| format!("worker stdout read failed: {e}"))?;
        if n == 0 {
            break;
        }
        let t = collected.trim();
        if t.starts_with('{') {
            last_json = t.to_string();
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
                if v.get("success").is_some() {
                    break;
                }
            }
        }
    }
    if last_json.is_empty() {
        return Err("validate-stdio worker returned no JSON".to_string());
    }
    Ok(last_json)
}

fn validate_on_worker(
    slot: &mut Option<WorkerState>,
    spawn: &WorkerSpawnConfig,
    request: &ValidateWorkerRequest,
) -> Result<ValidateWorkerResponse, String> {
    let ws = ensure_worker(slot, spawn)?;
    let payload = serde_json::json!({
        "model": request.model_name,
        "code": request.code,
        "embed_perf": true,
    });
    writeln!(ws.stdin, "{}", payload).map_err(|e| format!("worker stdin write: {e}"))?;
    ws.stdin
        .flush()
        .map_err(|e| format!("worker stdin flush: {e}"))?;
    let json = read_worker_json(&mut ws.stdout)?;
    if let Ok(Some(_status)) = ws.child.try_wait() {
        shutdown_worker(slot);
        return Err("validate-stdio worker exited unexpectedly".to_string());
    }
    let parsed: WorkerValidateJson =
        serde_json::from_str(&json).map_err(|e| format!("worker JSON parse: {e}"))?;
    Ok(ValidateWorkerResponse {
        success: parsed.success,
        warnings: parsed.warnings,
        errors: parsed.errors,
        state_vars: parsed.state_vars,
        output_vars: parsed.output_vars,
        validation_stop_phase: parsed.validation_stop_phase,
        validation_partial: parsed.validation_partial,
        compile_perf: parsed.compile_perf,
    })
}

fn handle_validate(
    slot: &mut Option<WorkerState>,
    request: ValidateWorkerRequest,
) -> Result<ValidateWorkerResponse, String> {
    let repo_root = crate::commands::common::jit_compiler_root()?;
    let (exe, extra_args) = compiler_config::resolve_compiler_exe(&repo_root)?;
    let spawn = WorkerSpawnConfig {
        exe,
        extra_args,
        lib_paths: request.lib_paths.clone(),
        validate_tier: request.validate_tier.clone(),
        validation_mode: request.validation_mode.clone(),
        eq_expand_parallel_mode: request.eq_expand_parallel_mode.clone(),
        coarse_constrainedby_only: request.coarse_constrainedby_only,
    };
    validate_on_worker(slot, &spawn, &request)
}

fn actor_loop(rx: mpsc::Receiver<ActorMessage>) {
    let mut slot: Option<WorkerState> = None;
    while let Ok(msg) = rx.recv() {
        match msg {
            ActorMessage::Validate { request, respond } => {
                let result = handle_validate(&mut slot, request);
                let _ = respond.send(result);
            }
        }
    }
    shutdown_worker(&mut slot);
}

fn sender() -> &'static mpsc::Sender<ActorMessage> {
    static SENDER: OnceLock<mpsc::Sender<ActorMessage>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("validate-stdio-worker".into())
            .spawn(move || actor_loop(rx))
            .expect("spawn validate-stdio worker actor");
        tx
    })
}

pub fn validate(request: ValidateWorkerRequest) -> Result<ValidateWorkerResponse, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    sender()
        .send(ActorMessage::Validate { request, respond: tx })
        .map_err(|e| format!("validate worker actor unavailable: {e}"))?;
    rx.recv()
        .map_err(|e| format!("validate worker actor dropped: {e}"))?
}
