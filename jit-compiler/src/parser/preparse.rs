use crate::ast::{ClassItem, Model};

pub fn try_parse_connector_alias_file(input: &str) -> Option<(String, String)> {
    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("within ") || line.starts_with("//") {
            continue;
        }
        if !line.starts_with("connector ") {
            return None;
        }
        let rest = line.strip_prefix("connector ")?.trim();
        let (name_part, rhs_part) = rest.split_once('=')?;
        let alias = name_part.trim().to_string();
        if alias.is_empty() {
            return None;
        }
        let mut rhs = rhs_part.trim();
        if let Some(x) = rhs.strip_prefix("input ") {
            rhs = x.trim();
        } else if let Some(x) = rhs.strip_prefix("output ") {
            rhs = x.trim();
        }
        let base = rhs
            .split(|c: char| c.is_whitespace() || c == ';' || c == '(')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if base.is_empty() {
            return None;
        }
        return Some((alias, base));
    }
    None
}

pub fn make_alias_model(alias: String, base: String) -> ClassItem {
    let type_aliases = if !alias.is_empty() && !base.is_empty() {
        vec![(alias.clone(), base)]
    } else {
        Vec::new()
    };
    ClassItem::Model(Model {
        name: alias,
        is_connector: false,
        is_function: false,
        is_record: false,
        is_block: false,
        extends: Vec::new(),
        declarations: Vec::new(),
        equations: Vec::new(),
        algorithms: Vec::new(),
        initial_equations: Vec::new(),
        initial_algorithms: Vec::new(),
        annotation: None,
        inner_classes: Vec::new(),
        inner_class_index: std::collections::HashMap::new(),
        is_operator_record: false,
        type_aliases,
        imports: Vec::new(),
        external_info: None,
    })
}
