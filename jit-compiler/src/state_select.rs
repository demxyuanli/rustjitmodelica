//! Resolve Modelica `StateSelect` attributes from expressions / modifications.

use crate::ast::{Expression, Modification, StateSelect};
use crate::string_intern::resolve_id;

fn base_is_state_select(expr: &Expression) -> bool {
    match expr {
        Expression::Variable(id) => {
            let n = resolve_id(*id);
            n == "StateSelect"
                || n == "Modelica.StateSelect"
                || n.ends_with(".StateSelect")
        }
        Expression::Dot(_, field) => field == "StateSelect",
        _ => false,
    }
}

fn field_to_state_select(field: &str) -> Option<StateSelect> {
    match field {
        "never" => Some(StateSelect::Never),
        "avoid" => Some(StateSelect::Avoid),
        "default" => Some(StateSelect::Default),
        "prefer" => Some(StateSelect::Prefer),
        "always" => Some(StateSelect::Always),
        _ => None,
    }
}

/// Parse a StateSelect literal / if-expression into an enumeration value.
pub fn parse_state_select_expr(expr: &Expression) -> Option<StateSelect> {
    match expr {
        Expression::Dot(base, field) if base_is_state_select(base) => field_to_state_select(field),
        Expression::Variable(id) => field_to_state_select(&resolve_id(*id)),
        Expression::If(cond, then_e, else_e) => match cond.as_ref() {
            Expression::Number(n) => {
                if *n != 0.0 {
                    parse_state_select_expr(then_e)
                } else {
                    parse_state_select_expr(else_e)
                }
            }
            // Unevaluated guard (e.g. Body.enforceStates): MSL default is false -> else.
            _ => parse_state_select_expr(else_e).or_else(|| parse_state_select_expr(then_e)),
        },
        _ => None,
    }
}

pub fn state_select_mod_value(mods: &[Modification]) -> Option<&Expression> {
    mods.iter()
        .find(|m| m.name == "stateSelect")
        .and_then(|m| m.value.as_ref())
}

pub fn state_select_from_mods(mods: &[Modification]) -> Option<StateSelect> {
    state_select_mod_value(mods).and_then(parse_state_select_expr)
}

pub fn each_state_select_mod(mods: &[Modification]) -> Option<&Expression> {
    mods.iter()
        .find(|m| m.each && m.name == "stateSelect")
        .and_then(|m| m.value.as_ref())
}

/// Apply a component-level `parameter StateSelect stateSelect=...` to DOF-like children.
pub fn is_state_select_param_decl(type_name: &str, name: &str) -> bool {
    (type_name == "StateSelect"
        || type_name == "Modelica.StateSelect"
        || type_name.ends_with(".StateSelect"))
        && name.ends_with("_stateSelect")
}

pub fn state_select_param_prefix(name: &str) -> Option<&str> {
    name.strip_suffix("_stateSelect")
}

/// Build per-variable StateSelect map from flattened declarations.
pub fn build_state_select_map(
    declarations: &[crate::ast::Declaration],
) -> std::collections::HashMap<String, StateSelect> {
    use std::collections::HashMap;

    let mut ranks: HashMap<String, StateSelect> = HashMap::new();

    for d in declarations {
        if let Some(ss) = d.state_select {
            ranks.insert(d.name.clone(), ss);
        }
    }

    let param_prefs: Vec<(String, StateSelect)> = declarations
        .iter()
        .filter(|d| d.is_parameter && is_state_select_param_decl(&d.type_name, &d.name))
        .filter_map(|d| {
            let prefix = state_select_param_prefix(&d.name)?;
            let ss = d
                .state_select
                .or_else(|| d.start_value.as_ref().and_then(parse_state_select_expr))?;
            Some((format!("{prefix}_"), ss))
        })
        .collect();

    for d in declarations {
        if d.is_parameter {
            continue;
        }
        let cur = ranks.get(&d.name).copied();
        if matches!(
            cur,
            Some(
                StateSelect::Prefer
                    | StateSelect::Always
                    | StateSelect::Never
                    | StateSelect::Avoid
            )
        ) {
            continue;
        }
        for (prefix, ss) in &param_prefs {
            if !d.name.starts_with(prefix) {
                continue;
            }
            if d.name.ends_with("_phi")
                || d.name.ends_with("_w")
                || d.name.ends_with("_s")
                || d.name.ends_with("_v")
                || d.name.contains("_Q")
                || d.name.contains("_r_0")
                || d.name.contains("_v_0")
                || d.name.contains("_w_a")
                || d.name.contains("_phi_d")
            {
                ranks.insert(d.name.clone(), *ss);
                break;
            }
        }
    }

    ranks
}

/// Rank used by dummy-derivative / alias preference (higher = keep as state).
pub fn state_select_rank_of(
    name: &str,
    ranks: &std::collections::HashMap<String, StateSelect>,
) -> StateSelect {
    ranks.get(name).copied().unwrap_or(StateSelect::Default)
}

