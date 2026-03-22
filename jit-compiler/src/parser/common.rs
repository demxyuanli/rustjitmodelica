use crate::ast::{Expression, Modification};
use crate::parser::{expression, helpers, Rule};

pub fn parse_annotation_to_string(pair: &pest::iterators::Pair<Rule>) -> String {
    pair.as_str().trim().to_string()
}

pub fn normalize_identifier(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return s[1..(s.len() - 1)].to_string();
    }
    s.to_string()
}

/// Parses a single `Rule::modification` pair.
pub fn parse_modification_from_pair(
    modification_pair: pest::iterators::Pair<'_, Rule>,
) -> Option<Modification> {
    let mod_raw = modification_pair.as_str();
    let mod_inner: Vec<_> = modification_pair.into_inner().collect();
    let is_public = mod_inner.iter().any(|p| p.as_rule() == Rule::public_kw);
    let is_protected = mod_inner.iter().any(|p| p.as_rule() == Rule::protected_kw);
    let is_inner = mod_inner.iter().any(|p| p.as_rule() == Rule::inner_kw);
    let is_outer = mod_inner.iter().any(|p| p.as_rule() == Rule::outer_kw);
    let mod_redeclare_type = mod_inner
        .iter()
        .find(|p| p.as_rule() == Rule::type_name)
        .map(|p| p.as_str().trim().to_string());
    let mod_redeclare = mod_inner.iter().any(|p| p.as_str().trim() == "redeclare")
        || (mod_raw.contains("redeclare") && mod_redeclare_type.is_some());
    let mod_each = mod_inner.iter().any(|p| p.as_str().trim() == "each");
    let name_pair = if let Some(p) = mod_inner.iter().find(|p| p.as_rule() == Rule::component_ref) {
        p
    } else if mod_redeclare && mod_redeclare_type.is_some() {
        // `redeclare ClassName componentName`: component is the first identifier after `type_name`.
        let tix = mod_inner
            .iter()
            .position(|p| p.as_rule() == Rule::type_name)?;
        mod_inner[(tix + 1)..]
            .iter()
            .find(|p| p.as_rule() == Rule::identifier)?
    } else {
        return None;
    };
    let name_expr = match name_pair.as_rule() {
        Rule::component_ref => expression::parse_component_ref(name_pair.clone()),
        Rule::identifier => Expression::var(&normalize_identifier(name_pair.as_str())),
        _ => return None,
    };
    let mod_name = helpers::expr_to_string(name_expr);
    let val = match mod_inner.iter().find(|p| p.as_rule() == Rule::expression) {
        Some(p) => Some(expression::parse_expression(p.clone())),
        None => {
            if mod_redeclare {
                None
            } else {
                return None;
            }
        }
    };
    Some(Modification {
        name: mod_name,
        value: val,
        each: mod_each,
        redeclare: mod_redeclare,
        redeclare_type: mod_redeclare_type,
        is_inner,
        is_outer,
        is_public,
        is_protected,
    })
}

pub fn parse_modifications_from_modification_part(
    token: pest::iterators::Pair<'_, Rule>,
) -> (Vec<Modification>, Option<Expression>) {
    let mut modifications = Vec::new();
    let mut start_value = None;
    let mod_list = token.into_inner().next().unwrap().into_inner();
    for mod_pair in mod_list {
        let modification_pair = if mod_pair.as_rule() == Rule::modification {
            mod_pair
        } else {
            match mod_pair
                .into_inner()
                .find(|p| p.as_rule() == Rule::modification)
            {
                Some(p) => p,
                None => continue,
            }
        };
        let Some(m) = parse_modification_from_pair(modification_pair) else {
            continue;
        };
        if m.name == "start" {
            start_value = m.value.clone();
        }
        modifications.push(m);
    }
    (modifications, start_value)
}
