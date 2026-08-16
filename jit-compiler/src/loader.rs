#[path = "loader_scan.rs"]
mod loader_scan;

use crate::ast::Model;
use crate::diag::ParseErrorInfo;
use crate::parser;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LoadError {
    #[error("Model not found: {0}")]
    NotFound(String),
    #[error("Recursive model load detected: {0}")]
    RecursiveLoad(String),
    #[error("{0}")]
    ParseFailedAt(ParseErrorInfo),
    #[error("IO error loading {0}: {1}")]
    Io(String, #[source] std::io::Error),
}

pub struct ModelLoader {
    pub library_paths: Vec<PathBuf>,
    loaded_models: HashMap<String, Arc<Model>>,
    /// DBG-4: path used to load each model (for source location in errors).
    loaded_paths: HashMap<String, PathBuf>,
    currently_loading: HashSet<String>,
    /// Candidate file paths that were probed during name resolution but did not
    /// exist. Recorded as NEGATIVE cache dependencies so that creating one of
    /// them later (which could change resolution) invalidates the flat cache.
    probed_absent: HashSet<PathBuf>,
    /// Memoized `parse_all` results per file path, used by the fallback scans
    /// (multi-top-level and package-directory) so each source file is read and
    /// parsed at most once per process. `None` = read or parse failed.
    scanned_file_items: HashMap<PathBuf, Option<Arc<Vec<crate::ast::ClassItem>>>>,
    /// Negative name-resolution cache: name -> registration epoch at the time
    /// the lookup failed with NotFound. Valid only while the epoch is unchanged
    /// (i.e. no new model/path/library registrations since), so a name that
    /// could resolve after more files are loaded is retried correctly.
    absent_name_epoch: HashMap<String, u64>,
    /// Cached `Path::exists` for direct model file probes (lib-relative).
    path_exists_cache: HashMap<String, bool>,
    /// When true, suppress "Loading dependency" and "Resolved inner class" so validate output is JSON-only.
    pub quiet: bool,
}

impl ModelLoader {
    pub(super) fn item_name(item: &crate::ast::ClassItem) -> &str {
        match item {
            crate::ast::ClassItem::Model(m) => &m.name,
            crate::ast::ClassItem::Function(f) => &f.name,
        }
    }

    pub(super) fn item_to_model(item: crate::ast::ClassItem) -> crate::ast::Model {
        match item {
            crate::ast::ClassItem::Model(m) => m,
            crate::ast::ClassItem::Function(f) => crate::ast::Model::from(f),
        }
    }
    pub(super) fn clone_model_shallow(m: &Model) -> Model {
        Model {
            name: m.name.clone(),
            is_connector: m.is_connector,
            is_function: m.is_function,
            is_operator_function: m.is_operator_function,
            is_record: m.is_record,
            is_block: m.is_block,
            is_expandable: false,
            is_partial: false,
            is_encapsulated: false,
            is_pure: false,
            is_impure: false,
            enumerations: std::collections::HashMap::new(),
            extends: m.extends.clone(),
            declarations: m.declarations.clone(),
            equations: m.equations.clone(),
            algorithms: m.algorithms.clone(),
            initial_equations: m.initial_equations.clone(),
            initial_algorithms: m.initial_algorithms.clone(),
            annotation: m.annotation.clone(),
            inner_class_index: m.inner_class_index.clone(),
            inner_classes: m.inner_classes.clone(),
            is_operator_record: m.is_operator_record,
            type_aliases: m.type_aliases.clone(),
            imports: m.imports.clone(),
            external_info: m.external_info.clone(),
            redeclare_extends: m.redeclare_extends.clone(),
        }
    }
    pub fn new() -> Self {
        ModelLoader {
            library_paths: Vec::new(),
            loaded_models: HashMap::new(),
            loaded_paths: HashMap::new(),
            currently_loading: HashSet::new(),
            probed_absent: HashSet::new(),
            scanned_file_items: HashMap::new(),
            absent_name_epoch: HashMap::new(),
            path_exists_cache: HashMap::new(),
            quiet: false,
        }
    }

    /// Fast existence check for type-name qualification (no parse, no dir scan).
    pub fn model_resolvable(&mut self, name: &str) -> bool {
        if self.loaded_models.contains_key(name) {
            return true;
        }
        if let Some(ep) = self.absent_name_epoch.get(name) {
            if *ep == self.registration_epoch() {
                return false;
            }
        }
        let rel = name.replace('.', "/");
        let libs: Vec<PathBuf> = self.library_paths.clone();
        for lib in &libs {
            let candidates = [
                lib.join(format!("{}/package.mo", rel)),
                lib.join(format!("{}.mo", rel)),
            ];
            for path in candidates {
                let key = path.to_string_lossy().into_owned();
                let exists = if let Some(&cached) = self.path_exists_cache.get(&key) {
                    cached
                } else {
                    let e = path.exists();
                    self.path_exists_cache.insert(key, e);
                    e
                };
                if exists {
                    return true;
                }
            }
        }
        if let Some((prefix, suffix)) = name.rsplit_once('.') {
            if let Some(base) = self.loaded_models.get(prefix) {
                if base.find_inner_class(suffix).is_some() {
                    return true;
                }
            }
        }
        self.absent_name_epoch
            .insert(name.to_string(), self.registration_epoch());
        false
    }

    /// Returns a model already registered in this loader without loading.
    pub fn peek_loaded_model(&self, name: &str) -> Option<Arc<Model>> {
        self.loaded_models.get(name).cloned()
    }

    /// Snapshot models already loaded (for inline warm-cache seeding).
    pub fn snapshot_warm_models(&self) -> HashMap<String, Arc<Model>> {
        self.loaded_models.clone()
    }

    /// Monotone counter that changes whenever any new model, path, or library
    /// root is registered; guards validity of the negative name cache.
    pub(super) fn registration_epoch(&self) -> u64 {
        (self.library_paths.len() + self.loaded_models.len() + self.loaded_paths.len()) as u64
    }

    /// Read + parse a file once and memoize the result for fallback scans.
    pub(super) fn parse_file_items_cached(
        &mut self,
        path: &PathBuf,
    ) -> Option<Arc<Vec<crate::ast::ClassItem>>> {
        if let Some(cached) = self.scanned_file_items.get(path) {
            return cached.clone();
        }
        let parsed = fs::read_to_string(path)
            .ok()
            .and_then(|content| parser::parse_all(&content).ok())
            .map(Arc::new);
        self.scanned_file_items
            .insert(path.clone(), parsed.clone());
        parsed
    }

    /// Candidate paths probed during resolution but found absent, recorded as
    /// negative cache dependencies (creating one could change resolution).
    pub fn probed_absent_paths(&self) -> Vec<PathBuf> {
        self.probed_absent.iter().cloned().collect()
    }

    pub fn set_quiet(&mut self, q: bool) {
        self.quiet = q;
    }

    /// DBG-4: Return the path from which the model was loaded, if known.
    pub fn get_path_for_model(&self, name: &str) -> Option<PathBuf> {
        self.loaded_paths.get(name).cloned()
    }

    /// DBG-4: Register a path for a model name (e.g. when root was loaded by another loader).
    pub fn register_path(&mut self, name: &str, path: PathBuf) {
        self.loaded_paths.insert(name.to_string(), path);
    }

    pub fn loaded_source_paths(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = self.loaded_paths.values().cloned().collect();
        out.sort();
        out.dedup();
        out
    }

    /// Fully-qualified names currently registered (e.g. after `load_model("Modelica")`).
    pub fn loaded_model_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.loaded_models.keys().cloned().collect();
        v.sort();
        v
    }

    pub fn add_path(&mut self, path: PathBuf) {
        if self.library_paths.contains(&path) {
            return;
        }
        self.library_paths.push(path.clone());
        self.path_exists_cache.clear();
        if path.join("Modelica").join("package.mo").is_file() {
            let _ = crate::cache::msl_pack::on_msl_library_path_added(&path);
        }
        // If `path` is itself a package root (contains package.mo), also register
        // its parent directory as a fallback root so sibling libraries in the
        // standard layout (e.g. ModelicaServices next to Modelica) resolve
        // without relying on the process working directory.
        if path.join("package.mo").is_file() {
            if let Some(parent) = path.parent() {
                let parent = parent.to_path_buf();
                if !parent.as_os_str().is_empty() && !self.library_paths.contains(&parent) {
                    self.library_paths.push(parent.clone());
                    if parent.join("Modelica").join("package.mo").is_file() {
                        let _ = crate::cache::msl_pack::on_msl_library_path_added(&parent);
                    }
                }
            }
        }
    }

    /// Drop a single cached model entry so the next `load_model_from_source`
    /// or `load_model` call for `name` re-parses fresh source. All other
    /// cached classes (typically MSL) stay warm. Used by the IDE
    /// equation-graph actor to reuse a Compiler across calls without
    /// being poisoned by a stale source for the model under edit.
    pub fn forget_model(&mut self, name: &str) {
        self.loaded_models.remove(name);
        self.loaded_paths.remove(name);
        let short = name.rsplit('.').next().unwrap_or(name);
        if short != name {
            self.loaded_models.remove(short);
            self.loaded_paths.remove(short);
        }
        // Source may have changed on disk; drop memoized scans and negative
        // resolutions so the next lookups observe fresh state.
        self.scanned_file_items.clear();
        self.absent_name_epoch.clear();
        self.path_exists_cache.clear();
    }


    pub fn load_model(&mut self, name: &str) -> Result<Arc<Model>, LoadError> {
        self.load_model_impl(name, false)
    }

    pub fn load_model_silent(&mut self, name: &str, silent: bool) -> Result<Arc<Model>, LoadError> {
        self.load_model_impl(name, silent)
    }

    pub(super) fn cache_compat_alias(&mut self, requested: &str, loaded_as: &str, arc: &Arc<Model>) {
        self.loaded_models
            .insert(requested.to_string(), Arc::clone(arc));
        if let Some(p) = self.loaded_paths.get(loaded_as).cloned() {
            self.loaded_paths.insert(requested.to_string(), p);
        }
    }

    pub(super) fn try_example_templates_alias(
        &mut self,
        name: &str,
    ) -> Option<Result<Arc<Model>, LoadError>> {
        if !name.contains(".ExampleTemplates") {
            return None;
        }
        let mut candidate = name.to_string();
        for _ in 0..8 {
            let parts: Vec<&str> = candidate.split('.').collect();
            let idx = match parts.iter().position(|p| *p == "ExampleTemplates") {
                Some(i) => i,
                None => break,
            };
            if idx < 1 {
                break;
            }
            let mut p = parts.clone();
            p.remove(idx - 1);
            let next = p.join(".");
            if next == candidate {
                break;
            }
            candidate = next;
            if let Ok(arc) = self.load_model_impl(&candidate, true) {
                self.cache_compat_alias(name, &candidate, &arc);
                return Some(Ok(arc));
            }
        }
        None
    }

}
