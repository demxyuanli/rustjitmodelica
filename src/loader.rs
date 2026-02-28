use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use crate::ast::Model;
use crate::diag::ParseErrorInfo;
use crate::parser;
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
}

impl ModelLoader {
    pub fn new() -> Self {
        ModelLoader {
            library_paths: Vec::new(),
            loaded_models: HashMap::new(),
        }
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
        let filename = format!("{}.mo", relative_path);

        for lib_path in &self.library_paths {
            let full_path = lib_path.join(&filename);
            if full_path.exists() {
                if !silent {
                    println!("{}", crate::i18n::msg("loading_dependency", &[&full_path.display().to_string() as &dyn std::fmt::Display]));
                }
                let content = fs::read_to_string(&full_path)
                    .map_err(|e| LoadError::Io(name.to_string(), e))?;
                match parser::parse(&content) {
                    Ok(item) => {
                        let model = match item {
                            crate::ast::ClassItem::Model(m) => m,
                            crate::ast::ClassItem::Function(f) => crate::ast::Model::from(f),
                        };
                        let arc = Arc::new(model);
                        self.loaded_models.insert(name.to_string(), Arc::clone(&arc));
                        return Ok(arc);
                    }
                    Err(e) => {
                        let (line, column) = crate::diag::line_col_from_pest(&e.line_col);
                        let path_str = full_path.display().to_string();
                        let message = crate::diag::short_message_from_pest_string(&e.to_string());
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

        if !silent {
            eprintln!("{}", crate::i18n::msg("could_not_find_model", &[&name as &dyn std::fmt::Display]));
        }
        Err(LoadError::NotFound(name.to_string()))
    }
}
