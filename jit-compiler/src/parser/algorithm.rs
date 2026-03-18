use crate::ast::{AlgorithmStatement, Expression};
use pest::iterators::Pair;

use super::expression;
use super::Rule;

pub(super) fn parse_algorithm_stmt(pair: Pair<Rule>) -> AlgorithmStatement {
    match pair.as_rule() {
        Rule::annotation_clause => AlgorithmStatement::NoOp,
        Rule::break_stmt => AlgorithmStatement::NoOp,
        Rule::return_stmt => AlgorithmStatement::NoOp,
        Rule::assignment_stmt => {
            let mut inner = pair.into_inner();
            let lhs_pair = inner.next().unwrap();
            let lhs_expr = expression::parse_component_ref(lhs_pair);
            let rhs_expr = expression::parse_expression(inner.next().unwrap());
            AlgorithmStatement::Assignment(lhs_expr, rhs_expr)
        }
        Rule::multi_assign_stmt => {
            let mut inner = pair.into_inner();
            let mut lhss: Vec<Expression> = Vec::new();
            while let Some(p) = inner.peek() {
                if p.as_rule() == Rule::component_ref {
                    lhss.push(expression::parse_component_ref(inner.next().unwrap()));
                } else {
                    break;
                }
            }
            let rhs = expression::parse_expression(inner.next().unwrap());
            AlgorithmStatement::MultiAssign(lhss, rhs)
        }
        Rule::call_stmt => {
            let inner = pair.into_inner().next().unwrap();
            // call_stmt = function_call ';'
            let expr = expression::parse_function_call_expr(inner);
            AlgorithmStatement::CallStmt(expr)
        }
        Rule::if_stmt => {
            let mut inner = pair.into_inner();
            let mut conditions = Vec::new();
            let mut bodies = Vec::new();
            let mut else_body = None;

            let cond = expression::parse_expression(inner.next().unwrap());
            let mut body = Vec::new();
            let mut current_cond = Some(cond);

            for token in inner {
                match token.as_rule() {
                    Rule::expression => {
                        if let Some(c) = current_cond.take() {
                            conditions.push(c);
                            bodies.push(body);
                            body = Vec::new();
                        }
                        current_cond = Some(expression::parse_expression(token));
                    }
                    Rule::algorithm_stmt => {
                        body.push(parse_algorithm_stmt(token.into_inner().next().unwrap()));
                    }
                    _ => {}
                }
            }

            if let Some(c) = current_cond {
                conditions.push(c);
                bodies.push(body);
            } else {
                else_body = Some(body);
            }

            if conditions.is_empty() {
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
                    let mut r_inner = range_or_expr.into_inner();
                    let start = expression::parse_expression(r_inner.next().unwrap());
                    let second = expression::parse_expression(r_inner.next().unwrap());

                    if let Some(third_pair) = r_inner.next() {
                        let third = expression::parse_expression(third_pair);
                        Expression::Range(Box::new(start), Box::new(second), Box::new(third))
                    } else {
                        Expression::Range(
                            Box::new(start),
                            Box::new(Expression::Number(1.0)),
                            Box::new(second),
                        )
                    }
                }
                Rule::expression => expression::parse_expression(range_or_expr),
                _ => unreachable!(
                    "Unexpected rule in for_stmt range: {:?}",
                    range_or_expr.as_rule()
                ),
            };

            let mut body = Vec::new();
            for stmt in inner {
                body.push(parse_algorithm_stmt(stmt.into_inner().next().unwrap()));
            }
            AlgorithmStatement::For(loop_var, Box::new(range_expr), body)
        }
        Rule::while_stmt => {
            let mut inner = pair.into_inner();
            let cond = expression::parse_expression(inner.next().unwrap());
            let mut body = Vec::new();
            for stmt in inner {
                body.push(parse_algorithm_stmt(stmt.into_inner().next().unwrap()));
            }
            AlgorithmStatement::While(cond, body)
        }
        Rule::when_stmt => {
            let mut inner = pair.into_inner();
            let cond = expression::parse_expression(inner.next().unwrap());
            let mut body = Vec::new();
            let else_whens = Vec::new();

            for stmt in inner {
                if stmt.as_rule() == Rule::algorithm_stmt {
                    body.push(parse_algorithm_stmt(stmt.into_inner().next().unwrap()));
                }
            }
            AlgorithmStatement::When(cond, body, else_whens)
        }
        Rule::reinit_clause => {
            let mut inner = pair.into_inner();
            let var_expr = expression::parse_component_ref(inner.next().unwrap());
            let val_expr = expression::parse_expression(inner.next().unwrap());
            let var_name = super::helpers::expr_to_string(var_expr);
            AlgorithmStatement::Reinit(var_name, val_expr)
        }
        Rule::assert_stmt => {
            let mut args: Vec<Expression> = Vec::new();
            if let Some(arg_list) = pair.into_inner().next() {
                for item in arg_list.into_inner() {
                    let item = if item.as_rule() == Rule::arg_item {
                        item.into_inner().next().unwrap()
                    } else {
                        item
                    };
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
            AlgorithmStatement::Assert(cond, msg)
        }
        Rule::terminate_stmt => {
            let mut inner = pair.into_inner();
            let msg = expression::parse_expression(inner.next().unwrap());
            AlgorithmStatement::Terminate(msg)
        }
        _ => unreachable!("Unknown algorithm stmt rule: {:?}", pair.as_rule()),
    }
}
