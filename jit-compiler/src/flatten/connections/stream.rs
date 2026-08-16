use super::super::structures::FlattenedModel;
use super::expand::has_connector_members;
use std::collections::HashSet;
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
