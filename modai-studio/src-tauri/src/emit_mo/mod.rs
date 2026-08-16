use crate::diagram::{DiagramDocument, DiagramNode, ParamDictionaryEntry, SimulationDefaults};
use crate::user_catalog::collect_referenced_sources;
use std::collections::HashMap;
use std::path::Path;

/// Emit a flat Modelica model, expanding subsystem nodes by instance-name prefix.
/// FMU co-sim nodes become nested block stubs (structure for validate; Run uses FMI host).
/// `dictionary` resolves block params of the form `$Name`.
pub fn emit_modelica(
    root: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    sim: Option<&SimulationDefaults>,
) -> String {
    emit_modelica_with_dict(root, subsystems, sim, &[], None)
}

pub fn emit_modelica_with_dict(
    root: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    sim: Option<&SimulationDefaults>,
    dictionary: &[ParamDictionaryEntry],
    project_root: Option<&Path>,
) -> String {
    let dict = dict_map(dictionary);
    let mut nested: Vec<String> = Vec::new();
    let mut decls: Vec<String> = Vec::new();
    let mut eqs: Vec<String> = Vec::new();
    emit_diagram(
        root,
        "",
        subsystems,
        &dict,
        &mut nested,
        &mut decls,
        &mut eqs,
    );

    let mut out = String::new();
    if let Some(pr) = project_root {
        let types = collect_type_names(root, subsystems);
        out.push_str(&collect_referenced_sources(pr, &types));
    }
    out.push_str(&format!("model {}\n", root.model_name));
    for n in &nested {
        out.push_str(n);
    }
    for d in &decls {
        out.push_str(d);
    }
    out.push_str("equation\n");
    if eqs.is_empty() {
        out.push_str("  // no connections\n");
    } else {
        for e in &eqs {
            out.push_str(e);
        }
    }
    if let Some(s) = sim {
        let interval = if s.number_of_intervals > 0 {
            s.stop_time / f64::from(s.number_of_intervals)
        } else {
            0.002
        };
        out.push_str(&format!(
            "  annotation(experiment(StopTime = {}, Interval = {}));\n",
            s.stop_time, interval
        ));
    }
    out.push_str(&format!("end {};\n", root.model_name));
    out
}

fn collect_type_names(
    root: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
) -> Vec<String> {
    let mut names = Vec::new();
    let mut visit = |d: &DiagramDocument| {
        for n in &d.nodes {
            if n.node_kind == "component" && !n.type_name.is_empty() {
                names.push(n.type_name.clone());
            }
        }
    };
    visit(root);
    for d in subsystems.values() {
        visit(d);
    }
    names.sort();
    names.dedup();
    names
}

fn fmu_block_type_name(node: &DiagramNode) -> String {
    format!("StudioFmu_{}", node.instance_name)
}

fn emit_fmu_nested_block(node: &DiagramNode) -> String {
    let ty = fmu_block_type_name(node);
    let mut s = format!("  block {ty}\n");
    for p in &node.params {
        if p.value.trim().is_empty() {
            continue;
        }
        s.push_str(&format!(
            "    parameter Real {} = {};\n",
            p.name, p.value
        ));
    }
    for p in &node.ports {
        if p.kind != "signal" {
            continue;
        }
        if p.direction == "in" {
            s.push_str(&format!(
                "    Modelica.Blocks.Interfaces.RealInput {};\n",
                p.id
            ));
        } else if p.direction == "out" {
            s.push_str(&format!(
                "    Modelica.Blocks.Interfaces.RealOutput {};\n",
                p.id
            ));
        }
    }
    s.push_str("  equation\n");
    let outs: Vec<_> = node
        .ports
        .iter()
        .filter(|p| p.kind == "signal" && p.direction == "out")
        .collect();
    if outs.is_empty() {
        s.push_str("    // FMU co-sim node (no Real outputs)\n");
    } else {
        for p in outs {
            s.push_str(&format!(
                "    {} = 0; // filled by Studio FMI cosim host at Run\n",
                p.id
            ));
        }
    }
    s.push_str(&format!("  end {ty};\n"));
    s
}

