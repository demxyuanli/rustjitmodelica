use crate::ai;
use futures_util::StreamExt;
use serde_json::json;
use tauri::Emitter;

#[tauri::command]
pub fn get_api_key() -> Result<String, String> {
    ai::get_api_key()
}

#[tauri::command]
pub fn set_api_key(api_key: String) -> Result<(), String> {
    ai::set_api_key(&api_key)
}

#[tauri::command]
pub fn get_grok_api_key() -> Result<String, String> {
    ai::get_grok_api_key()
}

#[tauri::command]
pub fn set_grok_api_key(api_key: String) -> Result<(), String> {
    ai::set_grok_api_key(&api_key)
}

#[tauri::command]
pub async fn ai_code_gen(payload: serde_json::Value) -> Result<String, String> {
    use ai::{AiCodeGenPayload, AiOptions};

    if let Some(prompt_str) = payload.as_str() {
        let api_key = ai::get_api_key().map_err(|e| e.to_string())?;
        return ai::deepseek_call(prompt_str.to_string(), api_key).await;
    }

    let parsed: AiCodeGenPayload = serde_json::from_value(payload)
        .map_err(|e| format!("invalid ai_code_gen payload: {}", e))?;
    let requested_model = parsed
        .options
        .as_ref()
        .and_then(|o: &AiOptions| o.model.clone())
        .unwrap_or_else(|| ai::DEFAULT_MODEL.to_string());
    let model = if requested_model.trim() == "deepseek-coder-v2" {
        ai::DEFAULT_MODEL.to_string()
    } else {
        requested_model
    };

    let use_ollama = ai::is_ollama_model(&model);
    let use_grok = ai::is_grok_model(&model);
    let api_key = if use_ollama {
        String::new()
    } else if use_grok {
        ai::get_grok_api_key().map_err(|e| e.to_string())?
    } else {
        ai::get_api_key().map_err(|e| e.to_string())?
    };

    let mut messages: Vec<serde_json::Value> = Vec::new();
    if let Some(system) = parsed.system.as_ref() {
        messages.push(json!({ "role": "system", "content": system }));
    }

    // Inject AI rules/skills/subagents/commands from settings.json (AiConfig).
    if let Ok(settings) = crate::app_settings::load_settings() {
        let ai_cfg = settings.ai;

        let mut rules_text = String::new();
        for r in ai_cfg.rules.into_iter().filter(|r| r.enabled) {
            if !rules_text.is_empty() {
                rules_text.push_str("\n\n");
            }
            rules_text.push_str(&format!("# Rule: {}\n{}", r.name, r.content));
        }
        if !rules_text.is_empty() {
            messages.push(json!({ "role": "system", "content": format!("Global rules for this session:\n\n{}", rules_text) }));
        }

        let mut skills_text = String::new();
        for s in ai_cfg.skills.into_iter().filter(|s| s.enabled) {
            if !skills_text.is_empty() {
                skills_text.push_str("\n\n");
            }
            if s.description.is_empty() {
                skills_text.push_str(&format!("# Skill: {}\n{}", s.name, s.content));
            } else {
                skills_text.push_str(&format!("# Skill: {} - {}\n{}", s.name, s.description, s.content));
            }
        }
        if !skills_text.is_empty() {
            messages.push(json!({ "role": "system", "content": format!("Available skills:\n\n{}", skills_text) }));
        }

        let mut subagents_text = String::new();
        for a in ai_cfg.subagents.into_iter().filter(|a| a.enabled) {
            if !subagents_text.is_empty() {
                subagents_text.push_str("\n\n");
            }
            if a.description.is_empty() {
                subagents_text.push_str(&format!("# Subagent: {}\n{}", a.name, a.content));
            } else {
                subagents_text.push_str(&format!("# Subagent: {} - {}\n{}", a.name, a.description, a.content));
            }
        }
        if !subagents_text.is_empty() {
            messages.push(json!({ "role": "system", "content": format!("Subagents:\n\n{}", subagents_text) }));
        }

        let mut commands_text = String::new();
        for c in ai_cfg.commands.into_iter().filter(|c| c.enabled) {
            if !commands_text.is_empty() {
                commands_text.push_str("\n\n");
            }
            if c.description.is_empty() {
                commands_text.push_str(&format!("# Command: {}\n{}", c.name, c.content));
            } else {
                commands_text.push_str(&format!("# Command: {} - {}\n{}", c.name, c.description, c.content));
            }
        }
        if !commands_text.is_empty() {
            messages.push(json!({ "role": "system", "content": format!("Reusable workflows:\n\n{}", commands_text) }));
        }
    }

    if let Some(blocks) = parsed.context_blocks.as_ref() {
        if !blocks.is_empty() {
            let mut ctx = String::new();
            for b in blocks {
                ctx.push_str("=== ");
                ctx.push_str(&b.path);
                ctx.push_str(" ===\n");
                ctx.push_str(&b.content);
                ctx.push_str("\n\n");
            }
            messages.push(json!({ "role": "system", "content": format!("Relevant code/context:\n\n{}", ctx) }));
        }
    }
    messages.push(json!({ "role": "user", "content": parsed.prompt }));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let base_dir = parsed
        .project_dir
        .clone()
        .or_else(|| crate::commands::common::repo_root().ok().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_default();
    let tools = crate::ai_tools::tools_schema();
    let mut tool_calls_used: Vec<String> = Vec::new();

    let (api_url, mut model_for_body) = if use_ollama {
        let name = ai::ollama_model_name(&model).unwrap_or("llama3.2");
        (ai::OLLAMA_URL, name.to_string())
    } else if use_grok {
        let name = ai::grok_model_name(&model).unwrap_or("grok-2");
        (ai::GROK_URL, name.to_string())
    } else {
        (ai::DEEPSEEK_URL, model.clone())
    };

    let mut steps: u32 = 0;
    let max_steps: u32 = 8;
    loop {
        steps += 1;
        if steps > max_steps {
            return Err("tool loop limit reached".to_string());
        }

        let body = json!({
            "model": model_for_body,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto"
        });

        let mut req = client
            .post(api_url)
            .header("Content-Type", "application/json")
            .json(&body);
        if !use_ollama {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
        let mut res = req.send().await.map_err(|e| e.to_string())?;

        let mut status = res.status();
        let mut text = res.text().await.map_err(|e| e.to_string())?;
        if !use_ollama
            && !status.is_success()
            && status.as_u16() == 400
            && text.contains("Model Not Exist")
            && model != ai::DEFAULT_MODEL
        {
            model_for_body = ai::DEFAULT_MODEL.to_string();
            let body_retry = json!({
                "model": model_for_body,
                "messages": messages,
                "tools": tools,
                "tool_choice": "auto"
            });
            res = client
                .post(ai::DEEPSEEK_URL)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body_retry)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            status = res.status();
            text = res.text().await.map_err(|e| e.to_string())?;
        }

        if !status.is_success() {
            return Err(format!("API error {}: {}", status, text));
        }

        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("parse: {}", e))?;
        let msg = v
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c0| c0.get("message"))
            .ok_or_else(|| "No choices in response".to_string())?;

        let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
        let tool_calls = msg.get("tool_calls").and_then(|tc| tc.as_array()).cloned().unwrap_or_default();

        if tool_calls.is_empty() {
            let out = json!({
                "content": content,
                "tool_calls_used": tool_calls_used,
            });
            return Ok(out.to_string());
        }

        // Save assistant message to keep context.
        messages.push(json!({ "role": "assistant", "content": content }));

        for tc in tool_calls {
            let call_id = tc.get("id").and_then(|x| x.as_str()).unwrap_or("tool_call").to_string();
            let func = tc.get("function").cloned().unwrap_or(json!({}));
            let name = func.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            tool_calls_used.push(name.clone());
            let arg_str = func.get("arguments").and_then(|x| x.as_str()).unwrap_or("{}");
            let mut args_v: serde_json::Value =
                serde_json::from_str(arg_str).unwrap_or_else(|_| json!({}));

            // Enforce base_dir.
            if let serde_json::Value::Object(ref mut map) = args_v {
                if !map.contains_key("base_dir") {
                    map.insert("base_dir".to_string(), serde_json::Value::String(base_dir.clone()));
                }
            }

            let tool_out = crate::ai_tools::exec_tool(&name, &args_v)
                .map_err(|e| format!("tool {} failed: {}", name, e))?;

            // Return tool output back to model.
            messages.push(json!({ "role": "tool", "tool_call_id": call_id, "content": tool_out }));
        }
    }
}

