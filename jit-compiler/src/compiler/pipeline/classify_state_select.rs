/// Demote independent states using Modelica `StateSelect` ranks.

use crate::ast::StateSelect;
use crate::state_select::{
    build_state_select_map, is_state_select_param_decl, parse_state_select_expr,
    state_select_param_prefix,
};

/// When Prefer/Always states exist, demote Never/Avoid.
fn demote_by_state_select(
    declarations: &[Declaration],
    states: &mut HashSet<String>,
) -> Vec<String> {
    let enabled = std::env::var("RUSTMODLICA_STATE_SELECT")
        .ok()
        .map(|v| {
            let t = v.trim();
            !(t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("off"))
        })
        .unwrap_or(true);
    if !enabled {
        return Vec::new();
    }

    let demote_trace = std::env::var("RUSTMODLICA_STATE_DEMOTE_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false);

    let mut ranks = build_state_select_map(declarations);

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
    for s in states.iter() {
        if ranks.contains_key(s) {
            continue;
        }
        for (prefix, ss) in &param_prefs {
            if s.starts_with(prefix) {
                ranks.insert(s.clone(), *ss);
                break;
            }
        }
    }

    let has_preferred = states.iter().any(|s| {
        ranks
            .get(s)
            .copied()
            .unwrap_or(StateSelect::Default)
            .is_preferred()
    });

    let mut demote: Vec<String> = Vec::new();
    for s in states.iter() {
        let rank = ranks.get(s).copied().unwrap_or(StateSelect::Default);
        if rank == StateSelect::Never
            || (has_preferred && rank.is_demotable_when_preferred_exists())
        {
            demote.push(s.clone());
            continue;
        }
        // Body `r_0 = frame_a.r_0`: frame connector often becomes the der()-state while
        // the Avoid-tagged body r_0 is algebraic. Demote frame when the paired body
        // absolute carries Avoid/Never.
        if has_preferred {
            if let Some(body_r0) = body_r0_of_frame_a_r0(s) {
                let br = ranks
                    .get(&body_r0)
                    .copied()
                    .unwrap_or(StateSelect::Default);
                if br == StateSelect::Never || br.is_demotable_when_preferred_exists() {
                    demote.push(s.clone());
                }
            }
        }
    }

    demote.sort();
    demote.dedup();
    for name in &demote {
        if demote_trace {
            let rank = ranks.get(name).copied().unwrap_or(StateSelect::Default);
            eprintln!("[state-demote] stateSelect {:?} -> {name}", rank);
        }
        states.remove(name);
    }
    demote
}

fn body_r0_of_frame_a_r0(name: &str) -> Option<String> {
    let (prefix, idx) = name.split_once("_frame_a_r_0_")?;
    if !idx.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{prefix}_r_0_{idx}"))
}
