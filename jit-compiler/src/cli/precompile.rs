use std::fs;

use super::RunError;

pub(crate) fn run_precompile(list_path: &str, lib_paths: &[String]) -> Result<(), RunError> {
    let text = fs::read_to_string(list_path)
        .map_err(|e| RunError::Message(format!("read '{}': {}", list_path, e)))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| RunError::Message(format!("json: {}", e)))?;
    let models: Vec<String> = if let Some(a) = v.as_array() {
        a.iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect()
    } else if let Some(a) = v.get("models").and_then(|x| x.as_array()) {
        a.iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        return Err(RunError::Message(
            "precompile json must be [\"A.B\", ...] or {\"models\":[\"A.B\"]}".into(),
        ));
    };
    if models.is_empty() {
        return Err(RunError::Message("precompile list is empty".into()));
    }
    let mut compiler = rustmodlica::Compiler::new();
    for p in lib_paths {
        compiler.loader.add_path(p.into());
    }
    for m in models {
        eprintln!("[precompile] {}", m);
        compiler
            .compile(&m)
            .map_err(|e| RunError::Message(format!("compile {}: {}", m, e)))?;
    }
    Ok(())
}
