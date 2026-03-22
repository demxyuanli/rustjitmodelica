use crate::ast::{Equation, Expression};
use crate::parser::{equation, expression, helpers, Rule};

pub fn parse_equation_section(pair: pest::iterators::Pair<Rule>, equations: &mut Vec<Equation>) {
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

pub fn parse_initial_equation_section(
    pair: pest::iterators::Pair<Rule>,
    initial_equations: &mut Vec<Equation>,
) {
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
                initial_equations.push(equation::parse_multi_assign_equation(inner_stmt));
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
