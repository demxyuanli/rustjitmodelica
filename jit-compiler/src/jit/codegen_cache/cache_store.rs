//! Filesystem-backed codegen cache: JSON metadata + raw/object payloads.

use std::collections::HashMap;
use std::path::PathBuf;

use super::cache_key::{
    codegen_cache_enabled, codegen_cache_read_dirs_for_scope, codegen_cache_write_dir_for_scope,
    codegen_host_tag, target_isa_id, CodegenCacheEntry, CodegenCacheKey, CODEGEN_CACHE_VERSION,
};
use crate::cache::cache_scope::CacheScope;
use crate::cache::codegen_cache_index;
use super::exec_buffer::ExecCodeBuffer;

#[cfg(windows)]
use super::coff_reloc::load_coff_object_exec_windows;

#[cfg(target_os = "linux")]
use super::elf_reloc::load_elf_object_exec;

#[cfg(target_os = "macos")]
use super::macho_reloc::load_macho_object_exec_macos;

/// Cached function handle (executable anonymous buffer; not file-backed mmap).
pub struct CachedFunction {
    #[allow(dead_code)]
    exec: Option<ExecCodeBuffer>,
    #[allow(dead_code)]
    import_slots: Vec<Box<usize>>,
    /// Function pointer to calc_derivs.
    pub func: crate::jit::types::CalcDerivsFunc,
    /// When clause count.
    pub when_count: usize,
    /// Crossing function count.
    pub crossings_count: usize,
}

impl CachedFunction {
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    fn from_exec(
        exec: ExecCodeBuffer,
        func: crate::jit::types::CalcDerivsFunc,
        when_count: usize,
        crossings_count: usize,
    ) -> Self {
        Self {
            exec: Some(exec),
            import_slots: Vec::new(),
            func,
            when_count,
            crossings_count,
        }
    }

    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    pub(crate) fn from_exec_with_import_slots(
        exec: ExecCodeBuffer,
        import_slots: Vec<Box<usize>>,
        func: crate::jit::types::CalcDerivsFunc,
        when_count: usize,
        crossings_count: usize,
    ) -> Self {
        Self {
            exec: Some(exec),
            import_slots,
            func,
            when_count,
            crossings_count,
        }
    }
}

/// The codegen cache manager.
pub struct CodegenCache {
    enabled: bool,
}

impl CodegenCache {
    /// Create a new cache manager.
    pub fn new() -> Self {
        let enabled = codegen_cache_enabled();

        if enabled {
            if let Some(r) = codegen_cache_write_dir_for_scope(CacheScope::Project) {
                let _ = std::fs::create_dir_all(&r);
                eprintln!(
                    "[jit-codegen-cache] enabled, project tier root={}",
                    r.display()
                );
            } else {
                let fallback = std::env::temp_dir().join("rustmodlica-jit-cache");
                let _ = std::fs::create_dir_all(&fallback);
                eprintln!(
                    "[jit-codegen-cache] enabled, no project cache dir; fallback={}",
                    fallback.display()
                );
            }
        }

        Self { enabled }
    }

