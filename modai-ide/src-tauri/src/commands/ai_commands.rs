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
    use ai::{AiCodeGenPayload, AiOptions, ChatMessage, ChatRequest};

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

    let mut messages: Vec<ChatMessage> = Vec::new();
    if let Some(system) = parsed.system.as_ref() {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: system.clone(),
        });
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
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: format!("Relevant code/context:\n\n{}", ctx),
            });
        }
    }
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: parsed.prompt,
    });

    let mut body = ChatRequest {
        model: model.clone(),
        messages,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

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
        body.model = model.clone();
        res = client
            .post(ai::DEEPSEEK_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        status = res.status();
        text = res.text().await.map_err(|e| e.to_string())?;
    }

    if !status.is_success() {
        return Err(format!("API error {}: {}", status, text));
    }

    let parsed: ai::ChatResponse =
        serde_json::from_str(&text).map_err(|e| format!("parse: {}", e))?;
    parsed
        .choices
        .and_then(|c| c.into_iter().next())
        .map(|c| c.message.content)
        .ok_or_else(|| "No choices in response".to_string())
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
