//! Persistent native code cache for JIT-compiled `calc_derivs` functions.
//!
//! Similar to Julia's `.ji` cache files, this module stores compiled machine code
//! to disk keyed by a stable hash of the model's flattened representation.
//! On subsequent runs (non-Windows only), cached bytes are copied into an executable
//! anonymous mapping. Windows skips load: JIT machine code is not relocatable across processes.
//!
//! Enable with `RUSTMODLICA_JIT_CODEGEN_CACHE=1`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

#[cfg(windows)]
use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget, SymbolSection};
use xxhash_rust::xxh64::Xxh64;

use crate::jit::types::ArrayInfo;

/// RWX anonymous mapping: file-backed `mmap` is not executable on Windows (and often not on Unix).
struct ExecCodeBuffer {
    ptr: *mut u8,
    len: usize,
}

impl ExecCodeBuffer {
    fn alloc_rw(len: usize) -> Option<Self> {
        if len == 0 {
            return None;
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Memory::{
                VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
            };
            let ptr = unsafe {
                VirtualAlloc(
                    std::ptr::null(),
                    len,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                )
            };
            if ptr.is_null() {
                return None;
            }
            return Some(Self {
                ptr: ptr as *mut u8,
                len,
            });
        }
        #[cfg(unix)]
        {
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            if ptr == libc::MAP_FAILED {
                return None;
            }
            let ptr = ptr as *mut u8;
            return Some(Self { ptr, len });
        }
        #[cfg(not(any(windows, unix)))]
        {
            None
        }
    }

    #[cfg_attr(windows, allow(dead_code))]
    fn copy_from_bytes(bytes: &[u8]) -> Option<Self> {
        let exec = Self::alloc_rw(bytes.len())?;
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), exec.as_mut_ptr(), bytes.len());
        }
        if !exec.make_rx() {
            return None;
        }
        Some(exec)
    }

    fn make_rx(&self) -> bool {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READ};
            let mut old_protect = 0u32;
            unsafe { VirtualProtect(self.ptr as *mut _, self.len, PAGE_EXECUTE_READ, &mut old_protect) != 0 }
        }
        #[cfg(unix)]
        {
            unsafe {
                libc::mprotect(
                    self.ptr as *mut libc::c_void,
                    self.len,
                    libc::PROT_READ | libc::PROT_EXEC,
                ) == 0
            }
        }
        #[cfg(not(any(windows, unix)))]
        {
            false
        }
    }

    fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr
    }

    fn as_ptr(&self) -> *const u8 {
        self.ptr
    }
}

impl Drop for ExecCodeBuffer {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Memory::{VirtualFree, MEM_RELEASE};
            if !self.ptr.is_null() && self.len > 0 {
                unsafe {
                    VirtualFree(self.ptr as *mut _, 0, MEM_RELEASE);
                }
            }
        }
        #[cfg(unix)]
        {
            if !self.ptr.is_null() && self.len > 0 {
                unsafe {
                    libc::munmap(self.ptr as *mut libc::c_void, self.len);
                }
            }
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = (self.ptr, self.len);
        }
    }
}

/// Cache version incremented when on-disk format or serialization contract changes.
const CODEGEN_CACHE_VERSION: u32 = 3;

/// Cranelift version for cache invalidation (keep aligned with workspace `cranelift-jit`).
const CRANELIFT_VERSION: &str = "0.128.4";

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
    #[serde(default = "default_artifact_kind_raw")]
    pub artifact_kind: String,
    pub func_offset: u64,  // Offset of calc_derivs in the object file
    pub func_size: usize,  // Size of the function
    pub when_count: usize,
    pub crossings_count: usize,
}

fn default_artifact_kind_raw() -> String {
    "raw".to_string()
}

