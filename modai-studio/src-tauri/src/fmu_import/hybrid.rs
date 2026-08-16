//! Physical-domain + FMU hybrid cosimulation.
//!
//! Prefer [`super::tight_cosim::simulate_tight`] (fixed-step co-integration).
//! This module keeps the weak Jacobi two-pass path as fallback.

use super::host::FmuInstance;
use super::parse::{port_id_from_name, read_description_from_asset};
use crate::diagram::{
    DiagramDocument, DiagramEdge, DiagramNode, ParamValue, PortDef, Position, StudioDiagnostic,
    StudioReport, StudioSimOptions,
};
use crate::emit_mo::emit_modelica;
use crate::sim_bridge;
use rustmodlica::SimulationResult;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(crate) fn err_report(stage: &str, msg: String) -> StudioReport {
    StudioReport {
        success: false,
        stage: stage.into(),
        errors: vec![StudioDiagnostic {
            message: msg,
            path: None,
            line: None,
            column: None,
        }],
        warnings: Vec::new(),
    }
}

fn interpolate(times: &[f64], values: &[f64], t: f64) -> f64 {
    if times.is_empty() || values.is_empty() {
        return 0.0;
    }
    if t <= times[0] {
        return values[0];
    }
    let last = times.len() - 1;
    if t >= times[last] {
        return values[values.len().min(times.len()) - 1];
    }
    for i in 0..last {
        if t >= times[i] && t <= times[i + 1] {
            let dt = times[i + 1] - times[i];
            if dt.abs() < 1e-15 {
                return values[i];
            }
            let a = (t - times[i]) / dt;
            let v0 = values.get(i).copied().unwrap_or(0.0);
            let v1 = values.get(i + 1).copied().unwrap_or(v0);
            return v0 + a * (v1 - v0);
        }
    }
    values[last]
}

fn series_at(result: &SimulationResult, key: &str, t: f64) -> f64 {
    let Some(vals) = result.series.get(key) else {
        return 0.0;
    };
    interpolate(&result.time, vals, t)
}

/// Build a plant diagram where each FMU is replaced by Constant sources on its outputs.
pub(crate) fn plant_with_constant_bridges(
    diagram: &DiagramDocument,
    fmu_output_values: &HashMap<(String, String), f64>,
) -> DiagramDocument {
    let mut nodes = Vec::new();
    let mut id_map: HashMap<String, String> = HashMap::new(); // old fmu id -> not used
    let mut edge_rewrites: HashMap<(String, String), (String, String)> = HashMap::new();

    for node in &diagram.nodes {
        if node.node_kind != "fmu" {
            nodes.push(node.clone());
            continue;
        }
        id_map.insert(node.id.clone(), node.id.clone());
        for p in &node.ports {
            if p.direction != "out" {
                continue;
            }
            let bridge_id = format!("bridge_{}_{}", node.instance_name, p.id);
            let val = fmu_output_values
                .get(&(node.id.clone(), p.id.clone()))
                .copied()
                .unwrap_or(0.0);
            let bridge = DiagramNode {
                id: bridge_id.clone(),
                instance_name: format!("sfmu_{}_{}", node.instance_name, p.id),
                type_name: "Modelica.Blocks.Sources.Constant".into(),
                domain: "signal".into(),
                catalog_id: "blocks.sources.constant".into(),
                position: Position {
                    x: node.position.x + 20.0,
                    y: node.position.y,
                },
                rotation: 0.0,
                params: vec![ParamValue {
                    name: "k".into(),
                    value: format!("{val}"),
                }],
                ports: vec![PortDef {
                    id: "y".into(),
                    kind: "signal".into(),
                    direction: "out".into(),
                    type_name: "RealOutput".into(),
                }],
                node_kind: "component".into(),
                subsystem_ref: None,
                plot_hints: vec!["y".into()],
                ui: None,
            };
            edge_rewrites.insert((node.id.clone(), p.id.clone()), (bridge_id.clone(), "y".into()));
            nodes.push(bridge);
        }
        // FMU input edges are dropped (sensors still exist and are logged from plant).
    }

    let mut edges = Vec::new();
    for e in &diagram.edges {
        // Drop edges that target an FMU input
        if diagram
            .nodes
            .iter()
            .any(|n| n.id == e.target_node_id && n.node_kind == "fmu")
        {
            continue;
        }
        // Rewrite edges that source from FMU outputs
        if let Some((nid, pid)) = edge_rewrites.get(&(e.source_node_id.clone(), e.source_port_id.clone()))
        {
            edges.push(DiagramEdge {
                id: e.id.clone(),
                source_node_id: nid.clone(),
                source_port_id: pid.clone(),
                target_node_id: e.target_node_id.clone(),
                target_port_id: e.target_port_id.clone(),
                signal_name: e.signal_name.clone(),
                unit: e.unit.clone(),
                dimension: e.dimension.clone(),
                vertices: e.vertices.clone(),
                routing: e.routing.clone(),
                ui: e.ui.clone(),
            });
            continue;
        }
        // Drop edges from FMU that we couldn't rewrite
        if diagram
            .nodes
            .iter()
            .any(|n| n.id == e.source_node_id && n.node_kind == "fmu")
        {
            continue;
        }
        edges.push(e.clone());
    }

    DiagramDocument {
        schema_version: diagram.schema_version,
        model_name: diagram.model_name.clone(),
        diagram_id: diagram.diagram_id.clone(),
        parent_diagram_id: diagram.parent_diagram_id.clone(),
        nodes,
        edges,
        annotations: diagram.annotations.clone(),
        viewport: diagram.viewport.clone(),
    }
}

