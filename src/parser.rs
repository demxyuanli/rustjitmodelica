use pest::Parser;
use pest_derive::Parser;
use crate::ast::*;

#[derive(Parser)]
#[grammar = "modelica.pest"]
pub struct ModelicaParser;

pub fn parse(input: &str) -> Result<ClassItem, pest::error::Error<Rule>> {
    let mut pairs = ModelicaParser::parse(Rule::model_file, input)?;
    let program = pairs.next().unwrap();
    let model_pair = program.into_inner().next().unwrap();
    parse_model(model_pair)
}

/// Extract annotation as raw string for storage in AST (parse-only; ignored in backend).
fn parse_annotation_to_string(pair: &pest::iterators::Pair<Rule>) -> String {
    pair.as_str().trim().to_string()
}

fn parse_model(pair: pest::iterators::Pair<Rule>) -> Result<ClassItem, pest::error::Error<Rule>> {
    let mut inner = pair.into_inner();
    
    // Grammar: class_prefixes ~ identifier ~ declaration_section ~ ...
    // 1. class_prefixes (partial? ~ (model | connector | function | record | ...))
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
    
    // 2. identifier (name)
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
                                Ok(crate::ast::ClassItem::Function(f)) => inner_classes.push(crate::ast::Model::from(f)),
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
                                        // Fallback if grammar didn't match dotted_identifier (shouldn't happen with new grammar)
                                        if !full_name.is_empty() {
                                            full_name.push('.');
                                        }
                                        full_name.push_str(token.as_str());
                                    }
                                    Rule::modification_part => {
                                        let mod_list = token.into_inner().next().unwrap().into_inner();
                                        for mod_pair in mod_list {
                                            let modification_pair = if mod_pair.as_rule() == Rule::modification {
                                                mod_pair
                                            } else {
                                                match mod_pair.into_inner().find(|p| p.as_rule() == Rule::modification) {
                                                    Some(p) => p,
                                                    None => continue,
                                                }
                                            };
                                            let mod_inner: Vec<_> = modification_pair.into_inner().collect();
                                            let mod_redeclare = mod_inner.iter().any(|p| p.as_str().trim() == "redeclare");
                                            let mod_redeclare_type = mod_inner.iter()
                                                .find(|p| p.as_rule() == Rule::type_name)
                                                .map(|p| p.as_str().trim().to_string());
                                            let mod_each = mod_inner.iter().any(|p| p.as_str().trim() == "each");
                                            let name_pair = match mod_inner.iter().find(|p| p.as_rule() == Rule::component_ref) {
                                                Some(p) => p,
                                                None => continue,
                                            };
                                            let name_expr = parse_component_ref(name_pair.clone());
                                            let mod_name = expr_to_string(name_expr);
                                            let expr_pair = match mod_inner.iter().find(|p| p.as_rule() == Rule::expression) {
                                                Some(p) => p,
                                                None => continue,
                                            };
                                            let val = Some(parse_expression(expr_pair.clone()));
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

                            while matches!(next_token.as_rule(), Rule::parameter_kw | Rule::constant_kw | Rule::flow_kw | Rule::discrete_kw | Rule::input_kw | Rule::output_kw) {
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
                                    let mut sub_inner = decl_inner.next().unwrap().into_inner();
                                    let size_expr = parse_expression(sub_inner.next().unwrap());
                                    array_size = Some(size_expr);
                                }
                            }

                            let mut start_value = None;
                            let mut modifications = Vec::new();
                            let mut decl_annotation: Option<String> = None;

                            for token in decl_inner {
                                match token.as_rule() {
                                    Rule::annotation => {
                                        decl_annotation = Some(parse_annotation_to_string(&token));
                                    }
                                    Rule::value_assignment => {
                                        let expr_pair = token.into_inner().next().unwrap();
                                        start_value = Some(parse_expression(expr_pair));
                                    }
                                    Rule::modification_part => {
                                        let mod_list = token.into_inner().next().unwrap().into_inner();
                                        for mod_pair in mod_list {
                                            let modification_pair = if mod_pair.as_rule() == Rule::modification {
                                                mod_pair
                                            } else {
                                                match mod_pair.into_inner().find(|p| p.as_rule() == Rule::modification) {
                                                    Some(p) => p,
                                                    None => continue,
                                                }
                                            };
                                            let mod_inner: Vec<_> = modification_pair.into_inner().collect();
                                            let mod_redeclare = mod_inner.iter().any(|p| p.as_str().trim() == "redeclare");
                                            let mod_redeclare_type = mod_inner.iter()
                                                .find(|p| p.as_rule() == Rule::type_name)
                                                .map(|p| p.as_str().trim().to_string());
                                            let mod_each = mod_inner.iter().any(|p| p.as_str().trim() == "each");
                                            let name_pair = match mod_inner.iter().find(|p| p.as_rule() == Rule::component_ref) {
                                                Some(p) => p,
                                                None => continue,
                                            };
                                            let name_expr = parse_component_ref(name_pair.clone());
                                            let mod_name = expr_to_string(name_expr);
                                            let expr_pair = match mod_inner.iter().find(|p| p.as_rule() == Rule::expression) {
                                                Some(p) => p,
                                                None => continue,
                                            };
                                            let val = Some(parse_expression(expr_pair.clone()));
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
                            let lhs = parse_expression(eq_parts.next().unwrap());
                            let rhs = parse_expression(eq_parts.next().unwrap());
                            equations.push(Equation::Simple(lhs, rhs));
                        }
                        Rule::connect_clause => {
                            // connect(a, b);
                            let mut conn_inner = inner_stmt.into_inner();
                            let a_expr = parse_expression(conn_inner.next().unwrap());
                            let b_expr = parse_expression(conn_inner.next().unwrap());
                            equations.push(Equation::Connect(a_expr, b_expr));
                        }
                        Rule::multi_assign_equation => {
                            equations.push(parse_multi_assign_equation(inner_stmt));
                        }
                        Rule::for_loop => {
                            equations.push(parse_for_loop(inner_stmt));
                        }
                        Rule::when_equation => {
                            // "when" ~ expression ~ "then" ~ equation_stmt+ ~ ...
                            equations.push(parse_when_equation(inner_stmt));
                        }
                        Rule::if_equation => {
                            equations.push(parse_if_equation(inner_stmt));
                        }
                        Rule::reinit_clause => {
                            // "reinit" ~ "(" ~ component_ref ~ "," ~ expression ~ ")" ~ ";"
                            let mut inner = inner_stmt.into_inner();
                            let var_expr = parse_component_ref(inner.next().unwrap());
                            let val_expr = parse_expression(inner.next().unwrap());
                            
                            // Extract variable name
                            let var_name = expr_to_string(var_expr);
                            equations.push(Equation::Reinit(var_name, val_expr));
                        }
                        Rule::assert_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let cond = parse_expression(inner.next().unwrap());
                            let msg = parse_expression(inner.next().unwrap());
                            equations.push(Equation::Assert(cond, msg));
                        }
                        Rule::terminate_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let msg = parse_expression(inner.next().unwrap());
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
                            let lhs = parse_expression(eq_parts.next().unwrap());
                            let rhs = parse_expression(eq_parts.next().unwrap());
                            initial_equations.push(Equation::Simple(lhs, rhs));
                        }
                        Rule::connect_clause => {
                            let mut conn_inner = inner_stmt.into_inner();
                            let a_expr = parse_expression(conn_inner.next().unwrap());
                            let b_expr = parse_expression(conn_inner.next().unwrap());
                            initial_equations.push(Equation::Connect(a_expr, b_expr));
                        }
                        Rule::multi_assign_equation => {
                            initial_equations.push(parse_multi_assign_equation(inner_stmt));
                        }
                        Rule::for_loop => {
                            initial_equations.push(parse_for_loop(inner_stmt));
                        }
                        Rule::when_equation => {
                            initial_equations.push(parse_when_equation(inner_stmt));
                        }
                        Rule::if_equation => {
                            initial_equations.push(parse_if_equation(inner_stmt));
                        }
                        Rule::reinit_clause => {
                            let mut inner = inner_stmt.into_inner();
                            let var_expr = parse_component_ref(inner.next().unwrap());
                            let val_expr = parse_expression(inner.next().unwrap());
                            
                            let var_name = expr_to_string(var_expr);
                            initial_equations.push(Equation::Reinit(var_name, val_expr));
                        }
                        Rule::assert_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let cond = parse_expression(inner.next().unwrap());
                            let msg = parse_expression(inner.next().unwrap());
                            initial_equations.push(Equation::Assert(cond, msg));
                        }
                        Rule::terminate_stmt => {
                            let mut inner = inner_stmt.into_inner();
                            let msg = parse_expression(inner.next().unwrap());
                            initial_equations.push(Equation::Terminate(msg));
                        }
                        _ => {}
                    }
                }
            }
            Rule::algorithm_section => {
                let alg_stmt_inner = pair.into_inner();
                for stmt in alg_stmt_inner {
                    // "algorithm" keyword is silent in algorithm_stmt+ ? No, "algorithm" is in algorithm_section.
                    // algorithm_stmt rules
                    let inner_stmt = stmt.into_inner().next().unwrap();
                    algorithms.push(parse_algorithm_stmt(inner_stmt));
                }
            }
            Rule::initial_algorithm_section => {
                let alg_stmt_inner = pair.into_inner();
                for stmt in alg_stmt_inner {
                    let inner_stmt = stmt.into_inner().next().unwrap();
                    initial_algorithms.push(parse_algorithm_stmt(inner_stmt));
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

fn expr_to_string(expr: Expression) -> String {
    match expr {
        Expression::Variable(n) => n,
        Expression::Dot(base, member) => format!("{}.{}", expr_to_string(*base), member),
        Expression::ArrayAccess(base, _idx) => {
             // This is tricky as we need to evaluate idx to string if possible, or keep it symbolic?
             // For modification names, usually indices are constants.
             // But for now let's just use a simple representation or panic if complex.
             // Actually, modifications like "R[1].v" are valid.
             // We'll simplify: return string representation.
             // Warning: This might need better handling if we want to support true AST-based modifications later.
             format!("{}[?]", expr_to_string(*base)) 
        }
        _ => "unknown".to_string(),
    }
}

fn parse_algorithm_stmt(pair: pest::iterators::Pair<Rule>) -> AlgorithmStatement {
    match pair.as_rule() {
        Rule::assignment_stmt => {
            let mut inner = pair.into_inner();
            let lhs_pair = inner.next().unwrap();
            // lhs is component_ref
            let lhs_expr = parse_component_ref(lhs_pair);
            let rhs_expr = parse_expression(inner.next().unwrap());
            AlgorithmStatement::Assignment(lhs_expr, rhs_expr)
        }
        Rule::if_stmt => {
            let mut inner = pair.into_inner();
            let mut conditions = Vec::new();
            let mut bodies = Vec::new();
            let mut else_body = None;
            
            // First condition
            let cond = parse_expression(inner.next().unwrap());
            let mut body = Vec::new();
            
            // Loop until we hit elseif, else or end
            // The grammar: "if" ~ expr ~ "then" ~ stmt+ ~ ("elseif" ~ expr ~ "then" ~ stmt+)* ~ ("else" ~ stmt+)? ...
            
            // Pest iteration is linear.
            // We need to check the rules of the children.
            // The children will be: expression, algorithm_stmt, algorithm_stmt..., [expression, algorithm_stmt...], [algorithm_stmt...]
            
            // Let's manually iterate and group.
            
            // Current group
            let mut current_cond = Some(cond);
            
            for token in inner {
                match token.as_rule() {
                    Rule::expression => {
                        // New elseif condition
                        // Push previous
                        if let Some(c) = current_cond.take() {
                             conditions.push(c);
                             bodies.push(body);
                             body = Vec::new();
                        }
                        current_cond = Some(parse_expression(token));
                    }
                    Rule::algorithm_stmt => {
                        body.push(parse_algorithm_stmt(token.into_inner().next().unwrap()));
                    }
                    _ => {}
                }
            }
            
            // Handle the last group
            if let Some(c) = current_cond {
                conditions.push(c);
                bodies.push(body);
            } else {
                // It was the else block
                else_body = Some(body);
            }
            
            // Construct AST
            // We need to convert flat vectors into nested If/ElseIf structure or the specific AST variant.
            // Our AST: If(Expression, Vec<AlgorithmStatement>, Vec<(Expression, Vec<AlgorithmStatement>)>, Option<Vec<AlgorithmStatement>>)
            
            if conditions.is_empty() {
                // Should not happen based on grammar
                return AlgorithmStatement::If(Expression::Number(0.0), vec![], vec![], None);
            }
            
            let main_cond = conditions.remove(0);
            let main_body = bodies.remove(0);
            
            let mut else_ifs = Vec::new();
            while !conditions.is_empty() {
                else_ifs.push((conditions.remove(0), bodies.remove(0)));
            }
            
            AlgorithmStatement::If(main_cond, main_body, else_ifs, else_body)
        }
        Rule::for_stmt => {
             let mut inner = pair.into_inner();
             let loop_var = inner.next().unwrap().as_str().to_string();
             
             let range_or_expr = inner.next().unwrap();
             let range_expr = match range_or_expr.as_rule() {
                 Rule::range => {
                     // range = expression : expression
                     // Parse into Expression::Range(start, step=1, end)
                     // Wait, range can be start:step:end
                     let mut r_inner = range_or_expr.into_inner();
                     let start = parse_expression(r_inner.next().unwrap());
                     let second = parse_expression(r_inner.next().unwrap());
                     
                     if let Some(third_pair) = r_inner.next() {
                         // start : step : end
                         let third = parse_expression(third_pair);
                         Expression::Range(Box::new(start), Box::new(second), Box::new(third))
                     } else {
                         // start : end
                         // Default step is 1.0
                         Expression::Range(Box::new(start), Box::new(Expression::Number(1.0)), Box::new(second))
                     }
                 }
                 Rule::expression => parse_expression(range_or_expr),
                 _ => unreachable!("Unexpected rule in for_stmt range: {:?}", range_or_expr.as_rule()),
             };
             
             let mut body = Vec::new();
             for stmt in inner {
                 body.push(parse_algorithm_stmt(stmt.into_inner().next().unwrap()));
             }
             AlgorithmStatement::For(loop_var, Box::new(range_expr), body)
        }
        Rule::while_stmt => {
             let mut inner = pair.into_inner();
             let cond = parse_expression(inner.next().unwrap());
             let mut body = Vec::new();
             for stmt in inner {
                 body.push(parse_algorithm_stmt(stmt.into_inner().next().unwrap()));
             }
             AlgorithmStatement::While(cond, body)
        }
        Rule::when_stmt => {
            // Similar to if
             let mut inner = pair.into_inner();
             let cond = parse_expression(inner.next().unwrap());
             let mut body = Vec::new();
             let else_whens = Vec::new();
             
             // ... parsing logic similar to if ...
             // Simplified for now: just main when
             for stmt in inner {
                 if stmt.as_rule() == Rule::algorithm_stmt {
                     body.push(parse_algorithm_stmt(stmt.into_inner().next().unwrap()));
                 }
             }
             AlgorithmStatement::When(cond, body, else_whens)
        }
        _ => unreachable!("Unknown algorithm stmt rule: {:?}", pair.as_rule()),
    }
}

fn parse_component_ref(pair: pest::iterators::Pair<Rule>) -> Expression {
    // Re-use logic from parse_factor but strictly for component_ref
    let mut pairs = pair.into_inner();
    let mut expr = Expression::Variable(pairs.next().unwrap().as_str().to_string());
    
    for part in pairs {
        match part.as_rule() {
            Rule::array_subscript => {
                let idx = parse_expression(part.into_inner().next().unwrap());
                expr = Expression::ArrayAccess(Box::new(expr), Box::new(idx));
            }
            Rule::member_access => {
                let name = part.into_inner().next().unwrap().as_str().to_string();
                expr = Expression::Dot(Box::new(expr), name);
            }
            _ => unreachable!("Unexpected component_ref part"),
        }
    }
    expr
}


fn parse_for_loop(pair: pest::iterators::Pair<Rule>) -> Equation {
    let mut inner = pair.into_inner();
    let loop_var = inner.next().unwrap().as_str().to_string();
    
    let range_pair = inner.next().unwrap();
    let mut range_inner = range_pair.into_inner();
    let start_expr = parse_expression(range_inner.next().unwrap());
    let end_expr = parse_expression(range_inner.next().unwrap());
    
    let mut body = Vec::new();
    
    // Remaining are equation statements
    for stmt in inner {
        let inner_stmt = stmt.into_inner().next().unwrap();
        match inner_stmt.as_rule() {
             Rule::equation => {
                let mut eq_parts = inner_stmt.into_inner();
                let lhs = parse_expression(eq_parts.next().unwrap());
                let rhs = parse_expression(eq_parts.next().unwrap());
                body.push(Equation::Simple(lhs, rhs));
            }
             Rule::connect_clause => {
                let mut conn_inner = inner_stmt.into_inner();
                let a_expr = parse_expression(conn_inner.next().unwrap());
                let b_expr = parse_expression(conn_inner.next().unwrap());
                body.push(Equation::Connect(a_expr, b_expr));
            }
             Rule::for_loop => {
                 body.push(parse_for_loop(inner_stmt));
             }
             _ => {}
        }
    }
    
    Equation::For(loop_var, Box::new(start_expr), Box::new(end_expr), body)
}

fn parse_when_equation(pair: pest::iterators::Pair<Rule>) -> Equation {
    let mut inner = pair.into_inner();
    let cond = parse_expression(inner.next().unwrap());
    
    let mut main_body: Vec<Equation> = Vec::new();
    let mut else_whens: Vec<(Expression, Vec<Equation>)> = Vec::new();
    
    for token in inner {
        match token.as_rule() {
            Rule::expression => {
                // New elsewhen condition
                let else_cond = parse_expression(token);
                else_whens.push((else_cond, Vec::new()));
            }
            Rule::equation_stmt => {
                let stmt = parse_equation_stmt_inner(token.into_inner().next().unwrap());
                if else_whens.is_empty() {
                    main_body.push(stmt);
                } else {
                    else_whens.last_mut().unwrap().1.push(stmt);
                }
            }
            _ => {}
        }
    }
    
    Equation::When(cond, main_body, else_whens)
}

fn parse_if_equation(pair: pest::iterators::Pair<Rule>) -> Equation {
    let mut inner = pair.into_inner();
    let cond = parse_expression(inner.next().unwrap());
    let mut then_eqs = Vec::new();
    let mut elseif_list: Vec<(Expression, Vec<Equation>)> = Vec::new();
    let mut else_eqs: Option<Vec<Equation>> = None;
    for token in inner {
        match token.as_rule() {
            Rule::equation_stmt => {
                let eq = parse_equation_stmt_inner(token.into_inner().next().unwrap());
                if let Some(ref mut v) = else_eqs {
                    v.push(eq);
                } else if let Some(last) = elseif_list.last_mut() {
                    last.1.push(eq);
                } else {
                    then_eqs.push(eq);
                }
            }
            Rule::elseif_branch => {
                let mut ib = token.into_inner();
                let c = parse_expression(ib.next().unwrap());
                let mut eqs = Vec::new();
                for stmt in ib {
                    if stmt.as_rule() == Rule::equation_stmt {
                        eqs.push(parse_equation_stmt_inner(stmt.into_inner().next().unwrap()));
                    }
                }
                elseif_list.push((c, eqs));
            }
            Rule::else_branch => {
                let mut eqs = Vec::new();
                for stmt in token.into_inner() {
                    if stmt.as_rule() == Rule::equation_stmt {
                        eqs.push(parse_equation_stmt_inner(stmt.into_inner().next().unwrap()));
                    }
                }
                else_eqs = Some(eqs);
            }
            _ => {}
        }
    }
    Equation::If(cond, then_eqs, elseif_list, else_eqs)
}

fn parse_equation_stmt_inner(inner_stmt: pest::iterators::Pair<Rule>) -> Equation {
    match inner_stmt.as_rule() {
        Rule::equation => {
            let mut eq_parts = inner_stmt.into_inner();
            let lhs = parse_expression(eq_parts.next().unwrap());
            let rhs = parse_expression(eq_parts.next().unwrap());
            Equation::Simple(lhs, rhs)
        }
        Rule::connect_clause => {
            let mut conn_inner = inner_stmt.into_inner();
            let a_expr = parse_expression(conn_inner.next().unwrap());
            let b_expr = parse_expression(conn_inner.next().unwrap());
            Equation::Connect(a_expr, b_expr)
        }
        Rule::for_loop => parse_for_loop(inner_stmt),
        Rule::when_equation => parse_when_equation(inner_stmt),
        Rule::if_equation => parse_if_equation(inner_stmt),
        Rule::reinit_clause => {
            let mut inner = inner_stmt.into_inner();
            let var_expr = parse_component_ref(inner.next().unwrap());
            let val_expr = parse_expression(inner.next().unwrap());
            let var_name = expr_to_string(var_expr);
            Equation::Reinit(var_name, val_expr)
        }
        Rule::assert_stmt => {
            let mut inner = inner_stmt.into_inner();
            let cond = parse_expression(inner.next().unwrap());
            let msg = parse_expression(inner.next().unwrap());
            Equation::Assert(cond, msg)
        }
        Rule::terminate_stmt => {
            let mut inner = inner_stmt.into_inner();
            let msg = parse_expression(inner.next().unwrap());
            Equation::Terminate(msg)
        }
        Rule::multi_assign_equation => parse_multi_assign_equation(inner_stmt),
        _ => unreachable!("Unknown equation stmt rule: {:?}", inner_stmt.as_rule()),
    }
}

fn parse_multi_assign_equation(pair: pest::iterators::Pair<Rule>) -> Equation {
    let mut lhss = Vec::new();
    let mut rhs = None;
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::component_ref => lhss.push(parse_component_ref(p)),
            Rule::expression => rhs = Some(parse_expression(p)),
            _ => {}
        }
    }
    Equation::MultiAssign(lhss, rhs.expect("multi_assign_equation must have rhs"))
}

