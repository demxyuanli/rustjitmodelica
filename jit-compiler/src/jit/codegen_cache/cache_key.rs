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

use crate::cache::build_id::binary_build_id;
use crate::cache::cache_scope::CacheScope;
use crate::flatten::flatten_cache::{flatten_cache_dir, std_cache_root, user_cache_root};
use crate::jit::types::ArrayInfo;
use std::collections::HashSet;

fn default_codegen_cache_scope() -> CacheScope {
    CacheScope::Project
}

/// Cache version incremented when on-disk format or serialization contract changes.
pub const CODEGEN_CACHE_VERSION: u32 = 5;

/// Bump when JIT `calc_derivs` ABI, raw blob layout, or reloc contract changes without a crate
/// version or IR epoch bump (dev builds otherwise keep the same `CARGO_PKG_VERSION`).
/// Rev 3: when_count/crossings_count now captured after all equation compilation (fix buffer truncation).
pub const CODEGEN_JIT_ABI_REVISION: u32 = 3;

/// Cranelift version for cache invalidation (keep aligned with workspace `cranelift-jit`).
const CRANELIFT_VERSION: &str = "0.128.4";

/// Target ISA identifier (e.g., "x86_64-unknown-linux-gnu").
pub fn target_isa_id() -> String {
    format!("{}-{}-{}", std::env::consts::OS, std::env::consts::ARCH, CRANELIFT_VERSION)
}

/// Host identity for JIT disk cache: crate version, flat IR epoch, manual ABI revision, and a
/// per-binary fingerprint (`binary_build_id`).
///
/// The `bid:...` component ties every codegen cache key — and therefore every `.bin` filename
/// and AOT TOC entry — to the exact running `rustmodlica.exe`. Rewriting the binary changes
/// `binary_build_id`, which flows into [`CodegenCacheKey::stable_hash`] and in turn into the
/// disk filename under `jit-codegen/`, so old machine-code blobs become unreachable (not
/// served, not looked up) even when the crate version / IR epoch / ABI revision have not been
/// bumped by hand. This closes the same "warm-cache -> Windows access violation" gap the
/// SQLite parent-hash chain covers for flatten/IR rows, but for the native code tier.
pub fn codegen_host_tag() -> String {
    format!(
        "{}|ir{}|abi{}|{}",
        env!("CARGO_PKG_VERSION"),
        crate::cache::ir_epoch::IR_SCHEMA_EPOCH,
        CODEGEN_JIT_ABI_REVISION,
        binary_build_id()
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

const JIT_CODEGEN_REL: &str = "jit-codegen";

fn legacy_global_codegen_dir() -> Option<PathBuf> {
    let from_env = std::env::var("RUSTMODLICA_JIT_CODEGEN_CACHE_DIR").ok().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(PathBuf::from(t))
        }
    });
    from_env.or_else(|| dirs::cache_dir().map(|d| d.join("rustmodlica").join(JIT_CODEGEN_REL)))
}

/// Legacy single-root codegen cache (env `RUSTMODLICA_JIT_CODEGEN_CACHE_DIR` or host cache dir).
/// When set, tiered roots are not used for JIT object I/O.
pub fn codegen_cache_legacy_dir() -> Option<PathBuf> {
    legacy_global_codegen_dir()
}

/// On-disk directory for writes for this tier (under std / user / project cache root).
pub fn codegen_cache_write_dir_for_scope(scope: CacheScope) -> Option<PathBuf> {
    if let Some(g) = codegen_cache_legacy_dir() {
        return Some(g);
    }
    let base = match scope {
        CacheScope::GlobalStd => std_cache_root().or_else(flatten_cache_dir)?,
        CacheScope::UserExt => user_cache_root().or_else(flatten_cache_dir)?,
        CacheScope::Project => flatten_cache_dir()?,
    };
    Some(base.join(JIT_CODEGEN_REL))
}

/// Read search order: same physical-root priority as flatten SQLite (`Project` -> user -> std, etc.).
pub fn codegen_cache_read_dirs_for_scope(primary: CacheScope) -> Vec<PathBuf> {
    if let Some(g) = codegen_cache_legacy_dir() {
        return vec![g];
    }
    let p = flatten_cache_dir();
    let u = user_cache_root();
    let s = std_cache_root();
    let ordered: Vec<Option<PathBuf>> = match primary {
        CacheScope::Project => vec![p, u, s],
        CacheScope::UserExt => vec![u, p, s],
        CacheScope::GlobalStd => vec![s, u, p],
    };
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for opt in ordered {
        if let Some(r) = opt {
            let j = r.join(JIT_CODEGEN_REL);
            if seen.insert(j.clone()) {
                out.push(j);
            }
        }
    }
    out
}

/// Primary project-tier codegen directory (for diagnostics and tools that expect one path).
pub fn codegen_cache_root() -> Option<PathBuf> {
    codegen_cache_write_dir_for_scope(CacheScope::Project)
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
    /// Tier used when composing this key (affects [`CodegenCacheKey::stable_hash`]).
    #[serde(default = "default_codegen_cache_scope")]
    pub cache_scope: CacheScope,
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
        cache_scope: CacheScope,
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
            cache_scope,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn stable_hash(&self) -> String {
        let mut h = Xxh64::new(0);
        h.update(&self.version.to_le_bytes());
        h.update(self.cache_scope.prefix().as_bytes());
        h.update(self.model_name.as_bytes());
        h.update(self.flat_hash.as_bytes());
        h.update(self.target_isa.as_bytes());
        h.update(self.opt_level.as_bytes());
        h.update(self.cache_variant.as_bytes());
        h.update(self.host_tag.as_bytes());
        // J10: the in-memory key folds these into config.rs, but the disk key
        // omitted them, so a type-specialized recompile (integer vs float param)
        // could match the old object from disk.
        h.update(self.type_profile_hash.as_bytes());
        h.update(self.param_signature.as_bytes());
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
            CacheScope::Project,
        );
        let k2 = CodegenCacheKey::new(
            "TestModel",
            "abc123",
            "speed",
            "speed",
            "disabled",
            "disabled",
            CacheScope::Project,
        );

        assert_eq!(k1.stable_hash(), k2.stable_hash());
    }
}
