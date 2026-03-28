use crate::diag::SourceLocation;
use crate::loader::LoadError;
use std::fmt;

#[derive(Debug)]
pub enum FlattenError {
    Load(LoadError),
    UnknownType(String, String, Option<SourceLocation>),
    IncompatibleConnector(String, String, String, String, Option<SourceLocation>),
    /// Modification/redeclare did not match any component in the model (strict mode).
    ModificationTargetNotFound {
        target: String,
        scope: String,
    },
    /// Redeclared type violates a coarse `constrainedby` check (replaceable component).
    RedeclareViolatesConstrainedBy {
        component: String,
        new_type: String,
        constraint: String,
    },
    /// Modifier lists both `inner` and `outer` for the same component (MLS 7.3).
    ConflictingInnerOuter { target: String },
    /// Modifier lists both `public` and `protected` for the same element (MLS 7.3).
    ConflictingPublicProtected { target: String },
    /// Array dimension expression could not be evaluated as a constant and no external override applied.
    UnevaluatedArraySize { flat_base_name: String },
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
            FlattenError::ModificationTargetNotFound { target, scope } => {
                write!(
                    f,
                    "[FLATTEN_MODIFICATION_TARGET] No component '{}' in model scope '{}'",
                    target, scope
                )
            }
            FlattenError::RedeclareViolatesConstrainedBy {
                component,
                new_type,
                constraint,
            } => {
                write!(
                    f,
                    "[FLATTEN_CONSTRAINEDBY] Redeclare type '{}' for '{}' does not satisfy constrainedby '{}'",
                    new_type, component, constraint
                )
            }
            FlattenError::ConflictingInnerOuter { target } => write!(
                f,
                "[FLATTEN_INNER_OUTER] '{}' cannot be both inner and outer in the same modifier",
                target
            ),
            FlattenError::ConflictingPublicProtected { target } => write!(
                f,
                "[FLATTEN_VISIBILITY] '{}' cannot be both public and protected in the same modifier",
                target
            ),
            FlattenError::UnevaluatedArraySize { flat_base_name } => write!(
                f,
                "[FLATTEN_ARRAY_SIZE] Could not evaluate array size for '{}'. Use --array-sizes-json with matching \"array_sizes\" keys, or --array-size-policy=legacy.",
                flat_base_name
            ),
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
