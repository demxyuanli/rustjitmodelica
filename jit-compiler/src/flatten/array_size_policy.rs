//! Optional array dimension policy and external size map for flatten (decl_expand).

use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ArraySizePolicy {
    /// On unevaluated `array_size`, warn (unless warnings suppressed) and treat as scalar (count 1).
    #[default]
    Legacy,
    /// On unevaluated size with no external override, fail flatten.
    Strict,
}

impl ArraySizePolicy {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "strict" => ArraySizePolicy::Strict,
            _ => ArraySizePolicy::Legacy,
        }
    }
}

/// Load `{"array_sizes": { "<flat_base_name>": <usize>, ... }}` from JSON.
/// Keys match flattened declaration base names used in `FlattenedModel::array_sizes` (underscore-separated path).
pub fn load_array_sizes_json(path: &Path) -> Result<HashMap<String, usize>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let obj = v
        .get("array_sizes")
        .and_then(|x| x.as_object())
        .ok_or_else(|| {
            "expected top-level object with \"array_sizes\" map (string keys to integer sizes)".to_string()
        })?;
    let mut m = HashMap::new();
    for (k, val) in obj {
        let n = val
            .as_u64()
            .ok_or_else(|| format!("array_sizes[\"{}\"] must be a non-negative integer", k))?;
        if n == 0 {
            return Err(format!("array_sizes[\"{}\"] must be positive", k));
        }
        if n > usize::MAX as u64 {
            return Err(format!("array_sizes[\"{}\"] too large for this platform", k));
        }
        m.insert(k.clone(), n as usize);
    }
    Ok(m)
}

pub fn load_array_sizes_json_optional(path: Option<&Path>) -> Result<HashMap<String, usize>, String> {
    match path {
        None => Ok(HashMap::new()),
        Some(p) => load_array_sizes_json(p),
    }
}
