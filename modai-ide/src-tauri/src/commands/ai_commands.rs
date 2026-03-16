use crate::ai;

#[tauri::command]
pub fn get_api_key() -> Result<String, String> {
    ai::get_api_key()
}

#[tauri::command]
pub fn set_api_key(api_key: String) -> Result<(), String> {
    ai::set_api_key(&api_key)
}

#[tauri::command]
pub async fn ai_code_gen(payload: serde_json::Value) -> Result<String, String> {
    use ai::{AiCodeGenPayload, AiOptions};
    use serde_json::json;

    let api_key = ai::get_api_key().map_err(|e| e.to_string())?;
    if let Some(prompt_str) = payload.as_str() {
        return ai::deepseek_call(prompt_str.to_string(), api_key).await;
    }

    let parsed: AiCodeGenPayload = serde_json::from_value(payload)
        .map_err(|e| format!("invalid ai_code_gen payload: {}", e))?;
    let requested_model = parsed
        .options
        .as_ref()
        .and_then(|o: &AiOptions| o.model.clone())
        .unwrap_or_else(|| ai::DEFAULT_MODEL.to_string());
    let mut model = if requested_model.trim() == "deepseek-coder-v2" {
        ai::DEFAULT_MODEL.to_string()
    } else {
        requested_model
    };

    let mut messages: Vec<serde_json::Value> = Vec::new();
    if let Some(system) = parsed.system.as_ref() {
        messages.push(json!({ "role": "system", "content": system }));
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

    let mut steps: u32 = 0;
    let max_steps: u32 = 8;
    loop {
        steps += 1;
        if steps > max_steps {
            return Err("tool loop limit reached".to_string());
        }

        let body = json!({
            "model": model,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto"
        });

        let mut res = client
            .post(ai::DEEPSEEK_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let mut status = res.status();
        let mut text = res.text().await.map_err(|e| e.to_string())?;
        if !status.is_success()
            && status.as_u16() == 400
            && text.contains("Model Not Exist")
            && model != ai::DEFAULT_MODEL
        {
            model = ai::DEFAULT_MODEL.to_string();
            let body_retry = json!({
                "model": model,
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
            return Ok(content);
        }

        // Save assistant message to keep context.
        messages.push(json!({ "role": "assistant", "content": content }));

        for tc in tool_calls {
            let call_id = tc.get("id").and_then(|x| x.as_str()).unwrap_or("tool_call").to_string();
            let func = tc.get("function").cloned().unwrap_or(json!({}));
            let name = func.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
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
