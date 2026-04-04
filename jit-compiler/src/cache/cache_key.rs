use crate::cache::cache_scope::CacheScope;
use crate::cache::ir_epoch::epoch_for_stage;
use serde::{Deserialize, Serialize};
use std::path::Path;
use xxhash_rust::xxh64::Xxh64;

// Re-export CacheStage for backwards compatibility with modules importing from this file.
pub use crate::cache::ir_epoch::CacheStage;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileFlagsKey {
    pub validation_mode: String,
    pub compile_stop: String,
    pub coarse_constrainedby_only: bool,
    pub array_size_policy: u8,
    pub warnings_level: String,
    pub target_platform: String,
}

impl Default for CompileFlagsKey {
    fn default() -> Self {
        Self {
            validation_mode: String::new(),
            compile_stop: String::new(),
            coarse_constrainedby_only: false,
            array_size_policy: 0,
            warnings_level: String::new(),
            target_platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheKeyV2 {
    pub compiler_version: &'static str,
    pub ir_schema_epoch: u32,
    pub stage: CacheStage,
    pub scope: CacheScope,
    pub model_name: String,
    pub libs_closure_hash: String,
    pub root_content_hash: String,
    pub compile_flags: CompileFlagsKey,
}

impl CacheKeyV2 {
    pub fn builder(stage: CacheStage, scope: CacheScope, model_name: impl Into<String>) -> CacheKeyV2Builder {
        CacheKeyV2Builder {
            key: CacheKeyV2 {
                compiler_version: env!("CARGO_PKG_VERSION"),
                ir_schema_epoch: epoch_for_stage(stage),
                stage,
                scope,
                model_name: model_name.into(),
                libs_closure_hash: String::new(),
                root_content_hash: String::new(),
                compile_flags: CompileFlagsKey::default(),
            },
        }
    }

    pub fn stable_hash(&self) -> String {
        let mut h = Xxh64::new(0);
        h.update(self.compiler_version.as_bytes());
        h.update(&self.ir_schema_epoch.to_le_bytes());
        h.update(self.stage.tag().as_bytes());
        h.update(self.scope.prefix().as_bytes());
        h.update(self.model_name.as_bytes());
        h.update(self.libs_closure_hash.as_bytes());
        h.update(self.root_content_hash.as_bytes());
        h.update(self.compile_flags.validation_mode.as_bytes());
        h.update(self.compile_flags.compile_stop.as_bytes());
        h.update(&[self.compile_flags.coarse_constrainedby_only as u8]);
        h.update(&[self.compile_flags.array_size_policy]);
        h.update(self.compile_flags.warnings_level.as_bytes());
        h.update(self.compile_flags.target_platform.as_bytes());
        format!("{:016x}", h.digest())
    }

    pub fn to_qualified_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.scope.prefix(),
            self.stage.tag(),
            self.stable_hash()
        )
    }
}

pub struct CacheKeyV2Builder {
    key: CacheKeyV2,
}

fn libs_closure_digest_from_normalized(mut normalized: Vec<String>) -> String {
    normalized.sort_unstable();
    let mut h = Xxh64::new(0);
    for p in normalized {
        h.update(p.as_bytes());
    }
    format!("{:016x}", h.digest())
}

impl CacheKeyV2Builder {
    pub fn libs_from_paths(mut self, libs: &[String]) -> Self {
        let normalized: Vec<String> = libs
            .iter()
            .map(|s| normalize_path_for_key(Path::new(s.as_str())))
            .collect();
        self.key.libs_closure_hash = libs_closure_digest_from_normalized(normalized);
        self
    }

    pub fn libs_from_path_bufs(mut self, libs: &[std::path::PathBuf]) -> Self {
        let normalized: Vec<String> = libs
            .iter()
            .map(|p| normalize_path_for_key(p.as_path()))
            .collect();
        self.key.libs_closure_hash = libs_closure_digest_from_normalized(normalized);
        self
    }

    pub fn root_content_hash(mut self, hash: impl Into<String>) -> Self {
        self.key.root_content_hash = hash.into();
        self
    }

    pub fn compile_flags(mut self, flags: CompileFlagsKey) -> Self {
        self.key.compile_flags = flags;
        self
    }

    pub fn build(self) -> CacheKeyV2 {
        self.key
    }
}

pub fn normalize_path_for_key(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "/")
        .to_ascii_lowercase()
}
