use crate::ast::*;
use crate::parser::common::{
    normalize_identifier, parse_annotation_to_string, parse_modifications_from_modification_part,
};
use crate::parser::{alg_parse, eq_parse, expression, Rule};

pub fn parse_declaration_pair(decl_pair: pest::iterators::Pair<Rule>, declarations: &mut Vec<Declaration>) {
    let mut decl_inner = decl_pair.into_inner();
    let mut is_parameter = false;
    let mut is_flow = false;
    let mut is_stream = false;
    let mut is_discrete = false;
    let mut is_input = false;
    let mut is_output = false;
    let mut is_replaceable = false;
    let mut is_inner = false;
    let mut is_outer = false;

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
            Rule::constant_kw => is_parameter = true,
            Rule::flow_kw => is_flow = true,
            Rule::stream_kw => is_stream = true,
            Rule::discrete_kw => is_discrete = true,
            Rule::input_kw => is_input = true,
            Rule::output_kw => is_output = true,
            Rule::replaceable_kw => is_replaceable = true,
            Rule::inner_kw => is_inner = true,
            Rule::outer_kw => is_outer = true,
            _ => {}
        }
        next_token = decl_inner.next().unwrap();
    }
    if next_token.as_rule() == Rule::replaceable_kw {
        is_replaceable = true;
        next_token = decl_inner.next().unwrap();
    }
    let type_name = next_token.as_str().trim().trim_start_matches('.').to_string();
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
    let mut component_items: Vec<(
        String,
        Vec<Modification>,
        Option<Expression>,
        Option<Expression>,
    )> = Vec::new();
    if name_pair.as_rule() == Rule::var_name_list {
        for p in name_pair.into_inner() {
            if p.as_rule() != Rule::component_decl_item {
                continue;
            }
            let mut item_it = p.into_inner();
            let id_pair = item_it.next().unwrap();
            let vname = normalize_identifier(id_pair.as_str().trim());
            if vname.is_empty() {
                continue;
            }
            let mut item_mods: Vec<Modification> = Vec::new();
            let mut item_start_mod: Option<Expression> = None;
            let mut item_assign: Option<Expression> = None;
            for part in item_it {
                match part.as_rule() {
                    Rule::modification_part => {
                        let (m, s) = parse_modifications_from_modification_part(part);
                        item_mods.extend(m);
                        if item_start_mod.is_none() {
                            item_start_mod = s;
                        }
                    }
                    Rule::value_assignment => {
                        let expr_pair = part.into_inner().next().unwrap();
                        item_assign = Some(expression::parse_expression(expr_pair));
                    }
                    _ => {}
                }
            }
            component_items.push((vname, item_mods, item_start_mod, item_assign));
        }
    } else {
        let n = normalize_identifier(name_pair.as_str().trim());
        if !n.is_empty() {
            component_items.push((n, Vec::new(), None, None));
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

    let mut global_modifications = Vec::new();
    let mut global_start: Option<Expression> = None;
    let mut decl_annotation: Option<String> = None;
    let mut is_rest = false;
    let mut decl_condition: Option<crate::ast::Expression> = None;
    let mut trailing_value: Option<Expression> = None;
    let mut constrainedby_type: Option<String> = None;

    for token in decl_inner {
        match token.as_rule() {
            Rule::annotation => {
                decl_annotation = Some(parse_annotation_to_string(&token));
            }
            Rule::conditional_clause => {
                let expr_pair = token.into_inner().next().unwrap();
                decl_condition = Some(expression::parse_expression(expr_pair));
            }
            Rule::value_assignment => {
                let expr_pair = token.into_inner().next().unwrap();
                trailing_value = Some(expression::parse_expression(expr_pair));
            }
            Rule::rest_param => {
                is_rest = true;
            }
            Rule::modification_part => {
                let (m, s) = parse_modifications_from_modification_part(token);
                global_modifications.extend(m);
                if global_start.is_none() {
                    global_start = s;
                }
            }
            Rule::constrainedby_clause => {
                let mut c = String::new();
                for cpart in token.into_inner() {
                    if cpart.as_rule() == Rule::type_name {
                        c = cpart.as_str().trim().to_string();
                    }
                }
                if !c.is_empty() {
                    constrainedby_type = Some(c);
                }
            }
            Rule::string_comment => {}
            _ => {}
        }
    }

    let single_component = component_items.len() == 1;
    for (var_name, item_mods, item_start_mod, item_assign) in component_items {
        let start_value = item_assign
            .clone()
            .or(item_start_mod.clone())
            .or(global_start.clone())
            .or_else(|| {
                if single_component {
                    trailing_value.clone()
                } else {
                    None
                }
            });
        let mut modifications = global_modifications.clone();
        modifications.extend(item_mods);
        declarations.push(Declaration {
            type_name: type_name.clone(),
            name: var_name,
            replaceable: is_replaceable,
            constrainedby_type: constrainedby_type.clone(),
            is_parameter,
            is_flow,
            is_stream,
            is_discrete,
            is_input,
            is_output,
            is_inner,
            is_outer,
            is_public: false,
            is_protected: false,
            start_value,
            array_size: array_size.clone(),
            modifications,
            is_rest,
            annotation: decl_annotation.clone(),
            condition: decl_condition.clone(),
        });
    }
}

fn parse_redeclare_extends_block(
    decl_pair: pest::iterators::Pair<Rule>,
    redeclare_extends: &mut Vec<RedeclareExtendsBlock>,
    parse_model_fn: for<'a> fn(
        pest::iterators::Pair<'a, Rule>,
    ) -> Result<ClassItem, pest::error::Error<Rule>>,
) -> Result<(), pest::error::Error<Rule>> {
    let mut extends_target = String::new();
    let mut is_function = false;
    let mut is_operator_function = false;
    let mut clause_modifications = Vec::new();
    let mut declarations = Vec::new();
    let mut inner_extends = Vec::new();
    let mut inner_classes = Vec::new();
    let mut type_aliases = Vec::new();
    let mut imports = Vec::new();
    let mut inner_redecl = Vec::new();
    let mut equations = Vec::new();
    let mut initial_equations = Vec::new();
    let mut algorithms = Vec::new();
    let mut initial_algorithms = Vec::new();

    for p in decl_pair.into_inner() {
        let p_text = p.as_str().trim().to_string();
        match p.as_rule() {
            Rule::identifier => {
                if extends_target.is_empty() {
                    extends_target = normalize_identifier(p.as_str().trim());
                }
            }
            Rule::function_prefix => {
                is_function = true;
            }
            Rule::modification_part => {
                let (m, _) = parse_modifications_from_modification_part(p);
                clause_modifications.extend(m);
            }
            Rule::declaration_section => {
                parse_declaration_section(
                    p,
                    &mut declarations,
                    &mut inner_extends,
                    &mut inner_classes,
                    &mut type_aliases,
                    &mut imports,
                    &mut inner_redecl,
                    parse_model_fn,
                )?;
            }
            Rule::equation_section => eq_parse::parse_equation_section(p, &mut equations),
            Rule::initial_equation_section => {
                eq_parse::parse_initial_equation_section(p, &mut initial_equations);
            }
            Rule::algorithm_section => {
                alg_parse::parse_algorithm_section(p, &mut algorithms, None);
            }
            Rule::initial_algorithm_section => {
                alg_parse::parse_initial_algorithm_section(p, &mut initial_algorithms, None);
            }
            Rule::external_section | Rule::annotation_clause | Rule::end_part | Rule::string_comment => {}
            _ => {}
        }
        if p_text == "operator" {
            is_operator_function = true;
            is_function = true;
        }
    }

    if extends_target.is_empty() {
        return Ok(());
    }

    redeclare_extends.push(RedeclareExtendsBlock {
        extends_target,
        is_function,
        is_operator_function,
        clause_modifications,
        declarations,
        equations,
        initial_equations,
        algorithms,
        initial_algorithms,
        inner_classes,
        extends: inner_extends,
        type_aliases,
        imports,
        nested_redeclare_extends: inner_redecl,
    });

    Ok(())
}

