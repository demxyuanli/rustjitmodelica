//! Cosimulation bridge: signal-domain diagram interpreter + FMI2 CS nodes.

use super::host::FmuInstance;
use super::parse::{port_id_from_name, read_description_from_asset};
use crate::diagram::{
    DiagramDocument, DiagramNode, StudioDiagnostic, StudioReport, StudioSimOptions,
};
use rustmodlica::SimulationResult;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Pause/cancel handle for in-process FMU cosim loops (mirrors SimControl boundaries).
pub struct CosimControl {
    pub cancel: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
    pub step_once: Arc<AtomicBool>,
}

impl CosimControl {
    pub fn from_flags(
        cancel: Arc<AtomicBool>,
        pause: Arc<AtomicBool>,
        step_once: Arc<AtomicBool>,
    ) -> Self {
        Self {
            cancel,
            pause,
            step_once,
        }
    }

    pub fn poll(&self, step: u64, n_steps: u64) -> Result<(), String> {
        let _ = (step, n_steps);
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(rustmodlica::SIM_CANCELLED_MSG.to_string());
            }
            if !self.pause.load(Ordering::SeqCst) {
                break;
            }
            if self.step_once.swap(false, Ordering::SeqCst) {
                break;
            }
            thread::sleep(Duration::from_millis(15));
        }
        if self.cancel.load(Ordering::SeqCst) {
            return Err(rustmodlica::SIM_CANCELLED_MSG.to_string());
        }
        Ok(())
    }
}

pub fn diagram_has_fmu(diagram: &DiagramDocument) -> bool {
    diagram.nodes.iter().any(|n| n.node_kind == "fmu")
}

pub fn any_fmu(
    root: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
) -> bool {
    if diagram_has_fmu(root) {
        return true;
    }
    subsystems.values().any(diagram_has_fmu)
}

fn param_f64(node: &DiagramNode, name: &str, default: f64) -> f64 {
    node.params
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.parse().ok())
        .unwrap_or(default)
}

fn eval_source(node: &DiagramNode, t: f64) -> Option<f64> {
    let tn = node.type_name.as_str();
    if tn.ends_with(".Constant") || node.catalog_id.contains("constant") {
        return Some(param_f64(node, "k", 0.0));
    }
    if tn.ends_with(".Step") || node.catalog_id.contains("step") {
        let height = param_f64(node, "height", 1.0);
        let start = param_f64(node, "startTime", 0.0);
        return Some(if t >= start { height } else { 0.0 });
    }
    if tn.ends_with(".Sine") || node.catalog_id.contains("sine") {
        let amp = param_f64(node, "amplitude", 1.0);
        let f = param_f64(node, "f", 1.0);
        let start = param_f64(node, "startTime", 0.0);
        if t < start {
            return Some(0.0);
        }
        return Some(amp * (2.0 * std::f64::consts::PI * f * (t - start)).sin());
    }
    None
}

fn is_physical(node: &DiagramNode) -> bool {
    node.domain == "electrical" || node.domain == "mechanical"
}

struct FmuRuntime {
    instance_name: String,
    node_id: String,
    fmu: FmuInstance,
    /// port_id -> valueReference for inputs
    input_vr: HashMap<String, u32>,
    /// port_id -> valueReference for outputs
    output_vr: HashMap<String, u32>,
    /// param name -> valueReference
    param_vr: HashMap<String, u32>,
}

fn resolve_asset(node: &DiagramNode, project_root: Option<&Path>) -> Result<PathBuf, String> {
    let asset = node
        .ui
        .as_ref()
        .and_then(|u| u.fmu_asset.as_ref())
        .ok_or_else(|| format!("FMU node {} missing ui.fmuAsset", node.instance_name))?;
    let p = PathBuf::from(asset);
    if p.is_absolute() && p.exists() {
        return Ok(p);
    }
    if let Some(root) = project_root {
        let joined = root.join(asset);
        if joined.exists() {
            return Ok(joined);
        }
        // also try imported_fmus relative
        let alt = root.join("imported_fmus").join(asset);
        if alt.exists() {
            return Ok(alt);
        }
    }
    if p.exists() {
        return Ok(p);
    }
    Err(format!(
        "FMU asset not found for {}: {asset}",
        node.instance_name
    ))
}

fn load_fmu_runtime(
    node: &DiagramNode,
    project_root: Option<&Path>,
) -> Result<FmuRuntime, String> {
    let asset = resolve_asset(node, project_root)?;
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
    let mut param_vr = HashMap::new();
    for s in &fmu.parameters {
        param_vr.insert(port_id_from_name(&s.name), s.value_reference);
    }
    Ok(FmuRuntime {
        instance_name: node.instance_name.clone(),
        node_id: node.id.clone(),
        fmu,
        input_vr,
        output_vr,
        param_vr,
    })
}

