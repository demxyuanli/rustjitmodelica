use super::structures::FlattenedModel;
use super::utils::are_types_compatible;
use super::FlattenError;
use crate::ast::{Equation, Expression, Operator};
use crate::diag::SourceLocation;
use crate::loader::ModelLoader;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

static CONNECT_WARN_ENABLED: OnceLock<bool> = OnceLock::new();

fn connect_warn_enabled() -> bool {
    *CONNECT_WARN_ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_CONNECT_WARN")
            .ok()
            .map(|v| {
                let t = v.trim().to_ascii_lowercase();
                t == "1" || t == "true" || t == "on" || t == "yes"
            })
            .unwrap_or(false)
    })
}

fn has_connector_members(path: &str, flat: &FlattenedModel) -> bool {
    let prefix = format!("{}_", path);
    flat.declarations.iter().any(|decl| decl.name.starts_with(&prefix))
}

/// All (parent, leaf) splits of a flattened connector-member suffix at each `_` boundary.
/// Modelica identifiers may contain `_`; this uses the same flat naming join as the flattener.
fn parent_leaf_splits(s: &str) -> Vec<(&str, &str)> {
    let mut out = vec![("", s)];
    for (i, b) in s.as_bytes().iter().enumerate() {
        if *b == b'_' {
            out.push((&s[..i], &s[i + 1..]));
        }
    }
    out
}

fn longest_matching_parent_len(stream_suffix: &str, flow_suffix: &str) -> Option<usize> {
    let ss = parent_leaf_splits(stream_suffix);
    let sf = parent_leaf_splits(flow_suffix);
    let mut best: Option<usize> = None;
    for (p1, l1) in &ss {
        for (p2, l2) in &sf {
            if p1 == p2 && l1 != l2 {
                let plen = p1.len();
                if best.map_or(true, |b| plen > b) {
                    best = Some(plen);
                }
            }
        }
    }
    best
}

fn resolve_stream_to_flow_under_prefix(
    stream_suffix: &str,
    flows: &[(String, String)],
) -> Option<String> {
    let mut scored: Vec<(usize, String)> = Vec::new();
    for (sf, full) in flows {
        if let Some(plen) = longest_matching_parent_len(stream_suffix, sf) {
            scored.push((plen, full.clone()));
        }
    }
    if scored.is_empty() {
        return None;
    }
    let max_plen = scored.iter().map(|(p, _)| *p).max()?;
    let at_max: Vec<&String> = scored
        .iter()
        .filter(|(p, _)| *p == max_plen)
        .map(|(_, f)| f)
        .collect();
    if at_max.len() != 1 {
        return None;
    }
    Some(at_max[0].clone())
}

fn instance_prefix_matches(var_name: &str, path: &str) -> bool {
    if var_name.len() <= path.len() {
        return false;
    }
    if !var_name.starts_with(path) {
        return false;
    }
    var_name.as_bytes().get(path.len()) == Some(&b'_')
}

fn declaration_underscore_prefixes(decl_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, b) in decl_name.as_bytes().iter().enumerate() {
        if *b == b'_' && i > 0 {
            out.push(decl_name[..i].to_string());
        }
    }
    out
}

fn preferred_connector_roots(flat: &FlattenedModel) -> HashSet<String> {
    let mut preferred: HashSet<String> = flat.instances.keys().cloned().collect();
    for p in flat.path_to_inst.keys() {
        preferred.insert(p.clone());
    }
    for (a, b) in &flat.connections {
        preferred.insert(a.clone());
        preferred.insert(b.clone());
    }
    for (_cond, (a, b)) in &flat.conditional_connections {
        preferred.insert(a.clone());
        preferred.insert(b.clone());
    }
    preferred
}