pub fn parse_declaration_section(
    pair: pest::iterators::Pair<Rule>,
    declarations: &mut Vec<Declaration>,
    extends: &mut Vec<ExtendsClause>,
    inner_classes: &mut Vec<Model>,
    type_aliases: &mut Vec<(String, String)>,
    imports: &mut Vec<(String, String)>,
    redeclare_extends: &mut Vec<RedeclareExtendsBlock>,
    parse_model_fn: for<'a> fn(
        pest::iterators::Pair<'a, Rule>,
    ) -> Result<ClassItem, pest::error::Error<Rule>>,
) -> Result<(), pest::error::Error<Rule>> {
    for decl_pair in pair.into_inner() {
        match decl_pair.as_rule() {
            Rule::import_clause => {
                let raw = decl_pair.as_str().trim().trim_end_matches(';').trim();
                let rest = raw.strip_prefix("import").unwrap_or(raw).trim();
                if let Some((a, b)) = rest.split_once('=') {
                    let alias = a.trim().to_string();
                    let qual = b.trim().trim_end_matches(';').trim().to_string();
                    if !alias.is_empty() && !qual.is_empty() {
                        imports.push((alias, qual));
                    }
                } else {
                    let qual_raw = rest.trim().trim_end_matches(';').trim();
                    if let (Some(lbrace), Some(rbrace)) = (qual_raw.find('{'), qual_raw.rfind('}')) {
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
                                    imports.push((item_name.clone(), format!("{}.{}", prefix, item_name)));
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
            Rule::visibility_clause => {}
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

                    let is_function = prefixes.contains("function");
                    let is_operator_function = prefixes.contains("operator") && prefixes.contains("function");
                    let is_record = prefixes.contains("record");
                    let is_block = prefixes.contains("block");
                    let is_connector = prefixes.contains("connector");
                    let is_expandable = is_connector && prefixes.contains("expandable");
                    let is_partial = prefixes.contains("partial");
                    let is_encapsulated = prefixes.contains("encapsulated");
                    let is_pure = prefixes.contains("pure") && !prefixes.contains("impure");
                    let is_impure = prefixes.contains("impure");
                    let is_operator_record = prefixes.contains("operator") && prefixes.contains("record");
                    inner_classes.push(Model {
                        name: alias,
                        is_connector,
                        is_function,
                        is_operator_function,
                        is_record,
                        is_block,
                        is_expandable,
                        is_partial,
                        is_encapsulated,
                        is_pure,
                        is_impure,
                        extends: vec![ExtendsClause {
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
                        inner_class_index: std::collections::HashMap::new(),
                        is_operator_record,
                        type_aliases: Vec::new(),
                        imports: Vec::new(),
                        external_info: None,
                        redeclare_extends: Vec::new(),
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
            Rule::model_definition => match parse_model_fn(decl_pair) {
                Ok(crate::ast::ClassItem::Model(m)) => inner_classes.push(m),
                Ok(crate::ast::ClassItem::Function(f)) => inner_classes.push(crate::ast::Model::from(f)),
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
                            let (m, _) = parse_modifications_from_modification_part(token);
                            modifications.extend(m);
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
                parse_declaration_pair(decl_pair, declarations);
            }
            Rule::redeclare_model_extends | Rule::redeclare_function_extends => {
                parse_redeclare_extends_block(decl_pair, redeclare_extends, parse_model_fn)?;
            }
            _ => {}
        }
    }
    Ok(())
}
