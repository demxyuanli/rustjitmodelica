//! Enumeration literal access validation.
//! Checks that `E.a` refers to a valid enumeration literal.

use crate::ast::Expression;
use std::collections::HashMap;

/// Validate an enum literal access `E.a`. Returns the integer index (0-based)
/// of the literal, or an error string if the enum type or literal is unknown.
pub fn validate_enum_access(
    enum_type_name: &str,
    field: &str,
    enumerations: &HashMap<String, Vec<String>>,
) -> Result<usize, String> {
    let literals = enumerations.get(enum_type_name).ok_or_else(|| {
        format!("Unknown enumeration type '{}'", enum_type_name)
    })?;
    literals.iter().position(|l| l == field).ok_or_else(|| {
        format!(
            "Invalid enumeration literal '{}.{}' — valid literals are: {}",
            enum_type_name,
            field,
            literals.join(", ")
        )
    })
}

/// Check if an expression is an enum access and validate it.
/// Returns `Some(index)` if valid enum access, `Some(usize::MAX)` if
/// the base type is not an enum (not an error), or `Err(msg)` if invalid.
pub fn check_enum_access(
    expr: &Expression,
    enumerations: &HashMap<String, Vec<String>>,
) -> Option<Result<usize, String>> {
    match expr {
        Expression::Dot(base, field) => {
            if let Expression::Variable(id) = base.as_ref() {
                let type_name = crate::string_intern::resolve_id(*id);
                // Only check if this type has an enum definition
                if enumerations.contains_key(&type_name) {
                    Some(validate_enum_access(&type_name, field, enumerations))
                } else {
                    None // Not an enum type, no validation needed
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_enums() -> HashMap<String, Vec<String>> {
        let mut m = HashMap::new();
        m.insert("E".to_string(), vec!["a".into(), "b".into(), "c".into()]);
        m
    }

    #[test]
    fn test_valid_enum_access() {
        let enums = make_enums();
        let idx = validate_enum_access("E", "a", &enums).unwrap();
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_valid_enum_access_last() {
        let enums = make_enums();
        let idx = validate_enum_access("E", "c", &enums).unwrap();
        assert_eq!(idx, 2);
    }

    #[test]
    fn test_invalid_enum_literal() {
        let enums = make_enums();
        let err = validate_enum_access("E", "d", &enums).unwrap_err();
        assert!(err.contains("Invalid enumeration literal"));
        assert!(err.contains("a, b, c"));
    }

    #[test]
    fn test_unknown_enum_type() {
        let enums = make_enums();
        let err = validate_enum_access("F", "a", &enums).unwrap_err();
        assert!(err.contains("Unknown enumeration type"));
    }
}
