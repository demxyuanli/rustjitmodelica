//! Order of when-algorithms vs algebraics that read when-assigned discrete vars.

use crate::ast::{AlgorithmStatement, Expression};

fn expr_is_t_start(expr: &Expression) -> bool {
    match expr {
        Expression::Variable(id) => {
            let n = crate::string_intern::resolve_id(*id);
            n == "t_start" || n.ends_with("_t_start")
        }
        Expression::Dot(_, mem) => mem == "t_start",
        _ => false,
    }
}

fn stmt_assigns_t_start(stmt: &AlgorithmStatement) -> bool {
    match stmt {
        AlgorithmStatement::Assignment(lhs, _) => expr_is_t_start(lhs),
        AlgorithmStatement::When(_, body, else_whens) => {
            body.iter().any(stmt_assigns_t_start)
                || else_whens.iter().any(|(_, b)| b.iter().any(stmt_assigns_t_start))
        }
        AlgorithmStatement::If(_, then_s, elseif, else_s) => {
            then_s.iter().any(stmt_assigns_t_start)
                || elseif.iter().any(|(_, b)| b.iter().any(stmt_assigns_t_start))
                || else_s.iter().flatten().any(stmt_assigns_t_start)
        }
        _ => false,
    }
}

/// `when enableFire then t_start = time` must run before `fire` reads `t_start`.
pub(super) fn when_assigns_t_start(stmt: &AlgorithmStatement) -> bool {
    matches!(stmt, AlgorithmStatement::When(..)) && stmt_assigns_t_start(stmt)
}
