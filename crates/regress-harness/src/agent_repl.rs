//! Line-based JSON REPL for AI agents.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use wait_timeout::ChildExt;
use crate::i18n::tr;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AgentContextState {
    config: Option<String>,
    data_root: Option<String>,
    tier: Option<String>,
    tags: Option<Vec<String>>,
    incremental: Option<String>,
    workers: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AgentCmd {
    cmd: String,
    #[serde(default)]
    config: Option<String>,
    #[serde(default)]
    data_root: Option<String>,
    #[serde(default)]
    tier: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    incremental: Option<String>,
    #[serde(default)]
    workers: Option<usize>,
    #[serde(default)]
    baseline: Option<String>,
    #[serde(default)]
    manifest: Option<String>,
    #[serde(default)]
    ndjson: Option<bool>,
    #[serde(default)]
    summary_compat: Option<bool>,
    #[serde(default)]
    progress: Option<bool>,
    #[serde(default)]
    tail: Option<usize>,
    #[serde(default)]
    follow_seconds: Option<u64>,
    #[serde(default)]
    max_lines: Option<usize>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    api_base: Option<String>,
    #[serde(default)]
    normalize: Option<bool>,
}

fn merged_str(v: Option<String>, default: Option<String>) -> Option<String> {
    v.or(default)
}

fn merged_vec(v: Option<Vec<String>>, default: Option<Vec<String>>) -> Option<Vec<String>> {
    v.or(default)
}

fn build_common_args(cmd: &AgentCmd, ctx: &AgentContextState, include_incremental: bool) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(c) = merged_str(cmd.config.clone(), ctx.config.clone()) {
        out.extend(["--config".to_string(), c]);
    }
    if let Some(t) = merged_str(cmd.tier.clone(), ctx.tier.clone()) {
        out.extend(["--tier".to_string(), t]);
    }
    if let Some(tags) = merged_vec(cmd.tags.clone(), ctx.tags.clone()) {
        if !tags.is_empty() {
            out.extend(["--tags".to_string(), tags.join(",")]);
        }
    }
    if let Some(w) = cmd.workers.or(ctx.workers) {
        out.extend(["--workers".to_string(), w.to_string()]);
    }
    if let Some(dr) = merged_str(cmd.data_root.clone(), ctx.data_root.clone()) {
        out.extend(["--data-root".to_string(), dr]);
    }
    if include_incremental {
        if let Some(inc) = merged_str(cmd.incremental.clone(), ctx.incremental.clone()) {
            out.extend(["--incremental".to_string(), inc]);
        }
        if let Some(b) = &cmd.baseline {
            out.extend(["--baseline".to_string(), b.clone()]);
        }
        if let Some(m) = &cmd.manifest {
            out.extend(["--manifest".to_string(), m.clone()]);
        }
    }
    out
}

fn exec_cli(exe: &PathBuf, args: &[String]) -> Result<(i32, String, String)> {
    let out = Command::new(exe)
        .args(args)
        .output()
        .with_context(|| format!("spawn {} {}", exe.display(), args.join(" ")))?;
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    Ok((code, stdout, stderr))
}

fn exec_cli_timeout(
    exe: &PathBuf,
    args: &[String],
    timeout: Duration,
) -> Result<(i32, String, String, bool)> {
    let mut child = Command::new(exe)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn {} {}", exe.display(), args.join(" ")))?;

    let timed_out = child.wait_timeout(timeout)?.is_none();
    if timed_out {
        let _ = child.kill();
    }
    let out = child.wait_with_output()?;
    let code = out.status.code().unwrap_or(if timed_out { 124 } else { -1 });
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    Ok((code, stdout, stderr, timed_out))
}

fn response_ok(result: Value) -> Value {
    json!({ "ok": true, "result": result })
}

fn response_err(err: &str) -> Value {
    json!({ "ok": false, "error": err })
}

fn keep_last_lines(s: String, max_lines: Option<usize>) -> String {
    let Some(max) = max_lines else {
        return s;
    };
    if max == 0 {
        return String::new();
    }
    let lines: Vec<&str> = s.lines().collect();
    let start = lines.len().saturating_sub(max);
    lines[start..].join("\n")
}

