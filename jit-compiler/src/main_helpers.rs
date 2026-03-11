fn collect_states_from_eq(eq: &Equation, states: &mut HashSet<String>) {
    match eq {
        Equation::Simple(lhs, rhs) => {
            collect_states_from_expr(lhs, states);
            collect_states_from_expr(rhs, states);
        }
        Equation::When(cond, body, else_whens) => {
            collect_states_from_expr(cond, states);
            for e in body { collect_states_from_eq(e, states); }
            for (c, b) in else_whens {
                collect_states_from_expr(c, states);
                for e in b { collect_states_from_eq(e, states); }
            }
        }
        Equation::For(_, start, end, body) => {
            collect_states_from_expr(start, states);
            collect_states_from_expr(end, states);
            for e in body { collect_states_from_eq(e, states); }
        }
        Equation::Reinit(_, expr) => collect_states_from_expr(expr, states),
        Equation::Connect(a, b) => {
            collect_states_from_expr(a, states);
            collect_states_from_expr(b, states);
        }
        Equation::SolvableBlock { .. } => {}
        Equation::Assert(cond, msg) => {
            collect_states_from_expr(cond, states);
            collect_states_from_expr(msg, states);
        }
        Equation::Terminate(msg) => collect_states_from_expr(msg, states),
        Equation::MultiAssign(lhss, rhs) => {
            for e in lhss {
                collect_states_from_expr(e, states);
            }
            collect_states_from_expr(rhs, states);
        }
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => {
            collect_states_from_expr(cond, states);
            for e in then_eqs { collect_states_from_eq(e, states); }
            for (c, b) in elseif_list {
                collect_states_from_expr(c, states);
                for e in b { collect_states_from_eq(e, states); }
            }
            if let Some(eqs) = else_eqs {
                for e in eqs { collect_states_from_eq(e, states); }
            }
        }
    }
}

fn collect_states_from_expr(expr: &Expression, states: &mut HashSet<String>) {
    match expr {
        Expression::Der(arg) => {
            if let Expression::Variable(name) = &**arg {
                states.insert(name.clone());
            }
        }
        Expression::BinaryOp(lhs, _, rhs) => {
            collect_states_from_expr(lhs, states);
            collect_states_from_expr(rhs, states);
        }
        Expression::Call(_, arg) => collect_states_from_expr(arg, states),
        Expression::ArrayAccess(arr, idx) => {
            collect_states_from_expr(arr, states);
            collect_states_from_expr(idx, states);
        }
        Expression::Dot(base, _) => collect_states_from_expr(base, states),
        Expression::If(cond, t, f) => {
            collect_states_from_expr(cond, states);
            collect_states_from_expr(t, states);
            collect_states_from_expr(f, states);
        }
        Expression::ArrayLiteral(es) => {
            for e in es { collect_states_from_expr(e, states); }
        }
        _ => {}
    }
}

fn normalize_der(expr: &Expression) -> Expression {
    match expr {
        Expression::Der(arg) => {
            if let Expression::Variable(name) = &**arg {
                return Expression::Variable(format!("der_{}", name));
            }
            Expression::Der(Box::new(normalize_der(arg)))
        }
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
            Box::new(normalize_der(lhs)),
            *op,
            Box::new(normalize_der(rhs))
        ),
        Expression::Call(name, arg) => Expression::Call(name.clone(), Box::new(normalize_der(arg))),
        Expression::ArrayAccess(arr, idx) => Expression::ArrayAccess(
            Box::new(normalize_der(arr)),
            Box::new(normalize_der(idx))
        ),
        Expression::Dot(base, m) => Expression::Dot(Box::new(normalize_der(base)), m.clone()),
        Expression::If(c, t, f) => Expression::If(
            Box::new(normalize_der(c)),
            Box::new(normalize_der(t)),
            Box::new(normalize_der(f))
        ),
        Expression::ArrayLiteral(es) => Expression::ArrayLiteral(es.iter().map(normalize_der).collect()),
        _ => expr.clone(),
    }
}
