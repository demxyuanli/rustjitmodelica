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
        let mod_inner: Vec<_> = modification_pair.into_inner().collect();
        let mod_redeclare = mod_inner.iter().any(|p| p.as_str().trim() == "redeclare");
        let mod_redeclare_type = mod_inner
            .iter()
            .find(|p| p.as_rule() == Rule::type_name)
            .map(|p| p.as_str().trim().to_string());
        let mod_each = mod_inner.iter().any(|p| p.as_str().trim() == "each");
        let name_pair = mod_inner.iter().find(|p| p.as_rule() == Rule::component_ref);
        let name_pair = match name_pair {
            Some(p) => p,
            None => continue,
        };
        let name_expr = expression::parse_component_ref(name_pair.clone());
        let mod_name = helpers::expr_to_string(name_expr);
        let expr_pair = mod_inner.iter().find(|p| p.as_rule() == Rule::expression);
        let expr_pair = match expr_pair {
            Some(p) => p,
            None => continue,
        };
        let val = Some(expression::parse_expression(expr_pair.clone()));
        if mod_name == "start" {
            start_value = val.clone();
        }
        modifications.push(Modification {
            name: mod_name,
            value: val,
            each: mod_each,
            redeclare: mod_redeclare,
            redeclare_type: mod_redeclare_type,
        });
    }
    (modifications, start_value)
}
