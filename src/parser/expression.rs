use pest::iterators::Pair;
use crate::ast::{Expression, Operator};

use super::Rule;

pub(super) fn parse_expression(pair: Pair<Rule>) -> Expression {
    let rule = pair.as_rule();
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::if_expression => parse_if_expression(inner),
        Rule::logical_or => parse_logical_or(inner),
        Rule::arithmetic_expression => parse_arithmetic(inner),
        _ => unreachable!(
            "Unexpected expression rule: {:?} in pair {:?}",
            inner.as_rule(),
            rule
        ),
    }
}

fn parse_if_expression(pair: Pair<Rule>) -> Expression {
    let mut inner = pair.into_inner();
    let cond = parse_expression(inner.next().unwrap());
    let true_expr = parse_expression(inner.next().unwrap());
    let false_expr = parse_expression(inner.next().unwrap());
    Expression::If(Box::new(cond), Box::new(true_expr), Box::new(false_expr))
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
        lhs = Expression::BinaryOp(
            Box::new(Expression::Number(0.0)),
            Operator::Sub,
            Box::new(lhs),
        );
    }

    lhs
}

fn parse_factor(pair: Pair<Rule>) -> Expression {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::number => Expression::Number(inner.as_str().parse().unwrap()),
        Rule::der_call => {
            let mut der_inner = inner.into_inner();
            let arg = parse_expression(der_inner.next().unwrap());
            Expression::Der(Box::new(arg))
        }
        Rule::function_call => {
            let mut func_inner = inner.into_inner();
            let func_name_pair = func_inner.next().unwrap();
            let func_name = if func_name_pair.as_rule() == Rule::dotted_identifier {
                func_name_pair
                    .into_inner()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            } else {
                func_name_pair.as_str().to_string()
            };

            let mut args = Vec::new();
            for arg_pair in func_inner {
                match arg_pair.as_rule() {
                    Rule::arg_list => {
                        for expr_pair in arg_pair.into_inner() {
                            args.push(parse_expression(expr_pair));
                        }
                    }
                    Rule::expression => {
                        args.push(parse_expression(arg_pair));
                    }
                    _ => {}
                }
            }
            Expression::Call(func_name, args)
        }
        Rule::initial_call => Expression::Call("initial".to_string(), Vec::new()),
        Rule::terminal_call => Expression::Call("terminal".to_string(), Vec::new()),
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
        Rule::component_ref => parse_component_ref(inner),
        Rule::expression => parse_expression(inner),
        Rule::array_literal => {
            let inner_exprs = inner.into_inner();
            let exprs: Vec<Expression> = inner_exprs.map(parse_expression).collect();
            Expression::ArrayLiteral(exprs)
        }
        _ => unreachable!("Unexpected factor rule: {:?}", inner.as_rule()),
    }
}

pub(super) fn parse_component_ref(pair: Pair<Rule>) -> Expression {
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
