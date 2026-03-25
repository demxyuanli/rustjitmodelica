use crate::ast::{ClassItem, Expression};
use crate::parser::model_parse::parse_model;
use crate::parser::preparse::{make_alias_model, try_parse_connector_alias_file};
use crate::parser::{expression, ModelicaParser, Rule};
use pest::Parser;

pub fn parse_all(input: &str) -> Result<Vec<ClassItem>, pest::error::Error<Rule>> {
    if let Some((alias, base)) = try_parse_connector_alias_file(input) {
        return Ok(vec![make_alias_model(alias, base)]);
    }
    let mut pairs = ModelicaParser::parse(Rule::model_file, input)?;
    let program = pairs.next().unwrap();
    let mut items = Vec::new();
    for item_pair in program.into_inner().filter(|p| {
        matches!(
            p.as_rule(),
            Rule::model_definition
                | Rule::short_class_definition
                | Rule::type_definition
                | Rule::connector_alias_definition
        )
    }) {
        let item = match item_pair.as_rule() {
            Rule::model_definition => parse_model(item_pair)?,
            Rule::short_class_definition | Rule::type_definition => {
                let mut alias = String::new();
                let mut base = String::new();
                for p in item_pair.into_inner() {
                    match p.as_rule() {
                        Rule::identifier => {
                            if alias.is_empty() {
                                alias = p.as_str().trim().to_string();
                            }
                        }
                        Rule::type_name => {
                            if base.is_empty() {
                                base = p.as_str().trim().to_string();
                            }
                        }
                        Rule::enumeration_type => {
                            if base.is_empty() {
                                base = "Integer".to_string();
                            }
                        }
                        Rule::component_ref => {
                            if base.is_empty() {
                                base = p.as_str().trim().to_string();
                            }
                        }
                        Rule::function_call => {
                            if base.is_empty() {
                                let mut it = p.into_inner();
                                if let Some(name_pair) = it.next() {
                                    base = name_pair.as_str().trim().to_string();
                                }
                            }
                        }
                        _ => {}
                    }
                }
                make_alias_model(alias, base)
            }
            Rule::connector_alias_definition => {
                let mut alias = String::new();
                let mut base = String::new();
                for p in item_pair.into_inner() {
                    match p.as_rule() {
                        Rule::identifier => {
                            if alias.is_empty() {
                                alias = p.as_str().trim().to_string();
                            }
                        }
                        Rule::type_name => {
                            if base.is_empty() {
                                base = p.as_str().trim().to_string();
                            }
                        }
                        _ => {}
                    }
                }
                make_alias_model(alias, base)
            }
            _ => parse_model(item_pair)?,
        };
        items.push(item);
    }
    Ok(items)
}

pub fn parse(input: &str) -> Result<ClassItem, pest::error::Error<Rule>> {
    let mut all = parse_all(input)?;
    Ok(all.remove(0))
}

pub fn parse_expression_from_str(input: &str) -> Result<Expression, pest::error::Error<Rule>> {
    let mut pairs = ModelicaParser::parse(Rule::expression, input)?;
    let pair = pairs.next().unwrap();
    Ok(expression::parse_expression(pair))
}
