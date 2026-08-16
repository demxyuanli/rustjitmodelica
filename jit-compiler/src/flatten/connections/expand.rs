use super::super::structures::FlattenedModel;
use crate::ast::{Equation, Expression};
use std::collections::HashSet;
pub(super) fn has_connector_members(path: &str, flat: &FlattenedModel) -> bool {
    let prefix = format!("{}_", path);
    flat.declarations.iter().any(|decl| decl.name.starts_with(&prefix))
}

fn connector_decl_has_suffix(path: &str, flat: &FlattenedModel, suffix: &str) -> bool {
    let name = format!("{}_{}", path, suffix);
    flat.declarations.iter().any(|d| d.name == name)
}

/// Potential/flow member pairs for common MSL connectors whose instances may be
/// recorded without expanding scalar members into `declarations`.
fn connector_potential_flow_members(type_name: &str) -> Option<(&'static str, &'static str)> {
    let tail = type_name.rsplit('.').next().unwrap_or(type_name);
    match tail {
        "Pin" | "PositivePin" | "NegativePin" => Some(("v", "i")),
        "Flange" | "Flange_a" | "Flange_b" | "Support" => {
            if type_name.contains("Translational") {
                Some(("s", "f"))
            } else if type_name.contains("Rotational") {
                Some(("phi", "tau"))
            } else {
                // Unqualified Flange_* is ambiguous (rot vs trans); resolve later.
                None
            }
        }
        _ => None,
    }
}

pub(super) fn resolve_potential_flow_members(
    type_a: Option<&str>,
    type_b: Option<&str>,
    a_path: &str,
    b_path: &str,
    flat: &FlattenedModel,
) -> Option<(&'static str, &'static str)> {
    if let Some(t) = type_a.and_then(connector_potential_flow_members) {
        return Some(t);
    }
    if let Some(t) = type_b.and_then(connector_potential_flow_members) {
        return Some(t);
    }
    // Only flange/support-like paths may infer domain from the owning component.
    // Paths such as `torque1_tau` or `step1_y` also have a Rotational/Blocks owner
    // but are Real signals, not flanges — do not synthesize phi/tau on them.
    let flange_like = |p: &str| p.contains("_flange") || p.ends_with("_support");
    if !(flange_like(a_path) || flange_like(b_path)) {
        let looks_flange = |t: &str| {
            let tail = t.rsplit('.').next().unwrap_or(t);
            matches!(tail, "Flange" | "Flange_a" | "Flange_b" | "Support")
        };
        if type_a.map(looks_flange).unwrap_or(false) || type_b.map(looks_flange).unwrap_or(false) {
            return Some(("phi", "tau"));
        }
        return None;
    }
    // Disambiguate bare Flange_* from existing member decls.
    if connector_decl_has_suffix(a_path, flat, "s")
        || connector_decl_has_suffix(b_path, flat, "s")
    {
        return Some(("s", "f"));
    }
    if connector_decl_has_suffix(a_path, flat, "phi")
        || connector_decl_has_suffix(b_path, flat, "phi")
    {
        return Some(("phi", "tau"));
    }
    // Infer domain from owning component instance type (mass2_flange_a -> mass2).
    let owner_domain = |path: &str| -> Option<&'static str> {
        let bytes = path.as_bytes();
        for (i, &b) in bytes.iter().enumerate().rev() {
            if b == b'_' {
                let owner = &path[..i];
                if let Some(ty) = flat.instances.get(owner) {
                    if ty.contains("Translational") {
                        return Some("trans");
                    }
                    if ty.contains("Rotational") {
                        return Some("rot");
                    }
                }
            }
        }
        None
    };
    match (owner_domain(a_path), owner_domain(b_path)) {
        (Some("trans"), _) | (_, Some("trans")) => return Some(("s", "f")),
        (Some("rot"), _) | (_, Some("rot")) => return Some(("phi", "tau")),
        _ => {}
    }
    let looks_flange = |t: &str| {
        let tail = t.rsplit('.').next().unwrap_or(t);
        matches!(tail, "Flange" | "Flange_a" | "Flange_b" | "Support")
    };
    if type_a.map(looks_flange).unwrap_or(false) || type_b.map(looks_flange).unwrap_or(false) {
        return Some(("phi", "tau"));
    }
    None
}

