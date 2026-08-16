//! Emit golden checks for subsystem / ModelRef-style / Goto-From (Q4-A P523–P524).

use crate::diagram::{
    DiagramDocument, DiagramEdge, DiagramNode, ParamValue, PortDef, Position,
};
use crate::emit_mo::emit_modelica;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn signal_port(id: &str, direction: &str) -> PortDef {
    PortDef {
        id: id.into(),
        kind: "signal".into(),
        direction: direction.into(),
        type_name: if direction == "in" {
            "RealInput".into()
        } else {
            "RealOutput".into()
        },
    }
}

fn node(
    id: &str,
    inst: &str,
    type_name: &str,
    catalog_id: &str,
    ports: Vec<PortDef>,
    params: Vec<ParamValue>,
) -> DiagramNode {
    DiagramNode {
        id: id.into(),
        instance_name: inst.into(),
        type_name: type_name.into(),
        domain: "signal".into(),
        catalog_id: catalog_id.into(),
        position: Position { x: 0.0, y: 0.0 },
        rotation: 0.0,
        params,
        ports,
        node_kind: "component".into(),
        subsystem_ref: None,
        plot_hints: vec![],
        ui: None,
    }
}

fn edge(id: &str, s: &str, sp: &str, t: &str, tp: &str) -> DiagramEdge {
    DiagramEdge {
        id: id.into(),
        source_node_id: s.into(),
        source_port_id: sp.into(),
        target_node_id: t.into(),
        target_port_id: tp.into(),
        signal_name: None,
        unit: None,
        dimension: None,
        vertices: vec![],
        routing: Some("orthogonal".into()),
        ui: None,
    }
}

/// Nested subsystem: root Step -> Sub(Gain) -> (outer) with In/Out terminals inside.
fn fixture_subsystem() -> (DiagramDocument, HashMap<String, DiagramDocument>) {
    let child = DiagramDocument {
        schema_version: 1,
        model_name: "Child".into(),
        diagram_id: "sub_child".into(),
        parent_diagram_id: Some("root".into()),
        nodes: vec![
            node(
                "n_in",
                "in1",
                "Modelica.Blocks.Interfaces.RealInput",
                "blocks.terminals.inport",
                vec![signal_port("y", "out")],
                vec![],
            ),
            node(
                "n_gain",
                "gain1",
                "Modelica.Blocks.Math.Gain",
                "blocks.math.gain",
                vec![signal_port("u", "in"), signal_port("y", "out")],
                vec![ParamValue {
                    name: "k".into(),
                    value: "2".into(),
                }],
            ),
            node(
                "n_out",
                "out1",
                "Modelica.Blocks.Interfaces.RealOutput",
                "blocks.terminals.outport",
                vec![signal_port("u", "in")],
                vec![],
            ),
        ],
        edges: vec![
            edge("e1", "n_in", "y", "n_gain", "u"),
            edge("e2", "n_gain", "y", "n_out", "u"),
        ],
        annotations: vec![],
        viewport: None,
    };

    let mut sub_node = DiagramNode {
        id: "n_sub".into(),
        instance_name: "plant".into(),
        type_name: "Subsystem".into(),
        domain: "signal".into(),
        catalog_id: "blocks.subsystem".into(),
        position: Position { x: 200.0, y: 100.0 },
        rotation: 0.0,
        params: vec![],
        ports: vec![signal_port("In1", "in"), signal_port("Out1", "out")],
        node_kind: "subsystem".into(),
        subsystem_ref: Some("sub_child".into()),
        plot_hints: vec![],
        ui: None,
    };
    // Mark active variant id unused; inactive sibling not present.
    sub_node.ui = Some(crate::diagram::NodeUi {
        label: Some("plant".into()),
        width: Some(140.0),
        height: Some(64.0),
        port_bindings: None,
        fmu_asset: None,
        fmu_guid: None,
        fmu_model_identifier: None,
        icon: None,
        label_below: None,
        mo_asset: None,
        variants: Some(vec![
            crate::diagram::VariantChoice {
                id: "v_a".into(),
                label: "A".into(),
                subsystem_ref: "sub_child".into(),
            },
            crate::diagram::VariantChoice {
                id: "v_b".into(),
                label: "B".into(),
                subsystem_ref: "sub_alt".into(),
            },
        ]),
        active_variant_id: Some("v_a".into()),
        dashboard_pins: None,
    });

    let alt = DiagramDocument {
        schema_version: 1,
        model_name: "Alt".into(),
        diagram_id: "sub_alt".into(),
        parent_diagram_id: Some("root".into()),
        nodes: vec![node(
            "n_alt_gain",
            "altGain",
            "Modelica.Blocks.Math.Gain",
            "blocks.math.gain",
            vec![signal_port("u", "in"), signal_port("y", "out")],
            vec![ParamValue {
                name: "k".into(),
                value: "99".into(),
            }],
        )],
        edges: vec![],
        annotations: vec![],
        viewport: None,
    };

    let root = DiagramDocument {
        schema_version: 1,
        model_name: "GateSubsystem".into(),
        diagram_id: "root".into(),
        parent_diagram_id: None,
        nodes: vec![
            node(
                "n_step",
                "step1",
                "Modelica.Blocks.Sources.Step",
                "blocks.sources.step",
                vec![signal_port("y", "out")],
                vec![
                    ParamValue {
                        name: "height".into(),
                        value: "1".into(),
                    },
                    ParamValue {
                        name: "startTime".into(),
                        value: "0".into(),
                    },
                ],
            ),
            sub_node,
        ],
        edges: vec![edge("e_root", "n_step", "y", "n_sub", "In1")],
        annotations: vec![],
        viewport: None,
    };

    let mut subs = HashMap::new();
    subs.insert("sub_child".into(), child);
    subs.insert("sub_alt".into(), alt);
    (root, subs)
}