/// `(preferred_paths, fallback_paths)` sorted by descending length. Preferred roots come from
/// `instances`, `path_to_inst`, and `connect()` paths; fallback adds `has_connector_members` roots
/// not in preferred (avoids picking spurious longer prefixes from flat-name `_` ambiguity).
fn connector_path_lists(flat: &FlattenedModel) -> (Vec<String>, Vec<String>) {
    let preferred_set = preferred_connector_roots(flat);
    let mut prefix_candidates: HashSet<String> = HashSet::new();
    for decl in &flat.declarations {
        for p in declaration_underscore_prefixes(&decl.name) {
            prefix_candidates.insert(p);
        }
    }
    let mut fallback_set: HashSet<String> = HashSet::new();
    for p in prefix_candidates {
        if has_connector_members(&p, flat) && !preferred_set.contains(&p) {
            fallback_set.insert(p);
        }
    }
    let mut preferred_paths: Vec<String> = preferred_set.into_iter().collect();
    preferred_paths.sort_by_key(|p| std::cmp::Reverse(p.len()));
    let mut fallback_paths: Vec<String> = fallback_set.into_iter().collect();
    fallback_paths.sort_by_key(|p| std::cmp::Reverse(p.len()));
    (preferred_paths, fallback_paths)
}

fn longest_connector_instance_prefix<'a>(
    var_name: &str,
    preferred: &'a [String],
    fallback: &'a [String],
) -> Option<&'a str> {
    for p in preferred {
        if instance_prefix_matches(var_name, p) {
            return Some(p.as_str());
        }
    }
    for p in fallback {
        if instance_prefix_matches(var_name, p) {
            return Some(p.as_str());
        }
    }
    None
}

/// Rebuild explicit stream->flow pairing from `is_stream` / `is_flow` declarations under each
/// connector root (longest preferred match, else longest fallback among `has_connector_members` roots).
/// Ambiguous pairings are skipped.
pub(crate) fn rebuild_stream_flow_map(flat: &mut FlattenedModel) {
    flat.stream_flow_map.clear();
    let (preferred_paths, fallback_paths) = connector_path_lists(flat);
    if preferred_paths.is_empty() && fallback_paths.is_empty() {
        return;
    }
    for decl in &flat.declarations {
        if !decl.is_stream {
            continue;
        }
        let Some(inst) =
            longest_connector_instance_prefix(decl.name.as_str(), &preferred_paths, &fallback_paths)
        else {
            continue;
        };
        let prefix = format!("{}_", inst);
        let Some(suffix_s) = decl.name.strip_prefix(&prefix) else {
            continue;
        };
        let flows: Vec<(String, String)> = flat
            .declarations
            .iter()
            .filter(|d| d.is_flow && d.name.starts_with(&prefix))
            .map(|d| {
                let sf = d.name.strip_prefix(&prefix).unwrap_or("").to_string();
                (sf, d.name.clone())
            })
            .collect();
        if flows.is_empty() {
            continue;
        }
        if let Some(flow_full) = resolve_stream_to_flow_under_prefix(suffix_s, &flows) {
            flat.stream_flow_map.insert(decl.name.clone(), flow_full);
        }
    }
}

fn connector_debug_candidates(path: &str, flat: &FlattenedModel) -> Vec<String> {
    let mut prefixes = vec![path.to_string()];
    if let Some((base, idx)) = path.rsplit_once('_') {
        if idx.parse::<usize>().is_ok() {
            prefixes.push(base.to_string());
        }
    }
    let mut out = Vec::new();
    for prefix in prefixes {
        let instance_prefix = format!("{}_", prefix);
        for key in flat.instances.keys() {
            if key == &prefix || key.starts_with(&instance_prefix) {
                out.push(format!("instance:{}", key));
            }
        }
        for decl in &flat.declarations {
            if decl.name == prefix || decl.name.starts_with(&instance_prefix) {
                out.push(format!("decl:{}", decl.name));
            }
        }
        if !out.is_empty() {
            break;
        }
    }
    out.sort();
    out.dedup();
    out.truncate(8);
    out
}

