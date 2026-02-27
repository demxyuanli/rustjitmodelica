use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
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
    loaded_models: HashMap<String, Model>,
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

    pub fn load_model(&mut self, name: &str) -> Result<Model, LoadError> {
        self.load_model_impl(name, false)
    }

    pub fn load_model_silent(&mut self, name: &str, silent: bool) -> Result<Model, LoadError> {
        self.load_model_impl(name, silent)
    }

    fn load_model_impl(&mut self, name: &str, silent: bool) -> Result<Model, LoadError> {
        if let Some(model) = self.loaded_models.get(name) {
            return Ok(model.clone());
        }

        let relative_path = name.replace('.', "/");
        let filename = format!("{}.mo", relative_path);

        for lib_path in &self.library_paths {
            let full_path = lib_path.join(&filename);
            if full_path.exists() {
                if !silent {
                    println!("Loading dependency: {}", full_path.display());
                }
                let content = fs::read_to_string(&full_path)
                    .map_err(|e| LoadError::Io(name.to_string(), e))?;
                match parser::parse(&content) {
                    Ok(model) => {
                        self.loaded_models.insert(name.to_string(), model.clone());
                        return Ok(model);
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
            eprintln!("Could not find model: {}", name);
        }
        Err(LoadError::NotFound(name.to_string()))
    }
}
