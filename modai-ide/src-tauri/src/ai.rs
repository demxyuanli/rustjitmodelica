// DeepSeek API and API key storage for ModAI IDE.

use keyring::Entry;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "modai-ide";
const ACCOUNT: &str = "deepseek-api-key";
const DEEPSEEK_URL: &str = "https://api.deepseek.com/v1/chat/completions";
const MODEL: &str = "deepseek-coder-v2";

fn entry() -> Result<Entry, keyring::Error> {
    Entry::new(SERVICE, ACCOUNT)
}

pub fn get_api_key() -> Result<String, String> {
    let e = entry().map_err(|err| format!("keyring entry: {}", err))?;
    e.get_password().map_err(|e| format!("keyring get: {}", e))
}

pub fn set_api_key(api_key: &str) -> Result<(), String> {
    let e = entry().map_err(|err| format!("keyring entry: {}", err))?;
    e.set_password(api_key).map_err(|e| format!("keyring set: {}", e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Option<Vec<ChatChoice>>,
}

pub async fn deepseek_call(prompt: String, api_key: String) -> Result<String, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let body = ChatRequest {
        model: MODEL.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
    };

    let res = client
        .post(DEEPSEEK_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;

    if !status.is_success() {
        return Err(format!("API error {}: {}", status, text));
    }

    let parsed: ChatResponse = serde_json::from_str(&text).map_err(|e| format!("parse: {}", e))?;
    let content = parsed
        .choices
        .and_then(|c| c.into_iter().next())
        .map(|c| c.message.content)
        .ok_or_else(|| "No choices in response".to_string())?;

    Ok(content)
}

const COMPILER_PATCH_SYSTEM: &str = "You are a Rust compiler engineer. The user will describe a change or feature for the rustmodlica Modelica JIT compiler (Rust crate). \
Reply with ONLY a valid unified diff (patch) that can be applied with `patch -p1`. Do not include markdown code fences or any text before/after the diff. \
The diff must reference existing source files under src/ (e.g. src/compiler/mod.rs).";

pub async fn generate_compiler_patch(target: String) -> Result<String, String> {
    let api_key = get_api_key()?;
    let prompt = format!("{}\n\nUser goal: {}", COMPILER_PATCH_SYSTEM, target);
    deepseek_call(prompt, api_key).await
}