/// Emit piecewise RealExpression blocks for FMU output trajectories.
fn plant_with_piecewise_bridges(
    diagram: &DiagramDocument,
    trajectories: &HashMap<(String, String), Vec<(f64, f64)>>,
) -> (DiagramDocument, String) {
    let plant = plant_with_constant_bridges(diagram, &HashMap::new());
    // We'll replace Constant.k with a nested RealExpression via custom emit appendix.
    // Simpler: keep Constants and instead append a second model fragment — instead,
    // rewrite Constant nodes that came from bridges to use RealExpression y=.
    let mut nodes = Vec::new();
    let mut nested_blocks = String::new();

    for node in &plant.nodes {
        let is_bridge = node.id.starts_with("bridge_");
        if !is_bridge {
            nodes.push(node.clone());
            continue;
        }
        let mut traj: Option<&Vec<(f64, f64)>> = None;
        for ((fid, pid), series) in trajectories {
            let expected = format!(
                "bridge_{}_{}",
                diagram
                    .nodes
                    .iter()
                    .find(|n| n.id == *fid)
                    .map(|n| n.instance_name.as_str())
                    .unwrap_or(""),
                pid
            );
            if expected == node.id {
                traj = Some(series);
                break;
            }
        }
        let samples = traj.cloned().unwrap_or_else(|| vec![(0.0, 0.0)]);
        let expr = piecewise_expr(&samples);
        let block_ty = format!("StudioPw_{}", node.instance_name);
        nested_blocks.push_str(&format!(
            "  block {block_ty}\n    Modelica.Blocks.Interfaces.RealOutput y;\n  equation\n    y = {expr};\n  end {block_ty};\n"
        ));
        nodes.push(DiagramNode {
            id: node.id.clone(),
            instance_name: node.instance_name.clone(),
            type_name: block_ty.clone(),
            domain: "signal".into(),
            catalog_id: "studio.piecewise".into(),
            position: node.position.clone(),
            rotation: 0.0,
            params: Vec::new(),
            ports: vec![PortDef {
                id: "y".into(),
                kind: "signal".into(),
                direction: "out".into(),
                type_name: "RealOutput".into(),
            }],
            node_kind: "component".into(),
            subsystem_ref: None,
            plot_hints: vec!["y".into()],
            ui: None,
        });
    }

    let doc = DiagramDocument {
        nodes,
        ..plant
    };
    (doc, nested_blocks)
}

fn piecewise_expr(samples: &[(f64, f64)]) -> String {
    if samples.is_empty() {
        return "0".into();
    }
    // Downsample to at most 64 pieces for emit size.
    let step = (samples.len() / 64).max(1);
    let mut pts: Vec<(f64, f64)> = samples.iter().step_by(step).copied().collect();
    if let Some(last) = samples.last() {
        if pts.last() != Some(last) {
            pts.push(*last);
        }
    }
    let mut expr = format!("{}", pts[pts.len() - 1].1);
    for (t, v) in pts.iter().rev().skip(1) {
        expr = format!("if time < {t} then {v} else ({expr})");
    }
    expr
}

