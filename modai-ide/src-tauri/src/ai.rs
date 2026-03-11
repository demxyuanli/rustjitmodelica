// DeepSeek API and API key storage for ModAI IDE.

use keyring::Entry;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "modai-ide";
const ACCOUNT: &str = "deepseek-api-key";
pub const DEEPSEEK_URL: &str = "https://api.deepseek.com/v1/chat/completions";
pub const DEFAULT_MODEL: &str = "deepseek-coder-v2";

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
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessage,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Option<Vec<ChatChoice>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiContextBlock {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub range: Option<AiRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiOptions {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCodeGenPayload {
    pub prompt: String,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub context_blocks: Option<Vec<AiContextBlock>>,
    #[serde(default)]
    pub options: Option<AiOptions>,
}

pub async fn deepseek_call(prompt: String, api_key: String) -> Result<String, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let body = ChatRequest {
        model: DEFAULT_MODEL.to_string(),
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
The diff must reference existing source files under src/ (e.g. src/compiler/mod.rs). \
The repo has TestLib/ for Modelica regression models (.mo) and jit_traceability.json with a \"cases\" array (name, expected); when adding a compiler feature you may add a new TestLib/*.mo and a new entry in jit_traceability.json cases.";

pub async fn generate_compiler_patch(target: String) -> Result<String, String> {
    let api_key = get_api_key()?;
    let prompt = format!("{}\n\nUser goal: {}", COMPILER_PATCH_SYSTEM, target);
    deepseek_call(prompt, api_key).await
}

pub async fn generate_compiler_patch_with_context(
    target: String,
    context_files: Vec<String>,
    test_cases: Vec<String>,
) -> Result<String, String> {
    let api_key = get_api_key()?;

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .unwrap_or(&manifest_dir)
        .parent()
        .unwrap_or(&manifest_dir);

    let mut context_parts = Vec::new();
    for file_path in &context_files {
        let full_path = repo_root.join(file_path);
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            let truncated = if content.len() > 8000 {
                format!("{}...(truncated)", &content[..8000])
            } else {
                content
            };
            context_parts.push(format!("=== {} ===\n{}", file_path, truncated));
        }
    }

    let mut test_parts = Vec::new();
    for case_name in &test_cases {
        let mo_path = repo_root.join(format!("{}.mo", case_name.replace('/', std::path::MAIN_SEPARATOR_STR)));
        if let Ok(content) = std::fs::read_to_string(&mo_path) {
            test_parts.push(format!("=== {} ===\n{}", case_name, content));
        }
    }

    let mut prompt = format!("{}\n\n", COMPILER_PATCH_SYSTEM);
    if !context_parts.is_empty() {
        prompt.push_str("### Relevant compiler source files:\n\n");
        prompt.push_str(&context_parts.join("\n\n"));
        prompt.push_str("\n\n");
    }
    if !test_parts.is_empty() {
        prompt.push_str("### Related Modelica test cases:\n\n");
        prompt.push_str(&test_parts.join("\n\n"));
        prompt.push_str("\n\n");
    }
    prompt.push_str(&format!("### User goal:\n{}", target));

    deepseek_call(prompt, api_key).await
}