/// Cached function handle (executable anonymous buffer; not file-backed mmap).
pub struct CachedFunction {
    #[allow(dead_code)]
    exec: Option<ExecCodeBuffer>,
    #[cfg(windows)]
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
    #[cfg(not(windows))]
    fn from_exec(
        exec: ExecCodeBuffer,
        func: crate::jit::types::CalcDerivsFunc,
        when_count: usize,
        crossings_count: usize,
    ) -> Self {
        Self {
            exec: Some(exec),
            #[cfg(windows)]
            import_slots: Vec::new(),
            func,
            when_count,
            crossings_count,
        }
    }

    #[cfg(windows)]
    fn from_exec_with_import_slots(
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
    pub fn get(
        &self,
        key: &CodegenCacheKey,
        runtime_symbols: &HashMap<String, *const u8>,
    ) -> Option<CachedFunction> {
        if !self.enabled {
            return None;
        }
        self.get_loaded_executable(key, runtime_symbols)
    }

    fn get_loaded_executable(
        &self,
        key: &CodegenCacheKey,
        runtime_symbols: &HashMap<String, *const u8>,
    ) -> Option<CachedFunction> {
        let object_path = self.object_path(key);
        let entry_path = self.entry_path(key);

        let entry_bytes = std::fs::read(&entry_path).ok()?;
        let entry: CodegenCacheEntry = serde_json::from_slice(&entry_bytes).ok()?;

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

        let raw = std::fs::read(&object_path).ok()?;
        if raw.len() != entry.object_size {
            eprintln!("[jit-codegen-cache] object size mismatch, invalidating");
            let _ = std::fs::remove_file(&object_path);
            let _ = std::fs::remove_file(&entry_path);
            return None;
        }

        match entry.artifact_kind.as_str() {
            "object" => self.load_object_artifact(key, &entry, &raw, runtime_symbols),
            "raw" => self.load_raw_artifact(key, &entry, &raw),
            other => {
                eprintln!(
                    "[jit-codegen-cache] unknown artifact kind='{}', invalidating",
                    other
                );
                let _ = std::fs::remove_file(&object_path);
                let _ = std::fs::remove_file(&entry_path);
                None
            }
        }
    }

    fn load_raw_artifact(
        &self,
        key: &CodegenCacheKey,
        entry: &CodegenCacheEntry,
        raw: &[u8],
    ) -> Option<CachedFunction> {
        let func_offset = entry.func_offset as usize;
        let func_size = entry.func_size;

        if func_offset + func_size > raw.len() {
            eprintln!("[jit-codegen-cache] function bounds exceed object");
            return None;
        }

        #[cfg(windows)]
        {
            let _ = (key, entry, raw);
            static WARNED: OnceLock<()> = OnceLock::new();
            WARNED.get_or_init(|| {
                eprintln!(
                    "[jit-codegen-cache] raw load skipped on Windows (non-relocatable machine code)"
                );
            });
            return None;
        }

        #[cfg(not(windows))]
        {
            let exec = ExecCodeBuffer::copy_from_bytes(raw)?;
            let func_ptr = unsafe { exec.as_ptr().add(func_offset) };
            let func: crate::jit::types::CalcDerivsFunc = unsafe { std::mem::transmute(func_ptr) };
            eprintln!(
                "[jit-codegen-cache] HIT(raw) model={} size={} bytes func_offset={} func_size={}",
                key.model_name, entry.object_size, func_offset, func_size
            );
            Some(CachedFunction::from_exec(
                exec,
                func,
                entry.when_count,
                entry.crossings_count,
            ))
        }
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
        #[cfg(not(windows))]
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
        let (exec, func_offset, import_slots) = load_coff_object_exec_windows(raw, runtime_symbols)?;
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

        // Write native code/object payload
        std::fs::write(&object_path, code_bytes)?;

        // Write entry metadata
        let entry = CodegenCacheEntry {
            key: key.clone(),
            object_size: code_bytes.len(),
            artifact_kind: "raw".to_string(),
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

    /// Store a relocatable object artifact (ObjectModule output).
    pub fn put_object(
        &self,
        key: &CodegenCacheKey,
        object_bytes: &[u8],
        when_count: usize,
        crossings_count: usize,
    ) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let object_path = self.object_path(key);
        let entry_path = self.entry_path(key);
        std::fs::write(&object_path, object_bytes)?;
        let entry = CodegenCacheEntry {
            key: key.clone(),
            object_size: object_bytes.len(),
            artifact_kind: "object".to_string(),
            func_offset: 0,
            func_size: 0,
            when_count,
            crossings_count,
        };
        let entry_bytes = serde_json::to_vec_pretty(&entry)?;
        std::fs::write(&entry_path, entry_bytes)?;
        eprintln!(
            "[jit-codegen-cache] WRITE(object) model={} size={} bytes",
            key.model_name,
            object_bytes.len()
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

#[cfg(windows)]
fn load_coff_object_exec_windows(
    raw: &[u8],
    runtime_symbols: &HashMap<String, *const u8>,
) -> Option<(ExecCodeBuffer, usize, Vec<Box<usize>>)> {
    let obj = object::File::parse(raw).ok()?;
    let mut layouts: HashMap<object::SectionIndex, (usize, usize)> = HashMap::new();
    let mut total_len = 0usize;

    let mut import_slots: Vec<Box<usize>> = Vec::new();
    for section in obj.sections() {
        let size = usize::try_from(section.size()).ok()?;
        if size == 0 {
            continue;
        }
        let name = section.name().ok().unwrap_or("");
        if name.starts_with(".debug") {
            continue;
        }
        let align = usize::try_from(section.align()).ok()?.max(1);
        total_len = align_up(total_len, align);
        layouts.insert(section.index(), (total_len, size));
        total_len = total_len.saturating_add(size);
    }

    if total_len == 0 {
        return None;
    }

    let exec = ExecCodeBuffer::alloc_rw(total_len)?;
    unsafe {
        std::ptr::write_bytes(exec.as_mut_ptr(), 0u8, total_len);
    }

    for section in obj.sections() {
        let Some((base_off, size)) = layouts.get(&section.index()).copied() else {
            continue;
        };
        let data = section.uncompressed_data().ok()?;
        let to_copy = data.len().min(size);
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), exec.as_mut_ptr().add(base_off), to_copy);
        }
    }

    let base_addr = exec.as_ptr() as usize;
    for section in obj.sections() {
        let Some((base_off, _)) = layouts.get(&section.index()).copied() else {
            continue;
        };
        for (rel_off, reloc) in section.relocations() {
            let place_off = base_off.saturating_add(usize::try_from(rel_off).ok()?);
            let target_addr = match reloc.target() {
                RelocationTarget::Symbol(sym_idx) => {
                    resolve_symbol_addr_windows(
                        &obj,
                        sym_idx,
                        &layouts,
                        base_addr,
                        runtime_symbols,
                        &mut import_slots,
                    )?
                }
                RelocationTarget::Section(sec_idx) => {
                    let (sec_off, _) = *layouts.get(&sec_idx)?;
                    base_addr.saturating_add(sec_off)
                }
                _ => return None,
            };
            apply_relocation_windows(
                exec.as_mut_ptr(),
                total_len,
                base_addr,
                place_off,
                target_addr,
                reloc.kind(),
                reloc.size(),
                reloc.addend(),
            )?;
        }
    }

    let sym = obj.symbol_by_name("calc_derivs")?;
    let func_offset = symbol_runtime_offset_windows(&sym, &layouts)?;
    if !exec.make_rx() {
        return None;
    }
    Some((exec, func_offset, import_slots))
}

#[cfg(windows)]
fn symbol_runtime_offset_windows(
    sym: &object::Symbol<'_, '_>,
    layouts: &HashMap<object::SectionIndex, (usize, usize)>,
) -> Option<usize> {
    match sym.section() {
        SymbolSection::Section(sec_idx) => {
            let (sec_off, sec_size) = *layouts.get(&sec_idx)?;
            let in_sec = usize::try_from(sym.address()).ok()?;
            if in_sec >= sec_size {
                return None;
            }
            Some(sec_off.saturating_add(in_sec))
        }
        _ => None,
    }
}

#[cfg(windows)]
fn resolve_symbol_addr_windows(
    obj: &object::File<'_>,
    sym_idx: object::SymbolIndex,
    layouts: &HashMap<object::SectionIndex, (usize, usize)>,
    base_addr: usize,
    runtime_symbols: &HashMap<String, *const u8>,
    import_slots: &mut Vec<Box<usize>>,
) -> Option<usize> {
    let sym = obj.symbol_by_index(sym_idx).ok()?;
    if let Some(off) = symbol_runtime_offset_windows(&sym, layouts) {
        return Some(base_addr.saturating_add(off));
    }
    resolve_external_symbol_windows(sym.name().ok()?, runtime_symbols, import_slots)
}

#[cfg(windows)]
fn resolve_external_symbol_windows(
    raw_name: &str,
    runtime_symbols: &HashMap<String, *const u8>,
    import_slots: &mut Vec<Box<usize>>,
) -> Option<usize> {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    let mut candidates = Vec::new();
    candidates.push(raw_name.to_string());
    if let Some(s) = raw_name.strip_prefix("__imp_") {
        candidates.push(s.to_string());
    }
    if let Some(s) = raw_name.strip_prefix('_') {
        candidates.push(s.to_string());
    }
    for c in &candidates {
        if let Some(&ptr) = runtime_symbols.get(c) {
            let addr = ptr as usize;
            if raw_name.starts_with("__imp_") {
                import_slots.push(Box::new(addr));
                return import_slots.last().map(|b| &**b as *const usize as usize);
            }
            return Some(addr);
        }
    }
    for c in &candidates {
        let mut c_bytes = c.as_bytes().to_vec();
        c_bytes.push(0);
        let ptr = unsafe { GetProcAddress(GetModuleHandleA(std::ptr::null()), c_bytes.as_ptr()) };
        if let Some(p) = ptr {
            let addr = p as usize;
            if raw_name.starts_with("__imp_") {
                import_slots.push(Box::new(addr));
                return import_slots.last().map(|b| &**b as *const usize as usize);
            }
            return Some(addr);
        }
    }
    None
}

#[cfg(windows)]
fn apply_relocation_windows(
    image: *mut u8,
    image_len: usize,
    base_addr: usize,
    place_off: usize,
    target_addr: usize,
    kind: object::RelocationKind,
    size_bits: u8,
    addend: i64,
) -> Option<()> {
    let target = (target_addr as i128).saturating_add(addend as i128);
    match (kind, size_bits) {
        (object::RelocationKind::Absolute, 64) | (object::RelocationKind::ImageOffset, 64) => {
            write_u64(image, image_len, place_off, target as u64)
        }
        (object::RelocationKind::Absolute, 32) | (object::RelocationKind::ImageOffset, 32) => {
            write_u32(image, image_len, place_off, target as u32)
        }
        (object::RelocationKind::Relative, 32) => {
            let place_next = (base_addr as i128).saturating_add(place_off as i128).saturating_add(4);
            let disp = target.saturating_sub(place_next);
            if disp < i32::MIN as i128 || disp > i32::MAX as i128 {
                return None;
            }
            write_u32(image, image_len, place_off, disp as i32 as u32)
        }
        _ => None,
    }
}

#[cfg(windows)]
fn write_u32(image: *mut u8, image_len: usize, off: usize, v: u32) -> Option<()> {
    if off.checked_add(4)? > image_len {
        return None;
    }
    let bytes = v.to_le_bytes();
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), image.add(off), 4);
    }
    Some(())
}

#[cfg(windows)]
fn write_u64(image: *mut u8, image_len: usize, off: usize, v: u64) -> Option<()> {
    if off.checked_add(8)? > image_len {
        return None;
    }
    let bytes = v.to_le_bytes();
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), image.add(off), 8);
    }
    Some(())
}

#[cfg(windows)]
fn align_up(value: usize, align: usize) -> usize {
    let rem = value % align;
    if rem == 0 {
        value
    } else {
        value + (align - rem)
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
