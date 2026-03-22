use crate::diag::SourceLocation;
use crate::loader::LoadError;
use std::fmt;

#[derive(Debug)]
pub enum FlattenError {
    Load(LoadError),
    UnknownType(String, String, Option<SourceLocation>),
    IncompatibleConnector(String, String, String, String, Option<SourceLocation>),
}

impl From<LoadError> for FlattenError {
    fn from(e: LoadError) -> Self {
        FlattenError::Load(e)
    }
}

impl fmt::Display for FlattenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlattenError::Load(e) => write!(f, "[FLATTEN_LOAD] {}", e),
            FlattenError::UnknownType(ty, inst, loc) => {
                write!(
                    f,
                    "[FLATTEN_UNKNOWN_TYPE] Unknown type '{}' for instance '{}'",
                    ty, inst
                )?;
                if let Some(l) = loc {
                    write!(f, "{}", l.fmt_suffix())?;
                }
                Ok(())
            }
            FlattenError::IncompatibleConnector(a, b, ta, tb, loc) => {
                write!(f, "[FLATTEN_INCOMPATIBLE_CONNECTOR] Error: Incompatible connector types in connect({}, {}): type '{}' vs '{}' (model/connector paths as shown)", a, b, ta, tb)?;
                if let Some(l) = loc {
                    write!(f, "{}", l.fmt_suffix())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for FlattenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FlattenError::Load(e) => Some(e),
            _ => None,
        }
    }
}