fn equations_for_connections(
    flat: &FlattenedModel,
    connections: &[(String, String)],
) -> Vec<Equation> {
    let mut potential_eqs = Vec::new();
    let mut flow_adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut flow_vars = HashSet::new();
    for (a_path, b_path) in connections {
        if flat.instances.contains_key(a_path) || has_connector_members(a_path, flat) {
            let prefix_a = format!("{}_", a_path);
            let prefix_b = format!("{}_", b_path);
            for decl in &flat.declarations {
                if decl.name.starts_with(&prefix_a) {
                    if let Some(suffix) = decl.name.strip_prefix(&prefix_a) {
                        let target_name = format!("{}{}", prefix_b, suffix);
                        if decl.is_flow {
                            flow_adj
                                .entry(decl.name.clone())
                                .or_default()
                                .push(target_name.clone());
                            flow_adj
                                .entry(target_name.clone())
                                .or_default()
                                .push(decl.name.clone());
                            flow_vars.insert(decl.name.clone());
                            flow_vars.insert(target_name);
                        } else {
                            potential_eqs.push(Equation::Simple(
                                Expression::var(&decl.name),
                                Expression::Variable(crate::string_intern::intern(&target_name)),
                            ));
                        }
                    }
                }
            }
        } else {
            let mut found = false;
            for decl in &flat.declarations {
                if decl.name == *a_path {
                    found = true;
                    if decl.is_flow {
                        flow_adj
                            .entry(a_path.clone())
                            .or_default()
                            .push(b_path.clone());
                        flow_adj
                            .entry(b_path.clone())
                            .or_default()
                            .push(a_path.clone());
                        flow_vars.insert(a_path.clone());
                        flow_vars.insert(b_path.clone());
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
        }
    }
    // Expandable connector: inject members from non-expandable side
    for (a_path, b_path) in connections {
        let a_is_expandable = flat.expandable_instances.contains(a_path.as_str());
        let b_is_expandable = flat.expandable_instances.contains(b_path.as_str());
        if a_is_expandable || b_is_expandable {
            if a_is_expandable && !b_is_expandable {
                inject_expandable_members(flat, a_path, b_path, &mut potential_eqs);
            } else if b_is_expandable && !a_is_expandable {
                inject_expandable_members(flat, b_path, a_path, &mut potential_eqs);
            } else {
                // Both expandable: cross-inject
                inject_expandable_members(flat, a_path, b_path, &mut potential_eqs);
                inject_expandable_members(flat, b_path, a_path, &mut potential_eqs);
            }
        }
    }
    let mut out = potential_eqs;
    let mut visited = HashSet::new();
    let mut flow_vars_sorted: Vec<String> = flow_vars.iter().cloned().collect();
    flow_vars_sorted.sort_unstable();
    for var in flow_vars_sorted {
        if visited.contains(&var) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![var.clone()];
        visited.insert(var);
        while let Some(curr) = stack.pop() {
            component.push(curr.clone());
            if let Some(neighbors) = flow_adj.get(&curr) {
                for n in neighbors {
                    if !visited.contains(n) {
                        visited.insert(n.clone());
                        stack.push(n.clone());
                    }
                }
            }
        }
        if !component.is_empty() {
            component.sort_unstable();
            let mut expr = Expression::Variable(crate::string_intern::intern(&component[0]));
            for i in 1..component.len() {
                expr = Expression::BinaryOp(
                    Box::new(expr),
                    Operator::Add,
                    Box::new(Expression::Variable(crate::string_intern::intern(&component[i]))),
                );
            }
            out.push(Equation::Simple(expr, Expression::Number(0.0)));
        }
    }
    out
}

fn flatten_resolve_parallel_enabled() -> bool {
    std::env::var("RUSTMODLICA_FLATTEN_RESOLVE_PARALLEL")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

fn flatten_resolve_parallel_min_items() -> usize {
    std::env::var("RUSTMODLICA_FLATTEN_RESOLVE_PARALLEL_MIN_ITEMS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(64)
}

struct ConnBuildOut {
    potential_eqs: Vec<Equation>,
    flow_pairs: Vec<(String, String)>,
    stream_pairs: Vec<(String, String)>,
    clock_pairs: Vec<(String, String)>,
}

fn process_connection_pair(
    flat: &FlattenedModel,
    loader: &ModelLoader,
    root_model_name: Option<&str>,
    a_path: &str,
    b_path: &str,
) -> Result<ConnBuildOut, FlattenError> {
    let mut out = ConnBuildOut {
        potential_eqs: Vec::new(),
        flow_pairs: Vec::new(),
        stream_pairs: Vec::new(),
        clock_pairs: Vec::new(),
    };
    let type_a = find_connector_type(a_path, flat);
    let type_b = find_connector_type(b_path, flat);
    if let (Some(ta), Some(tb)) = (&type_a, &type_b) {
        if !are_types_compatible(ta, tb) {
            let loc = root_model_name
                .and_then(|n| loader.get_path_for_model(n))
                .map(|p| SourceLocation {
                    file: p.display().to_string(),
                    line: 0,
                    column: 0,
                });
            return Err(FlattenError::IncompatibleConnector(
                a_path.to_string(),
                b_path.to_string(),
                ta.clone(),
                tb.clone(),
                loc,
            ));
        }
        let heat_scalar_array = |t: &str| {
            t.ends_with("HeatPort_a") || t.ends_with("HeatPort_b") || t.ends_with("HeatPorts_a")
        };
        if heat_scalar_array(ta) && heat_scalar_array(tb) && ta != tb {
            return Ok(out);
        }
        let clock_conn = |t: &str| {
            let u = t.to_ascii_lowercase();
            u.contains("clock") && (u.contains("input") || u.contains("output"))
        };
        if clock_conn(ta) && clock_conn(tb) {
            out.clock_pairs
                .push((a_path.to_string(), b_path.to_string()));
        }
    } else if !loader.quiet && connect_warn_enabled() {
        if type_a.is_none() {
            let cands = connector_debug_candidates(a_path, flat);
            eprintln!(
                "Warning: Could not determine type for connector '{}' (path in model){}",
                a_path,
                if cands.is_empty() {
                    String::new()
                } else {
                    format!("; nearby={}", cands.join(", "))
                }
            );
        }
        if type_b.is_none() {
            let cands = connector_debug_candidates(b_path, flat);
            eprintln!(
                "Warning: Could not determine type for connector '{}' (path in model){}",
                b_path,
                if cands.is_empty() {
                    String::new()
                } else {
                    format!("; nearby={}", cands.join(", "))
                }
            );
        }
    }

    if flat.instances.contains_key(a_path) || has_connector_members(a_path, flat) {
        let prefix_a = format!("{}_", a_path);
        let prefix_b = format!("{}_", b_path);
        for decl in &flat.declarations {
            if decl.name.starts_with(&prefix_a) {
                if let Some(suffix) = decl.name.strip_prefix(&prefix_a) {
                    let target_name = format!("{}{}", prefix_b, suffix);
                    if decl.is_flow {
                        out.flow_pairs.push((decl.name.clone(), target_name));
                    } else if decl.is_stream {
                        out.stream_pairs
                            .push((decl.name.clone(), target_name.clone()));
                        out.stream_pairs.push((target_name, decl.name.clone()));
                    } else {
                        out.potential_eqs.push(Equation::Simple(
                            Expression::var(&decl.name),
                            Expression::Variable(crate::string_intern::intern(&target_name)),
                        ));
                    }
                }
            }
        }
    } else {
        let mut found = false;
        for decl in &flat.declarations {
            if decl.name == a_path {
                found = true;
                if decl.is_flow {
                    out.flow_pairs.push((a_path.to_string(), b_path.to_string()));
                } else if decl.is_stream {
                    out.stream_pairs.push((a_path.to_string(), b_path.to_string()));
                    out.stream_pairs.push((b_path.to_string(), a_path.to_string()));
                } else {
                    out.potential_eqs
                        .push(Equation::Simple(Expression::var(a_path), Expression::var(b_path)));
                }
                break;
            }
        }
        if !found {
            if !loader.quiet && connect_warn_enabled() {
                let cands = connector_debug_candidates(a_path, flat);
                eprintln!(
                    "Warning: Connect involving unknown variable '{}'. Assuming potential equality.",
                    a_path
                );
                if !cands.is_empty() {
                    eprintln!("  Nearby connector candidates: {}", cands.join(", "));
                }
            }
            out.potential_eqs
                .push(Equation::Simple(Expression::var(a_path), Expression::var(b_path)));
        }
    }
    Ok(out)
}

pub fn resolve_connections(
    flat: &mut FlattenedModel,
    root_model_name: Option<&str>,
    loader: &ModelLoader,
) -> Result<(), FlattenError> {
    let mut potential_eqs = Vec::new();
    let mut flow_adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut flow_vars = HashSet::new();
    let parallel_enabled = flatten_resolve_parallel_enabled();
    let parallel_min_items = flatten_resolve_parallel_min_items();
    if parallel_enabled && flat.connections.len() >= parallel_min_items {
        crate::query_db::perf_record_add("flatten_parallel_poc_enabled", 1);
        let outs: Vec<Result<ConnBuildOut, FlattenError>> = flat
            .connections
            .par_iter()
            .map(|(a_path, b_path)| {
                process_connection_pair(flat, loader, root_model_name, a_path, b_path)
            })
            .collect();
        for item in outs {
            let out = item?;
            potential_eqs.extend(out.potential_eqs);
            for (a, b) in out.flow_pairs {
                flow_adj.entry(a.clone()).or_default().push(b.clone());
                flow_adj.entry(b.clone()).or_default().push(a.clone());
                flow_vars.insert(a);
                flow_vars.insert(b);
            }
            for (a, b) in out.stream_pairs {
                flat.stream_peer_map.insert(a, b);
            }
            flat.clock_signal_connections.extend(out.clock_pairs);
        }
    } else {
        for (a_path, b_path) in &flat.connections {
        // Type Checking: Verify connector compatibility
        let type_a = find_connector_type(a_path, flat);
        let type_b = find_connector_type(b_path, flat);

        if let (Some(ta), Some(tb)) = (&type_a, &type_b) {
            if !are_types_compatible(ta, tb) {
                let loc = root_model_name
                    .and_then(|n| loader.get_path_for_model(n))
                    .map(|p| SourceLocation {
                        file: p.display().to_string(),
                        line: 0,
                        column: 0,
                    });
                return Err(FlattenError::IncompatibleConnector(
                    a_path.clone(),
                    b_path.clone(),
                    ta.clone(),
                    tb.clone(),
                    loc,
                ));
            }
            let heat_scalar_array = |t: &str| {
                t.ends_with("HeatPort_a") || t.ends_with("HeatPort_b") || t.ends_with("HeatPorts_a")
            };
            if heat_scalar_array(ta) && heat_scalar_array(tb) && ta != tb {
                continue;
            }
            let clock_conn = |t: &str| {
                let u = t.to_ascii_lowercase();
                u.contains("clock") && (u.contains("input") || u.contains("output"))
            };
            if clock_conn(ta) && clock_conn(tb) {
                flat.clock_signal_connections
                    .push((a_path.clone(), b_path.clone()));
            }
        } else {
            if !loader.quiet && connect_warn_enabled() {
                if type_a.is_none() {
                    let cands = connector_debug_candidates(a_path, flat);
                    eprintln!(
                        "Warning: Could not determine type for connector '{}' (path in model){}",
                        a_path,
                        if cands.is_empty() {
                            String::new()
                        } else {
                            format!("; nearby={}", cands.join(", "))
                        }
                    );
                }
                if type_b.is_none() {
                    let cands = connector_debug_candidates(b_path, flat);
                    eprintln!(
                        "Warning: Could not determine type for connector '{}' (path in model){}",
                        b_path,
                        if cands.is_empty() {
                            String::new()
                        } else {
                            format!("; nearby={}", cands.join(", "))
                        }
                    );
                }
            }
        }

        if flat.instances.contains_key(a_path) || has_connector_members(a_path, flat) {
            let prefix_a = format!("{}_", a_path);
            let prefix_b = format!("{}_", b_path);

            for decl in &flat.declarations {
                if decl.name.starts_with(&prefix_a) {
                    if let Some(suffix) = decl.name.strip_prefix(&prefix_a) {
                        let target_name = format!("{}{}", prefix_b, suffix);
                        if decl.is_flow {
                            flow_adj
                                .entry(decl.name.clone())
                                .or_default()
                                .push(target_name.clone());
                            flow_adj
                                .entry(target_name.clone())
                                .or_default()
                                .push(decl.name.clone());
                            flow_vars.insert(decl.name.clone());
                            flow_vars.insert(target_name);
                        } else if decl.is_stream {
                            flat.stream_peer_map
                                .insert(decl.name.clone(), target_name.clone());
                            flat.stream_peer_map
                                .insert(target_name.clone(), decl.name.clone());
                        } else {
                            potential_eqs.push(Equation::Simple(
                                Expression::var(&decl.name),
                                Expression::Variable(crate::string_intern::intern(&target_name)),
                            ));
                        }
                    }
                }
            }
        } else {
            let mut found = false;
            for decl in &flat.declarations {
                if decl.name == *a_path {
                    found = true;
                    if decl.is_flow {
                        flow_adj
                            .entry(a_path.clone())
                            .or_default()
                            .push(b_path.clone());
                        flow_adj
                            .entry(b_path.clone())
                            .or_default()
                            .push(a_path.clone());
                        flow_vars.insert(a_path.clone());
                        flow_vars.insert(b_path.clone());
                    } else if decl.is_stream {
                        flat.stream_peer_map.insert(a_path.clone(), b_path.clone());
                        flat.stream_peer_map.insert(b_path.clone(), a_path.clone());
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
                if !loader.quiet && connect_warn_enabled() {
                    let cands = connector_debug_candidates(a_path, flat);
                    eprintln!("Warning: Connect involving unknown variable '{}'. Assuming potential equality.", a_path);
                    if !cands.is_empty() {
                        eprintln!("  Nearby connector candidates: {}", cands.join(", "));
                    }
                }
                potential_eqs.push(Equation::Simple(
                    Expression::var(a_path),
                    Expression::var(b_path),
                ));
            }
        }
    }
    }

    // Expandable connector: inject members from non-expandable side
    for (a_path, b_path) in &flat.connections {
        let a_is_expandable = flat.expandable_instances.contains(a_path.as_str());
        let b_is_expandable = flat.expandable_instances.contains(b_path.as_str());
        if a_is_expandable || b_is_expandable {
            if a_is_expandable && !b_is_expandable {
                inject_expandable_members(flat, a_path, b_path, &mut potential_eqs);
            } else if b_is_expandable && !a_is_expandable {
                inject_expandable_members(flat, b_path, a_path, &mut potential_eqs);
            } else {
                // Both expandable: cross-inject
                inject_expandable_members(flat, a_path, b_path, &mut potential_eqs);
                inject_expandable_members(flat, b_path, a_path, &mut potential_eqs);
            }
        }
    }

    flat.equations.extend(potential_eqs);

    // Build stream connection-set map (multi-port): each stream variable maps to all peers
    // reachable in the same stream-connectivity component.
    let mut stream_adj: HashMap<String, Vec<String>> = HashMap::new();
    for (a, b) in &flat.stream_peer_map {
        stream_adj.entry(a.clone()).or_default().push(b.clone());
    }
    let mut stream_component_visited = HashSet::new();
    let mut stream_nodes: Vec<String> = stream_adj.keys().cloned().collect();
    stream_nodes.sort_unstable();
    flat.stream_connection_set.clear();
    for s in stream_nodes {
        if stream_component_visited.contains(&s) {
            continue;
        }
        let mut comp: Vec<String> = Vec::new();
        let mut stack = vec![s.clone()];
        stream_component_visited.insert(s);
        while let Some(curr) = stack.pop() {
            comp.push(curr.clone());
            if let Some(nei) = stream_adj.get(&curr) {
                for n in nei {
                    if !stream_component_visited.contains(n) {
                        stream_component_visited.insert(n.clone());
                        stack.push(n.clone());
                    }
                }
            }
        }
        comp.sort_unstable();
        for node in &comp {
            let peers: Vec<String> = comp
                .iter()
                .filter(|p| *p != node)
                .cloned()
                .collect();
            flat.stream_connection_set.insert(node.clone(), peers);
        }
    }

    let mut visited = HashSet::new();
    let mut flow_vars_sorted: Vec<String> = flow_vars.iter().cloned().collect();
    flow_vars_sorted.sort_unstable();
    for var in flow_vars_sorted {
        if !visited.contains(&var) {
            let mut component = Vec::new();
            let mut stack = vec![var.clone()];
            visited.insert(var);

            while let Some(curr) = stack.pop() {
                component.push(curr.clone());
                if let Some(neighbors) = flow_adj.get(&curr) {
                    for n in neighbors {
                        if !visited.contains(n) {
                            visited.insert(n.clone());
                            stack.push(n.clone());
                        }
                    }
                }
            }

            if !component.is_empty() {
                component.sort_unstable();
                let mut expr = Expression::Variable(crate::string_intern::intern(&component[0]));
                for i in 1..component.len() {
                    expr = Expression::BinaryOp(
                        Box::new(expr),
                        Operator::Add,
                        Box::new(Expression::Variable(crate::string_intern::intern(&component[i]))),
                    );
                }
                flat.equations
                    .push(Equation::Simple(expr, Expression::Number(0.0)));
            }
        }
    }

    if !flat.conditional_connections.is_empty() {
        let mut groups: Vec<(Expression, Vec<(String, String)>)> = Vec::new();
        for (cond, conn) in &flat.conditional_connections {
            let type_a = find_connector_type(&conn.0, flat);
            let type_b = find_connector_type(&conn.1, flat);
            if let (Some(ref ta), Some(ref tb)) = (&type_a, &type_b) {
                if !are_types_compatible(ta, tb) {
                    let loc = root_model_name
                        .and_then(|n| loader.get_path_for_model(n))
                        .map(|p| SourceLocation {
                            file: p.display().to_string(),
                            line: 0,
                            column: 0,
                        });
                    return Err(FlattenError::IncompatibleConnector(
                        conn.0.clone(),
                        conn.1.clone(),
                        ta.clone(),
                        tb.clone(),
                        loc,
                    ));
                }
                let heat_scalar_array = |t: &str| {
                    t.ends_with("HeatPort_a") || t.ends_with("HeatPort_b") || t.ends_with("HeatPorts_a")
                };
                if heat_scalar_array(ta) && heat_scalar_array(tb) && ta != tb {
                    continue;
                }
            }
            if let Some((_, list)) = groups.iter_mut().find(|(c, _)| c == cond) {
                list.push(conn.clone());
            } else {
                groups.push((cond.clone(), vec![conn.clone()]));
            }
        }
        for (cond, conns) in groups {
            let eqs = equations_for_connections(flat, &conns);
            if !eqs.is_empty() {
                flat.equations.push(Equation::When(cond, eqs, Vec::new()));
            }
        }
    }

    rebuild_stream_flow_map(flat);
    Ok(())
}

fn find_connector_type(path: &str, flat: &FlattenedModel) -> Option<String> {
    if let Some(type_name) = flat.instances.get(path) {
        return Some(type_name.clone());
    }
    for decl in &flat.declarations {
        if decl.name == path {
            return Some(decl.type_name.clone());
        }
    }
    if has_connector_members(path, flat) {
        return Some("connector".to_string());
    }

    // Hierarchical prefix matching: try progressively longer prefixes against instances.
    // Flattened paths use '_' as separator, so try splitting at each '_'.
    let bytes = path.as_bytes();
    let mut best_prefix_len = 0;
    let mut best_type = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'_' {
            let prefix = &path[..i];
            if let Some(type_name) = flat.instances.get(prefix) {
                best_prefix_len = i;
                best_type = Some(type_name.clone());
            }
        }
    }
    if let Some(_parent_type) = best_type {
        let full_prefix = &path[..best_prefix_len];
        let suffix = &path[best_prefix_len + 1..];
        let suffix_parts: Vec<&str> = suffix.split('_').collect();
        for end in (1..=suffix_parts.len()).rev() {
            let candidate = format!("{}_{}", full_prefix, suffix_parts[..end].join("_"));
            if let Some(t) = flat.instances.get(&candidate) {
                return Some(t.clone());
            }
            if has_connector_members(&candidate, flat) {
                return Some("connector".to_string());
            }
        }
    }

    None
}

/// Inject members from a non-expandable source connector into an expandable target.
/// For each declaration prefixed by `source_path_`, generate an equation equating
/// the projected name under `expandable_path_` to the source's flattened variable.
fn inject_expandable_members(
    flat: &FlattenedModel,
    expandable_path: &str,
    source_path: &str,
    potential_eqs: &mut Vec<Equation>,
) {
    let source_prefix = format!("{}_", source_path);
    let target_prefix = format!("{}_", expandable_path);
    for decl in &flat.declarations {
        if let Some(suffix) = decl.name.strip_prefix(&source_prefix) {
            let target_name = format!("{}{}", target_prefix, suffix);
            // Only inject if target doesn't already have this member
            if !flat.declarations.iter().any(|d| d.name == target_name) {
                potential_eqs.push(Equation::Simple(
                    Expression::var(&target_name),
                    Expression::var(&decl.name),
                ));
            }
        }
    }
}
