//! Persistent native code cache for JIT-compiled `calc_derivs` functions.
//!
//! Similar to Julia's `.ji` cache files, this module stores compiled machine code
//! to disk keyed by a stable hash of the model's flattened representation.
//! On subsequent runs, cached code is memory-mapped and executed directly,
//! skipping the Cranelift compilation step entirely.
//!
//! Enable with `RUSTMODLICA_JIT_CODEGEN_CACHE=1`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use xxhash_rust::xxh64::Xxh64;

use crate::jit::types::ArrayInfo;

/// Cache version incremented when on-disk format changes.
const CODEGEN_CACHE_VERSION: u32 = 1;

/// Cranelift version for cache invalidation.
const CRANELIFT_VERSION: &str = "0.128.3";

/// Target ISA identifier (e.g., "x86_64-unknown-linux-gnu").
fn target_isa_id() -> String {
    format!("{}-{}-{}", std::env::consts::OS, std::env::consts::ARCH, CRANELIFT_VERSION)
}

/// Check if codegen cache is enabled via environment variable.
pub fn codegen_cache_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_JIT_CODEGEN_CACHE")
            .ok()
            .map(|v| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

/// Get the cache root directory for codegen artifacts.
fn codegen_cache_root() -> Option<PathBuf> {
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
) -> String {
    let mut h = Xxh64::new(0);

    // Version and target
    h.update(&CODEGEN_CACHE_VERSION.to_le_bytes());
    h.update(target_isa_id().as_bytes());
    h.update(opt_level.as_bytes());

    // Model identity
    h.update(model_name.as_bytes());

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
    pub created_at: u64,  // Unix timestamp
}

impl CodegenCacheKey {
    pub fn new(model_name: &str, flat_hash: &str, opt_level: &str) -> Self {
        Self {
            version: CODEGEN_CACHE_VERSION,
            model_name: model_name.to_string(),
            flat_hash: flat_hash.to_string(),
            target_isa: target_isa_id(),
            opt_level: opt_level.to_string(),
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
        format!("{:016x}", h.digest())
    }
}

/// Object file entry in the cache.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodegenCacheEntry {
    pub key: CodegenCacheKey,
    pub object_size: usize,
    pub func_offset: u64,  // Offset of calc_derivs in the object file
    pub func_size: usize,  // Size of the function
    pub when_count: usize,
    pub crossings_count: usize,
}

/// Cached function handle with memory mapping.
pub struct CachedFunction {
    /// Memory-mapped region (kept alive for the function pointer).
    #[allow(dead_code)]
    mapping: memmap2::Mmap,
    /// Function pointer to calc_derivs.
    pub func: crate::jit::types::CalcDerivsFunc,
    /// When clause count.
    pub when_count: usize,
    /// Crossing function count.
    pub crossings_count: usize,
}

/// The codegen cache manager.
pub struct CodegenCache {
    root: PathBuf,
    enabled: bool,
}

impl CodegenCache {
    /// Create a new cache manager.
    pub fn new() -> Self {
        let enabled = codegen_cache_enabled();
        let root = codegen_cache_root().unwrap_or_else(|| std::env::temp_dir().join("rustmodlica-jit-cache"));

        if enabled {
            // Ensure cache directory exists
            let _ = std::fs::create_dir_all(&root);
            eprintln!("[jit-codegen-cache] enabled, root={}", root.display());
        }

        Self { root, enabled }
    }

    /// Check if cache is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the cache file path for a given key.
    fn object_path(&self, key: &CodegenCacheKey) -> PathBuf {
        self.root.join(format!("{}.bin", key.stable_hash()))
    }

    fn entry_path(&self, key: &CodegenCacheKey) -> PathBuf {
        self.root.join(format!("{}.json", key.stable_hash()))
    }

    /// Try to load cached native code for the given model.
    /// Returns CachedFunction if found and valid.
    pub fn get(&self, key: &CodegenCacheKey) -> Option<CachedFunction> {
        if !self.enabled {
            return None;
        }

        let object_path = self.object_path(key);
        let entry_path = self.entry_path(key);

        // Read entry metadata
        let entry_bytes = std::fs::read(&entry_path).ok()?;
        let entry: CodegenCacheEntry = serde_json::from_slice(&entry_bytes).ok()?;

        // Validate entry matches key
        if entry.key.version != CODEGEN_CACHE_VERSION {
            eprintln!("[jit-codegen-cache] version mismatch, invalidating");
            let _ = std::fs::remove_file(&object_path);
            let _ = std::fs::remove_file(&entry_path);
            return None;
        }

        if entry.key.target_isa != target_isa_id() {
            eprintln!("[jit-codegen-cache] target ISA mismatch, invalidating");
            let _ = std::fs::remove_file(&object_path);
            let _ = std::fs::remove_file(&entry_path);
            return None;
        }

        // Read and map the native code
        let file = std::fs::File::open(&object_path).ok()?;
        let mapping = unsafe { memmap2::Mmap::map(&file).ok()? };

        // Verify size
        if mapping.len() != entry.object_size {
            eprintln!("[jit-codegen-cache] object size mismatch, invalidating");
            let _ = std::fs::remove_file(&object_path);
            let _ = std::fs::remove_file(&entry_path);
            return None;
        }

        // Get function pointer from the mapped region
        let func_offset = entry.func_offset as usize;
        let func_size = entry.func_size;

        if func_offset + func_size > mapping.len() {
            eprintln!("[jit-codegen-cache] function bounds exceed mapping");
            return None;
        }

        let func_ptr = mapping[func_offset..].as_ptr();
        let func: crate::jit::types::CalcDerivsFunc = unsafe { std::mem::transmute(func_ptr) };

        eprintln!(
            "[jit-codegen-cache] HIT model={} size={} bytes func_offset={} func_size={}",
            key.model_name, entry.object_size, func_offset, func_size
        );

        Some(CachedFunction {
            mapping,
            func,
            when_count: entry.when_count,
            crossings_count: entry.crossings_count,
        })
    }

    /// Store compiled native code to the cache.
    pub fn put(
        &self,
        key: &CodegenCacheKey,
        code_bytes: &[u8],
        func_offset: u64,
        func_size: usize,
        when_count: usize,
        crossings_count: usize,
    ) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let object_path = self.object_path(key);
        let entry_path = self.entry_path(key);

        // Write native code
        std::fs::write(&object_path, code_bytes)?;

        // Write entry metadata
        let entry = CodegenCacheEntry {
            key: key.clone(),
            object_size: code_bytes.len(),
            func_offset,
            func_size,
            when_count,
            crossings_count,
        };
        let entry_bytes = serde_json::to_vec_pretty(&entry)?;
        std::fs::write(&entry_path, entry_bytes)?;

        eprintln!(
            "[jit-codegen-cache] WRITE model={} size={} bytes",
            key.model_name, code_bytes.len()
        );

        Ok(())
    }

    /// Clear all cached entries.
    pub fn clear(&self) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let mut count = 0;
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "bin" || e == "json").unwrap_or(false) {
                std::fs::remove_file(&path)?;
                count += 1;
            }
        }

        eprintln!("[jit-codegen-cache] cleared {} entries", count);
        Ok(())
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        if !self.enabled {
            return stats;
        }

        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "bin").unwrap_or(false) {
                    stats.object_count += 1;
                    if let Ok(metadata) = entry.metadata() {
                        stats.total_bytes += metadata.len() as u64;
                    }
                }
            }
        }

        stats
    }
}

impl Default for CodegenCache {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub object_count: u64,
    pub total_bytes: u64,
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
        );

        // Same inputs should produce same hash
        let h2 = flat_model_hash(
            "TestModel",
            &["x".to_string(), "y".to_string()],
            &[],
            &["p".to_string()],
            &[],
            &HashMap::new(),
            "speed",
        );

        assert_eq!(h1, h2);

        // Different model name should produce different hash
        let h3 = flat_model_hash(
            "OtherModel",
            &["x".to_string(), "y".to_string()],
            &[],
            &["p".to_string()],
            &[],
            &HashMap::new(),
            "speed",
        );
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_codegen_cache_key_stability() {
        let k1 = CodegenCacheKey::new("TestModel", "abc123", "speed");
        let k2 = CodegenCacheKey::new("TestModel", "abc123", "speed");

        assert_eq!(k1.stable_hash(), k2.stable_hash());
    }
}
