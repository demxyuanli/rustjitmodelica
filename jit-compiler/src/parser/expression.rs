use crate::ast::{Expression, Operator};
use pest::iterators::Pair;

use super::Rule;

pub(super) fn parse_expression(pair: Pair<Rule>) -> Expression {
    match pair.as_rule() {
        Rule::expression => parse_expression(pair.into_inner().next().unwrap()),
        Rule::if_expression => parse_if_expression(pair),
        Rule::range_expression => parse_range_expression(pair),
        Rule::iterator_expression => parse_iterator_expression(pair),
        Rule::logical_or => parse_logical_or(pair),
        Rule::logical_and => parse_logical_and(pair),
        Rule::relation => parse_relation(pair),
        Rule::arithmetic_expression => parse_arithmetic(pair),
        _ => unreachable!("Unexpected expression rule: {:?}", pair.as_rule()),
    }
}

fn parse_range_expression(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let start = parse_expression(inner.next().unwrap());
    let second = parse_expression(inner.next().unwrap());
    if let Some(third) = inner.next() {
        let end = parse_expression(third);
        Expression::Range(Box::new(start), Box::new(second), Box::new(end))
    } else {
        Expression::Range(
            Box::new(start),
            Box::new(Expression::Number(1.0)),
            Box::new(second),
        )
    }
}

fn parse_if_expression(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let cond = parse_expression(inner.next().unwrap());
    let true_expr = parse_expression(inner.next().unwrap());
    let false_expr = parse_expression(inner.next().unwrap());
    Expression::If(Box::new(cond), Box::new(true_expr), Box::new(false_expr))
}

fn parse_iterator_expression(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let expr = parse_expression(inner.next().unwrap());
    let iter_var = inner.next().unwrap().as_str().to_string();
    let range_or_expr = inner.next().unwrap();
    let iter_range = match range_or_expr.as_rule() {
        Rule::range => {
            let mut r_inner = range_or_expr.into_inner();
            let start = parse_expression(r_inner.next().unwrap());
            let second = parse_expression(r_inner.next().unwrap());
            if let Some(third_pair) = r_inner.next() {
                let end = parse_expression(third_pair);
                Expression::Range(Box::new(start), Box::new(second), Box::new(end))
            } else {
                Expression::Range(
                    Box::new(start),
                    Box::new(Expression::Number(1.0)),
                    Box::new(second),
                )
            }
        }
        Rule::expression => parse_expression(range_or_expr),
        _ => parse_expression(range_or_expr),
    };
    Expression::ArrayComprehension {
        expr: Box::new(expr),
        iter_var,
        iter_range: Box::new(iter_range),
    }
}

fn parse_logical_or(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let mut lhs = parse_logical_and(inner.next().unwrap());

    while inner.peek().is_some() {
        inner.next();
        let rhs = parse_logical_and(inner.next().unwrap());
        lhs = Expression::BinaryOp(Box::new(lhs), Operator::Or, Box::new(rhs));
    }
    lhs
}

fn parse_logical_and(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let mut lhs = parse_relation(inner.next().unwrap());

    while inner.peek().is_some() {
        inner.next();
        let rhs = parse_relation(inner.next().unwrap());
        lhs = Expression::BinaryOp(Box::new(lhs), Operator::And, Box::new(rhs));
    }
    lhs
}