    /// Check if cache is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn object_path_in(root: &std::path::Path, key: &CodegenCacheKey) -> PathBuf {
        root.join(format!("{}.bin", key.stable_hash()))
    }

    /// Read raw object/code bytes from the codegen disk cache for this key (if present).
    pub fn try_read_object_bytes(&self, key: &CodegenCacheKey) -> Option<Vec<u8>> {
        for root in codegen_cache_read_dirs_for_scope(key.cache_scope) {
            let raw = std::fs::read(Self::object_path_in(&root, key)).ok()?;
            if !raw.is_empty() {
                return Some(raw);
            }
        }
        None
    }

    fn entry_path_in(root: &std::path::Path, key: &CodegenCacheKey) -> PathBuf {
        root.join(format!("{}.json", key.stable_hash()))
    }

    /// Try to load cached native code for the given model.
    pub fn get(
        &self,
        key: &CodegenCacheKey,
        runtime_symbols: &HashMap<String, *const u8>,
        runtime_param_values: Option<&HashMap<String, f64>>,
    ) -> Option<CachedFunction> {
        if !self.enabled {
            return None;
        }
        self.get_loaded_executable(key, runtime_symbols, runtime_param_values)
    }

    fn get_loaded_executable(
        &self,
        key: &CodegenCacheKey,
        runtime_symbols: &HashMap<String, *const u8>,
        runtime_param_values: Option<&HashMap<String, f64>>,
    ) -> Option<CachedFunction> {
        for root in codegen_cache_read_dirs_for_scope(key.cache_scope) {
            let object_path = Self::object_path_in(&root, key);
            let entry_path = Self::entry_path_in(&root, key);

            let entry_bytes = std::fs::read(&entry_path).ok()?;
            let entry: CodegenCacheEntry = serde_json::from_slice(&entry_bytes).ok()?;

            if entry.key.version != CODEGEN_CACHE_VERSION {
                eprintln!("[jit-codegen-cache] version mismatch, invalidating");
                let _ = std::fs::remove_file(&object_path);
                let _ = std::fs::remove_file(&entry_path);
                continue;
            }

            if entry.key.target_isa != target_isa_id() {
                eprintln!("[jit-codegen-cache] target ISA mismatch, invalidating");
                let _ = std::fs::remove_file(&object_path);
                let _ = std::fs::remove_file(&entry_path);
                continue;
            }

            if entry.key.host_tag != codegen_host_tag() {
                eprintln!("[jit-codegen-cache] host tag mismatch, invalidating");
                let _ = std::fs::remove_file(&object_path);
                let _ = std::fs::remove_file(&entry_path);
                continue;
            }

            match const_fold_params_match_detail(&entry, runtime_param_values) {
                ConstFoldMatchResult::AllMatch => {}
                ConstFoldMatchResult::ValueOnlyDiff { ref changed } => {
                    eprintln!(
                        "[jit-codegen-cache] const-fold value-only diff ({}), reusing code",
                        changed.join(", ")
                    );
                }
                ConstFoldMatchResult::StructuralMismatch { ref param } => {
                    eprintln!(
                        "[jit-codegen-cache] const-fold structural mismatch (param={}), invalidating",
                        param
                    );
                    let _ = std::fs::remove_file(&object_path);
                    let _ = std::fs::remove_file(&entry_path);
                    continue;
                }
                ConstFoldMatchResult::NoRuntimeMap => {
                    eprintln!("[jit-codegen-cache] no runtime param map but entry has const-fold params, invalidating");
                    let _ = std::fs::remove_file(&object_path);
                    let _ = std::fs::remove_file(&entry_path);
                    continue;
                }
            }

            let raw = std::fs::read(&object_path).ok()?;
            if raw.len() != entry.object_size {
                eprintln!("[jit-codegen-cache] object size mismatch, invalidating");
                let _ = std::fs::remove_file(&object_path);
                let _ = std::fs::remove_file(&entry_path);
                continue;
            }

            let loaded = match entry.artifact_kind.as_str() {
                "object" => self.load_object_artifact(key, &entry, &raw, runtime_symbols),
                // "raw" entries are byte copies of finalized JIT code. They are
                // NOT position-independent: RIP-relative calls to imported
                // symbols (math builtins, Newton linear solvers) only resolve
                // at the original JIT base address, so re-executing them from a
                // fresh buffer crashes (STATUS_ACCESS_VIOLATION on MultiBody
                // models with tearing blocks). Treat them as stale.
                "raw" => {
                    eprintln!(
                        "[jit-codegen-cache] raw artifact is not relocatable, invalidating (model={})",
                        key.model_name
                    );
                    let _ = std::fs::remove_file(&object_path);
                    let _ = std::fs::remove_file(&entry_path);
                    None
                }
                other => {
                    eprintln!(
                        "[jit-codegen-cache] unknown artifact kind='{}', invalidating",
                        other
                    );
                    let _ = std::fs::remove_file(&object_path);
                    let _ = std::fs::remove_file(&entry_path);
                    None
                }
            };
            if loaded.is_some() {
                return loaded;
            }
        }
        None
    }


    fn load_object_artifact(
        &self,
        key: &CodegenCacheKey,
        entry: &CodegenCacheEntry,
        raw: &[u8],
        runtime_symbols: &HashMap<String, *const u8>,
    ) -> Option<CachedFunction> {
        #[cfg(windows)]
        {
            return self.load_object_artifact_windows(key, entry, raw, runtime_symbols);
        }
        #[cfg(target_os = "linux")]
        {
            return self.load_object_artifact_linux(key, entry, raw, runtime_symbols);
        }
        #[cfg(target_os = "macos")]
        {
            return self.load_object_artifact_macos(key, entry, raw, runtime_symbols);
        }
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            let _ = (key, entry, raw, runtime_symbols);
            None
        }
    }

    #[cfg(windows)]
    fn load_object_artifact_windows(
        &self,
        key: &CodegenCacheKey,
        entry: &CodegenCacheEntry,
        raw: &[u8],
        runtime_symbols: &HashMap<String, *const u8>,
    ) -> Option<CachedFunction> {
        let (exec, func_offset, import_slots) =
            load_coff_object_exec_windows(raw, runtime_symbols, "jit-disk-object")?;
        let func_ptr = unsafe { exec.as_ptr().add(func_offset) };
        let func: crate::jit::types::CalcDerivsFunc = unsafe { std::mem::transmute(func_ptr) };

        eprintln!(
            "[jit-codegen-cache] HIT(object/windows) model={} size={} bytes",
            key.model_name, entry.object_size
        );
        Some(CachedFunction::from_exec_with_import_slots(
            exec,
            import_slots,
            func,
            entry.when_count,
            entry.crossings_count,
        ))
    }

    #[cfg(target_os = "linux")]
    fn load_object_artifact_linux(
        &self,
        key: &CodegenCacheKey,
        entry: &CodegenCacheEntry,
        raw: &[u8],
        runtime_symbols: &HashMap<String, *const u8>,
    ) -> Option<CachedFunction> {
        let (exec, func_offset, import_slots) = load_elf_object_exec(raw, runtime_symbols)?;
        let func_ptr = unsafe { exec.as_ptr().add(func_offset) };
        let func: crate::jit::types::CalcDerivsFunc = unsafe { std::mem::transmute(func_ptr) };

        eprintln!(
            "[jit-codegen-cache] HIT(object/linux) model={} size={} bytes",
            key.model_name, entry.object_size
        );
        Some(CachedFunction::from_exec_with_import_slots(
            exec,
            import_slots,
            func,
            entry.when_count,
            entry.crossings_count,
        ))
    }

    #[cfg(target_os = "macos")]
    fn load_object_artifact_macos(
        &self,
        key: &CodegenCacheKey,
        entry: &CodegenCacheEntry,
        raw: &[u8],
        runtime_symbols: &HashMap<String, *const u8>,
    ) -> Option<CachedFunction> {
        let (exec, func_offset, import_slots) =
            load_macho_object_exec_macos(raw, runtime_symbols, "jit-disk-object")?;
        let func_ptr = unsafe { exec.as_ptr().add(func_offset) };
        let func: crate::jit::types::CalcDerivsFunc = unsafe { std::mem::transmute(func_ptr) };

        eprintln!(
            "[jit-codegen-cache] HIT(object/macos) model={} size={} bytes",
            key.model_name, entry.object_size
        );
        Some(CachedFunction::from_exec_with_import_slots(
            exec,
            import_slots,
            func,
            entry.when_count,
            entry.crossings_count,
        ))
    }

    pub fn put(
        &self,
        key: &CodegenCacheKey,
        code_bytes: &[u8],
        func_offset: u64,
        func_size: usize,
        when_count: usize,
        crossings_count: usize,
        const_fold_params: &[(String, f64)],
    ) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let write_root = codegen_cache_write_dir_for_scope(key.cache_scope)
            .unwrap_or_else(|| std::env::temp_dir().join("rustmodlica-jit-cache"));
        std::fs::create_dir_all(&write_root)?;

        let object_path = Self::object_path_in(&write_root, key);
        let entry_path = Self::entry_path_in(&write_root, key);

        std::fs::write(&object_path, code_bytes)?;

        let entry = CodegenCacheEntry {
            key: key.clone(),
            object_size: code_bytes.len(),
            artifact_kind: "raw".to_string(),
            func_offset,
            func_size,
            when_count,
            crossings_count,
            const_fold_params: const_fold_params.to_vec(),
        };
        let entry_bytes = serde_json::to_vec_pretty(&entry)?;
        std::fs::write(&entry_path, entry_bytes)?;

        eprintln!(
            "[jit-codegen-cache] WRITE model={} size={} bytes",
            key.model_name, code_bytes.len()
        );
        codegen_cache_index::record_codegen_pair(&write_root, &key.stable_hash());

        Ok(())
    }

    pub fn put_object(
        &self,
        key: &CodegenCacheKey,
        object_bytes: &[u8],
        when_count: usize,
        crossings_count: usize,
        const_fold_params: &[(String, f64)],
    ) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let write_root = codegen_cache_write_dir_for_scope(key.cache_scope)
            .unwrap_or_else(|| std::env::temp_dir().join("rustmodlica-jit-cache"));
        std::fs::create_dir_all(&write_root)?;
        let object_path = Self::object_path_in(&write_root, key);
        let entry_path = Self::entry_path_in(&write_root, key);
        std::fs::write(&object_path, object_bytes)?;
        let entry = CodegenCacheEntry {
            key: key.clone(),
            object_size: object_bytes.len(),
            artifact_kind: "object".to_string(),
            func_offset: 0,
            func_size: 0,
            when_count,
            crossings_count,
            const_fold_params: const_fold_params.to_vec(),
        };
        let entry_bytes = serde_json::to_vec_pretty(&entry)?;
        std::fs::write(&entry_path, entry_bytes)?;
        eprintln!(
            "[jit-codegen-cache] WRITE(object) model={} size={} bytes",
            key.model_name,
            object_bytes.len()
        );
        codegen_cache_index::record_codegen_pair(&write_root, &key.stable_hash());
        Ok(())
    }

    pub fn clear(&self) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let mut count = 0;
        for root in codegen_cache_read_dirs_for_scope(CacheScope::Project) {
            if let Ok(rd) = std::fs::read_dir(&root) {
                for entry in rd {
                    let entry = entry?;
                    let path = entry.path();
                    if path.extension().map(|e| e == "bin" || e == "json").unwrap_or(false) {
                        std::fs::remove_file(&path)?;
                        count += 1;
                    }
                }
            }
        }

        eprintln!("[jit-codegen-cache] cleared {} entries", count);
        Ok(())
    }

    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        if !self.enabled {
            return stats;
        }

        let mut roots: Vec<PathBuf> = codegen_cache_read_dirs_for_scope(CacheScope::Project);
        for s in [CacheScope::UserExt, CacheScope::GlobalStd] {
            if let Some(p) = codegen_cache_write_dir_for_scope(s) {
                if !roots.iter().any(|x| x == &p) {
                    roots.push(p);
                }
            }
        }

        for root in roots {
            if let Ok(entries) = std::fs::read_dir(&root) {
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
        }

        stats
    }
}

