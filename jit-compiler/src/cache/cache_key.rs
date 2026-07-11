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
        if self.key.model_name.starts_with("Modelica.") {
            if let Some(d) = crate::cache::msl_pack::context::pack_libs_closure_digest() {
                self.key.libs_closure_hash = d;
                return self;
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_v2_deterministic_hash() {
        let k1 = CacheKeyV2::builder(
            CacheStage::FlatFull,
            CacheScope::Project,
            "BouncingBall",
        )
        .root_content_hash("abc123")
        .build();
        let k2 = CacheKeyV2::builder(
            CacheStage::FlatFull,
            CacheScope::Project,
            "BouncingBall",
        )
        .root_content_hash("abc123")
        .build();
        assert_eq!(k1.stable_hash(), k2.stable_hash());
        assert_eq!(k1.to_qualified_key(), k2.to_qualified_key());
    }

    #[test]
    fn test_cache_key_v2_different_stages_produce_different_keys() {
        let k_flat = CacheKeyV2::builder(
            CacheStage::FlatFull,
            CacheScope::Project,
            "TestModel",
        )
        .root_content_hash("hash")
        .build();
        let k_parse = CacheKeyV2::builder(
            CacheStage::Parse,
            CacheScope::Project,
            "TestModel",
        )
        .root_content_hash("hash")
        .build();
        assert_ne!(k_flat.stable_hash(), k_parse.stable_hash());
        assert_ne!(k_flat.to_qualified_key(), k_parse.to_qualified_key());
    }

    #[test]
    fn test_cache_key_v2_different_scopes_produce_different_keys() {
        let k_l0 = CacheKeyV2::builder(
            CacheStage::Inheritance,
            CacheScope::GlobalStd,
            "Modelica.Math",
        )
        .root_content_hash("hash")
        .build();
        let k_l1 = CacheKeyV2::builder(
            CacheStage::Inheritance,
            CacheScope::UserExt,
            "Modelica.Math",
        )
        .root_content_hash("hash")
        .build();
        let k_l2 = CacheKeyV2::builder(
            CacheStage::Inheritance,
            CacheScope::Project,
            "Modelica.Math",
        )
        .root_content_hash("hash")
        .build();
        assert_ne!(k_l0.stable_hash(), k_l1.stable_hash());
        assert_ne!(k_l1.stable_hash(), k_l2.stable_hash());
        assert_ne!(k_l0.stable_hash(), k_l2.stable_hash());
    }

    #[test]
    fn test_cache_key_v2_different_root_hash_produces_different_key() {
        let k_a = CacheKeyV2::builder(
            CacheStage::FlatFull,
            CacheScope::Project,
            "M",
        )
        .root_content_hash("hash_a")
        .build();
        let k_b = CacheKeyV2::builder(
            CacheStage::FlatFull,
            CacheScope::Project,
            "M",
        )
        .root_content_hash("hash_b")
        .build();
        assert_ne!(k_a.stable_hash(), k_b.stable_hash());
    }

    #[test]
    fn test_cache_key_v2_different_compile_flags_produce_different_keys() {
        let flags_a = CompileFlagsKey {
            validation_mode: "full".into(),
            ..CompileFlagsKey::default()
        };
        let flags_b = CompileFlagsKey {
            validation_mode: "quick".into(),
            ..CompileFlagsKey::default()
        };
        let k_a = CacheKeyV2::builder(CacheStage::DeclExpand, CacheScope::Project, "M")
            .root_content_hash("h")
            .compile_flags(flags_a)
            .build();
        let k_b = CacheKeyV2::builder(CacheStage::DeclExpand, CacheScope::Project, "M")
            .root_content_hash("h")
            .compile_flags(flags_b)
            .build();
        assert_ne!(k_a.stable_hash(), k_b.stable_hash());
    }

    #[test]
    fn test_cache_key_v2_qualified_key_format() {
        let key = CacheKeyV2::builder(
            CacheStage::EqExpand,
            CacheScope::UserExt,
            "MyModel",
        )
        .root_content_hash("deadbeef")
        .build();
        let qk = key.to_qualified_key();
        assert!(
            qk.starts_with("L1:eq_expand_v2:"),
            "qualified key should start with scope:stage, got: {qk}"
        );
        let parts: Vec<&str> = qk.splitn(3, ':').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "L1");
        assert_eq!(parts[1], "eq_expand_v2");
        assert_eq!(parts[2].len(), 16); // 64-bit hex
    }

    #[test]
    fn test_libs_closure_hash_stable_sorting() {
        let mut paths: Vec<String> = vec![
            "/d:/LibC".into(),
            "/d:/LibA".into(),
            "/d:/LibB".into(),
        ];
        paths.sort_unstable();
        let mut h1 = Xxh64::new(0);
        for p in &paths {
            h1.update(p.as_bytes());
        }
        let d1 = format!("{:016x}", h1.digest());

        // Reverse order should produce same hash (sorted)
        let mut paths2: Vec<String> = vec![
            "/d:/LibB".into(),
            "/d:/LibA".into(),
            "/d:/LibC".into(),
        ];
        paths2.sort_unstable();
        let mut h2 = Xxh64::new(0);
        for p in &paths2 {
            h2.update(p.as_bytes());
        }
        let d2 = format!("{:016x}", h2.digest());
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_normalize_path_for_key_backslash_to_forward() {
        let result = normalize_path_for_key(Path::new("C:\\Users\\test\\model.mo"));
        assert!(!result.contains('\\'));
        assert!(result.contains('/'));
        assert_eq!(result, result.to_ascii_lowercase());
    }

    #[test]
    fn test_compile_flags_key_default_target_platform() {
        let flags = CompileFlagsKey::default();
        assert!(!flags.target_platform.is_empty());
        assert!(flags.target_platform.contains(std::env::consts::OS));
    }

    #[test]
    fn test_builder_libs_from_paths_sets_closure_hash() {
        let key = CacheKeyV2::builder(CacheStage::ModelAst, CacheScope::Project, "M")
            .root_content_hash("h")
            .libs_from_paths(&["/a/b.mo".into(), "/a/c.mo".into()])
            .build();
        assert!(!key.libs_closure_hash.is_empty());
        assert_eq!(key.libs_closure_hash.len(), 16);
    }

    #[test]
    fn test_stable_hash_hex_format() {
        let key = CacheKeyV2::builder(CacheStage::Parse, CacheScope::Project, "X")
            .root_content_hash("r")
            .build();
        let hash = key.stable_hash();
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