fn emit_diagram(
    diagram: &DiagramDocument,
    prefix: &str,
    subsystems: &HashMap<String, DiagramDocument>,
    dict: &HashMap<String, String>,
    nested: &mut Vec<String>,
    decls: &mut Vec<String>,
    eqs: &mut Vec<String>,
) {
    let mut node_by_id: HashMap<&str, &DiagramNode> = HashMap::new();
    for n in &diagram.nodes {
        node_by_id.insert(n.id.as_str(), n);
    }

    for node in &diagram.nodes {
        if node.node_kind == "subsystem" {
            if let Some(ref_id) = &node.subsystem_ref {
                if let Some(child) = subsystems.get(ref_id) {
                    let child_prefix = if prefix.is_empty() {
                        format!("{}_", node.instance_name)
                    } else {
                        format!("{prefix}{}_", node.instance_name)
                    };
                    emit_diagram(
                        child,
                        &child_prefix,
                        subsystems,
                        dict,
                        nested,
                        decls,
                        eqs,
                    );
                }
            }
            continue;
        }
        if node.node_kind == "fmu" {
            nested.push(emit_fmu_nested_block(node));
            let inst = format!("{prefix}{}", node.instance_name);
            let ty = fmu_block_type_name(node);
            let mut line = format!("  {ty} {inst}");
            if !node.params.is_empty() {
                let parts: Vec<String> = node
                    .params
                    .iter()
                    .filter(|p| !p.value.trim().is_empty())
                    .map(|p| format!("{} = {}", p.name, p.value))
                    .collect();
                if !parts.is_empty() {
                    line.push('(');
                    line.push_str(&parts.join(", "));
                    line.push(')');
                }
            }
            line.push_str(";\n");
            decls.push(line);
            continue;
        }
        // draw / annotation graphics are canvas-only
        if node.node_kind == "draw"
            || node.node_kind == "annotation"
            || node.domain == "draw"
        {
            continue;
        }
        if node.node_kind != "component" {
            continue;
        }
        let inst = format!("{prefix}{}", node.instance_name);
        let inner = if node.type_name.contains("MultiBody.World") {
            "inner "
        } else {
            ""
        };
        let mut line = format!("  {inner}{} {}", node.type_name, inst);
        if !node.params.is_empty() {
            let parts: Vec<String> = node
                .params
                .iter()
                .filter(|p| !p.value.trim().is_empty())
                .filter(|p| emit_param_allowed(node, &p.name))
                .map(|p| {
                    let raw = resolve_dict_value(&p.value, dict);
                    format_instance_modifier(&p.name, &raw)
                })
                .collect();
            if !parts.is_empty() {
                line.push('(');
                line.push_str(&parts.join(", "));
                line.push(')');
            }
        }
        line.push_str(";\n");
        decls.push(line);
    }

    for edge in &diagram.edges {
        let Some(src_node) = node_by_id.get(edge.source_node_id.as_str()) else {
            continue;
        };
        let Some(dst_node) = node_by_id.get(edge.target_node_id.as_str()) else {
            continue;
        };
        let (src_inst, src_port) =
            resolve_endpoint(src_node, &edge.source_port_id, prefix, subsystems);
        let (dst_inst, dst_port) =
            resolve_endpoint(dst_node, &edge.target_port_id, prefix, subsystems);
        eqs.push(format!(
            "  connect({}, {});\n",
            format_connect_ref(&src_inst, &src_port),
            format_connect_ref(&dst_inst, &dst_port)
        ));
    }

    emit_goto_from_links(diagram, prefix, eqs);
}

fn dict_map(dictionary: &[ParamDictionaryEntry]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for e in dictionary {
        let name = e.name.trim();
        if name.is_empty() {
            continue;
        }
        m.insert(name.to_string(), e.value.trim().to_string());
    }
    m
}

/// Resolve `$Name` against the project dictionary; otherwise return trimmed value.
pub fn resolve_dict_value(value: &str, dict: &HashMap<String, String>) -> String {
    let v = value.trim();
    if let Some(rest) = v.strip_prefix('$') {
        let key = rest.trim();
        if let Some(found) = dict.get(key) {
            return found.clone();
        }
    }
    v.to_string()
}

