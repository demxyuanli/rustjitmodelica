//! Persistent native code cache for JIT-compiled `calc_derivs` functions.
//!
//! Similar to Julia's `.ji` cache files, this module stores compiled machine code
//! to disk keyed by a stable hash of the model's flattened representation.
//! On subsequent runs (non-Windows only), cached bytes are copied into an executable
//! anonymous mapping. Windows skips load: JIT machine code is not relocatable across processes.
//!
//! Enable with `RUSTMODLICA_JIT_CODEGEN_CACHE=1`.

mod cache_key;
mod cache_store;
mod exec_buffer;

#[cfg(windows)]
mod coff_reloc;

pub use cache_key::{
    codegen_cache_enabled, flat_model_hash, param_values_by_name, CodegenCacheEntry, CodegenCacheKey,
    CODEGEN_CACHE_VERSION,
};
pub use cache_store::{CacheStats, CachedFunction, CodegenCache};