fn parse_expression(pair: pest::iterators::Pair<Rule>) -> Expression {
    let rule = pair.as_rule();
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::if_expression => parse_if_expression(inner),
        Rule::logical_or => parse_logical_or(inner),
        Rule::arithmetic_expression => parse_arithmetic(inner),
        _ => {
             unreachable!("Unexpected expression rule: {:?} in pair {:?}", inner.as_rule(), rule)
        }
    }
}

fn parse_if_expression(pair: pest::iterators::Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let cond = parse_expression(inner.next().unwrap());
    // String literals "if", "then", "else" are not yielded as pairs by Pest.
    let true_expr = parse_expression(inner.next().unwrap());
    let false_expr = parse_expression(inner.next().unwrap());
    Expression::If(Box::new(cond), Box::new(true_expr), Box::new(false_expr))
}

fn parse_logical_or(pair: pest::iterators::Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let mut lhs = parse_logical_and(inner.next().unwrap());
    
    while let Some(_) = inner.peek() {
        // next is or_op rule
        inner.next(); 
        let rhs = parse_logical_and(inner.next().unwrap());
        lhs = Expression::BinaryOp(Box::new(lhs), Operator::Or, Box::new(rhs));
    }
    lhs
}

fn parse_logical_and(pair: pest::iterators::Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let mut lhs = parse_relation(inner.next().unwrap());
    
    while let Some(_) = inner.peek() {
        // next is and_op rule
        inner.next(); 
        let rhs = parse_relation(inner.next().unwrap());
        lhs = Expression::BinaryOp(Box::new(lhs), Operator::And, Box::new(rhs));
    }
    lhs
}

