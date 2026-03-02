use crate::ast::{Equation, Expression, AlgorithmStatement};

pub fn convert_eq_to_alg_stmt(eq: Equation) -> AlgorithmStatement {
    match eq {
        Equation::Simple(lhs, rhs) => AlgorithmStatement::Assignment(lhs, rhs),
        Equation::Reinit(var, val) => AlgorithmStatement::Reinit(var, val),
        Equation::For(var, start, end, body) => {
            let alg_body = body.into_iter().map(convert_eq_to_alg_stmt).collect();
            let range = Expression::Range(start, Box::new(Expression::Number(1.0)), end);
            AlgorithmStatement::For(var, Box::new(range), alg_body)
        }
        Equation::When(cond, body, else_whens) => {
            let alg_body = body.into_iter().map(convert_eq_to_alg_stmt).collect();
            let alg_else = else_whens
                .into_iter()
                .map(|(c, b)| (c, b.into_iter().map(convert_eq_to_alg_stmt).collect()))
                .collect();
            AlgorithmStatement::When(cond, alg_body, alg_else)
        }
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => {
            let then_alg = then_eqs.into_iter().map(convert_eq_to_alg_stmt).collect();
            let elseif_alg = elseif_list
                .into_iter()
                .map(|(c, eb)| (c, eb.into_iter().map(convert_eq_to_alg_stmt).collect()))
                .collect();
            let else_alg = else_eqs.map(|eqs| eqs.into_iter().map(convert_eq_to_alg_stmt).collect());
            AlgorithmStatement::If(cond, then_alg, elseif_alg, else_alg)
        }
        Equation::Assert(cond, msg) => AlgorithmStatement::Assert(cond, msg),
        Equation::Terminate(msg) => AlgorithmStatement::Terminate(msg),
        Equation::Connect(_, _) => panic!(
            "connect() inside when/algorithm is not supported; use equation section"
        ),
        Equation::SolvableBlock { .. } => panic!(
            "SolvableBlock (algebraic loop) inside when/algorithm is not supported; put equations in the equation section instead"
        ),
        Equation::MultiAssign(_, _) => panic!(
            "(a,b,...)=f(x) in when/algorithm is not supported; use equation section"
        ),
    }
}

pub fn parse_array_index(name: &str) -> Option<(String, usize)> {
    if let Some((base, idx_str)) = name.rsplit_once('_') {
        if let Ok(idx) = idx_str.parse::<usize>() {
            return Some((base.to_string(), idx));
        }
    }
    None
}