/// Expand one connect(a,b) into potential equalities and flow/stream pairs.
/// Prefers declared members on either side; if neither side has members but the
/// connector type is known, synthesizes potential/flow names instead of equating
/// bare connector roots (which breaks Damper `s_rel = flange_b.s - flange_a.s`).
pub(super) fn expand_connect_pair(
    flat: &FlattenedModel,
    a_path: &str,
    b_path: &str,
    type_a: Option<&str>,
    type_b: Option<&str>,
) -> (Vec<Equation>, Vec<(String, String)>, Vec<(String, String)>) {
    let mut potential_eqs = Vec::new();
    let mut flow_pairs = Vec::new();
    let mut stream_pairs = Vec::new();

    let emit_from = |src: &str, dst: &str, potential_eqs: &mut Vec<Equation>, flow_pairs: &mut Vec<(String, String)>, stream_pairs: &mut Vec<(String, String)>| {
        let prefix_src = format!("{}_", src);
        let prefix_dst = format!("{}_", dst);
        for decl in &flat.declarations {
            if let Some(suffix) = decl.name.strip_prefix(&prefix_src) {
                let target_name = format!("{}{}", prefix_dst, suffix);
                if decl.is_flow {
                    flow_pairs.push((decl.name.clone(), target_name));
                } else if decl.is_stream {
                    stream_pairs.push((decl.name.clone(), target_name.clone()));
                    stream_pairs.push((target_name, decl.name.clone()));
                } else {
                    // Prefer b.member = a.member so the second connect() argument
                    // (typically the undriven frame) is assigned from the first.
                    potential_eqs.push(Equation::Simple(
                        Expression::var(&target_name),
                        Expression::var(&decl.name),
                    ));
                }
            }
        }
    };

    if has_connector_members(a_path, flat) || flat.instances.contains_key(a_path) {
        let before = potential_eqs.len() + flow_pairs.len() + stream_pairs.len();
        emit_from(
            a_path,
            b_path,
            &mut potential_eqs,
            &mut flow_pairs,
            &mut stream_pairs,
        );
        if potential_eqs.len() + flow_pairs.len() + stream_pairs.len() > before {
            return (potential_eqs, flow_pairs, stream_pairs);
        }
    }
    if has_connector_members(b_path, flat) || flat.instances.contains_key(b_path) {
        let before = potential_eqs.len() + flow_pairs.len() + stream_pairs.len();
        emit_from(
            b_path,
            a_path,
            &mut potential_eqs,
            &mut flow_pairs,
            &mut stream_pairs,
        );
        if potential_eqs.len() + flow_pairs.len() + stream_pairs.len() > before {
            return (potential_eqs, flow_pairs, stream_pairs);
        }
    }

    if let Some((pot, flow)) =
        resolve_potential_flow_members(type_a, type_b, a_path, b_path, flat)
    {
        potential_eqs.push(Equation::Simple(
            Expression::var(&format!("{}_{}", a_path, pot)),
            Expression::var(&format!("{}_{}", b_path, pot)),
        ));
        flow_pairs.push((
            format!("{}_{}", a_path, flow),
            format!("{}_{}", b_path, flow),
        ));
        return (potential_eqs, flow_pairs, stream_pairs);
    }

    // Scalar / unknown: keep root equality fallback.
    let mut found = false;
    for decl in &flat.declarations {
        if decl.name == a_path {
            found = true;
            if decl.is_flow {
                flow_pairs.push((a_path.to_string(), b_path.to_string()));
            } else if decl.is_stream {
                stream_pairs.push((a_path.to_string(), b_path.to_string()));
                stream_pairs.push((b_path.to_string(), a_path.to_string()));
            } else {
                potential_eqs.push(Equation::Simple(
                    Expression::var(a_path),
                    Expression::var(b_path),
                ));
            }
            break;
        }
    }
    if !found {
        potential_eqs.push(Equation::Simple(
            Expression::var(a_path),
            Expression::var(b_path),
        ));
    }
    (potential_eqs, flow_pairs, stream_pairs)
}

fn connector_member_decl(path: &str, member: &str, type_name: &str, is_flow: bool) -> crate::ast::Declaration {
    crate::ast::Declaration {
        type_name: type_name.to_string(),
        name: format!("{}_{}", path, member),
        replaceable: false,
        constrainedby_type: None,
        is_parameter: false,
        is_flow,
        is_stream: false,
        is_discrete: false,
        is_input: false,
        is_output: false,
        is_inner: false,
        is_outer: false,
        is_public: true,
        is_protected: false,
        start_value: None,
        array_size: None,
        modifications: Vec::new(),
        is_rest: false,
        annotation: None,
        condition: None,
        state_select: None,
    }
}

/// Connector instances (e.g. `vsine_p: PositivePin`, `inertia1_flange_a: Flange_a`)
/// may be recorded in `instances` without expanding scalar members into `declarations`.
/// Connect expansion then finds zero `path_*` decls and emits no potential/flow equations.
pub(super) fn ensure_connector_member_decls(flat: &mut FlattenedModel) {
    let mut existing: HashSet<String> = flat.declarations.iter().map(|d| d.name.clone()).collect();
    let mut to_add: Vec<crate::ast::Declaration> = Vec::new();
    let mut paths: Vec<(String, Option<String>)> = flat
        .instances
        .iter()
        .map(|(path, ty)| (path.clone(), Some(ty.clone())))
        .collect();
    for (a, b) in &flat.connections {
        paths.push((a.clone(), None));
        paths.push((b.clone(), None));
    }
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    paths.dedup_by(|a, b| a.0 == b.0);
    for (path, ty_opt) in paths {
        let ty = ty_opt.or_else(|| flat.instances.get(&path).cloned());
        let members = resolve_potential_flow_members(
            ty.as_deref(),
            None,
            &path,
            &path,
            flat,
        );
        let Some((pot, flow)) = members else {
            continue;
        };
        let pot_name = format!("{}_{}", path, pot);
        let flow_name = format!("{}_{}", path, flow);
        if !existing.contains(&pot_name) {
            to_add.push(connector_member_decl(&path, pot, "Real", false));
            existing.insert(pot_name);
        }
        if !existing.contains(&flow_name) {
            to_add.push(connector_member_decl(&path, flow, "Real", true));
            existing.insert(flow_name);
        }
    }
    flat.declarations.extend(to_add);
}
