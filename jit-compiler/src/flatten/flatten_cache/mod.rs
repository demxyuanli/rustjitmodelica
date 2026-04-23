//! On-disk flatten / query cache under `<install_root>/cache` by default (override with `RUSTMODLICA_FLATTEN_CACHE_DIR`).

#![allow(unused_imports)]

mod io;
mod keys;
mod mem;

pub use io::{
    get_or_compute_flattened_model_v1, hot_full_cache_evict_matching_needles,
    merge_cached_array_sizes, try_read_flat_cache_v1, try_read_flat_cache_v2,
    write_array_sizes_cache, write_flat_cache_v1, write_flat_cache_v2,
};
pub use keys::{
    flatten_array_sizes_cache_key, flatten_full_cache_key, flatten_full_cache_key_with_deps,
    ArraySizesCacheV2, ARRAY_SIZES_CACHE_SCHEMA_V2,
};
pub use mem::{
    all_disk_cache_roots, analyze_input_mem_get, analyze_input_mem_put,
    array_sizes_cache_counters_snapshot_reset, flatten_cache_dir, inline_result_mem_get,
    inline_result_mem_put, std_cache_root, sync_flatten_cache_root_ir_epoch, user_cache_root,
};
