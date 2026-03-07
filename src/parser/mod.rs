mod algorithm;
mod equation;
mod expression;
mod helpers;

use pest::Parser;
use pest_derive::Parser;
use crate::ast::*;

#[derive(Parser)]
#[grammar = "src/modelica.pest"]
pub struct ModelicaParser;

pub fn parse(input: &str) -> Result<ClassItem, pest::error::Error<Rule>> {
    let mut pairs = ModelicaParser::parse(Rule::model_file, input)?;
    let program = pairs.next().unwrap();
    let model_pair = program.into_inner().next().unwrap();
    parse_model(model_pair)
}

fn parse_annotation_to_string(pair: &pest::iterators::Pair<Rule>) -> String {
    pair.as_str().trim().to_string()
}

fn parse_model(pair: pest::iterators::Pair<Rule>) -> Result<ClassItem, pest::error::Error<Rule>> {
    let mut inner = pair.into_inner();

    let prefix_pair = inner.next().unwrap();
    let mut is_connector = false;
    let mut is_function = false;
    let mut is_record = false;
    let mut is_block = false;
    let mut is_operator_record = false;
    for p in prefix_pair.into_inner() {
        if p.as_rule() == Rule::function_prefix {
            is_function = true;
        } else if p.as_str().trim() == "operator" {
            is_operator_record = true;
        } else if p.as_str().trim() == "connector" {
            is_connector = true;
        } else if p.as_str().trim() == "record" {
            is_record = true;
        } else if p.as_str().trim() == "block" {
            is_block = true;
        }
    }

    let name = inner.next().unwrap().as_str().to_string();

    let mut declarations = Vec::new();
    let mut equations = Vec::new();
    let mut algorithms = Vec::new();
    let mut initial_equations = Vec::new();
    let mut initial_algorithms = Vec::new();
    let mut extends = Vec::new();
    let mut inner_classes = Vec::new();
    let mut type_aliases = Vec::new();
    let mut class_annotation: Option<String> = None;
    let mut external_info: Option<crate::ast::ExternalDecl> = None;

    for pair in inner {
        match pair.as_rule() {
            Rule::declaration_section => {
                for decl_pair in pair.into_inner() {
                    match decl_pair.as_rule() {
                        Rule::type_definition => {
                            let mut type_id = String::new();
                            let mut base = String::new();
                            for p in decl_pair.into_inner() {
                                match p.as_rule() {
                                    Rule::identifier => {
                                        if type_id.is_empty() {
                                            type_id = p.as_str().trim().to_string();
                                        }
                                    }
                                    Rule::type_name => base = p.as_str().trim().to_string(),
                                    _ => {}
                                }
                            }
                            if !type_id.is_empty() && !base.is_empty() {
                                type_aliases.push((type_id, base));
                            }
                        }
                        Rule::model_definition => {
                            match parse_model(decl_pair) {
                                Ok(crate::ast::ClassItem::Model(m)) => inner_classes.push(m),
                                Ok(crate::ast::ClassItem::Function(f)) => {
                                    inner_classes.push(crate::ast::Model::from(f))
                                }
                                Err(e) => return Err(e),
                            }
                        }
                        Rule::extends_clause => {
                            let ext_inner = decl_pair.into_inner();
                            let mut full_name = String::new();
                            let mut modifications = Vec::new();

                            for token in ext_inner {
                                match token.as_rule() {
                                    Rule::dotted_identifier => {
                                        full_name = token.as_str().to_string();
                                    }
                                    Rule::identifier => {
                                        if !full_name.is_empty() {
                                            full_name.push('.');
                                        }
                                        full_name.push_str(token.as_str());
                                    }
                                    Rule::modification_part => {
                                        let mod_list = token.into_inner().next().unwrap().into_inner();
                                        for mod_pair in mod_list {
                                            let modification_pair = if mod_pair.as_rule()
                                                == Rule::modification
                                            {
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
                                            let mod_inner: Vec<_> =
                                                modification_pair.into_inner().collect();
                                            let mod_redeclare = mod_inner
                                                .iter()
                                                .any(|p| p.as_str().trim() == "redeclare");
                                            let mod_redeclare_type = mod_inner
                                                .iter()
                                                .find(|p| p.as_rule() == Rule::type_name)
                                                .map(|p| p.as_str().trim().to_string());
                                            let mod_each =
                                                mod_inner.iter().any(|p| p.as_str().trim() == "each");
                                            let name_pair = mod_inner
                                                .iter()
                                                .find(|p| p.as_rule() == Rule::component_ref);
                                            let name_pair = match name_pair {
                                                Some(p) => p,
                                                None => continue,
                                            };
                                            let name_expr =
                                                expression::parse_component_ref(name_pair.clone());
                                            let mod_name = helpers::expr_to_string(name_expr);
                                            let expr_pair = mod_inner
                                                .iter()
                                                .find(|p| p.as_rule() == Rule::expression);
                                            let expr_pair = match expr_pair {
                                                Some(p) => p,
                                                None => continue,
                                            };
                                            let val = Some(expression::parse_expression(
                                                expr_pair.clone(),
                                            ));
                                            modifications.push(Modification {
                                                name: mod_name,
                                                value: val,
                                                each: mod_each,
                                                redeclare: mod_redeclare,
                                                redeclare_type: mod_redeclare_type,
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            extends.push(ExtendsClause {
                                model_name: full_name,
                                modifications,
                            });
                        }
                        Rule::declaration => {
                            let mut decl_inner = decl_pair.into_inner();
                            let mut is_parameter = false;
                            let mut is_flow = false;
                            let mut is_discrete = false;
                            let mut is_input = false;
                            let mut is_output = false;
                            let mut is_replaceable = false;

                            let mut next_token = decl_inner.next().unwrap();

                            while matches!(
                                next_token.as_rule(),
                                Rule::parameter_kw
                                    | Rule::constant_kw
                                    | Rule::flow_kw
                                    | Rule::discrete_kw
                                    | Rule::input_kw
                                    | Rule::output_kw
                            ) {
                                match next_token.as_rule() {
                                    Rule::parameter_kw => is_parameter = true,
                                    Rule::constant_kw => is_parameter = true,
                                    Rule::flow_kw => is_flow = true,
                                    Rule::discrete_kw => is_discrete = true,
                                    Rule::input_kw => is_input = true,
                                    Rule::output_kw => is_output = true,
                                    _ => {}
                                }
                                next_token = decl_inner.next().unwrap();
                            }
                            if next_token.as_rule() == Rule::replaceable_kw {
                                is_replaceable = true;
                                next_token = decl_inner.next().unwrap();
                            }
                            let type_name = next_token.as_str().trim().to_string();
                            let var_name = decl_inner.next().unwrap().as_str().trim().to_string();

                            let mut array_size = None;
                            if let Some(token) = decl_inner.peek() {
                                if token.as_rule() == Rule::array_subscript {
                                    let mut sub_inner =
                                        decl_inner.next().unwrap().into_inner();
                                    let size_expr =
                                        expression::parse_expression(sub_inner.next().unwrap());
                                    array_size = Some(size_expr);
                                }
                            }

                            let mut start_value = None;
                            let mut modifications = Vec::new();
                            let mut decl_annotation: Option<String> = None;
                            let mut is_rest = false;

                            for token in decl_inner {
                                match token.as_rule() {
                                    Rule::annotation => {
                                        decl_annotation =
                                            Some(parse_annotation_to_string(&token));
                                    }
                                    Rule::value_assignment => {
                                        let expr_pair = token.into_inner().next().unwrap();
                                        start_value =
                                            Some(expression::parse_expression(expr_pair));
                                    }
                                    Rule::rest_param => {
                                        is_rest = true;
                                    }
                                    Rule::modification_part => {
                                        let mod_list = token
                                            .into_inner()
                                            .next()
                                            .unwrap()
                                            .into_inner();
                                        for mod_pair in mod_list {
                                            let modification_pair = if mod_pair.as_rule()
                                                == Rule::modification
                                            {
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
                                            let mod_inner: Vec<_> =
                                                modification_pair.into_inner().collect();
                                            let mod_redeclare = mod_inner
                                                .iter()
                                                .any(|p| p.as_str().trim() == "redeclare");
                                            let mod_redeclare_type = mod_inner
                                                .iter()
                                                .find(|p| p.as_rule() == Rule::type_name)
                                                .map(|p| p.as_str().trim().to_string());
                                            let mod_each =
                                                mod_inner.iter().any(|p| p.as_str().trim() == "each");
                                            let name_pair = mod_inner
                                                .iter()
                                                .find(|p| p.as_rule() == Rule::component_ref);
                                            let name_pair = match name_pair {
                                                Some(p) => p,
                                                None => continue,
                                            };
                                            let name_expr =
                                                expression::parse_component_ref(name_pair.clone());
                                            let mod_name = helpers::expr_to_string(name_expr);
                                            let expr_pair = mod_inner
                                                .iter()
                                                .find(|p| p.as_rule() == Rule::expression);
                                            let expr_pair = match expr_pair {
                                                Some(p) => p,
                                                None => continue,
                                            };
                                            let val = Some(expression::parse_expression(
                                                expr_pair.clone(),
                                            ));
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
                                    }
                                    _ => {}
                                }
                            }

                            declarations.push(Declaration {
                                type_name,
                                name: var_name,
                                replaceable: is_replaceable,
                                is_parameter,
                                is_flow,
                                is_discrete,
                                is_input,
                                is_output,
                                start_value,
                                array_size,
                                modifications,
                                is_rest,
                                annotation: decl_annotation,
                            });
                        }
                        _ => {}
                    }
                }
            }
            Rule::equation_section => {
                let eq_stmt_inner = pair.into_inner();
                for stmt in eq_stmt_inner {
                    let inner_stmt = stmt.into_inner().next().unwrap();
                    match inner_stmt.as_rule() {
                        Rule::equation => {
                            let mut eq_parts = inner_stmt.into_inner();
                            let lhs = expression::parse_expression(eq_parts.next().unwrap());
                            let rhs = expression::parse_expression(eq_parts.next().unwrap());
                            equations.push(Equation::Simple(lhs, rhs));
                        }
                        Rule::connect_clause => {
                            let mut conn_inner = inner_stmt.into_inner();
                            let a_expr =
                                expression::parse_expression(conn_inner.next().unwrap());
                            let b_expr =
                                expression::parse_expression(conn_inner.next().unwrap());
                            equations.push(Equation::Connect(a_expr, b_expr));
                        }
                        Rule::multi_assign_equation => {
                            equations.push(equation::parse_multi_assign_equation(inner_stmt));
                        }
                        Rule::for_loop => {
                            equations.push(equation::parse_for_loop(inner_stmt));
                        }
                        Rule::when_equation => {
                            equations.push(equation::parse_when_equation(inner_stmt));
                        }
                        Rule::if_equation => {
                            equations.push(equation::parse_if_equation(inner_stmt));
                        }
                        Rule::reinit_clause => {
                            let mut inner = inner_stmt.into_inner();
                            let var_expr =
                                expression::parse_component_ref(inner.next().unwrap());
                            let val_expr =
                                expression::parse_expression(inner.next().unwrap());
                            let var_name = helpers::expr_to_string(var_expr);
                            equations.push(Equation::Reinit(var_name, val_expr));
                        }
                        Rule::assert_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let cond = expression::parse_expression(inner.next().unwrap());
                            let msg = expression::parse_expression(inner.next().unwrap());
                            equations.push(Equation::Assert(cond, msg));
                        }
                        Rule::terminate_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let msg =
                                expression::parse_expression(inner.next().unwrap());
                            equations.push(Equation::Terminate(msg));
                        }
                        _ => {}
                    }
                }
            }
            Rule::initial_equation_section => {
                let eq_stmt_inner = pair.into_inner();
                for stmt in eq_stmt_inner {
                    let inner_stmt = stmt.into_inner().next().unwrap();
                    match inner_stmt.as_rule() {
                        Rule::equation => {
                            let mut eq_parts = inner_stmt.into_inner();
                            let lhs = expression::parse_expression(eq_parts.next().unwrap());
                            let rhs = expression::parse_expression(eq_parts.next().unwrap());
                            initial_equations.push(Equation::Simple(lhs, rhs));
                        }
                        Rule::connect_clause => {
                            let mut conn_inner = inner_stmt.into_inner();
                            let a_expr =
                                expression::parse_expression(conn_inner.next().unwrap());
                            let b_expr =
                                expression::parse_expression(conn_inner.next().unwrap());
                            initial_equations.push(Equation::Connect(a_expr, b_expr));
                        }
                        Rule::multi_assign_equation => {
                            initial_equations
                                .push(equation::parse_multi_assign_equation(inner_stmt));
                        }
                        Rule::for_loop => {
                            initial_equations.push(equation::parse_for_loop(inner_stmt));
                        }
                        Rule::when_equation => {
                            initial_equations
                                .push(equation::parse_when_equation(inner_stmt));
                        }
                        Rule::if_equation => {
                            initial_equations
                                .push(equation::parse_if_equation(inner_stmt));
                        }
                        Rule::reinit_clause => {
                            let mut inner = inner_stmt.into_inner();
                            let var_expr =
                                expression::parse_component_ref(inner.next().unwrap());
                            let val_expr =
                                expression::parse_expression(inner.next().unwrap());
                            let var_name = helpers::expr_to_string(var_expr);
                            initial_equations.push(Equation::Reinit(var_name, val_expr));
                        }
                        Rule::assert_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let cond = expression::parse_expression(inner.next().unwrap());
                            let msg = expression::parse_expression(inner.next().unwrap());
                            initial_equations.push(Equation::Assert(cond, msg));
                        }
                        Rule::terminate_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let msg =
                                expression::parse_expression(inner.next().unwrap());
                            initial_equations.push(Equation::Terminate(msg));
                        }
                        _ => {}
                    }
                }
            }
            Rule::algorithm_section => {
                let alg_stmt_inner = pair.into_inner();
                for stmt in alg_stmt_inner {
                    let inner_stmt = stmt.into_inner().next().unwrap();
                    algorithms.push(algorithm::parse_algorithm_stmt(inner_stmt));
                }
            }
            Rule::initial_algorithm_section => {
                let alg_stmt_inner = pair.into_inner();
                for stmt in alg_stmt_inner {
                    let inner_stmt = stmt.into_inner().next().unwrap();
                    initial_algorithms.push(algorithm::parse_algorithm_stmt(inner_stmt));
                }
            }
            Rule::external_section => {
                let ext_inner: Vec<_> = pair.into_inner().collect();
                let mut lang = None;
                let mut c_name = None;
                for p in &ext_inner {
                    if p.as_rule() == Rule::string_comment {
                        let s = p.as_str().trim();
                        lang = Some(s.trim_matches('"').to_string());
                    } else if p.as_rule() == Rule::identifier && c_name.is_none() {
                        c_name = Some(p.as_str().trim().to_string());
                    }
                }
                external_info = Some(crate::ast::ExternalDecl {
                    language: lang,
                    c_name,
                });
            }
            Rule::end_part => {
                let end_inner = pair.into_inner();
                for p in end_inner {
                    if p.as_rule() == Rule::annotation {
                        class_annotation = Some(parse_annotation_to_string(&p));
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    if is_function {
        Ok(ClassItem::Function(Function {
            name,
            extends,
            declarations,
            algorithms,
            initial_algorithms,
            external_info,
        }))
    } else {
        Ok(ClassItem::Model(Model {
            name,
            is_connector,
            is_function: false,
            is_record,
            is_block,
            extends,
            declarations,
            equations,
            algorithms,
            initial_equations,
            initial_algorithms,
            annotation: class_annotation,
            inner_classes,
            is_operator_record,
            type_aliases,
            external_info: None,
        }))
    }
}
