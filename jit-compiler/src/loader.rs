use crate::ast::Model;
use crate::diag::ParseErrorInfo;
use crate::loader_compat::{early_compat, late_compat, EarlyCompat, LateCompat};
use crate::parser;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
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
    /// When true, suppress "Loading dependency" and "Resolved inner class" so validate output is JSON-only.
    pub quiet: bool,
}

impl ModelLoader {
    fn clone_model_without_inner_classes(m: &Model) -> Model {
        let leaf_aliases: Vec<Model> = m
            .inner_classes
            .iter()
            .filter(|ic| {
                ic.extends.len() == 1
                    && ic.declarations.is_empty()
                    && ic.equations.is_empty()
                    && ic.inner_classes.is_empty()
            })
            .cloned()
            .collect();
        Model {
            name: m.name.clone(),
            is_connector: m.is_connector,
            is_function: m.is_function,
            is_record: m.is_record,
            is_block: m.is_block,
            extends: m.extends.clone(),
            declarations: m.declarations.clone(),
            equations: m.equations.clone(),
            algorithms: m.algorithms.clone(),
            initial_equations: m.initial_equations.clone(),
            initial_algorithms: m.initial_algorithms.clone(),
            annotation: m.annotation.clone(),
            inner_class_index: {
                let mut idx = std::collections::HashMap::new();
                for (i, ic) in leaf_aliases.iter().enumerate() {
                    idx.insert(ic.name.clone(), i);
                }
                idx
            },
            inner_classes: leaf_aliases,
            is_operator_record: m.is_operator_record,
            type_aliases: m.type_aliases.clone(),
            imports: m.imports.clone(),
            external_info: m.external_info.clone(),
        }
    }
    pub fn new() -> Self {
        ModelLoader {
            library_paths: Vec::new(),
            loaded_models: HashMap::new(),
            loaded_paths: HashMap::new(),
            currently_loading: HashSet::new(),
            quiet: false,
        }
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

    pub fn add_path(&mut self, path: PathBuf) {
        self.library_paths.push(path);
    }

    pub fn load_model(&mut self, name: &str) -> Result<Arc<Model>, LoadError> {
        self.load_model_impl(name, false)
    }

    pub fn load_model_silent(&mut self, name: &str, silent: bool) -> Result<Arc<Model>, LoadError> {
        self.load_model_impl(name, silent)
    }

    fn cache_compat_alias(&mut self, requested: &str, loaded_as: &str, arc: &Arc<Model>) {
        self.loaded_models
            .insert(requested.to_string(), Arc::clone(arc));
        if let Some(p) = self.loaded_paths.get(loaded_as).cloned() {
            self.loaded_paths.insert(requested.to_string(), p);
        }
    }

    fn try_example_templates_alias(
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

    fn load_model_impl(&mut self, name: &str, silent: bool) -> Result<Arc<Model>, LoadError> {
        fn trace_enabled() -> bool {
            static ENABLED: OnceLock<bool> = OnceLock::new();
            *ENABLED.get_or_init(|| {
                std::env::var("RUSTMODLICA_LOAD_TRACE")
                    .ok()
                    .map(|v| {
                        let v = v.trim();
                        v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
                    })
                    .unwrap_or(false)
            })
        }
        if trace_enabled() && !silent {
            eprintln!("[load-trace] {}", name);
        }
        if let Some(r) = self.try_example_templates_alias(name) {
            return r;
        }
        match early_compat(name) {
            EarlyCompat::None => {}
            EarlyCompat::Hard(targets) => {
                let mut last_err: Option<LoadError> = None;
                for t in targets {
                    match self.load_model_impl(&t, true) {
                        Ok(arc) => {
                            self.cache_compat_alias(name, &t, &arc);
                            return Ok(arc);
                        }
                        Err(e) => last_err = Some(e),
                    }
                }
                if let Some(e) = last_err {
                    return Err(e);
                }
            }
            EarlyCompat::Soft(targets) => {
                for t in targets {
                    if let Ok(arc) = self.load_model_impl(&t, true) {
                        self.cache_compat_alias(name, &t, &arc);
                        return Ok(arc);
                    }
                }
            }
        }
        if let Some(arc) = self.loaded_models.get(name) {
            return Ok(Arc::clone(arc));
        }
        if self.currently_loading.contains(name) {
            return Err(LoadError::RecursiveLoad(name.to_string()));
        }
        let name_key = name.to_string();
        self.currently_loading.insert(name_key.clone());

        let result = (|| {
            let relative_path = name.replace('.', "/");
        // Modelica libraries commonly represent packages as directories containing `package.mo`,
        // even when the qualified name contains '.'.
        // Always try both forms to support both layouts:
        // - <rel>.mo
        // - <rel>/package.mo
        let filenames: Vec<std::path::PathBuf> = vec![
            PathBuf::from(format!("{}/package.mo", relative_path)),
            PathBuf::from(format!("{}.mo", relative_path)),
        ];

        for lib_path in &self.library_paths {
            for filename in &filenames {
                let full_path = lib_path.join(filename);
                if full_path.exists() {
                    if !self.quiet && !silent {
                        println!(
                            "{}",
                            crate::i18n::msg(
                                "loading_dependency",
                                &[&full_path.display().to_string() as &dyn std::fmt::Display]
                            )
                        );
                    }
                    let content = fs::read_to_string(&full_path)
                        .map_err(|e| LoadError::Io(name.to_string(), e))?;
                    match parser::parse(&content) {
                        Ok(item) => {
                            let mut model = match item {
                                crate::ast::ClassItem::Model(m) => m,
                                crate::ast::ClassItem::Function(f) => crate::ast::Model::from(f),
                            };
                            // Inherit imports from parent package if available.
                            if let Some((prefix, _)) = name.rsplit_once('.') {
                                if let Ok(parent) = self.load_model_impl(prefix, true) {
                                    if !parent.imports.is_empty() {
                                        for (a, q) in &parent.imports {
                                            if !model.imports.iter().any(|(aa, qq)| aa == a && qq == q) {
                                                model.imports.push((a.clone(), q.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                            let arc = Arc::new(model);
                            self.loaded_models
                                .insert(name.to_string(), Arc::clone(&arc));
                            self.loaded_paths
                                .insert(name.to_string(), full_path.clone());
                            self.register_inner_classes(name, arc.as_ref());
                            return Ok(arc);
                        }
                        Err(e) => {
                            let (line, column) = crate::diag::line_col_from_pest(&e.line_col);
                            let path_str = full_path.display().to_string();
                            let message =
                                crate::diag::short_message_from_pest_string(&e.to_string());
                            let info = ParseErrorInfo {
                                path: path_str,
                                source: content.clone(),
                                line,
                                column,
                                message,
                            };
                            return Err(LoadError::ParseFailedAt(info));
                        }
                    }
                }
            }
        }

        // If direct file lookup failed, try resolving as an inner class of the nearest parent.
        // This matches common Modelica library layouts where a package is defined in a single
        // `<Package>.mo` (e.g. `Modelica/Blocks/Sources.mo`) and contains many inner classes.
        if let Some((prefix, suffix)) = name.rsplit_once('.') {
            let base = self.load_model_impl(prefix, silent)?;
            // `load_model_impl(prefix, ..)` registers all inner classes under `prefix.*` into
            // `loaded_models`. Prefer returning the already-registered Arc to avoid deep cloning
            // large inner-class trees (which can cause stack overflow).
            if let Some(arc) = self.loaded_models.get(name) {
                return Ok(Arc::clone(arc));
            }
            let inner = base.find_inner_class(suffix).cloned();
            if let Some(m) = inner {
                if !self.quiet && !silent {
                    eprintln!("Resolved inner class: {} via {}", name, prefix);
                }
                let mut m = m;
                if !base.imports.is_empty() {
                    // Inherit imports from parent package/class so that short names like
                    // `Interfaces.SISO` work after `import Modelica.Blocks.Interfaces;`.
                    // Keep child's own imports as well.
                    for (a, q) in &base.imports {
                        if !m.imports.iter().any(|(aa, qq)| aa == a && qq == q) {
                            m.imports.push((a.clone(), q.clone()));
                        }
                    }
                }
                let arc = Arc::new(m);
                self.loaded_models
                    .insert(name.to_string(), Arc::clone(&arc));
                self.loaded_paths.insert(
                    name.to_string(),
                    self.loaded_paths
                        .get(prefix)
                        .cloned()
                        .unwrap_or_else(|| PathBuf::from(prefix)),
                );
                self.register_inner_classes(name, arc.as_ref());
                return Ok(arc);
            }
        }

        match late_compat(name) {
            LateCompat::None => {}
            LateCompat::Soft(targets) => {
                for t in targets {
                    if let Ok(arc) = self.load_model_impl(&t, true) {
                        self.cache_compat_alias(name, &t, &arc);
                        return Ok(arc);
                    }
                }
            }
        }

        if !self.quiet && !silent {
            eprintln!(
                "{}",
                crate::i18n::msg("could_not_find_model", &[&name as &dyn std::fmt::Display])
            );
        }
        Err(LoadError::NotFound(name.to_string()))
        })();

        self.currently_loading.remove(&name_key);
        result
    }

    /// Load a model from source code in memory (for IDE / single-file compile).
    /// Registers the parsed model under `model_name` and uses a virtual path for diagnostics.
    pub fn load_model_from_source(
        &mut self,
        model_name: &str,
        code: &str,
    ) -> Result<Arc<Model>, LoadError> {
        if let Some(arc) = self.loaded_models.get(model_name) {
            return Ok(Arc::clone(arc));
        }
        let item = parser::parse(code).map_err(|e| {
            let (line, column) = crate::diag::line_col_from_pest(&e.line_col);
            let message = crate::diag::short_message_from_pest_string(&e.to_string());
            LoadError::ParseFailedAt(ParseErrorInfo {
                path: format!("<{}>", model_name),
                source: code.to_string(),
                line,
                column,
                message,
            })
        })?;
        let model = match item {
            crate::ast::ClassItem::Model(m) => m,
            crate::ast::ClassItem::Function(f) => crate::ast::Model::from(f),
        };
        let arc = Arc::new(model);
        self.loaded_models
            .insert(model_name.to_string(), Arc::clone(&arc));
        self.loaded_paths.insert(
            model_name.to_string(),
            PathBuf::from(format!("<{}>", model_name)),
        );
        self.register_inner_classes(model_name, arc.as_ref());
        Ok(arc)
    }

    fn register_inner_classes(&mut self, prefix: &str, model: &Model) {
        // Iterative to avoid stack overflow on large package trees (e.g. Modelica.Media.*).
        // Also avoid deep cloning `inner_classes` trees: we register each inner class as a shallow
        // model (without its own `inner_classes`) and traverse children by reference.
        let mut stack: Vec<(String, &Model)> = model
            .inner_classes
            .iter()
            .map(|m| (prefix.to_string(), m))
            .collect();

        while let Some((parent_prefix, inner)) = stack.pop() {
            let full_name = format!("{}.{}", parent_prefix, inner.name);
            if self.loaded_models.contains_key(&full_name) {
                continue;
            }
            let arc = Arc::new(Self::clone_model_without_inner_classes(inner));
            self.loaded_models
                .insert(full_name.clone(), Arc::clone(&arc));
            let path = self
                .loaded_paths
                .get(&parent_prefix)
                .cloned()
                .unwrap_or_else(|| PathBuf::from(&parent_prefix));
            self.loaded_paths.insert(full_name.clone(), path);

            for child in &inner.inner_classes {
                stack.push((full_name.clone(), child));
            }
        }
    }
}