pub(crate) fn emit_plant(
    diagram: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    sim: &StudioSimOptions,
    nested_extra: &str,
) -> String {
    let base = emit_modelica(
        diagram,
        subsystems,
        Some(&crate::diagram::SimulationDefaults {
            stop_time: sim.stop_time,
            number_of_intervals: sim.number_of_intervals,
            solver: "rk4".into(),
            tolerance: sim.tolerance,
            max_step: None,
            min_step: None,
            logged_variables: Vec::new(),
        }),
    );
    if nested_extra.is_empty() {
        return base;
    }
    if let Some(pos) = base.find('\n') {
        let (head, rest) = base.split_at(pos + 1);
        format!("{head}{nested_extra}{rest}")
    } else {
        base
    }
}

struct FmuRt {
    node: DiagramNode,
    fmu: FmuInstance,
    input_vr: HashMap<String, u32>,
    output_vr: HashMap<String, u32>,
    /// Edges into this FMU: port_id -> (src_inst.out_port as series key prefix)
    input_series_keys: HashMap<String, String>,
}

fn load_fmus(
    diagram: &DiagramDocument,
    project_root: Option<&Path>,
) -> Result<Vec<FmuRt>, String> {
    let mut out = Vec::new();
    for node in &diagram.nodes {
        if node.node_kind != "fmu" {
            continue;
        }
        let asset = {
            let a = node
                .ui
                .as_ref()
                .and_then(|u| u.fmu_asset.as_ref())
                .ok_or_else(|| format!("FMU {} missing asset", node.instance_name))?;
            let p = PathBuf::from(a);
            if p.is_absolute() && p.exists() {
                p
            } else if let Some(root) = project_root {
                let j = root.join(a);
                if j.exists() {
                    j
                } else {
                    root.join("imported_fmus").join(a)
                }
            } else {
                p
            }
        };
        if !asset.exists() {
            return Err(format!("FMU asset missing: {}", asset.display()));
        }
        let desc = read_description_from_asset(&asset)?;
        let fmu = FmuInstance::load(&asset, desc)?;
        let mut input_vr = HashMap::new();
        for s in &fmu.inputs {
            input_vr.insert(port_id_from_name(&s.name), s.value_reference);
        }
        let mut output_vr = HashMap::new();
        for s in &fmu.outputs {
            output_vr.insert(port_id_from_name(&s.name), s.value_reference);
        }
        let mut input_series_keys = HashMap::new();
        for e in &diagram.edges {
            if e.target_node_id != node.id {
                continue;
            }
            if let Some(src) = diagram.nodes.iter().find(|n| n.id == e.source_node_id) {
                input_series_keys.insert(
                    e.target_port_id.clone(),
                    format!("{}.{}", src.instance_name, e.source_port_id),
                );
            }
        }
        // Apply parameters
        let mut pairs = Vec::new();
        for p in &node.params {
            let key = &p.name;
            if let Some(s) = fmu.parameters.iter().find(|s| port_id_from_name(&s.name) == *key)
            {
                if let Ok(v) = p.value.parse::<f64>() {
                    pairs.push((s.value_reference, v));
                }
            }
        }
        for s in &fmu.parameters {
            let key = port_id_from_name(&s.name);
            if !node.params.iter().any(|p| p.name == key) {
                if let Some(start) = s.start {
                    pairs.push((s.value_reference, start));
                }
            }
        }
        out.push(FmuRt {
            node: node.clone(),
            fmu,
            input_vr,
            output_vr,
            input_series_keys,
        });
        // store pairs temporarily via setup after
        let _ = pairs;
    }
    Ok(out)
}

fn map_err_str(e: String) -> StudioReport {
    err_report("fmu_hybrid", e)
}

