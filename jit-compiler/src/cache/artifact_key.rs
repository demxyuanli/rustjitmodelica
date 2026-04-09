use crate::compiler::CompilerOptions;
use crate::loader::ModelLoader;
use xxhash_rust::xxh64::Xxh64;

pub fn compile_flags_hash(opts: &CompilerOptions) -> String {
    let mut h = Xxh64::new(0);
    h.update(opts.index_reduction_method.as_bytes());
    h.update(opts.tearing_method.as_bytes());
    h.update(opts.generate_dynamic_jacobian.as_bytes());
    h.update(opts.validation_mode.as_bytes());
    h.update(opts.warnings_level.as_bytes());
    h.update(opts.array_size_policy.as_bytes());
    h.update(opts.solver.as_bytes());
    h.update(&opts.t_end.to_bits().to_le_bytes());
    h.update(&opts.dt.to_bits().to_le_bytes());
    h.update(&opts.atol.to_bits().to_le_bytes());
    h.update(&opts.rtol.to_bits().to_le_bytes());
    h.update(format!("{:?}", opts.compile_stop).as_bytes());
    format!("{:016x}", h.digest())
}

pub fn libs_fingerprint(loader: &ModelLoader) -> String {
    crate::cache::lib_epoch::DepClosureFingerprint::compute(&loader.library_paths, &loader.library_paths)
        .combined_hash()
}

pub fn artifact_cache_key(model_name: &str, opts: &CompilerOptions, loader: &ModelLoader) -> String {
    let mut h = Xxh64::new(0);
    h.update(model_name.as_bytes());
    h.update(compile_flags_hash(opts).as_bytes());
    h.update(libs_fingerprint(loader).as_bytes());
    h.update(std::env::consts::OS.as_bytes());
    h.update(std::env::consts::ARCH.as_bytes());
    format!("artifact_v1:{}:{:016x}", model_name.replace('.', "_"), h.digest())
}