fn parse_relation(pair: pest::iterators::Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let lhs = parse_arithmetic(inner.next().unwrap());
    
    if let Some(op_pair) = inner.next() {
        let op = match op_pair.as_str() {
            "<" => Operator::Less,
            "<=" => Operator::LessEq,
            ">" => Operator::Greater,
            ">=" => Operator::GreaterEq,
            "==" => Operator::Equal,
            "<>" => Operator::NotEqual,
            _ => unreachable!("Unknown relation operator: {}", op_pair.as_str()),
        };
        let rhs = parse_arithmetic(inner.next().unwrap());
        return Expression::BinaryOp(Box::new(lhs), op, Box::new(rhs));
    }
    lhs
}

fn parse_arithmetic(pair: pest::iterators::Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let mut lhs = parse_term(inner.next().unwrap());
    
    while let Some(op) = inner.next() {
        let rhs = parse_term(inner.next().unwrap());
        let operator = match op.as_str() {
            "+" => Operator::Add,
            "-" => Operator::Sub,
            _ => unreachable!(),
        };
        lhs = Expression::BinaryOp(Box::new(lhs), operator, Box::new(rhs));
    }
    lhs
}

fn parse_term(pair: pest::iterators::Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    
    // Check for unary minus
    let mut negate = false;
    let first = inner.peek().unwrap();
    if first.as_rule() == Rule::unary_minus {
        negate = true;
        inner.next(); // consume
    }
    
    let mut lhs = parse_factor(inner.next().unwrap());
    
    while let Some(op) = inner.next() {
        let rhs = parse_factor(inner.next().unwrap());
        let operator = match op.as_str() {
            "*" => Operator::Mul,
            "/" => Operator::Div,
            _ => unreachable!(),
        };
        lhs = Expression::BinaryOp(Box::new(lhs), operator, Box::new(rhs));
    }
    
    if negate {
        // -x is 0 - x
        lhs = Expression::BinaryOp(Box::new(Expression::Number(0.0)), Operator::Sub, Box::new(lhs));
    }
    
    lhs
}