fn parse_relation(pair: Pair<Rule>) -> Expression {
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

fn parse_arithmetic(pair: Pair<Rule>) -> Expression {
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

fn parse_term(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();

    let mut negate = false;
    let first = inner.peek().unwrap();
    if first.as_rule() == Rule::unary_minus {
        negate = true;
        inner.next();
    } else if first.as_rule() == Rule::unary_plus {
        inner.next();
    }

    let mut lhs = parse_power(inner.next().unwrap());

    while let Some(op) = inner.next() {
        let rhs = parse_power(inner.next().unwrap());
        let operator = match op.as_str() {
            "*" => Operator::Mul,
            ".*" => Operator::Mul,
            "/" => Operator::Div,
            "./" => Operator::Div,
            _ => unreachable!(),
        };
        lhs = Expression::BinaryOp(Box::new(lhs), operator, Box::new(rhs));
    }

    if negate {
        lhs = Expression::BinaryOp(
            Box::new(Expression::Number(0.0)),
            Operator::Sub,
            Box::new(lhs),
        );
    }

    lhs
}

fn parse_power(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let mut lhs = parse_factor(inner.next().unwrap());
    while let Some(op) = inner.next() {
        let _ = op; // '^'
        let rhs = parse_factor(inner.next().unwrap());
        lhs = Expression::Call("pow".to_string(), vec![lhs, rhs]);
    }
    lhs
}

pub(super) fn parse_function_call_expr(pair: Pair<Rule>) -> Expression {
    debug_assert_eq!(pair.as_rule(), Rule::function_call);
    let mut func_inner = pair.into_inner();
    let func_name_pair = func_inner.next().unwrap();
    let func_name = if func_name_pair.as_rule() == Rule::dotted_identifier {
                func_name_pair
                    .as_str()
                    .trim()
                    .trim_start_matches('.')
                    .to_string()
    } else {
        func_name_pair.as_str().to_string()
    };

    let mut args = Vec::new();
    for arg_pair in func_inner {
        match arg_pair.as_rule() {
            Rule::arg_list => {
                for item in arg_pair.into_inner() {
                    let item = if item.as_rule() == Rule::arg_item {
                        item.into_inner().next().unwrap()
                    } else {
                        item
                    };
                    match item.as_rule() {
                                Rule::redeclare_arg => {
                                    // Parse-only (MSL): record constructor like
                                    // Complex(redeclare SI.Current re "...", redeclare SI.Current im "...")
                                    // Ignore these items for now.
                                }
                        Rule::named_arg => {
                            let mut ni = item.into_inner();
                            let name = ni.next().unwrap().as_str().to_string();
                            let val = parse_expression(ni.next().unwrap());
                            args.push(Expression::Call(
                                "named".to_string(),
                                vec![Expression::StringLiteral(name), val],
                            ));
                        }
                        Rule::expression => args.push(parse_expression(item)),
                        _ => {}
                    }
                }
            }
            Rule::expression => {
                args.push(parse_expression(arg_pair));
            }
            _ => {}
        }
    }

    if func_name.eq_ignore_ascii_case("sample") && args.len() == 1 {
        Expression::Sample(Box::new(args.into_iter().next().unwrap()))
    } else if func_name.eq_ignore_ascii_case("interval") && args.len() == 1 {
        Expression::Interval(Box::new(args.into_iter().next().unwrap()))
    } else if func_name.eq_ignore_ascii_case("hold") && args.len() == 1 {
        Expression::Hold(Box::new(args.into_iter().next().unwrap()))
    } else if func_name.eq_ignore_ascii_case("previous") && args.len() == 1 {
        Expression::Previous(Box::new(args.into_iter().next().unwrap()))
    } else if func_name.eq_ignore_ascii_case("subsample") && args.len() == 2 {
        let mut a = args.into_iter();
        Expression::SubSample(Box::new(a.next().unwrap()), Box::new(a.next().unwrap()))
    } else if func_name.eq_ignore_ascii_case("supersample") && args.len() == 2 {
        let mut a = args.into_iter();
        Expression::SuperSample(Box::new(a.next().unwrap()), Box::new(a.next().unwrap()))
    } else if func_name.eq_ignore_ascii_case("shiftsample") && args.len() == 2 {
        let mut a = args.into_iter();
        Expression::ShiftSample(Box::new(a.next().unwrap()), Box::new(a.next().unwrap()))
    } else {
        Expression::Call(func_name, args)
    }
}

fn parse_factor(pair: Pair<Rule>) -> Expression {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::number => Expression::Number(inner.as_str().parse().unwrap()),
        Rule::not_factor => {
            let mut a = inner.into_inner();
            let expr = parse_factor(a.next().unwrap());
            Expression::Call("not".to_string(), vec![expr])
        }
        Rule::der_call => {
            let mut der_inner = inner.into_inner();
            let arg = parse_expression(der_inner.next().unwrap());
            Expression::Der(Box::new(arg))
        }
        Rule::function_call => parse_function_call_expr(inner),
        Rule::initial_call => Expression::Call("initial".to_string(), Vec::new()),
        Rule::terminal_call => Expression::Call("terminal".to_string(), Vec::new()),
        Rule::assert_call => {
            let mut positional = Vec::new();
            let arg_list_pair = inner.into_inner().next().unwrap();
            for item in arg_list_pair.into_inner() {
                let item = if item.as_rule() == Rule::arg_item {
                    item.into_inner().next().unwrap()
                } else {
                    item
                };
                if item.as_rule() == Rule::expression {
                    positional.push(parse_expression(item));
                    if positional.len() >= 2 {
                        break;
                    }
                }
            }
            let cond = positional.get(0).cloned().unwrap_or(Expression::Number(1.0));
            let msg = positional.get(1).cloned().unwrap_or(Expression::StringLiteral(String::new()));
            Expression::Call("assert".to_string(), vec![cond, msg])
        }
        Rule::terminate_call => {
            let mut a = inner.into_inner();
            let msg = parse_expression(a.next().unwrap());
            Expression::Call("terminate".to_string(), vec![msg])
        }
        Rule::component_ref => parse_component_ref(inner),
        Rule::expression => parse_expression(inner),
        Rule::array_literal => {
            let inner_exprs = inner.into_inner();
            let exprs: Vec<Expression> = inner_exprs.map(parse_expression).collect();
            Expression::ArrayLiteral(exprs)
        }
        Rule::array_comprehension => {
            let mut inner_pairs = inner.into_inner();
            let expr = parse_expression(inner_pairs.next().unwrap());
            let iter_var = inner_pairs.next().unwrap().as_str().to_string();
            let range_or_expr = inner_pairs.next().unwrap();
            let iter_range = match range_or_expr.as_rule() {
                Rule::range => {
                    let mut r_inner = range_or_expr.into_inner();
                    let start = parse_expression(r_inner.next().unwrap());
                    let second = parse_expression(r_inner.next().unwrap());
                    if let Some(third_pair) = r_inner.next() {
                        let end = parse_expression(third_pair);
                        Expression::Range(Box::new(start), Box::new(second), Box::new(end))
                    } else {
                        Expression::Range(
                            Box::new(start),
                            Box::new(Expression::Number(1.0)),
                            Box::new(second),
                        )
                    }
                }
                Rule::expression => parse_expression(range_or_expr),
                _ => parse_expression(range_or_expr),
            };
            Expression::ArrayComprehension {
                expr: Box::new(expr),
                iter_var,
                iter_range: Box::new(iter_range),
            }
        }
        Rule::bracket_array_literal => {
            let inner_exprs = inner.into_inner();
            let exprs: Vec<Expression> = inner_exprs.map(parse_expression).collect();
            Expression::ArrayLiteral(exprs)
        }
        Rule::string_literal => {
            let s = inner.as_str();
            let content = s
                .strip_prefix('"')
                .and_then(|t| t.strip_suffix('"'))
                .unwrap_or(s);
            let unescaped = content
                .replace("\\\"", "\"")
                .replace("\\\\", "\\")
                .replace("\\n", "\n")
                .replace("\\r", "\r")
                .replace("\\t", "\t");
            Expression::StringLiteral(unescaped)
        }
        Rule::string_concat => {
            let mut out = String::new();
            for part in inner.into_inner() {
                let s = part.as_str();
                let content = s
                    .strip_prefix('"')
                    .and_then(|t| t.strip_suffix('"'))
                    .unwrap_or(s);
                let unescaped = content
                    .replace("\\\"", "\"")
                    .replace("\\\\", "\\")
                    .replace("\\n", "\n")
                    .replace("\\r", "\r")
                    .replace("\\t", "\t");
                out.push_str(&unescaped);
            }
            Expression::StringLiteral(out)
        }
        Rule::boolean_literal => {
            let v = match inner.as_str().trim() {
                "true" => 1.0,
                "false" => 0.0,
                _ => 0.0,
            };
            Expression::Number(v)
        }
        Rule::end_ref => Expression::Variable("end".to_string()),
        _ => unreachable!("Unexpected factor rule: {:?}", inner.as_rule()),
    }
}

pub(super) fn parse_component_ref(pair: Pair<Rule>) -> Expression {
    let mut pairs = pair.into_inner();
    let mut expr = Expression::Variable(pairs.next().unwrap().as_str().to_string());

    for part in pairs {
        match part.as_rule() {
            Rule::array_subscript => {
                let dim_inner = part.into_inner().next().unwrap();
                // Multi-dimensional subscripts exist in MSL (e.g. x[:, 2]).
                // Minimal handling: use the first subscript item as index placeholder.
                let idx = if dim_inner.as_rule() == Rule::expression {
                    parse_expression(dim_inner)
                } else {
                    // array_dim_unspecified ([:]) in expression context; use placeholder
                    Expression::Number(0.0)
                };
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
