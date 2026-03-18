use crate::ast::Model;
use crate::diag::ParseErrorInfo;
use crate::parser;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LoadError {
    #[error("Model not found: {0}")]
    NotFound(String),
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
    /// When true, suppress "Loading dependency" and "Resolved inner class" so validate output is JSON-only.
    pub quiet: bool,
}

impl ModelLoader {
    pub fn new() -> Self {
        ModelLoader {
            library_paths: Vec::new(),
            loaded_models: HashMap::new(),
            loaded_paths: HashMap::new(),
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

    fn load_model_impl(&mut self, name: &str, silent: bool) -> Result<Arc<Model>, LoadError> {
        if let Some(arc) = self.loaded_models.get(name) {
            return Ok(Arc::clone(arc));
        }

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
            let inner = base
                .inner_classes
                .iter()
                .find(|m| m.name == suffix)
                .cloned();
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

        if !self.quiet && !silent {
            eprintln!(
                "{}",
                crate::i18n::msg("could_not_find_model", &[&name as &dyn std::fmt::Display])
            );
        }
        Err(LoadError::NotFound(name.to_string()))
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
        for inner in &model.inner_classes {
            let full_name = format!("{}.{}", prefix, inner.name);
            if self.loaded_models.contains_key(&full_name) {
                continue;
            }
            let arc = Arc::new(inner.clone());
            self.loaded_models
                .insert(full_name.clone(), Arc::clone(&arc));
            let path = self
                .loaded_paths
                .get(prefix)
                .cloned()
                .unwrap_or_else(|| PathBuf::from(prefix));
            self.loaded_paths.insert(full_name.clone(), path);
            self.register_inner_classes(&full_name, inner);
        }
    }
}
