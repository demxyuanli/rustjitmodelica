//! Stable hashes and JSON metadata for codegen disk cache.

use std::collections::HashMap;
use std::path::PathBuf;

/// Map parameter names to runtime values for const-fold cache validation.
pub fn param_values_by_name(param_vars: &[String], values: &[f64]) -> HashMap<String, f64> {
    param_vars
        .iter()
        .zip(values.iter().copied())
        .map(|(k, v)| (k.clone(), v))
        .collect()
}
use std::sync::OnceLock;

use xxhash_rust::xxh64::Xxh64;

use crate::jit::types::ArrayInfo;

/// Cache version incremented when on-disk format or serialization contract changes.
pub const CODEGEN_CACHE_VERSION: u32 = 4;

/// Bump when JIT `calc_derivs` ABI, raw blob layout, or reloc contract changes without a crate
/// version or IR epoch bump (dev builds otherwise keep the same `CARGO_PKG_VERSION`).
pub const CODEGEN_JIT_ABI_REVISION: u32 = 2;

/// Cranelift version for cache invalidation (keep aligned with workspace `cranelift-jit`).
const CRANELIFT_VERSION: &str = "0.128.4";

/// Target ISA identifier (e.g., "x86_64-unknown-linux-gnu").
pub fn target_isa_id() -> String {
    format!("{}-{}-{}", std::env::consts::OS, std::env::consts::ARCH, CRANELIFT_VERSION)
}

/// Host identity for JIT disk cache: crate version, flat IR epoch, and manual ABI revision.
pub fn codegen_host_tag() -> String {
    format!(
        "{}|ir{}|abi{}",
        env!("CARGO_PKG_VERSION"),
        crate::cache::ir_epoch::IR_SCHEMA_EPOCH,
        CODEGEN_JIT_ABI_REVISION
    )
}

/// Check if codegen cache is enabled via environment variable.
pub fn codegen_cache_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        match std::env::var("RUSTMODLICA_JIT_CODEGEN_CACHE") {
            Ok(v) => {
                let t = v.trim();
                if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                    return false;
                }
                t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") || t.is_empty()
            }
            Err(_) => true,
        }
    })
}

/// Get the cache root directory for codegen artifacts.
pub fn codegen_cache_root() -> Option<PathBuf> {
    let root = std::env::var("RUSTMODLICA_JIT_CODEGEN_CACHE_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            dirs::cache_dir().map(|d| d.join("rustmodlica").join("jit-codegen"))
        })?;
    Some(root)
}

/// Compute a stable hash of the flattened model for cache key generation.
pub fn flat_model_hash(
    model_name: &str,
    state_vars: &[String],
    discrete_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    array_info: &HashMap<String, ArrayInfo>,
    opt_level: &str,
    cache_variant: &str,
    _type_profile_hash: &str,
    _param_signature: &str,
    connector_connection_degree: Option<&HashMap<String, usize>>,
) -> String {
    let mut h = Xxh64::new(0);

    // Version and target
    h.update(&CODEGEN_CACHE_VERSION.to_le_bytes());
    h.update(target_isa_id().as_bytes());
    h.update(opt_level.as_bytes());
    h.update(cache_variant.as_bytes());
    // Structural disk key: param values and type profiles are runtime; code loads params via pointer.

    // Model identity
    h.update(model_name.as_bytes());

    if let Some(m) = connector_connection_degree {
        h.update(
            crate::jit::connector_degree::connector_degree_cache_digest(m).as_bytes(),
        );
    }

    // Variable lists (sorted for stability)
    let mut sorted_vars: Vec<&String> = state_vars.iter().collect();
    sorted_vars.sort();
    for v in &sorted_vars {
        h.update(v.as_bytes());
    }

    let mut sorted_discrete: Vec<&String> = discrete_vars.iter().collect();
    sorted_discrete.sort();
    for v in &sorted_discrete {
        h.update(v.as_bytes());
    }

    let mut sorted_params: Vec<&String> = param_vars.iter().collect();
    sorted_params.sort();
    for v in &sorted_params {
        h.update(v.as_bytes());
    }

    let mut sorted_outputs: Vec<&String> = output_vars.iter().collect();
    sorted_outputs.sort();
    for v in &sorted_outputs {
        h.update(v.as_bytes());
    }

    // Array info (sorted by name)
    let mut sorted_array_info: Vec<(&String, &ArrayInfo)> = array_info.iter().collect();
    sorted_array_info.sort_by_key(|(k, _)| *k);
    for (name, info) in sorted_array_info {
        h.update(name.as_bytes());
        h.update(&[info.array_type as u8]);
        h.update(&info.start_index.to_le_bytes());
        h.update(&info.size.to_le_bytes());
    }

    format!("{:016x}", h.digest())
}