fn extract_answer(v: &Value) -> Option<String> {
    v.get("choices")
        .and_then(|x| x.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
}

fn extract_thinking(v: &Value) -> Option<String> {
    let msg = v
        .get("choices")
        .and_then(|x| x.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("message"));

    let from_reasoning_content = msg
        .and_then(|m| m.get("reasoning_content"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    if from_reasoning_content.is_some() {
        return from_reasoning_content;
    }

    let from_reasoning = msg
        .and_then(|m| m.get("reasoning"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    if from_reasoning.is_some() {
        return from_reasoning;
    }

    msg.and_then(|m| m.get("reasoning"))
        .map(|x| x.to_string())
}

fn call_deepseek_chat(api_key: &str, api_base: &str, model: &str, prompt: &str) -> Result<Value> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;
    let body = json!({
        "model": model,
        "messages": [
            {"role":"user", "content": prompt}
        ],
        "stream": false
    });
    let resp = client
        .post(api_base)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()?;
    let status = resp.status();
    let text = resp.text()?;
    let parsed = serde_json::from_str::<Value>(&text).unwrap_or(json!({ "raw": text }));
    Ok(json!({
        "http_status": status.as_u16(),
        "response": parsed
    }))
}

pub fn run_agent_repl() -> Result<()> {
    let exe = std::env::current_exe().context("current_exe")?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut state = AgentContextState::default();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            writeln!(stdout, "{}", response_err(tr("agent_err_empty_command")))?;
            stdout.flush()?;
            continue;
        }

        let parsed: AgentCmd = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                writeln!(
                    stdout,
                    "{}",
                    response_err(&format!("{}: {e}", tr("agent_err_invalid_json")))
                )?;
                stdout.flush()?;
                continue;
            }
        };

        let out = match parsed.cmd.as_str() {
            "help" => response_ok(json!({
                "help_text": tr("agent_help_text"),
                "commands": [
                    "help",
                    "set_context",
                    "plan",
                    "run",
                    "status",
                    "monitor_tail",
                    "monitor_follow",
                    "deepseek_chat",
                    "deepseek_reasoner",
                    "glm_chat",
                    "agent_context",
                    "quit"
                ]
            })),
            "set_context" => {
                if parsed.config.is_some() {
                    state.config = parsed.config.clone();
                }
                if parsed.data_root.is_some() {
                    state.data_root = parsed.data_root.clone();
                }
                if parsed.tier.is_some() {
                    state.tier = parsed.tier.clone();
                }
                if parsed.tags.is_some() {
                    state.tags = parsed.tags.clone();
                }
                if parsed.incremental.is_some() {
                    state.incremental = parsed.incremental.clone();
                }
                if parsed.workers.is_some() {
                    state.workers = parsed.workers;
                }
                response_ok(json!({ "context": state }))
            }
            "plan" => {
                let mut args = vec!["plan".to_string()];
                args.extend(build_common_args(&parsed, &state, true));
                args.extend(["--format".to_string(), "json".to_string()]);
                match exec_cli(&exe, &args) {
                    Ok((code, s, e)) => {
                        let parsed_out = serde_json::from_str::<Value>(&s).unwrap_or(json!({ "raw_stdout": s }));
                        response_ok(json!({ "exit_code": code, "result": parsed_out, "stderr": e }))
                    }
                    Err(err) => response_err(&err.to_string()),
                }
            }
            "run" => {
                let mut args = vec!["run".to_string()];
                args.extend(build_common_args(&parsed, &state, true));
                if parsed.ndjson.unwrap_or(false) {
                    args.push("--ndjson".to_string());
                }
                if parsed.summary_compat.unwrap_or(false) {
                    args.push("--summary-compat".to_string());
                }
                if parsed.progress.unwrap_or(false) {
                    args.push("--progress".to_string());
                }
                match exec_cli(&exe, &args) {
                    Ok((code, s, e)) => {
                        let dr = merged_str(parsed.data_root.clone(), state.data_root.clone())
                            .unwrap_or_else(|| "build/regression_data".to_string());
                        let report_path = PathBuf::from(&dr).join("report.json");
                        let summary = if report_path.exists() {
                            match std::fs::read_to_string(&report_path) {
                                Ok(text) => serde_json::from_str::<Value>(&text).ok()
                                    .and_then(|v| v.get("summary").cloned()),
                                Err(_) => None,
                            }
                        } else {
                            None
                        };
                        response_ok(json!({
                            "exit_code": code,
                            "report_path": report_path,
                            "summary": summary,
                            "stdout": s,
                            "stderr": e
                        }))
                    }
                    Err(err) => response_err(&err.to_string()),
                }
            }
            "status" => {
                let mut args = vec!["status".to_string()];
                let dr = merged_str(parsed.data_root.clone(), state.data_root.clone())
                    .unwrap_or_else(|| "build/regression_data".to_string());
                args.extend(["--data-root".to_string(), dr]);
                args.extend(["--format".to_string(), "json".to_string()]);
                match exec_cli(&exe, &args) {
                    Ok((code, s, e)) => {
                        let parsed_out = serde_json::from_str::<Value>(&s).unwrap_or(json!({ "raw_stdout": s }));
                        response_ok(json!({ "exit_code": code, "result": parsed_out, "stderr": e }))
                    }
                    Err(err) => response_err(&err.to_string()),
                }
            }
            "monitor_tail" => {
                let mut args = vec!["monitor".to_string()];
                let dr = merged_str(parsed.data_root.clone(), state.data_root.clone())
                    .unwrap_or_else(|| "build/regression_data".to_string());
                args.extend(["--data-root".to_string(), dr]);
                args.extend([
                    "--tail".to_string(),
                    parsed.tail.unwrap_or(20).to_string(),
                ]);
                match exec_cli(&exe, &args) {
                    Ok((code, s, e)) => response_ok(json!({
                        "exit_code": code,
                        "stdout": keep_last_lines(s, parsed.max_lines),
                        "stderr": keep_last_lines(e, parsed.max_lines)
                    })),
                    Err(err) => response_err(&err.to_string()),
                }
            }
            "monitor_follow" => {
                let mut args = vec!["monitor".to_string()];
                let dr = merged_str(parsed.data_root.clone(), state.data_root.clone())
                    .unwrap_or_else(|| "build/regression_data".to_string());
                args.extend(["--data-root".to_string(), dr]);
                args.extend(["--follow".to_string()]);
                if let Some(t) = parsed.tail {
                    args.extend(["--tail".to_string(), t.to_string()]);
                }
                let secs = parsed.follow_seconds.unwrap_or(5).max(1);
                match exec_cli_timeout(&exe, &args, Duration::from_secs(secs)) {
                    Ok((code, s, e, timed_out)) => response_ok(json!({
                        "exit_code": code,
                        "timed_out": timed_out,
                        "follow_seconds": secs,
                        "stdout": keep_last_lines(s, parsed.max_lines),
                        "stderr": keep_last_lines(e, parsed.max_lines)
                    })),
                    Err(err) => response_err(&err.to_string()),
                }
            }
            "deepseek_chat" | "deepseek_reasoner" | "glm_chat" => {
                let prompt = match parsed.prompt.clone() {
                    Some(p) if !p.trim().is_empty() => p,
                    _ => {
                        let err = response_err(tr("agent_err_missing_prompt"));
                        writeln!(stdout, "{}", err)?;
                        stdout.flush()?;
                        continue;
                    }
                };
                let api_key = match std::env::var("DEEPSEEK_API_KEY") {
                    Ok(v) if !v.trim().is_empty() => v,
                    _ => {
                        let err = response_err(tr("agent_err_deepseek_key_not_set"));
                        writeln!(stdout, "{}", err)?;
                        stdout.flush()?;
                        continue;
                    }
                };
                let api_base = parsed
                    .api_base
                    .clone()
                    .unwrap_or_else(|| "https://api.deepseek.com/chat/completions".to_string());
                let model = parsed.model.clone().unwrap_or_else(|| {
                    if parsed.cmd == "deepseek_reasoner" {
                        "deepseek-reasoner".to_string()
                    } else {
                        "deepseek-chat".to_string()
                    }
                });
                let normalize = parsed.normalize.unwrap_or(true);
                match call_deepseek_chat(&api_key, &api_base, &model, &prompt) {
                    Ok(v) => {
                        let answer = v.get("response").and_then(extract_answer).unwrap_or_default();
                        let thinking = v
                            .get("response")
                            .and_then(extract_thinking)
                            .unwrap_or_default();
                        if normalize {
                            response_ok(json!({
                                "provider": "deepseek",
                                "model": model,
                                "answer": answer,
                                "thinking": thinking
                            }))
                        } else {
                            response_ok(json!({
                                "provider": "deepseek",
                                "model": model,
                                "api_base": api_base,
                                "answer": answer,
                                "thinking": thinking,
                                "result": v
                            }))
                        }
                    }
                    Err(err) => response_err(&format!(
                        "{}: {err}",
                        tr("agent_err_deepseek_call_failed")
                    )),
                }
            }
            "agent_context" => {
                let mut args = vec!["agent-context".to_string()];
                let dr = merged_str(parsed.data_root.clone(), state.data_root.clone())
                    .unwrap_or_else(|| "build/regression_data".to_string());
                args.extend(["--data-root".to_string(), dr]);
                if let Some(c) = merged_str(parsed.config.clone(), state.config.clone()) {
                    args.extend(["--config".to_string(), c]);
                }
                match exec_cli(&exe, &args) {
                    Ok((code, s, e)) => {
                        let parsed_out = serde_json::from_str::<Value>(&s).unwrap_or(json!({ "raw_stdout": s }));
                        response_ok(json!({ "exit_code": code, "result": parsed_out, "stderr": e }))
                    }
                    Err(err) => response_err(&err.to_string()),
                }
            }
            "quit" => {
                let o = response_ok(json!({ "bye": true }));
                writeln!(stdout, "{}", o)?;
                stdout.flush()?;
                break;
            }
            _ => response_err(tr("agent_err_unknown_command")),
        };

        writeln!(stdout, "{}", out)?;
        stdout.flush()?;
    }
    Ok(())
}