fn parse_factor(pair: pest::iterators::Pair<Rule>) -> Expression {
    // pair is Rule::factor
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::number => {
             // inner is Rule::number
             // "1.0" or "1"
             Expression::Number(inner.as_str().parse().unwrap())
        }
        Rule::der_call => {
            let mut der_inner = inner.into_inner();
            let arg = parse_expression(der_inner.next().unwrap());
            Expression::Der(Box::new(arg))
        }
        Rule::function_call => {
             let mut func_inner = inner.into_inner();
             let func_name_pair = func_inner.next().unwrap();
             // func_name_pair is dotted_identifier
             let func_name = if func_name_pair.as_rule() == Rule::dotted_identifier {
                 // Reconstruct without whitespace
                 func_name_pair.into_inner()
                     .map(|p| p.as_str())
                     .collect::<Vec<_>>()
                     .join(".")
             } else {
                 func_name_pair.as_str().to_string()
             };
             
             let mut args = Vec::new();
             for arg_pair in func_inner {
                 match arg_pair.as_rule() {
                     // arg_list = expression ("," ~ expression)*
                     Rule::arg_list => {
                         for expr_pair in arg_pair.into_inner() {
                             args.push(parse_expression(expr_pair));
                         }
                     }
                     // Fallback: single expression directly (older grammar)
                     Rule::expression => {
                         args.push(parse_expression(arg_pair));
                     }
                     _ => {}
                 }
             }
             Expression::Call(func_name, args)
        }
        Rule::initial_call => {
            Expression::Call("initial".to_string(), Vec::new())
        }
        Rule::terminal_call => {
            Expression::Call("terminal".to_string(), Vec::new())
        }
        Rule::assert_call => {
            let mut a = inner.into_inner();
            let cond = parse_expression(a.next().unwrap());
            let msg = parse_expression(a.next().unwrap());
            Expression::Call("assert".to_string(), vec![cond, msg])
        }
        Rule::terminate_call => {
            let mut a = inner.into_inner();
            let msg = parse_expression(a.next().unwrap());
            Expression::Call("terminate".to_string(), vec![msg])
        }
        Rule::component_ref => {
            parse_component_ref(inner)
        }
        Rule::expression => parse_expression(inner), // Parentheses: "(" ~ expression ~ ")"
        Rule::array_literal => {
            let inner_exprs = inner.into_inner();
            let exprs: Vec<Expression> = inner_exprs.map(parse_expression).collect();
            Expression::ArrayLiteral(exprs)
        }
        _ => unreachable!("Unexpected factor rule: {:?}", inner.as_rule()),
    }
}

#[allow(dead_code)]
fn parse_const_expression(pair: pest::iterators::Pair<Rule>) -> Option<f64> {
    let expr = parse_expression(pair);
    eval_const_expr(&expr)
}

#[allow(dead_code)]
fn eval_const_expr(expr: &Expression) -> Option<f64> {
    match expr {
        Expression::Number(n) => Some(*n),
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = eval_const_expr(lhs)?;
            let r = eval_const_expr(rhs)?;
            match op {
                Operator::Add => Some(l + r),
                Operator::Sub => Some(l - r),
                Operator::Mul => Some(l * r),
                Operator::Div => Some(l / r),
                _ => None, // Relational/logical operators not supported in constant evaluation for now
            }
        }
        Expression::If(cond, t_expr, f_expr) => {
            let c = eval_const_expr(cond)?;
            if c != 0.0 {
                eval_const_expr(t_expr)
            } else {
                eval_const_expr(f_expr)
            }
        }
        _ => None,
    }
}