/// Result of per-parameter const-fold comparison.
#[derive(Debug)]
enum ConstFoldMatchResult {
    /// All folded params match cached values exactly.
    AllMatch,
    /// Some value-only params differ but no structural param changed.
    /// The cached code is still valid; caller should update param vector at runtime.
    ValueOnlyDiff { changed: Vec<String> },
    /// A param that affects IR structure changed, or a param is missing.
    /// Cached code must be invalidated.
    StructuralMismatch { param: String },
    /// No runtime param map provided but cached entry has folded params.
    NoRuntimeMap,
}

fn const_fold_params_match_detail(
    entry: &CodegenCacheEntry,
    runtime_param_values: Option<&HashMap<String, f64>>,
) -> ConstFoldMatchResult {
    if entry.const_fold_params.is_empty() {
        return ConstFoldMatchResult::AllMatch;
    }
    let Some(map) = runtime_param_values else {
        return ConstFoldMatchResult::NoRuntimeMap;
    };
    let mut changed_names = Vec::new();
    for (name, cached_v) in &entry.const_fold_params {
        match map.get(name) {
            Some(rv) if *rv == *cached_v => {}
            Some(_rv) => {
                changed_names.push(name.clone());
            }
            None => {
                return ConstFoldMatchResult::StructuralMismatch {
                    param: name.clone(),
                };
            }
        }
    }
    if changed_names.is_empty() {
        ConstFoldMatchResult::AllMatch
    } else {
        ConstFoldMatchResult::ValueOnlyDiff {
            changed: changed_names,
        }
    }
}

/// Check if const-fold params are compatible (all match or value-only diff).
#[allow(dead_code)]
pub(crate) fn const_fold_params_compatible(
    entry: &CodegenCacheEntry,
    runtime_param_values: Option<&HashMap<String, f64>>,
) -> bool {
    matches!(
        const_fold_params_match_detail(entry, runtime_param_values),
        ConstFoldMatchResult::AllMatch | ConstFoldMatchResult::ValueOnlyDiff { .. }
    )
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
