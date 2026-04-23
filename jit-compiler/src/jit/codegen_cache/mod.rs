//! Persistent native code cache for JIT-compiled `calc_derivs` functions.
//!
//! Similar to Julia's `.ji` cache files, this module stores compiled machine code
//! to disk keyed by a stable hash of the model's flattened representation.
//! On disk hit, object artifacts are relocated into an anonymous executable mapping.
//! Windows uses COFF (`coff_reloc`); Linux uses ELF64 (`elf_reloc`). Raw blobs are a
//! fixed-offset copy path when the artifact is not a relocatable object.
//!
//! Enabled by default when `RUSTMODLICA_JIT_CODEGEN_CACHE` is unset; set to `0`/`false`/`no` to
//! disable (see `CHANGELOG.md` for AOT archive interplay).

mod cache_key;
mod cache_store;
mod exec_buffer;
mod object_imports;
mod reloc_trace;

#[cfg(windows)]
mod coff_reloc;

#[cfg(target_os = "linux")]
mod elf_reloc;

pub use cache_key::{
    codegen_cache_enabled, codegen_cache_legacy_dir, codegen_cache_read_dirs_for_scope,
    codegen_cache_root, codegen_cache_write_dir_for_scope, flat_model_hash, param_values_by_name,
    CodegenCacheEntry, CodegenCacheKey, CODEGEN_CACHE_VERSION,
};
pub use cache_store::{CacheStats, CachedFunction, CodegenCache};
pub use object_imports::undefined_import_names_from_object;

/// Merge JIT builtins with caller-supplied pointers (externals, stubs). Caller's entries win on
/// key collision so `--external-lib` overrides a builtin name.
///
/// Several compile fast paths (sim-bundle reuse, early artifact) passed **`all_symbols`** —
/// externals only — into native COFF/ELF relocation. That map is often **empty** for TestLib
/// models, so `[coff-reloc] runtime_syms=0` in traces was normal but **wrong** for parity with
/// [`CodegenCache::get`], which relocates against the full `Jit::runtime_symbols` (builtins +
/// extras). Relocation then depended on OS `dlsym` / `GetProcAddress` fallbacks for `sin` and
/// friends, which can disagree with the addresses Cranelift/JIT registered — a stability risk.
fn merged_symbols_for_native_reloc(
    runtime_symbols: &std::collections::HashMap<String, *const u8>,
) -> std::collections::HashMap<String, *const u8> {
    let mut m = crate::jit::native::builtin_jit_symbol_ptrs();
    for (k, v) in runtime_symbols {
        m.insert(k.clone(), *v);
    }
    m
}

/// Load a relocatable object blob (e.g. from an AOT archive) into executable
/// memory: **COFF** on Windows, **ELF64** on Linux. Returns `None` if loading fails
/// or on platforms without a relocator (e.g. macOS).
#[cfg(windows)]
pub fn load_aot_code_blob(
    raw: &[u8],
    import_symbols: &[String],
    runtime_symbols: &std::collections::HashMap<String, *const u8>,
    when_count: usize,
    crossings_count: usize,
) -> Option<CachedFunction> {
    let merged = merged_symbols_for_native_reloc(runtime_symbols);
    let (exec, func_offset, import_slots) =
        coff_reloc::load_aot_blob_exec_windows(raw, import_symbols, &merged, "native-reloc")?;
    let func_ptr = unsafe { exec.as_ptr().add(func_offset) };
    let func: super::types::CalcDerivsFunc = unsafe { std::mem::transmute(func_ptr) };
    Some(CachedFunction::from_exec_with_import_slots(
        exec,
        import_slots,
        func,
        when_count,
        crossings_count,
    ))
}

#[cfg(target_os = "linux")]
pub fn load_aot_code_blob(
    raw: &[u8],
    import_symbols: &[String],
    runtime_symbols: &std::collections::HashMap<String, *const u8>,
    when_count: usize,
    crossings_count: usize,
) -> Option<CachedFunction> {
    let merged = merged_symbols_for_native_reloc(runtime_symbols);
    let (exec, func_offset, import_slots) =
        elf_reloc::load_aot_blob_exec_linux(raw, import_symbols, &merged)?;
    let func_ptr = unsafe { exec.as_ptr().add(func_offset) };
    let func: super::types::CalcDerivsFunc = unsafe { std::mem::transmute(func_ptr) };
    Some(CachedFunction::from_exec_with_import_slots(
        exec,
        import_slots,
        func,
        when_count,
        crossings_count,
    ))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn load_aot_code_blob(
    _raw: &[u8],
    _import_symbols: &[String],
    _runtime_symbols: &std::collections::HashMap<String, *const u8>,
    _when_count: usize,
    _crossings_count: usize,
) -> Option<CachedFunction> {
    None
}