fn simulate_hybrid_jacobi(
    diagram: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    project_root: Option<&Path>,
    library_paths: &[String],
    sim: &StudioSimOptions,
) -> Result<SimulationResult, StudioReport> {
    let intervals = sim.number_of_intervals.max(1);
    let dt = sim.stop_time / f64::from(intervals);

    let plant_a = plant_with_constant_bridges(diagram, &HashMap::new());
    let code_a = emit_plant(&plant_a, subsystems, sim, "");
    let result_a = sim_bridge::simulate_model(
        &code_a,
        &diagram.model_name,
        library_paths,
        sim,
        None,
    )
    .map_err(|mut r| {
        r.stage = "fmu_hybrid_plant_a".into();
        r
    })?;

    let mut fmus = load_fmus(diagram, project_root).map_err(map_err_str)?;
    if fmus.is_empty() {
        return Err(err_report("fmu_hybrid", "no FMU nodes".into()));
    }

    let mut trajectories: HashMap<(String, String), Vec<(f64, f64)>> = HashMap::new();
    let mut fmu_series: HashMap<String, Vec<f64>> = HashMap::new();

    for rt in &mut fmus {
        rt.fmu
            .setup(0.0, sim.stop_time, sim.tolerance)
            .map_err(map_err_str)?;
        let mut pairs = Vec::new();
        for p in &rt.node.params {
            if let Some(s) = rt
                .fmu
                .parameters
                .iter()
                .find(|s| port_id_from_name(&s.name) == p.name)
            {
                if let Ok(v) = p.value.parse::<f64>() {
                    pairs.push((s.value_reference, v));
                }
            }
        }
        for s in &rt.fmu.parameters {
            let key = port_id_from_name(&s.name);
            if !rt.node.params.iter().any(|p| p.name == key) {
                if let Some(start) = s.start {
                    pairs.push((s.value_reference, start));
                }
            }
        }
        rt.fmu
            .set_parameters_from_map(&pairs)
            .map_err(map_err_str)?;
        rt.fmu.exit_initialization().map_err(map_err_str)?;

        for pid in rt.output_vr.keys() {
            let key = format!("{}.{}", rt.node.instance_name, pid);
            fmu_series.insert(key, Vec::with_capacity(intervals as usize + 1));
            trajectories.insert((rt.node.id.clone(), pid.clone()), Vec::new());
        }
    }

    let n_steps = intervals as usize;
    for step in 0..=n_steps {
        let t = step as f64 * dt;
        for rt in &mut fmus {
            if step < n_steps {
                let mut pairs = Vec::new();
                for (pid, vr) in &rt.input_vr {
                    let val = rt
                        .input_series_keys
                        .get(pid)
                        .map(|k| series_at(&result_a, k, t))
                        .unwrap_or(0.0);
                    pairs.push((*vr, val));
                }
                rt.fmu.set_inputs(&pairs).map_err(map_err_str)?;
                rt.fmu.do_step(t, dt).map_err(map_err_str)?;
            }
            let outs = rt.fmu.get_outputs().map_err(map_err_str)?;
            for (vr, val) in outs {
                if let Some((pid, _)) = rt.output_vr.iter().find(|(_, v)| **v == vr) {
                    let key = format!("{}.{}", rt.node.instance_name, pid);
                    if let Some(v) = fmu_series.get_mut(&key) {
                        v.push(val);
                    }
                    if let Some(tr) = trajectories.get_mut(&(rt.node.id.clone(), pid.clone())) {
                        tr.push((t, val));
                    }
                }
            }
        }
    }

    let (plant_c, nested) = plant_with_piecewise_bridges(diagram, &trajectories);
    let code_c = emit_plant(&plant_c, subsystems, sim, &nested);
    let mut result_c = sim_bridge::simulate_model(
        &code_c,
        &diagram.model_name,
        library_paths,
        sim,
        None,
    )
    .map_err(|mut r| {
        r.stage = "fmu_hybrid_plant_c".into();
        r
    })?;

    for (k, vals) in fmu_series {
        if vals.len() == result_c.time.len() {
            result_c.series.insert(k, vals);
        } else if !vals.is_empty() {
            let mut aligned = Vec::with_capacity(result_c.time.len());
            let ft: Vec<f64> = (0..=n_steps).map(|i| i as f64 * dt).collect();
            for &t in &result_c.time {
                aligned.push(interpolate(&ft, &vals, t));
            }
            result_c.series.insert(k, aligned);
        }
    }

    Ok(result_c)
}

/// Jacobi fallback entry (tight path is tried by `simulate_with_fmu` first).
pub fn simulate_hybrid(
    diagram: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    project_root: Option<&Path>,
    library_paths: &[String],
    sim: &StudioSimOptions,
) -> Result<SimulationResult, StudioReport> {
    simulate_hybrid_jacobi(diagram, subsystems, project_root, library_paths, sim)
}