#[tauri::command]
pub async fn ai_code_gen_stream(
    window: tauri::Window,
    request_id: String,
    payload: serde_json::Value,
) -> Result<(), String> {
    use ai::{AiCodeGenPayload, AiOptions};

    if payload.is_string() {
        return Err("string payload not supported for streaming".to_string());
    }

    let parsed: AiCodeGenPayload = serde_json::from_value(payload)
        .map_err(|e| format!("invalid ai_code_gen payload: {}", e))?;

    let requested_model = parsed
        .options
        .as_ref()
        .and_then(|o: &AiOptions| o.model.clone())
        .unwrap_or_else(|| ai::DEFAULT_MODEL.to_string());
    let model = if requested_model.trim() == "deepseek-coder-v2" {
        ai::DEFAULT_MODEL.to_string()
    } else {
        requested_model
    };

    let use_ollama = ai::is_ollama_model(&model);
    let use_grok = ai::is_grok_model(&model);
    let api_key = if use_ollama {
        String::new()
    } else if use_grok {
        ai::get_grok_api_key().map_err(|e| e.to_string())?
    } else {
        ai::get_api_key().map_err(|e| e.to_string())?
    };

    let (api_url, model_for_body) = if use_ollama {
        let name = ai::ollama_model_name(&model).unwrap_or("llama3.2");
        (ai::OLLAMA_URL, name.to_string())
    } else if use_grok {
        let name = ai::grok_model_name(&model).unwrap_or("grok-2");
        (ai::GROK_URL, name.to_string())
    } else {
        (ai::DEEPSEEK_URL, model.clone())
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let base_dir = parsed
        .project_dir
        .clone()
        .or_else(|| crate::commands::common::repo_root().ok().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_default();
    let tools = crate::ai_tools::tools_schema();
    let mut tool_calls_used: Vec<String> = Vec::new();

    let mut messages: Vec<serde_json::Value> = Vec::new();
    if let Some(system) = parsed.system.as_ref() {
        messages.push(json!({ "role": "system", "content": system }));
    }
    if let Ok(settings) = crate::app_settings::load_settings() {
        let ai_cfg = settings.ai;
        let mut rules_text = String::new();
        for r in ai_cfg.rules.into_iter().filter(|r| r.enabled) {
            if !rules_text.is_empty() {
                rules_text.push_str("\n\n");
            }
            rules_text.push_str(&format!("# Rule: {}\n{}", r.name, r.content));
        }
        if !rules_text.is_empty() {
            messages.push(json!({ "role": "system", "content": format!("Global rules for this session:\n\n{}", rules_text) }));
        }

        let mut skills_text = String::new();
        for s in ai_cfg.skills.into_iter().filter(|s| s.enabled) {
            if !skills_text.is_empty() {
                skills_text.push_str("\n\n");
            }
            if s.description.is_empty() {
                skills_text.push_str(&format!("# Skill: {}\n{}", s.name, s.content));
            } else {
                skills_text.push_str(&format!("# Skill: {} - {}\n{}", s.name, s.description, s.content));
            }
        }
        if !skills_text.is_empty() {
            messages.push(json!({ "role": "system", "content": format!("Available skills:\n\n{}", skills_text) }));
        }

        let mut subagents_text = String::new();
        for a in ai_cfg.subagents.into_iter().filter(|a| a.enabled) {
            if !subagents_text.is_empty() {
                subagents_text.push_str("\n\n");
            }
            if a.description.is_empty() {
                subagents_text.push_str(&format!("# Subagent: {}\n{}", a.name, a.content));
            } else {
                subagents_text.push_str(&format!("# Subagent: {} - {}\n{}", a.name, a.description, a.content));
            }
        }
        if !subagents_text.is_empty() {
            messages.push(json!({ "role": "system", "content": format!("Subagents:\n\n{}", subagents_text) }));
        }

        let mut commands_text = String::new();
        for c in ai_cfg.commands.into_iter().filter(|c| c.enabled) {
            if !commands_text.is_empty() {
                commands_text.push_str("\n\n");
            }
            if c.description.is_empty() {
                commands_text.push_str(&format!("# Command: {}\n{}", c.name, c.content));
            } else {
                commands_text.push_str(&format!("# Command: {} - {}\n{}", c.name, c.description, c.content));
            }
        }
        if !commands_text.is_empty() {
            messages.push(json!({ "role": "system", "content": format!("Reusable workflows:\n\n{}", commands_text) }));
        }
    }
    if let Some(blocks) = parsed.context_blocks.as_ref() {
        if !blocks.is_empty() {
            let mut ctx = String::new();
            for b in blocks {
                ctx.push_str("=== ");
                ctx.push_str(&b.path);
                ctx.push_str(" ===\n");
                ctx.push_str(&b.content);
                ctx.push_str("\n\n");
            }
            messages.push(json!({ "role": "system", "content": format!("Relevant code/context:\n\n{}", ctx) }));
        }
    }
    messages.push(json!({ "role": "user", "content": parsed.prompt }));

    let emit_delta = |delta: &str| {
        let _ = window.emit(
            "ai-stream-delta",
            json!({ "requestId": request_id, "delta": delta }),
        );
    };
    let emit_tool = |stage: &str, name: &str| {
        let _ = window.emit(
            "ai-stream-tool",
            json!({ "requestId": request_id, "stage": stage, "name": name }),
        );
    };
    let emit_error = |err: &str| {
        let _ = window.emit(
            "ai-stream-error",
            json!({ "requestId": request_id, "error": err }),
        );
    };
    let emit_done = |content: &str, tool_calls_used: &Vec<String>| {
        let _ = window.emit(
            "ai-stream-done",
            json!({ "requestId": request_id, "content": content, "toolCallsUsed": tool_calls_used }),
        );
    };

    #[derive(Default)]
    struct ToolCallAcc {
        id: String,
        name: String,
        args: String,
    }

    let mut steps: u32 = 0;
    let max_steps: u32 = 8;
    let mut final_content = String::new();

    loop {
        steps += 1;
        if steps > max_steps {
            emit_error("tool loop limit reached");
            return Err("tool loop limit reached".to_string());
        }

        let body = json!({
            "model": model_for_body,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto",
            "stream": true
        });

        let mut req = client
            .post(api_url)
            .header("Content-Type", "application/json")
            .json(&body);
        if !use_ollama {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        let res = req.send().await.map_err(|e| e.to_string())?;
        let status = res.status();
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            emit_error(&format!("API error {}: {}", status, text));
            return Err(format!("API error {}: {}", status, text));
        }

        let mut content = String::new();
        let mut tool_calls: Vec<ToolCallAcc> = Vec::new();
        let mut buf = String::new();

        let mut stream = res.bytes_stream();
        while let Some(next) = stream.next().await {
            let chunk = match next {
                Ok(c) => c,
                Err(e) => {
                    emit_error(&format!("stream error: {}", e));
                    return Err(e.to_string());
                }
            };
            let s = String::from_utf8_lossy(&chunk);
            buf.push_str(&s);

            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim_end_matches('\r').to_string();
                buf = buf[pos + 1..].to_string();
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if !line.starts_with("data:") {
                    continue;
                }
                let data = line.trim_start_matches("data:").trim();
                if data == "[DONE]" {
                    break;
                }
                let v: serde_json::Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let choice = v
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let delta = choice.get("delta").cloned().unwrap_or_else(|| json!({}));
                if let Some(c) = delta.get("content").and_then(|x| x.as_str()) {
                    if !c.is_empty() {
                        content.push_str(c);
                        final_content.push_str(c);
                        emit_delta(c);
                    }
                }
                if let Some(tc_arr) = delta.get("tool_calls").and_then(|x| x.as_array()) {
                    for tc in tc_arr {
                        let idx = tc.get("index").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                        while tool_calls.len() <= idx {
                            tool_calls.push(ToolCallAcc::default());
                        }
                        if let Some(id) = tc.get("id").and_then(|x| x.as_str()) {
                            tool_calls[idx].id = id.to_string();
                        }
                        let func = tc.get("function").cloned().unwrap_or_else(|| json!({}));
                        if let Some(name) = func.get("name").and_then(|x| x.as_str()) {
                            tool_calls[idx].name.push_str(name);
                        }
                        if let Some(args) = func.get("arguments").and_then(|x| x.as_str()) {
                            tool_calls[idx].args.push_str(args);
                        }
                    }
                }
            }
        }

        let has_tool_calls = tool_calls.iter().any(|tc| !tc.name.is_empty());
        if !has_tool_calls {
            emit_done(&final_content, &tool_calls_used);
            return Ok(());
        }

        messages.push(json!({ "role": "assistant", "content": content }));
        for tc in tool_calls.into_iter() {
            if tc.name.trim().is_empty() {
                continue;
            }
            let call_id = if tc.id.is_empty() { "tool_call".to_string() } else { tc.id };
            let name = tc.name.trim().to_string();
            tool_calls_used.push(name.clone());
            emit_tool("start", &name);

            let mut args_v: serde_json::Value =
                serde_json::from_str(&tc.args).unwrap_or_else(|_| json!({}));
            if let serde_json::Value::Object(ref mut map) = args_v {
                if !map.contains_key("base_dir") {
                    map.insert("base_dir".to_string(), serde_json::Value::String(base_dir.clone()));
                }
            }
            let tool_out = crate::ai_tools::exec_tool(&name, &args_v)
                .map_err(|e| format!("tool {} failed: {}", name, e))?;
            emit_tool("end", &name);
            messages.push(json!({ "role": "tool", "tool_call_id": call_id, "content": tool_out }));
        }
    }
}

#[tauri::command]
pub async fn ai_generate_compiler_patch(target: String) -> Result<String, String> {
    ai::generate_compiler_patch(target).await
}

#[tauri::command]
pub async fn ai_generate_compiler_patch_with_context(
    target: String,
    context_files: Vec<String>,
    test_cases: Vec<String>,
) -> Result<String, String> {
    ai::generate_compiler_patch_with_context(target, context_files, test_cases).await
}