fn fixture_goto_from() -> (DiagramDocument, HashMap<String, DiagramDocument>) {
    let root = DiagramDocument {
        schema_version: 1,
        model_name: "GateGoto".into(),
        diagram_id: "root".into(),
        parent_diagram_id: None,
        nodes: vec![
            node(
                "n_step",
                "step1",
                "Modelica.Blocks.Sources.Step",
                "blocks.sources.step",
                vec![signal_port("y", "out")],
                vec![ParamValue {
                    name: "height".into(),
                    value: "1".into(),
                }],
            ),
            node(
                "n_goto",
                "goto1",
                "Modelica.Blocks.Routing.RealPassThrough",
                "blocks.routing.goto",
                vec![signal_port("u", "in"), signal_port("y", "out")],
                vec![ParamValue {
                    name: "tag".into(),
                    value: "sigA".into(),
                }],
            ),
            node(
                "n_from",
                "from1",
                "Modelica.Blocks.Routing.RealPassThrough",
                "blocks.routing.from",
                vec![signal_port("u", "in"), signal_port("y", "out")],
                vec![ParamValue {
                    name: "tag".into(),
                    value: "sigA".into(),
                }],
            ),
            node(
                "n_gain",
                "gain1",
                "Modelica.Blocks.Math.Gain",
                "blocks.math.gain",
                vec![signal_port("u", "in"), signal_port("y", "out")],
                vec![ParamValue {
                    name: "k".into(),
                    value: "1".into(),
                }],
            ),
        ],
        edges: vec![
            edge("e1", "n_step", "y", "n_goto", "u"),
            edge("e2", "n_from", "y", "n_gain", "u"),
        ],
        annotations: vec![],
        viewport: None,
    };
    (root, HashMap::new())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoldenCheck {
    id: String,
    ok: bool,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoldenReport {
    schema_version: u32,
    checks: Vec<GoldenCheck>,
    passed: usize,
    failed: usize,
}

fn require_contains(mo: &str, needle: &str, id: &str) -> GoldenCheck {
    if mo.contains(needle) {
        GoldenCheck {
            id: id.into(),
            ok: true,
            detail: format!("contains `{needle}`"),
        }
    } else {
        GoldenCheck {
            id: id.into(),
            ok: false,
            detail: format!("missing `{needle}`"),
        }
    }
}

fn require_absent(mo: &str, needle: &str, id: &str) -> GoldenCheck {
    if !mo.contains(needle) {
        GoldenCheck {
            id: id.into(),
            ok: true,
            detail: format!("absent `{needle}`"),
        }
    } else {
        GoldenCheck {
            id: id.into(),
            ok: false,
            detail: format!("should not contain `{needle}`"),
        }
    }
}

pub fn run_emit_golden_checks(out_path: &Path) -> Result<GoldenReport, String> {
    let mut checks = Vec::new();

    let (root, subs) = fixture_subsystem();
    let mo = emit_modelica(&root, &subs, None);
    // Active child gain inlined with plant_ prefix; inactive altGain must not appear.
    checks.push(require_contains(&mo, "plant_gain1", "P523.subsystem_prefix"));
    checks.push(require_contains(&mo, "Modelica.Blocks.Math.Gain", "P523.gain_type"));
    checks.push(require_absent(&mo, "altGain", "P524.variant_inactive_absent"));
    checks.push(require_absent(&mo, "k = 99", "P524.variant_inactive_param"));

    // Persist emitted text for inspection.
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let _ = fs::write(parent.join("emit_golden_subsystem.mo"), &mo);
    }

    let (root2, subs2) = fixture_goto_from();
    let mo2 = emit_modelica(&root2, &subs2, None);
    checks.push(require_contains(
        &mo2,
        "connect(goto1.y, from1.u)",
        "P524.goto_from_connect",
    ));
    if let Some(parent) = out_path.parent() {
        let _ = fs::write(parent.join("emit_golden_goto.mo"), &mo2);
    }

    let passed = checks.iter().filter(|c| c.ok).count();
    let failed = checks.len() - passed;
    let report = GoldenReport {
        schema_version: 1,
        checks,
        passed,
        failed,
    };
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    fs::write(out_path, json).map_err(|e| e.to_string())?;
    Ok(report)
}

pub fn try_run_emit_golden_cli() -> Option<std::process::ExitCode> {
    let mut out: Option<String> = None;
    for a in std::env::args() {
        if let Some(p) = a.strip_prefix("--check-emit-golden=") {
            out = Some(p.to_string());
        }
    }
    let Some(path) = out else {
        return None;
    };
    match run_emit_golden_checks(Path::new(&path)) {
        Ok(rep) => {
            eprintln!(
                "emit golden: passed={} failed={} -> {path}",
                rep.passed, rep.failed
            );
            if rep.failed > 0 {
                Some(std::process::ExitCode::from(2))
            } else {
                Some(std::process::ExitCode::SUCCESS)
            }
        }
        Err(e) => {
            eprintln!("check-emit-golden failed: {e}");
            Some(std::process::ExitCode::from(1))
        }
    }
}
