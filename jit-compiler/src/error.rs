use crate::flatten::FlattenError;
use crate::loader::LoadError;

pub type AppResult<T> = Result<T, AppError>;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Load(#[from] LoadError),
    #[error(transparent)]
    Flatten(#[from] FlattenError),
    #[error("{0}")]
    Message(String),
    #[error("{0}")]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("thread spawn failed: {0}")]
    ThreadSpawn(String),
    #[error("worker thread panicked")]
    ThreadPanic,
}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

impl From<&str> for AppError {
    fn from(value: &str) -> Self {
        Self::Message(value.to_string())
    }
}
