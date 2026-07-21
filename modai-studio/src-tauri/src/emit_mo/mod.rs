use crate::diagram::{DiagramDocument, DiagramNode, SimulationDefaults};
use std::collections::HashMap;

/// Emit a flat Modelica model, expanding subsystem nodes by instance-name prefix.
/// FMU co-sim nodes become nested block stubs (structure for validate; Run uses FMI host).
pub fn emit_modelica(
    root: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    sim: Option<&SimulationDefaults>,
) -> String {
    let mut nested: Vec<String> = Vec::new();
    let mut decls: Vec<String> = Vec::new();
    let mut eqs: Vec<String> = Vec::new();
    emit_diagram(root, "", subsystems, &mut nested, &mut decls, &mut eqs);

    let mut out = String::new();
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
                    emit_diagram(child, &child_prefix, subsystems, nested, decls, eqs);
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
        if node.node_kind != "component" {
            continue;
        }
        let inst = format!("{prefix}{}", node.instance_name);
        let mut line = format!("  {} {}", node.type_name, inst);
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
            "  connect({}.{}, {}.{});\n",
            src_inst, src_port, dst_inst, dst_port
        ));
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
