//! Tight-coupled fixed-step DAE/FMU co-integration.
//!
//! One outer communication loop:
//!   1) Read plant sensor/signal outputs
//!   2) FMU setReal + doStep(dt)
//!   3) Write FMU outputs into plant bridge parameters (Constant.k)
//!   4) Advance plant one RK4 step of size dt with updated params
//!
//! Plant is compiled once; FMU and DAE share the same communication grid.

use super::cosim::CosimControl;
use super::host::FmuInstance;
use super::hybrid::{emit_plant, err_report, plant_with_constant_bridges};
use super::parse::{port_id_from_name, read_description_from_asset};
use crate::diagram::{DiagramDocument, DiagramNode, StudioSimOptions, StudioReport};
use crate::sim_bridge;
use rustmodlica::{Artifacts, SimulationResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct FmuBind {
    node: DiagramNode,
    fmu: FmuInstance,
    input_vr: HashMap<String, u32>,
    output_vr: HashMap<String, u32>,
    /// FMU input port -> plant output variable name
    input_from_output: HashMap<String, String>,
    /// FMU output port -> index in artifacts.params (bridge Constant.k)
    output_param_idx: HashMap<String, usize>,
}

fn resolve_asset(node: &DiagramNode, project_root: Option<&Path>) -> Result<PathBuf, String> {
    let asset = node
        .ui
        .as_ref()
        .and_then(|u| u.fmu_asset.as_ref())
        .ok_or_else(|| format!("FMU {} missing asset", node.instance_name))?;
    let p = PathBuf::from(asset);
    if p.is_absolute() && p.exists() {
        return Ok(p);
    }
    if let Some(root) = project_root {
        let j = root.join(asset);
        if j.exists() {
            return Ok(j);
        }
        let alt = root.join("imported_fmus").join(asset);
        if alt.exists() {
            return Ok(alt);
        }
    }
    if p.exists() {
        return Ok(p);
    }
    Err(format!("FMU asset not found: {asset}"))
}

fn find_param_index(param_vars: &[String], bridge_inst: &str) -> Option<usize> {
    let candidates = [
        format!("{bridge_inst}.k"),
        format!("{bridge_inst}_k"),
        bridge_inst.to_string(),
    ];
    for (i, name) in param_vars.iter().enumerate() {
        for c in &candidates {
            if name == c || name.ends_with(&format!(".{c}")) {
                return Some(i);
            }
        }
        if name.contains(bridge_inst) && (name.ends_with(".k") || name.ends_with("_k")) {
            return Some(i);
        }
    }
    None
}

fn find_output_index(output_vars: &[String], key: &str) -> Option<usize> {
    output_vars.iter().position(|n| n == key).or_else(|| {
        output_vars
            .iter()
            .position(|n| n.ends_with(key) || n.ends_with(&format!(".{key}")))
    })
}

unsafe fn call_derivs(
    a: &Artifacts,
    time: f64,
    states: &mut [f64],
    discrete: &mut [f64],
    derivs: &mut [f64],
    params: &[f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    pre_states: &[f64],
    pre_discrete: &[f64],
) -> Result<(), String> {
    let mut diag_residual = 0.0_f64;
    let mut diag_x = 0.0_f64;
    let homotopy = 1.0_f64;
    let status = (a.calc_derivs)(
        time,
        states.as_mut_ptr(),
        discrete.as_mut_ptr(),
        derivs.as_mut_ptr(),
        params.as_ptr(),
        outputs.as_mut_ptr(),
        when_states.as_mut_ptr(),
        crossings.as_mut_ptr(),
        pre_states.as_ptr(),
        pre_discrete.as_ptr(),
        a.t_end,
        &mut diag_residual,
        &mut diag_x,
        &homotopy,
    );
    if status != 0 {
        return Err(format!("calc_derivs status={status} at t={time}"));
    }
    Ok(())
}

fn rk4_step(
    a: &Artifacts,
    time: f64,
    dt: f64,
    states: &mut [f64],
    discrete: &mut [f64],
    params: &[f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
) -> Result<(), String> {
    let n = states.len();
    if n == 0 {
        // Still evaluate outputs for algebraic models.
        let mut derivs = Vec::new();
        let pre_s = states.to_vec();
        let pre_d = discrete.to_vec();
        unsafe {
            call_derivs(
                a,
                time,
                states,
                discrete,
                &mut derivs,
                params,
                outputs,
                when_states,
                crossings,
                &pre_s,
                &pre_d,
            )?;
        }
        return Ok(());
    }
    let mut k1 = vec![0.0; n];
    let mut k2 = vec![0.0; n];
    let mut k3 = vec![0.0; n];
    let mut k4 = vec![0.0; n];
    let mut tmp = vec![0.0; n];
    let pre_s = states.to_vec();
    let pre_d = discrete.to_vec();

    unsafe {
        call_derivs(
            a, time, states, discrete, &mut k1, params, outputs, when_states, crossings, &pre_s,
            &pre_d,
        )?;
        for i in 0..n {
            tmp[i] = states[i] + 0.5 * dt * k1[i];
        }
        call_derivs(
            a,
            time + 0.5 * dt,
            &mut tmp,
            discrete,
            &mut k2,
            params,
            outputs,
            when_states,
            crossings,
            &pre_s,
            &pre_d,
        )?;
        for i in 0..n {
            tmp[i] = states[i] + 0.5 * dt * k2[i];
        }
        call_derivs(
            a,
            time + 0.5 * dt,
            &mut tmp,
            discrete,
            &mut k3,
            params,
            outputs,
            when_states,
            crossings,
            &pre_s,
            &pre_d,
        )?;
        for i in 0..n {
            tmp[i] = states[i] + dt * k3[i];
        }
        call_derivs(
            a,
            time + dt,
            &mut tmp,
            discrete,
            &mut k4,
            params,
            outputs,
            when_states,
            crossings,
            &pre_s,
            &pre_d,
        )?;
        for i in 0..n {
            states[i] += (dt / 6.0) * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
        // Refresh outputs at end of step
        call_derivs(
            a,
            time + dt,
            states,
            discrete,
            &mut k1,
            params,
            outputs,
            when_states,
            crossings,
            &pre_s,
            &pre_d,
        )?;
    }
    Ok(())
}

fn setup_fmu_binds(
    diagram: &DiagramDocument,
    project_root: Option<&Path>,
    param_vars: &[String],
) -> Result<Vec<FmuBind>, String> {
    let mut binds = Vec::new();
    for node in &diagram.nodes {
        if node.node_kind != "fmu" {
            continue;
        }
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
        let mut input_from_output = HashMap::new();
        for e in &diagram.edges {
            if e.target_node_id != node.id {
                continue;
            }
            if let Some(src) = diagram.nodes.iter().find(|n| n.id == e.source_node_id) {
                input_from_output.insert(
                    e.target_port_id.clone(),
                    format!("{}.{}", src.instance_name, e.source_port_id),
                );
            }
        }
        let mut output_param_idx = HashMap::new();
        for p in &node.ports {
            if p.direction != "out" {
                continue;
            }
            let bridge_inst = format!("sfmu_{}_{}", node.instance_name, p.id);
            if let Some(idx) = find_param_index(param_vars, &bridge_inst) {
                output_param_idx.insert(p.id.clone(), idx);
            }
        }
        if output_param_idx.is_empty() && !output_vr.is_empty() {
            return Err(format!(
                "no plant params matched FMU {} bridges (expected sfmu_{}_*.k in {:?})",
                node.instance_name, node.instance_name, param_vars
            ));
        }
        binds.push(FmuBind {
            node: node.clone(),
            fmu,
            input_vr,
            output_vr,
            input_from_output,
            output_param_idx,
        });
    }
    Ok(binds)
}

/// Fixed-step co-integration of Modelica plant DAE + FMI2 CS FMUs.
pub fn simulate_tight(
    diagram: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    project_root: Option<&Path>,
    library_paths: &[String],
    sim: &StudioSimOptions,
    control: Option<&CosimControl>,
) -> Result<SimulationResult, StudioReport> {
    let intervals = sim.number_of_intervals.max(1);
    let dt = sim.stop_time / f64::from(intervals);
    let n_steps = intervals as usize;

    let plant = plant_with_constant_bridges(diagram, &HashMap::new());
    let code = emit_plant(&plant, subsystems, sim, "");
    let artifacts = sim_bridge::compile_artifacts(&code, &diagram.model_name, library_paths, sim)
        .map_err(|mut r| {
            r.stage = "fmu_tight_compile".into();
            r
        })?;

    let mut binds = setup_fmu_binds(diagram, project_root, &artifacts.param_vars)
        .map_err(|e| err_report("fmu_tight", e))?;
    if binds.is_empty() {
        return Err(err_report("fmu_tight", "no FMU nodes".into()));
    }

    for rt in &mut binds {
        rt.fmu
            .setup(0.0, sim.stop_time, sim.tolerance)
            .map_err(|e| err_report("fmu_tight", e))?;
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
            .map_err(|e| err_report("fmu_tight", e))?;
        rt.fmu
            .exit_initialization()
            .map_err(|e| err_report("fmu_tight", e))?;
    }

    let mut states = artifacts.states.clone();
    let mut discrete = artifacts.discrete_vals.clone();
    let mut params = artifacts.params.clone();
    let mut outputs = if artifacts.output_start_vals.len() == artifacts.output_vars.len() {
        artifacts.output_start_vals.clone()
    } else {
        vec![0.0; artifacts.output_vars.len()]
    };
    let mut when_states = vec![0.0; artifacts.when_count.max(1) * 2];
    let mut crossings = vec![0.0; artifacts.crossings_count.max(1)];

    let mut time = Vec::with_capacity(n_steps + 1);
    let mut series: HashMap<String, Vec<f64>> = HashMap::new();
    for name in artifacts
        .state_vars
        .iter()
        .chain(artifacts.discrete_vars.iter())
        .chain(artifacts.output_vars.iter())
    {
        series.insert(name.clone(), Vec::with_capacity(n_steps + 1));
    }
    for rt in &binds {
        for pid in rt.output_vr.keys() {
            series.insert(
                format!("{}.{}", rt.node.instance_name, pid),
                Vec::with_capacity(n_steps + 1),
            );
        }
    }

    let record = |t: f64,
                  time: &mut Vec<f64>,
                  series: &mut HashMap<String, Vec<f64>>,
                  states: &[f64],
                  discrete: &[f64],
                  outputs: &[f64],
                  artifacts: &Artifacts,
                  binds: &[FmuBind]| {
        time.push(t);
        for (i, name) in artifacts.state_vars.iter().enumerate() {
            if let Some(v) = series.get_mut(name) {
                v.push(states.get(i).copied().unwrap_or(0.0));
            }
        }
        for (i, name) in artifacts.discrete_vars.iter().enumerate() {
            if let Some(v) = series.get_mut(name) {
                v.push(discrete.get(i).copied().unwrap_or(0.0));
            }
        }
        for (i, name) in artifacts.output_vars.iter().enumerate() {
            if let Some(v) = series.get_mut(name) {
                v.push(outputs.get(i).copied().unwrap_or(0.0));
            }
        }
        for rt in binds {
            // FMU outputs already pushed into params/series below by caller
            let _ = rt;
        }
    };

    // Initial eval
    {
        let mut derivs = vec![0.0; states.len()];
        let pre_s = states.clone();
        let pre_d = discrete.clone();
        unsafe {
            call_derivs(
                &artifacts,
                0.0,
                &mut states,
                &mut discrete,
                &mut derivs,
                &params,
                &mut outputs,
                &mut when_states,
                &mut crossings,
                &pre_s,
                &pre_d,
            )
            .map_err(|e| err_report("fmu_tight", e))?;
        }
    }
    record(
        0.0,
        &mut time,
        &mut series,
        &states,
        &discrete,
        &outputs,
        &artifacts,
        &binds,
    );
    // initial FMU outputs
    for rt in &binds {
        if let Ok(outs) = rt.fmu.get_outputs() {
            for (vr, val) in outs {
                if let Some((pid, _)) = rt.output_vr.iter().find(|(_, v)| **v == vr) {
                    let key = format!("{}.{}", rt.node.instance_name, pid);
                    if let Some(v) = series.get_mut(&key) {
                        v.push(val);
                    }
                    if let Some(&idx) = rt.output_param_idx.get(pid) {
                        if let Some(p) = params.get_mut(idx) {
                            *p = val;
                        }
                    }
                }
            }
        } else {
            for pid in rt.output_vr.keys() {
                let key = format!("{}.{}", rt.node.instance_name, pid);
                if let Some(v) = series.get_mut(&key) {
                    v.push(0.0);
                }
            }
        }
    }

    for step in 0..n_steps {
        if let Some(c) = control {
            c.poll(step as u64, n_steps as u64)
                .map_err(|e| err_report("fmu_tight", e))?;
        }
        let t = step as f64 * dt;

        // 1-2) Plant -> FMU -> doStep
        for rt in &mut binds {
            let mut pairs = Vec::new();
            for (pid, vr) in &rt.input_vr {
                let val = rt
                    .input_from_output
                    .get(pid)
                    .and_then(|key| find_output_index(&artifacts.output_vars, key))
                    .and_then(|i| outputs.get(i).copied())
                    .unwrap_or(0.0);
                pairs.push((*vr, val));
            }
            rt.fmu
                .set_inputs(&pairs)
                .map_err(|e| err_report("fmu_tight", e))?;
            rt.fmu
                .do_step(t, dt)
                .map_err(|e| err_report("fmu_tight", e))?;
            let outs = rt
                .fmu
                .get_outputs()
                .map_err(|e| err_report("fmu_tight", e))?;
            for (vr, val) in outs {
                if let Some((pid, _)) = rt.output_vr.iter().find(|(_, v)| **v == vr) {
                    if let Some(&idx) = rt.output_param_idx.get(pid) {
                        if let Some(p) = params.get_mut(idx) {
                            *p = val;
                        }
                    }
                }
            }
        }

        // 3-4) Integrate plant one fixed step with updated FMU params
        rk4_step(
            &artifacts,
            t,
            dt,
            &mut states,
            &mut discrete,
            &params,
            &mut outputs,
            &mut when_states,
            &mut crossings,
        )
        .map_err(|e| err_report("fmu_tight", e))?;

        let t_next = t + dt;
        record(
            t_next,
            &mut time,
            &mut series,
            &states,
            &discrete,
            &outputs,
            &artifacts,
            &binds,
        );
        for rt in &binds {
            for (pid, &idx) in &rt.output_param_idx {
                let key = format!("{}.{}", rt.node.instance_name, pid);
                let val = params.get(idx).copied().unwrap_or(0.0);
                if let Some(v) = series.get_mut(&key) {
                    v.push(val);
                }
            }
            // Also record outputs that may not have param bridge (still got from FMU)
            for pid in rt.output_vr.keys() {
                let key = format!("{}.{}", rt.node.instance_name, pid);
                if let Some(v) = series.get_mut(&key) {
                    if v.len() < time.len() {
                        let val = rt
                            .output_param_idx
                            .get(pid)
                            .and_then(|i| params.get(*i).copied())
                            .unwrap_or(0.0);
                        v.push(val);
                    }
                }
            }
        }
    }

    // Align series lengths
    let n = time.len();
    series.retain(|_, v| {
        if v.len() > n {
            v.truncate(n);
        }
        while v.len() < n {
            let last = v.last().copied().unwrap_or(0.0);
            v.push(last);
        }
        true
    });

    Ok(SimulationResult { time, series })
}
