mod algorithm;
mod equation;
mod expression;
mod helpers;

use crate::ast::*;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "src/modelica.pest"]
pub struct ModelicaParser;

fn try_parse_connector_alias_file(input: &str) -> Option<(String, String)> {
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

pub fn parse(input: &str) -> Result<ClassItem, pest::error::Error<Rule>> {
    if let Some((alias, base)) = try_parse_connector_alias_file(input) {
        return Ok(ClassItem::Model(Model {
            name: alias.clone(),
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
            is_operator_record: false,
            type_aliases: vec![(alias, base)],
            imports: Vec::new(),
            external_info: None,
        }));
    }
    let mut pairs = ModelicaParser::parse(Rule::model_file, input)?;
    let program = pairs.next().unwrap();
    let item_pair = program
        .into_inner()
        .find(|p| {
            matches!(
                p.as_rule(),
                Rule::model_definition
                    | Rule::short_class_definition
                    | Rule::type_definition
                    | Rule::connector_alias_definition
            )
        })
        .expect("model_file must contain a top-level class item");
    match item_pair.as_rule() {
        Rule::model_definition => parse_model(item_pair),
        Rule::short_class_definition | Rule::type_definition => {
            // MSL: many Types/*.mo files are single short type definitions without enclosing package.mo.
            // Represent as a minimal Model whose type_aliases include (self_name -> base_type).
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
            let type_aliases = if !alias.is_empty() && !base.is_empty() {
                vec![(alias.clone(), base)]
            } else {
                Vec::new()
            };
            Ok(ClassItem::Model(Model {
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
                is_operator_record: false,
                type_aliases,
                imports: Vec::new(),
                external_info: None,
            }))
        }
        Rule::connector_alias_definition => {
            // Minimal connector alias definition at top-level; treat as type alias.
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
            let type_aliases = if !alias.is_empty() && !base.is_empty() {
                vec![(alias.clone(), base)]
            } else {
                Vec::new()
            };
            Ok(ClassItem::Model(Model {
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
                is_operator_record: false,
                type_aliases,
                imports: Vec::new(),
                external_info: None,
            }))
        }
        _ => parse_model(item_pair),
    }
}

pub fn parse_expression_from_str(input: &str) -> Result<Expression, pest::error::Error<Rule>> {
    let mut pairs = ModelicaParser::parse(Rule::expression, input)?;
    let pair = pairs.next().unwrap();
    Ok(expression::parse_expression(pair))
}

fn parse_annotation_to_string(pair: &pest::iterators::Pair<Rule>) -> String {
    pair.as_str().trim().to_string()
}

fn normalize_identifier(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return s[1..(s.len() - 1)].to_string();
    }
    s.to_string()
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
    let mut imports: Vec<(String, String)> = Vec::new();
    let mut class_annotation: Option<String> = None;
    let mut external_info: Option<crate::ast::ExternalDecl> = None;

    for pair in inner {
        match pair.as_rule() {
            Rule::declaration_section => {
                for decl_pair in pair.into_inner() {
                    match decl_pair.as_rule() {
                        Rule::import_clause => {
                            // Parse-only for now, but keep alias mapping for name resolution.
                            let raw = decl_pair.as_str().trim().trim_end_matches(';').trim();
                            // Supported forms:
                            //   import Modelica.Blocks.Interfaces;
                            //   import SI = Modelica.SIunits;
                            //   import A.B.{x,y,z};
                            let rest = raw.strip_prefix("import").unwrap_or(raw).trim();
                            if let Some((a, b)) = rest.split_once('=') {
                                let alias = a.trim().to_string();
                                let qual = b.trim().trim_end_matches(';').trim().to_string();
                                if !alias.is_empty() && !qual.is_empty() {
                                    imports.push((alias, qual));
                                }
                            } else {
                                let qual_raw = rest.trim().trim_end_matches(';').trim();
                                // Handle brace imports: import A.B.{x, y, z};
                                if let (Some(lbrace), Some(rbrace)) =
                                    (qual_raw.find('{'), qual_raw.rfind('}'))
                                {
                                    let prefix = qual_raw[..lbrace].trim().trim_end_matches('.').trim();
                                    let inside = qual_raw[(lbrace + 1)..rbrace].trim();
                                    if !prefix.is_empty() && !inside.is_empty() {
                                        for item in inside.split(',') {
                                            let item = item.trim();
                                            if item.is_empty() {
                                                continue;
                                            }
                                            let item_name = normalize_identifier(item);
                                            if !item_name.is_empty() {
                                                imports.push((
                                                    item_name.clone(),
                                                    format!("{}.{}", prefix, item_name),
                                                ));
                                            }
                                        }
                                    }
                                } else {
                                    let qual = qual_raw.to_string();
                                    if !qual.is_empty() {
                                        let alias = qual
                                            .split('.')
                                            .last()
                                            .unwrap_or("")
                                            .trim()
                                            .to_string();
                                        if !alias.is_empty() {
                                            imports.push((alias, qual));
                                        }
                                    }
                                }
                            }
                        }
                        Rule::visibility_clause => {
                            // Parse-only: ignore visibility sections (public/protected).
                        }
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
                                    Rule::type_name => {
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
                                    Rule::component_ref => {
                                        if base.is_empty() {
                                            base = p.as_str().trim().to_string();
                                        }
                                    }
                                    Rule::enumeration_type => base = "Integer".to_string(),
                                    _ => {}
                                }
                            }
                            if !type_id.is_empty() && !base.is_empty() {
                                // Common MSL pattern: type X = Modelica.Icons.TypeReal(...);
                                if base.contains("TypeInteger") {
                                    base = "Integer".to_string();
                                } else if base.contains("TypeBoolean") {
                                    base = "Boolean".to_string();
                                } else if base.contains("TypeString") {
                                    base = "String".to_string();
                                } else if base.contains("TypeReal") {
                                    base = "Real".to_string();
                                }
                                type_aliases.push((type_id, base));
                            }
                        }
                        Rule::short_class_definition => {
                            let mut prefixes = String::new();
                            let mut alias = String::new();
                            let mut base = String::new();
                            let mut rhs_is_type_name = false;
                            for p in decl_pair.into_inner() {
                                match p.as_rule() {
                                    Rule::class_prefixes => {
                                        if prefixes.is_empty() {
                                            prefixes = p.as_str().trim().to_string();
                                        }
                                    }
                                    Rule::identifier => {
                                        if alias.is_empty() {
                                            alias = p.as_str().trim().to_string();
                                        }
                                    }
                                    Rule::type_name => {
                                        if base.is_empty() {
                                            rhs_is_type_name = true;
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
                                    Rule::component_ref => {
                                        if base.is_empty() {
                                            base = p.as_str().trim().to_string();
                                        }
                                    }
                                    Rule::short_class_definition_rhs => {
                                        for rhs in p.into_inner() {
                                            match rhs.as_rule() {
                                                Rule::type_name => {
                                                    if base.is_empty() {
                                                        rhs_is_type_name = true;
                                                        base = rhs.as_str().trim().to_string();
                                                    }
                                                }
                                                Rule::function_call => {
                                                    if base.is_empty() {
                                                        let mut it = rhs.into_inner();
                                                        if let Some(name_pair) = it.next() {
                                                            base = name_pair.as_str().trim().to_string();
                                                        }
                                                    }
                                                }
                                                Rule::component_ref => {
                                                    if base.is_empty() {
                                                        base = rhs.as_str().trim().to_string();
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if !alias.is_empty() && !base.is_empty() {
                                // `type X = ...` remains a type alias. Other short class definitions like
                                // `model PointMass = Some.Package.PointMass(...)` must be loadable as inner classes.
                                if prefixes.contains("type") {
                                    if rhs_is_type_name {
                                        type_aliases.push((alias, base));
                                    } else {
                                        let prim = if base.contains("TypeInteger") {
                                            "Integer"
                                        } else if base.contains("TypeBoolean") {
                                            "Boolean"
                                        } else if base.contains("TypeString") {
                                            "String"
                                        } else {
                                            "Real"
                                        };
                                        type_aliases.push((alias, prim.to_string()));
                                    }
                                    continue;
                                }

                                // Short class definitions for model/package/block/etc. need to be loadable
                                // as inner classes (MSL).
                                let is_function = prefixes.contains("function");
                                let is_record = prefixes.contains("record");
                                let is_block = prefixes.contains("block");
                                let is_connector = prefixes.contains("connector");
                                let is_operator_record =
                                    prefixes.contains("operator") && prefixes.contains("record");
                                inner_classes.push(Model {
                                    name: alias,
                                    is_connector,
                                    is_function,
                                    is_record,
                                    is_block,
                                    extends: vec![crate::ast::ExtendsClause {
                                        model_name: base.trim_start_matches('.').to_string(),
                                        modifications: Vec::new(),
                                    }],
                                    declarations: Vec::new(),
                                    equations: Vec::new(),
                                    algorithms: Vec::new(),
                                    initial_equations: Vec::new(),
                                    initial_algorithms: Vec::new(),
                                    annotation: None,
                                    inner_classes: Vec::new(),
                                    is_operator_record,
                                    type_aliases: Vec::new(),
                                    imports: Vec::new(),
                                    external_info: None,
                                });
                            }
                        }
                        Rule::connector_alias_definition => {
                            let mut alias = String::new();
                            let mut base = String::new();
                            for p in decl_pair.into_inner() {
                                match p.as_rule() {
                                    Rule::identifier => {
                                        if alias.is_empty() {
                                            alias = p.as_str().trim().to_string();
                                        } else if base.is_empty() {
                                            // type_name can also appear as identifier in some cases
                                            // but we prefer type_name rule below.
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
                            if !alias.is_empty() && !base.is_empty() {
                                type_aliases.push((alias, base));
                            }
                        }
                        Rule::model_definition => match parse_model(decl_pair) {
                            Ok(crate::ast::ClassItem::Model(m)) => inner_classes.push(m),
                            Ok(crate::ast::ClassItem::Function(f)) => {
                                inner_classes.push(crate::ast::Model::from(f))
                            }
                            Err(e) => return Err(e),
                        },
                        Rule::extends_clause => {
                            let ext_inner = decl_pair.into_inner();
                            let mut full_name = String::new();
                            let mut modifications = Vec::new();

                            for token in ext_inner {
                                match token.as_rule() {
                                    Rule::dotted_identifier => {
                                        full_name = token.as_str().trim_start_matches('.').to_string();
                                    }
                                    Rule::identifier => {
                                        if !full_name.is_empty() {
                                            full_name.push('.');
                                        }
                                        full_name.push_str(token.as_str());
                                    }
                                    Rule::modification_part => {
                                        let mod_list =
                                            token.into_inner().next().unwrap().into_inner();
                                        for mod_pair in mod_list {
                                            let modification_pair =
                                                if mod_pair.as_rule() == Rule::modification {
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
                                            let mod_each = mod_inner
                                                .iter()
                                                .any(|p| p.as_str().trim() == "each");
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
                                    | Rule::final_kw
                                    | Rule::constant_kw
                                    | Rule::flow_kw
                                    | Rule::stream_kw
                                    | Rule::discrete_kw
                                    | Rule::input_kw
                                    | Rule::output_kw
                                    | Rule::inner_kw
                                    | Rule::outer_kw
                                    | Rule::replaceable_kw
                            ) {
                                match next_token.as_rule() {
                                    Rule::parameter_kw => is_parameter = true,
                                    Rule::final_kw => {}
                                    Rule::constant_kw => is_parameter = true,
                                    Rule::flow_kw => is_flow = true,
                                    Rule::stream_kw => {}
                                    Rule::discrete_kw => is_discrete = true,
                                    Rule::input_kw => is_input = true,
                                    Rule::output_kw => is_output = true,
                                    Rule::inner_kw | Rule::outer_kw => {}
                                    Rule::replaceable_kw => is_replaceable = true,
                                    _ => {}
                                }
                                next_token = decl_inner.next().unwrap();
                            }
                            if next_token.as_rule() == Rule::replaceable_kw {
                                is_replaceable = true;
                                next_token = decl_inner.next().unwrap();
                            }
                            let type_name = next_token.as_str().trim().to_string();
                            let type_name = type_name.trim_start_matches('.').to_string();

                            let mut array_size = None;
                            if let Some(token) = decl_inner.peek() {
                                if token.as_rule() == Rule::array_subscript {
                                    let mut sub_inner = decl_inner.next().unwrap().into_inner();
                                    let dim_inner = sub_inner.next().unwrap();
                                    let dim_expr = if dim_inner.as_rule() == Rule::subscript_item {
                                        dim_inner.into_inner().next()
                                    } else {
                                        Some(dim_inner)
                                    };
                                    if let Some(dim_expr) = dim_expr {
                                        if dim_expr.as_rule() == Rule::expression {
                                            array_size = Some(expression::parse_expression(dim_expr));
                                        }
                                    }
                                }
                            }
                            let name_pair = decl_inner.next().unwrap();
                            let mut var_names: Vec<String> = Vec::new();
                            if name_pair.as_rule() == Rule::var_name_list {
                                for p in name_pair.into_inner() {
                                    if p.as_rule() == Rule::identifier {
                                        let n = normalize_identifier(p.as_str().trim());
                                        if !n.is_empty() {
                                            var_names.push(n);
                                        }
                                    }
                                }
                            } else {
                                let n = normalize_identifier(name_pair.as_str().trim());
                                if !n.is_empty() {
                                    var_names.push(n);
                                }
                            }
                            if let Some(token) = decl_inner.peek() {
                                if token.as_rule() == Rule::array_subscript {
                                    let mut sub_inner = decl_inner.next().unwrap().into_inner();
                                    let dim_inner = sub_inner.next().unwrap();
                                    let dim_expr = if dim_inner.as_rule() == Rule::subscript_item {
                                        dim_inner.into_inner().next()
                                    } else {
                                        Some(dim_inner)
                                    };
                                    if let Some(dim_expr) = dim_expr {
                                        if dim_expr.as_rule() == Rule::expression {
                                            array_size = Some(expression::parse_expression(dim_expr));
                                        }
                                    }
                                }
                            }

                            let mut start_value = None;
                            let mut modifications = Vec::new();
                            let mut decl_annotation: Option<String> = None;
                            let mut is_rest = false;
                            let mut decl_condition: Option<crate::ast::Expression> = None;

                            for token in decl_inner {
                                match token.as_rule() {
                                    Rule::annotation => {
                                        decl_annotation = Some(parse_annotation_to_string(&token));
                                    }
                                    Rule::conditional_clause => {
                                        let expr_pair = token.into_inner().next().unwrap();
                                        decl_condition =
                                            Some(expression::parse_expression(expr_pair));
                                    }
                                    Rule::value_assignment => {
                                        let expr_pair = token.into_inner().next().unwrap();
                                        start_value = Some(expression::parse_expression(expr_pair));
                                    }
                                    Rule::rest_param => {
                                        is_rest = true;
                                    }
                                    Rule::modification_part => {
                                        let mod_list =
                                            token.into_inner().next().unwrap().into_inner();
                                        for mod_pair in mod_list {
                                            let modification_pair =
                                                if mod_pair.as_rule() == Rule::modification {
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
                                            let mod_each = mod_inner
                                                .iter()
                                                .any(|p| p.as_str().trim() == "each");
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

                            for var_name in var_names {
                                declarations.push(Declaration {
                                    type_name: type_name.clone(),
                                    name: var_name,
                                    replaceable: is_replaceable,
                                    is_parameter,
                                    is_flow,
                                    is_discrete,
                                    is_input,
                                    is_output,
                                    start_value: start_value.clone(),
                                    array_size: array_size.clone(),
                                    modifications: modifications.clone(),
                                    is_rest,
                                    annotation: decl_annotation.clone(),
                                    condition: decl_condition.clone(),
                                });
                            }
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
                            let a_expr = expression::parse_expression(conn_inner.next().unwrap());
                            let b_expr = expression::parse_expression(conn_inner.next().unwrap());
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
                            let var_expr = expression::parse_component_ref(inner.next().unwrap());
                            let val_expr = expression::parse_expression(inner.next().unwrap());
                            let var_name = helpers::expr_to_string(var_expr);
                            equations.push(Equation::Reinit(var_name, val_expr));
                        }
                        Rule::assert_stmt => {
                            let mut args: Vec<Expression> = Vec::new();
                            if let Some(arg_list) = inner_stmt.into_inner().next() {
                                for item in arg_list.into_inner() {
                                    match item.as_rule() {
                                        Rule::named_arg => {
                                            let mut ni = item.into_inner();
                                            let name = ni.next().unwrap().as_str().to_string();
                                            let val = expression::parse_expression(ni.next().unwrap());
                                            args.push(Expression::Call(
                                                "named".to_string(),
                                                vec![Expression::StringLiteral(name), val],
                                            ));
                                        }
                                        Rule::expression => args.push(expression::parse_expression(item)),
                                        _ => {}
                                    }
                                }
                            }
                            let cond = args.get(0).cloned().unwrap_or(Expression::Number(0.0));
                            let msg = args.get(1).cloned().unwrap_or(Expression::Number(0.0));
                            equations.push(Equation::Assert(cond, msg));
                        }
                        Rule::terminate_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let msg = expression::parse_expression(inner.next().unwrap());
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
                            let a_expr = expression::parse_expression(conn_inner.next().unwrap());
                            let b_expr = expression::parse_expression(conn_inner.next().unwrap());
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
                            initial_equations.push(equation::parse_when_equation(inner_stmt));
                        }
                        Rule::if_equation => {
                            initial_equations.push(equation::parse_if_equation(inner_stmt));
                        }
                        Rule::reinit_clause => {
                            let mut inner = inner_stmt.into_inner();
                            let var_expr = expression::parse_component_ref(inner.next().unwrap());
                            let val_expr = expression::parse_expression(inner.next().unwrap());
                            let var_name = helpers::expr_to_string(var_expr);
                            initial_equations.push(Equation::Reinit(var_name, val_expr));
                        }
                        Rule::assert_stmt => {
                            let mut args: Vec<Expression> = Vec::new();
                            if let Some(arg_list) = inner_stmt.into_inner().next() {
                                for item in arg_list.into_inner() {
                                    match item.as_rule() {
                                        Rule::named_arg => {
                                            let mut ni = item.into_inner();
                                            let name = ni.next().unwrap().as_str().to_string();
                                            let val = expression::parse_expression(ni.next().unwrap());
                                            args.push(Expression::Call(
                                                "named".to_string(),
                                                vec![Expression::StringLiteral(name), val],
                                            ));
                                        }
                                        Rule::expression => args.push(expression::parse_expression(item)),
                                        _ => {}
                                    }
                                }
                            }
                            let cond = args.get(0).cloned().unwrap_or(Expression::Number(0.0));
                            let msg = args.get(1).cloned().unwrap_or(Expression::Number(0.0));
                            initial_equations.push(Equation::Assert(cond, msg));
                        }
                        Rule::terminate_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let msg = expression::parse_expression(inner.next().unwrap());
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
            imports,
            external_info: None,
        }))
    }
}
