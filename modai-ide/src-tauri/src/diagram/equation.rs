use rustmodlica::ast::{ClassItem, Declaration, Equation, Expression};
use rustmodlica::parser;
use rustmodlica::unparse;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquationEntry {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub is_when: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableDecl {
    pub name: String,
    pub type_name: String,
    pub variability: String,
    pub start_value: String,
    pub unit: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEquationsAndVars {
    pub model_name: String,
    pub variables: Vec<VariableDecl>,
    pub equations: Vec<EquationEntry>,
}

pub fn extract_equations_from_source(source: &str) -> Result<ModelEquationsAndVars, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };

    let mut variables = Vec::new();
    for decl in &m.declarations {
        let variability = if decl.is_parameter {
            "parameter"
        } else {
            "variable"
        };
        let start_value = decl
            .start_value
            .as_ref()
            .map(|v| format!("{:?}", v))
            .unwrap_or_default();
        variables.push(VariableDecl {
            name: decl.name.clone(),
            type_name: decl.type_name.clone(),
            variability: variability.to_string(),
            start_value,
            unit: String::new(),
            description: String::new(),
        });
    }

    let mut equations = Vec::new();
    for (idx, eq) in m.equations.iter().enumerate() {
        let text = unparse::equation_to_string(eq);
        let is_when = matches!(eq, Equation::When(_, _, _));
        equations.push(EquationEntry {
            id: format!("eq_{}", idx),
            text,
            is_when,
        });
    }

    Ok(ModelEquationsAndVars {
        model_name: m.name.clone(),
        variables,
        equations,
    })
}

pub fn apply_equation_edits(
    source: &str,
    variables: &[VariableDecl],
    equations: &[EquationEntry],
) -> Result<String, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let mut m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };

    let existing_component_names: HashSet<&str> = m
        .declarations
        .iter()
        .filter(|d| !d.is_parameter)
        .map(|d| d.name.as_str())
        .collect();

    let mut new_decls: Vec<Declaration> = Vec::new();
    for var in variables {
        if existing_component_names.contains(var.name.as_str()) {
            if let Some(existing) = m.declarations.iter().find(|d| d.name == var.name) {
                new_decls.push(existing.clone());
                continue;
            }
        }
        let start_val = if var.start_value.is_empty() {
            None
        } else {
            Some(Expression::var(&var.start_value))
        };
        let decl = Declaration {
            name: var.name.clone(),
            type_name: var.type_name.clone(),
            replaceable: false,
            constrainedby_type: None,
            is_parameter: var.variability == "parameter",
            is_flow: false,
            is_stream: false,
            is_discrete: false,
            is_input: false,
            is_output: false,
            is_inner: false,
            is_outer: false,
            is_public: false,
            is_protected: false,
            start_value: start_val,
            array_size: None,
            modifications: vec![],
            is_rest: false,
            annotation: None,
            condition: None,
        };
        new_decls.push(decl);
    }

    for existing in &m.declarations {
        if !variables.iter().any(|v| v.name == existing.name) {
            if existing_component_names.contains(existing.name.as_str()) {
                new_decls.push(existing.clone());
            }
        }
    }

    m.declarations = new_decls;

    let mut new_eqs: Vec<Equation> = Vec::new();
    for eq_entry in equations {
        let text = eq_entry.text.trim();
        if text.is_empty() {
            continue;
        }
        let eq_source = format!("model _Tmp\nequation\n  {};\nend _Tmp;\n", text.trim_end_matches(';'));
        if let Ok(ClassItem::Model(tmp)) = parser::parse(&eq_source) {
            for parsed_eq in tmp.equations {
                new_eqs.push(parsed_eq);
            }
        }
    }
    m.equations = new_eqs;

    Ok(unparse::model_to_mo(&m))
}
