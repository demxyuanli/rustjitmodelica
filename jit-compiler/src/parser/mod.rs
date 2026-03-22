mod algorithm;
mod alg_parse;
mod common;
mod decl_parse;
mod entry;
mod equation;
mod eq_parse;
mod expression;
mod helpers;
mod model_parse;
mod preparse;

use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "src/modelica.pest"]
pub struct ModelicaParser;

pub fn parse(input: &str) -> Result<crate::ast::ClassItem, pest::error::Error<Rule>> {
    entry::parse(input)
}

pub fn parse_expression_from_str(
    input: &str,
) -> Result<crate::ast::Expression, pest::error::Error<Rule>> {
    entry::parse_expression_from_str(input)
}