/// Simulate a diagram that contains one or more FMU co-sim nodes.
/// Signal-domain: in-process interpreter + FMI2 CS.
/// Physical + FMU: tight fixed-step DAE/FMU co-integration (Jacobi fallback).
pub fn simulate_with_fmu(
    diagram: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    project_root: Option<&Path>,
    library_paths: &[String],
    sim: &StudioSimOptions,
    control: Option<&CosimControl>,
) -> Result<SimulationResult, StudioReport> {
    let err = |msg: String| StudioReport {
        success: false,
        stage: "fmu_cosim".into(),
        errors: vec![StudioDiagnostic {
            message: msg,
            path: None,
            line: None,
            column: None,
        }],
        warnings: Vec::new(),
    };

    if diagram.nodes.iter().any(is_physical) {
        // Prefer tight co-integration with the same CosimControl for Pause/Step.
        return match super::tight_cosim::simulate_tight(
            diagram,
            subsystems,
            project_root,
            library_paths,
            sim,
            control,
        ) {
            Ok(r) => Ok(r),
            Err(_) => super::hybrid::simulate_hybrid(
                diagram,
                subsystems,
                project_root,
                library_paths,
                sim,
            ),
        };
    }

    let mut fmus = Vec::new();
    for node in &diagram.nodes {
        if node.node_kind == "fmu" {
            match load_fmu_runtime(node, project_root) {
                Ok(rt) => fmus.push(rt),
                Err(e) => return Err(err(e)),
            }
        }
    }
    if fmus.is_empty() {
        return Err(err("no FMU nodes to cosimulate".into()));
    }

    let intervals = sim.number_of_intervals.max(1);
    let dt = sim.stop_time / f64::from(intervals);
    let n_steps = intervals as usize;

    // Integrator / FirstOrder state: node_id -> y
    let mut cont_state: HashMap<String, f64> = HashMap::new();
    for node in &diagram.nodes {
        if node.type_name.ends_with(".Integrator") || node.catalog_id.contains("integrator")
        {
            cont_state.insert(node.id.clone(), param_f64(node, "y_start", 0.0));
        }
        if node.type_name.ends_with(".FirstOrder") || node.catalog_id.contains("firstOrder")
        {
            cont_state.insert(node.id.clone(), 0.0);
        }
    }

    // Setup FMUs
    for rt in &fmus {
        if let Err(e) = rt.fmu.setup(0.0, sim.stop_time, sim.tolerance) {
            return Err(err(e));
        }
        let node = diagram
            .nodes
            .iter()
            .find(|n| n.id == rt.node_id)
            .unwrap();
        let mut pairs = Vec::new();
        for p in &node.params {
            if let Some(&vr) = rt.param_vr.get(&p.name) {
                if let Ok(v) = p.value.parse::<f64>() {
                    pairs.push((vr, v));
                }
            }
        }
        for s in &rt.fmu.parameters {
            let key = port_id_from_name(&s.name);
            if !node.params.iter().any(|p| p.name == key) {
                if let Some(start) = s.start {
                    pairs.push((s.value_reference, start));
                }
            }
        }
        if let Err(e) = rt.fmu.set_parameters_from_map(&pairs) {
            return Err(err(e));
        }
        if let Err(e) = rt.fmu.exit_initialization() {
            return Err(err(e));
        }
    }

    let mut port_vals: HashMap<(String, String), f64> = HashMap::new();
    let mut time = Vec::with_capacity(n_steps + 1);
    let mut series: HashMap<String, Vec<f64>> = HashMap::new();

    for rt in &fmus {
        for (pid, _) in &rt.output_vr {
            series.insert(
                format!("{}.{}", rt.instance_name, pid),
                Vec::with_capacity(n_steps + 1),
            );
        }
    }
    for node in &diagram.nodes {
        if node.node_kind == "fmu" {
            continue;
        }
        for p in &node.ports {
            if p.direction == "out" {
                series
                    .entry(format!("{}.{}", node.instance_name, p.id))
                    .or_insert_with(|| Vec::with_capacity(n_steps + 1));
            }
        }
    }

    let mut incoming: HashMap<(String, String), (String, String)> = HashMap::new();
    for e in &diagram.edges {
        incoming.insert(
            (e.target_node_id.clone(), e.target_port_id.clone()),
            (e.source_node_id.clone(), e.source_port_id.clone()),
        );
    }

    for step in 0..=n_steps {
        if let Some(c) = control {
            if let Err(e) = c.poll(step as u64, n_steps as u64) {
                return Err(err(e));
            }
        }
        let t = step as f64 * dt;
        time.push(t);

        for node in &diagram.nodes {
            if let Some(y) = eval_source(node, t) {
                for p in &node.ports {
                    if p.direction == "out" {
                        port_vals.insert((node.id.clone(), p.id.clone()), y);
                    }
                }
            }
        }

        for node in &diagram.nodes {
            if let Some(&y) = cont_state.get(&node.id) {
                for p in &node.ports {
                    if p.direction == "out" {
                        port_vals.insert((node.id.clone(), p.id.clone()), y);
                    }
                }
            }
        }

        for rt in &fmus {
            if step == 0 {
                if let Ok(outs) = rt.fmu.get_outputs() {
                    for (vr, val) in outs {
                        if let Some((pid, _)) = rt.output_vr.iter().find(|(_, v)| **v == vr) {
                            port_vals.insert((rt.node_id.clone(), pid.clone()), val);
                        }
                    }
                }
            }
        }

        for _ in 0..8 {
            for node in &diagram.nodes {
                let tn = node.type_name.as_str();
                if tn.ends_with(".Gain") || node.catalog_id.contains("gain") {
                    let u = incoming
                        .get(&(node.id.clone(), "u".into()))
                        .and_then(|(sid, sp)| port_vals.get(&(sid.clone(), sp.clone())))
                        .copied()
                        .unwrap_or(0.0);
                    let k = param_f64(node, "k", 1.0);
                    port_vals.insert((node.id.clone(), "y".into()), k * u);
                } else if tn.ends_with(".Add")
                    || node.catalog_id.contains(".add")
                    || node.catalog_id.ends_with("add")
                {
                    let u1 = incoming
                        .get(&(node.id.clone(), "u1".into()))
                        .and_then(|(sid, sp)| port_vals.get(&(sid.clone(), sp.clone())))
                        .copied()
                        .unwrap_or(0.0);
                    let u2 = incoming
                        .get(&(node.id.clone(), "u2".into()))
                        .and_then(|(sid, sp)| port_vals.get(&(sid.clone(), sp.clone())))
                        .copied()
                        .unwrap_or(0.0);
                    let k1 = param_f64(node, "k1", 1.0);
                    let k2 = param_f64(node, "k2", 1.0);
                    port_vals.insert((node.id.clone(), "y".into()), k1 * u1 + k2 * u2);
                } else if tn.ends_with(".Feedback") || node.catalog_id.contains("feedback") {
                    let u1 = incoming
                        .get(&(node.id.clone(), "u1".into()))
                        .and_then(|(sid, sp)| port_vals.get(&(sid.clone(), sp.clone())))
                        .copied()
                        .unwrap_or(0.0);
                    let u2 = incoming
                        .get(&(node.id.clone(), "u2".into()))
                        .and_then(|(sid, sp)| port_vals.get(&(sid.clone(), sp.clone())))
                        .copied()
                        .unwrap_or(0.0);
                    port_vals.insert((node.id.clone(), "y".into()), u1 - u2);
                }
            }
        }

        if step < n_steps {
            for rt in &fmus {
                let mut pairs = Vec::new();
                for (pid, vr) in &rt.input_vr {
                    let val = incoming
                        .get(&(rt.node_id.clone(), pid.clone()))
                        .and_then(|(sid, sp)| port_vals.get(&(sid.clone(), sp.clone())))
                        .copied()
                        .unwrap_or(0.0);
                    pairs.push((*vr, val));
                }
                if let Err(e) = rt.fmu.set_inputs(&pairs) {
                    return Err(err(e));
                }
                if let Err(e) = rt.fmu.do_step(t, dt) {
                    return Err(err(e));
                }
                match rt.fmu.get_outputs() {
                    Ok(outs) => {
                        for (vr, val) in outs {
                            if let Some((pid, _)) =
                                rt.output_vr.iter().find(|(_, v)| **v == vr)
                            {
                                port_vals.insert((rt.node_id.clone(), pid.clone()), val);
                            }
                        }
                    }
                    Err(e) => return Err(err(e)),
                }
            }

            for node in &diagram.nodes {
                if node.type_name.ends_with(".Integrator")
                    || node.catalog_id.contains("integrator")
                {
                    let u = incoming
                        .get(&(node.id.clone(), "u".into()))
                        .and_then(|(sid, sp)| port_vals.get(&(sid.clone(), sp.clone())))
                        .copied()
                        .unwrap_or(0.0);
                    let k = param_f64(node, "k", 1.0);
                    let y = cont_state.entry(node.id.clone()).or_insert(0.0);
                    *y += dt * k * u;
                }
                if node.type_name.ends_with(".FirstOrder")
                    || node.catalog_id.contains("firstOrder")
                {
                    let u = incoming
                        .get(&(node.id.clone(), "u".into()))
                        .and_then(|(sid, sp)| port_vals.get(&(sid.clone(), sp.clone())))
                        .copied()
                        .unwrap_or(0.0);
                    let k = param_f64(node, "k", 1.0);
                    let t_const = param_f64(node, "T", 1.0).max(1e-12);
                    let y = cont_state.entry(node.id.clone()).or_insert(0.0);
                    *y += dt * (k * u - *y) / t_const;
                }
            }
        }

        for node in &diagram.nodes {
            for p in &node.ports {
                if p.direction != "out" {
                    continue;
                }
                let key = format!("{}.{}", node.instance_name, p.id);
                let val = port_vals
                    .get(&(node.id.clone(), p.id.clone()))
                    .copied()
                    .unwrap_or(0.0);
                if let Some(v) = series.get_mut(&key) {
                    v.push(val);
                }
            }
        }
    }

    Ok(SimulationResult { time, series })
}