/// Cache key structure stored alongside the object file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodegenCacheKey {
    pub version: u32,
    pub model_name: String,
    pub flat_hash: String,
    pub target_isa: String,
    pub opt_level: String,
    #[serde(default)]
    pub cache_variant: String,
    #[serde(default)]
    pub type_profile_hash: String,
    #[serde(default)]
    pub param_signature: String,
    /// Invalidates disk blobs across compiler releases and JIT ABI changes.
    #[serde(default)]
    pub host_tag: String,
    pub created_at: u64, // Unix timestamp
}

impl CodegenCacheKey {
    pub fn new(
        model_name: &str,
        flat_hash: &str,
        opt_level: &str,
        cache_variant: &str,
        type_profile_hash: &str,
        param_signature: &str,
    ) -> Self {
        Self {
            version: CODEGEN_CACHE_VERSION,
            model_name: model_name.to_string(),
            flat_hash: flat_hash.to_string(),
            target_isa: target_isa_id(),
            opt_level: opt_level.to_string(),
            cache_variant: cache_variant.to_string(),
            type_profile_hash: type_profile_hash.to_string(),
            param_signature: param_signature.to_string(),
            host_tag: codegen_host_tag(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn stable_hash(&self) -> String {
        let mut h = Xxh64::new(0);
        h.update(&self.version.to_le_bytes());
        h.update(self.model_name.as_bytes());
        h.update(self.flat_hash.as_bytes());
        h.update(self.target_isa.as_bytes());
        h.update(self.opt_level.as_bytes());
        h.update(self.cache_variant.as_bytes());
        h.update(self.host_tag.as_bytes());
        format!("{:016x}", h.digest())
    }
}

/// Object file entry in the cache.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodegenCacheEntry {
    pub key: CodegenCacheKey,
    pub object_size: usize,
    #[serde(default = "default_artifact_kind_raw")]
    pub artifact_kind: String,
    pub func_offset: u64, // Offset of calc_derivs in the object file
    pub func_size: usize, // Size of the function
    pub when_count: usize,
    pub crossings_count: usize,
    #[serde(default)]
    pub const_fold_params: Vec<(String, f64)>,
}

fn default_artifact_kind_raw() -> String {
    "raw".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_model_hash_stability() {
        let h1 = flat_model_hash(
            "TestModel",
            &["x".to_string(), "y".to_string()],
            &[],
            &["p".to_string()],
            &[],
            &HashMap::new(),
            "speed",
            "speed",
            "disabled",
            "disabled",
            None,
        );

        let h2 = flat_model_hash(
            "TestModel",
            &["x".to_string(), "y".to_string()],
            &[],
            &["p".to_string()],
            &[],
            &HashMap::new(),
            "speed",
            "speed",
            "disabled",
            "disabled",
            None,
        );

        assert_eq!(h1, h2);

        let h3 = flat_model_hash(
            "OtherModel",
            &["x".to_string(), "y".to_string()],
            &[],
            &["p".to_string()],
            &[],
            &HashMap::new(),
            "speed",
            "speed",
            "disabled",
            "disabled",
            None,
        );
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_codegen_cache_key_stability() {
        let k1 = CodegenCacheKey::new(
            "TestModel",
            "abc123",
            "speed",
            "speed",
            "disabled",
            "disabled",
        );
        let k2 = CodegenCacheKey::new(
            "TestModel",
            "abc123",
            "speed",
            "speed",
            "disabled",
            "disabled",
        );

        assert_eq!(k1.stable_hash(), k2.stable_hash());
    }
}
