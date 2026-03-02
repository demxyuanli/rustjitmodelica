use pest::iterators::Pair;
use crate::ast::{Equation, Expression};

use super::expression;
use super::helpers;
use super::Rule;

pub(super) fn parse_for_loop(pair: Pair<Rule>) -> Equation {
    let mut inner = pair.into_inner();
    let loop_var = inner.next().unwrap().as_str().to_string();

    let range_pair = inner.next().unwrap();
    let mut range_inner = range_pair.into_inner();
    let start_expr = expression::parse_expression(range_inner.next().unwrap());
    let end_expr = expression::parse_expression(range_inner.next().unwrap());

    let mut body = Vec::new();

    for stmt in inner {
        let inner_stmt = stmt.into_inner().next().unwrap();
        match inner_stmt.as_rule() {
            Rule::equation => {
                let mut eq_parts = inner_stmt.into_inner();
                let lhs = expression::parse_expression(eq_parts.next().unwrap());
                let rhs = expression::parse_expression(eq_parts.next().unwrap());
                body.push(Equation::Simple(lhs, rhs));
            }
            Rule::connect_clause => {
                let mut conn_inner = inner_stmt.into_inner();
                let a_expr = expression::parse_expression(conn_inner.next().unwrap());
                let b_expr = expression::parse_expression(conn_inner.next().unwrap());
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

pub(super) fn parse_when_equation(pair: Pair<Rule>) -> Equation {
    let mut inner = pair.into_inner();
    let cond = expression::parse_expression(inner.next().unwrap());

    let mut main_body: Vec<Equation> = Vec::new();
    let mut else_whens: Vec<(Expression, Vec<Equation>)> = Vec::new();

    for token in inner {
        match token.as_rule() {
            Rule::expression => {
                let else_cond = expression::parse_expression(token);
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

pub(super) fn parse_if_equation(pair: Pair<Rule>) -> Equation {
    let mut inner = pair.into_inner();
    let cond = expression::parse_expression(inner.next().unwrap());
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
                let c = expression::parse_expression(ib.next().unwrap());
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

pub(super) fn parse_equation_stmt_inner(inner_stmt: Pair<Rule>) -> Equation {
    match inner_stmt.as_rule() {
        Rule::equation => {
            let mut eq_parts = inner_stmt.into_inner();
            let lhs = expression::parse_expression(eq_parts.next().unwrap());
            let rhs = expression::parse_expression(eq_parts.next().unwrap());
            Equation::Simple(lhs, rhs)
        }
        Rule::connect_clause => {
            let mut conn_inner = inner_stmt.into_inner();
            let a_expr = expression::parse_expression(conn_inner.next().unwrap());
            let b_expr = expression::parse_expression(conn_inner.next().unwrap());
            Equation::Connect(a_expr, b_expr)
        }
        Rule::for_loop => parse_for_loop(inner_stmt),
        Rule::when_equation => parse_when_equation(inner_stmt),
        Rule::if_equation => parse_if_equation(inner_stmt),
        Rule::reinit_clause => {
            let mut inner = inner_stmt.into_inner();
            let var_expr = expression::parse_component_ref(inner.next().unwrap());
            let val_expr = expression::parse_expression(inner.next().unwrap());
            let var_name = helpers::expr_to_string(var_expr);
            Equation::Reinit(var_name, val_expr)
        }
        Rule::assert_stmt => {
            let mut inner = inner_stmt.into_inner();
            let cond = expression::parse_expression(inner.next().unwrap());
            let msg = expression::parse_expression(inner.next().unwrap());
            Equation::Assert(cond, msg)
        }
        Rule::terminate_stmt => {
            let mut inner = inner_stmt.into_inner();
            let msg = expression::parse_expression(inner.next().unwrap());
            Equation::Terminate(msg)
        }
        Rule::multi_assign_equation => parse_multi_assign_equation(inner_stmt),
        _ => unreachable!("Unknown equation stmt rule: {:?}", inner_stmt.as_rule()),
    }
}

pub(super) fn parse_multi_assign_equation(pair: Pair<Rule>) -> Equation {
    let mut lhss = Vec::new();
    let mut rhs = None;
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::component_ref => lhss.push(expression::parse_component_ref(p)),
            Rule::expression => rhs = Some(expression::parse_expression(p)),
            _ => {}
        }
    }
    Equation::MultiAssign(lhss, rhs.expect("multi_assign_equation must have rhs"))
}