fn param_value(node: &DiagramNode, name: &str) -> Option<String> {
    node.params
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.value.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Skip diagram-only params that are not Modelica modifiers on the target type.
fn emit_param_allowed(node: &DiagramNode, name: &str) -> bool {
    match name {
        "tag" if is_goto_node(node) || is_from_node(node) => false,
        "portNumber" => false,
        _ => true,
    }
}

/// `phi` + `(start = 0.5)` -> `phi(start = 0.5)`; otherwise `name = value`.
fn format_instance_modifier(name: &str, value: &str) -> String {
    let v = value.trim();
    if v.starts_with('(') && v.ends_with(')') {
        format!("{name}{v}")
    } else {
        format!("{} = {}", name, format_param_value(v))
    }
}

fn format_param_value(value: &str) -> String {
    let v = value.trim();
    if v.is_empty() {
        return "0".into();
    }
    if v.starts_with('"') || v.starts_with('[') || v.starts_with('{') {
        return v.to_string();
    }
    if v.parse::<f64>().is_ok() || v == "true" || v == "false" {
        return v.to_string();
    }
    // Enum paths / identifiers (e.g. Modelica.Blocks.Types....)
    if v.contains('.') && !v.contains(' ') {
        return v.to_string();
    }
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
}

fn is_goto_node(node: &DiagramNode) -> bool {
    node.catalog_id.to_lowercase().contains("routing.goto")
        || node.catalog_id.to_lowercase().ends_with(".goto")
}

fn is_from_node(node: &DiagramNode) -> bool {
    node.catalog_id.to_lowercase().contains("routing.from")
        || node.catalog_id.to_lowercase().ends_with(".from")
}

/// Wire From.u <- Goto.y for matching `tag` params (Simulink-like jump).
fn emit_goto_from_links(diagram: &DiagramDocument, prefix: &str, eqs: &mut Vec<String>) {
    let gotos: Vec<(&DiagramNode, String)> = diagram
        .nodes
        .iter()
        .filter(|n| n.node_kind == "component" && is_goto_node(n))
        .filter_map(|n| Some((n, param_value(n, "tag")?)))
        .collect();
    for node in diagram.nodes.iter().filter(|n| n.node_kind == "component" && is_from_node(n))
    {
        let Some(tag) = param_value(node, "tag") else {
            continue;
        };
        let Some((goto, _)) = gotos.iter().find(|(_, t)| t == &tag) else {
            continue;
        };
        let goto_inst = format!("{prefix}{}", goto.instance_name);
        let from_inst = format!("{prefix}{}", node.instance_name);
        eqs.push(format!("  connect({goto_inst}.y, {from_inst}.u);\n"));
    }
}

/// Port id `self` means the component is a connector (RealInput/RealOutput).
fn format_connect_ref(inst: &str, port: &str) -> String {
    if port.is_empty() || port == "self" {
        inst.to_string()
    } else {
        format!("{inst}.{port}")
    }
}

fn resolve_endpoint(
    node: &DiagramNode,
    port_id: &str,
    prefix: &str,
    subsystems: &HashMap<String, DiagramDocument>,
) -> (String, String) {
    if node.node_kind == "subsystem" {
        if let Some(binding) = port_binding(node, port_id) {
            let child_prefix = if prefix.is_empty() {
                format!("{}_", node.instance_name)
            } else {
                format!("{prefix}{}_", node.instance_name)
            };
            if let Some(ref_id) = &node.subsystem_ref {
                if let Some(child) = subsystems.get(ref_id) {
                    if let Some(inner) = child.nodes.iter().find(|n| n.id == binding.node_id) {
                        if inner.node_kind == "subsystem" {
                            return resolve_endpoint(
                                inner,
                                &binding.port_id,
                                &child_prefix,
                                subsystems,
                            );
                        }
                        return (
                            format!("{child_prefix}{}", inner.instance_name),
                            binding.port_id,
                        );
                    }
                }
            }
            return (
                format!("{child_prefix}{}", binding.node_id),
                binding.port_id,
            );
        }
    }
    (format!("{prefix}{}", node.instance_name), port_id.to_string())
}

#[derive(Clone)]
struct PortBinding {
    node_id: String,
    port_id: String,
}

fn port_binding(node: &DiagramNode, port_id: &str) -> Option<PortBinding> {
    let ui = node.ui.as_ref()?;
    let bindings = ui.port_bindings.as_ref()?;
    let v = bindings.get(port_id)?;
    Some(PortBinding {
        node_id: v.node_id.clone(),
        port_id: v.port_id.clone(),
    })
}
