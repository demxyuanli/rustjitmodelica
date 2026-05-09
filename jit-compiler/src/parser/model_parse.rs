use crate::ast::*;
use crate::parser::common::parse_annotation_to_string;
use crate::parser::{alg_parse, decl_parse, eq_parse, Rule};

fn function_call_callee_name(call: pest::iterators::Pair<Rule>) -> Option<String> {
    debug_assert_eq!(call.as_rule(), Rule::function_call);
    let mut it = call.into_inner();
    let name_pair = it.next()?;
    Some(if name_pair.as_rule() == Rule::dotted_identifier {
        name_pair
            .as_str()
            .trim()
            .trim_start_matches('.')
            .to_string()
    } else {
        name_pair.as_str().trim().to_string()
    })
}

fn first_function_call_under(pair: pest::iterators::Pair<Rule>) -> Option<String> {
    if pair.as_rule() == Rule::function_call {
        return function_call_callee_name(pair);
    }
    for c in pair.into_inner() {
        if let Some(n) = first_function_call_under(c) {
            return Some(n);
        }
    }
    None
}

/// `external "C" y = foo(x)` stores the **C** callee `foo`, not the Modelica output `y`.
fn external_c_name_from_binding(binding: pest::iterators::Pair<Rule>) -> Option<String> {
    let parts: Vec<_> = binding.into_inner().collect();
    if parts.len() == 1 {
        let p = parts.into_iter().next().unwrap();
        if p.as_rule() == Rule::function_call {
            return function_call_callee_name(p);
        }
        return None;
    }
    let mut fallback: Option<String> = None;
    for p in parts {
        match p.as_rule() {
            Rule::expression => {
                if let Some(n) = first_function_call_under(p) {
                    return Some(n);
                }
            }
            Rule::function_call => {
                fallback = function_call_callee_name(p);
            }
            _ => {}
        }
    }
    fallback
}

pub fn parse_model(pair: pest::iterators::Pair<Rule>) -> Result<ClassItem, pest::error::Error<Rule>> {
    let mut inner = pair.into_inner();

    let prefix_pair = inner.next().unwrap();
    let mut is_connector = false;
    let mut is_function = false;
    let mut is_operator_function = false;
    let mut is_record = false;
    let mut is_block = false;
    let mut is_operator_record = false;
    for p in prefix_pair.into_inner() {
        if p.as_rule() == Rule::function_prefix {
            is_function = true;
        } else if p.as_rule() == Rule::operator_function_prefix {
            is_function = true;
            is_operator_function = true;
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
    let mut imports: Vec<(String, String)> = Vec::new();
    let mut redeclare_extends: Vec<crate::ast::RedeclareExtendsBlock> = Vec::new();
    let mut class_annotation: Option<String> = None;
    let mut external_info: Option<crate::ast::ExternalDecl> = None;

    for pair in inner {
        match pair.as_rule() {
            Rule::declaration_section => {
                decl_parse::parse_declaration_section(
                    pair,
                    &mut declarations,
                    &mut extends,
                    &mut inner_classes,
                    &mut type_aliases,
                    &mut imports,
                    &mut redeclare_extends,
                    parse_model,
                )?;
            }
            Rule::equation_section => {
                eq_parse::parse_equation_section(pair, &mut equations);
            }
            Rule::initial_equation_section => {
                eq_parse::parse_initial_equation_section(pair, &mut initial_equations);
            }
            Rule::algorithm_section => {
                alg_parse::parse_algorithm_section(pair, &mut algorithms, Some(&mut declarations));
            }
            Rule::initial_algorithm_section => {
                alg_parse::parse_initial_algorithm_section(
                    pair,
                    &mut initial_algorithms,
                    Some(&mut declarations),
                );
            }
            Rule::external_section => {
                let ext_inner: Vec<_> = pair.into_inner().collect();
                let mut lang = None;
                let mut c_name = None;
                for p in ext_inner {
                    if matches!(p.as_rule(), Rule::string_comment | Rule::string_literal) {
                        let s = p.as_str().trim();
                        lang = Some(s.trim_matches('"').to_string());
                    } else if p.as_rule() == Rule::external_binding {
                        c_name = external_c_name_from_binding(p);
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
            is_operator_function,
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
            is_operator_function: false,
            is_record,
            is_block,
            is_expandable: false,
            extends,
            declarations,
            equations,
            algorithms,
            initial_equations,
            initial_algorithms,
            annotation: class_annotation,
            inner_class_index: {
                let mut idx = std::collections::HashMap::new();
                for (i, m) in inner_classes.iter().enumerate() {
                    idx.insert(m.name.clone(), i);
                }
                idx
            },
            inner_classes,
            is_operator_record,
            type_aliases,
            imports,
            external_info: None,
            redeclare_extends,
        }))
    }
}
