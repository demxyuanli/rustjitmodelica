use crate::ast::{Equation, Expression};
use crate::string_intern::resolve_id;
use std::collections::HashSet;

use crate::analysis::variable_collection::collect_vars_expr;

pub(super) fn reorder_simple_variable_equations(
    equations: Vec<Equation>,
    known_vars: &HashSet<String>,
    params: &[String],
) -> Vec<Equation> {
    if !equations
        .iter()
        .all(|eq| matches!(eq, Equation::Simple(Expression::Variable(_), _)))
    {
        return equations;
    }
    let mut ready_known = known_vars.clone();
    for p in params {
        ready_known.insert(p.clone());
    }
    ready_known.insert("time".to_string());

    let mut pending = equations;
    let mut reordered = Vec::new();
    loop {
        let mut progressed = false;
        let mut remaining = Vec::new();
        for eq in pending {
            match &eq {
                Equation::Simple(Expression::Variable(id), rhs) => {
                    let lhs_name = resolve_id(*id);
                    let mut rhs_vars = HashSet::new();
                    collect_vars_expr(rhs, &mut rhs_vars);
                    rhs_vars.remove(&lhs_name);
                    if rhs_vars.iter().all(|v| ready_known.contains(v)) {
                        ready_known.insert(lhs_name);
                        reordered.push(eq);
                        progressed = true;
                    } else {
                        remaining.push(eq);
                    }
                }
                _ => remaining.push(eq),
            }
        }
        if !progressed {
            reordered.extend(remaining);
            break;
        }
        pending = remaining;
        if pending.is_empty() {
            break;
        }
    }
    reordered
}
