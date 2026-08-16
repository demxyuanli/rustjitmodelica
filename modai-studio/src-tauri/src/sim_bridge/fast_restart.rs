//! Fast Restart: reuse compiled `CompiledModel` when only parameters / sim options change.
//!
//! A dedicated OS thread owns JIT keepalives (Artifacts are not Send across threads).

use crate::diagram::{
    DiagramDocument, DiagramNode, SimProfile, StudioDiagnostic, StudioReport, StudioSimOptions,
};
use rustmodlica::{run_simulation_collect, CompiledModel, SimControl, SimulationResult};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use super::{build_compiler, options_from_sim};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartMode {
    FullCompile,
    FastRestart,
}

#[derive(Debug)]
pub struct SimulateOutcome {
    pub result: SimulationResult,
    pub mode: RestartMode,
    pub profile: crate::diagram::SimProfile,
}

struct Session {
    structure_key: u64,
    model: CompiledModel,
    baseline_params: Vec<f64>,
    library_paths: Vec<String>,
}

enum Job {
    Simulate {
        structure_key: u64,
        code: String,
        model_name: String,
        library_paths: Vec<String>,
        sim: StudioSimOptions,
        param_overrides: HashMap<String, f64>,
        enable_fast: bool,
        control: Option<SimControl>,
        reply: Sender<Result<SimulateOutcome, StudioReport>>,
    },
    Invalidate {
        reply: Sender<()>,
    },
}

static WORKER_TX: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();

fn worker_tx() -> Sender<Job> {
    WORKER_TX
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel::<Job>();
            std::thread::Builder::new()
                .name("studio-fast-restart".into())
                .spawn(move || worker_loop(rx))
                .expect("spawn studio-fast-restart worker");
            Mutex::new(tx)
        })
        .lock()
        .expect("fast-restart worker mutex")
        .clone()
}

fn worker_loop(rx: Receiver<Job>) {
    let mut session: Option<Session> = None;
    while let Ok(job) = rx.recv() {
        match job {
            Job::Invalidate { reply } => {
                session = None;
                let _ = reply.send(());
            }
            Job::Simulate {
                structure_key,
                code,
                model_name,
                library_paths,
                sim,
                param_overrides,
                enable_fast,
                control,
                reply,
            } => {
                let out = run_on_worker(
                    &mut session,
                    structure_key,
                    &code,
                    &model_name,
                    &library_paths,
                    &sim,
                    &param_overrides,
                    enable_fast,
                    control,
                );
                let _ = reply.send(out);
            }
        }
    }
}

fn run_on_worker(
    session: &mut Option<Session>,
    structure_key: u64,
    code: &str,
    model_name: &str,
    library_paths: &[String],
    sim: &StudioSimOptions,
    param_overrides: &HashMap<String, f64>,
    enable_fast: bool,
    mut control: Option<SimControl>,
) -> Result<SimulateOutcome, StudioReport> {
    let reuse = enable_fast
        && session.as_ref().is_some_and(|s| {
            s.structure_key == structure_key
                && s.library_paths == library_paths
                && s.model.model_name() == model_name
        });

    let mode = if reuse {
        RestartMode::FastRestart
    } else {
        RestartMode::FullCompile
    };

    let mut profile = SimProfile {
        compile_skipped: reuse,
        restart_mode: Some(if reuse {
            "fastRestart".into()
        } else {
            "fullCompile".into()
        }),
        ..Default::default()
    };

    if !reuse {
        let t_compile = Instant::now();
        let opts = options_from_sim(sim);
        let mut compiler = build_compiler(library_paths, opts);
        // Prefer runtime params so Fast Restart can swap the vector safely.
        let _ = std::env::set_var("RUSTMODLICA_CONST_FOLD", "0");
        let model = match compiler.compile_from_source_to_model(model_name, code) {
            Ok(m) => m,
            Err(e) => {
                return Err(StudioReport {
                    success: false,
                    stage: "compile".into(),
                    errors: vec![StudioDiagnostic {
                        message: e.to_string(),
                        path: Some(format!("{model_name}.mo")),
                        line: None,
                        column: None,
                    }],
                    warnings: Vec::new(),
                });
            }
        };
        profile.compile_ms = t_compile.elapsed().as_millis() as u64;
        let baseline_params = model.params().to_vec();
        *session = Some(Session {
            structure_key,
            model,
            baseline_params,
            library_paths: library_paths.to_vec(),
        });
    }

    let t_apply = Instant::now();
    let sess = session
        .as_ref()
        .expect("session populated after compile");
    let new_params = merge_params(
        sess.model.param_vars(),
        &sess.baseline_params,
        param_overrides,
    );
    let mut artifacts = sess.model.with_params(new_params);
    apply_sim_options(&mut artifacts, sim);
    profile.apply_params_ms = t_apply.elapsed().as_millis() as u64;

    if let Some(ctrl) = control.as_mut() {
        let min_dt = artifacts
            .output_interval
            .max(artifacts.t_end / 200.0)
            .max(1e-9);
        ctrl.set_progress_min_dt(min_dt);
    }

    let lib_paths: Vec<std::path::PathBuf> =
        library_paths.iter().map(std::path::PathBuf::from).collect();
    let t_integrate = Instant::now();
    let result = run_simulation_collect(
        artifacts.calc_derivs,
        artifacts.when_count,
        artifacts.crossings_count,
        artifacts.states,
        artifacts.discrete_vals,
        artifacts.params,
        &artifacts.state_vars,
        &artifacts.discrete_vars,
        &artifacts.output_vars,
        &artifacts.output_start_vals,
        &artifacts.state_var_index,
        artifacts.t_end,
        artifacts.dt,
        artifacts.numeric_ode_jacobian,
        artifacts.symbolic_ode_jacobian.as_ref(),
        &artifacts.newton_tearing_var_names,
        artifacts.atol,
        artifacts.rtol,
        artifacts.differential_index,
        artifacts.ida_component_id.as_slice(),
        &artifacts.solver,
        artifacts.output_interval,
        &artifacts.clock_partition_schedule,
        None,
        model_name,
        lib_paths,
        control.as_mut(),
    )
    .map_err(|e| StudioReport {
        success: false,
        stage: "simulate".into(),
        errors: vec![StudioDiagnostic {
            message: e.to_string(),
            path: None,
            line: None,
            column: None,
        }],
        warnings: Vec::new(),
    })?;
    profile.integrate_ms = t_integrate.elapsed().as_millis() as u64;
    profile.finalize();

    Ok(SimulateOutcome {
        result,
        mode,
        profile,
    })
}

fn apply_sim_options(a: &mut rustmodlica::Artifacts, sim: &StudioSimOptions) {
    let intervals = sim.number_of_intervals.max(1);
    let output_interval = sim.stop_time / f64::from(intervals);
    a.t_end = sim.stop_time;
    a.output_interval = output_interval;
    a.dt = output_interval;
    a.atol = sim.tolerance;
    a.rtol = sim.tolerance;
    a.solver = sim.solver.clone();
}

fn merge_params(
    names: &[String],
    baseline: &[f64],
    overrides: &HashMap<String, f64>,
) -> Vec<f64> {
    let mut out = if baseline.len() == names.len() {
        baseline.to_vec()
    } else {
        vec![0.0; names.len()]
    };
    for (i, name) in names.iter().enumerate() {
        if let Some(v) = overrides.get(name) {
            out[i] = *v;
        }
    }
    out
}

/// Public entry: simulate with optional Fast Restart (blocks until done).
pub fn simulate_with_fast_restart(
    structure_key: u64,
    code: &str,
    model_name: &str,
    library_paths: &[String],
    sim: &StudioSimOptions,
    param_overrides: HashMap<String, f64>,
    enable_fast: bool,
    control: Option<SimControl>,
) -> Result<SimulateOutcome, StudioReport> {
    let (reply_tx, reply_rx) = mpsc::channel();
    worker_tx()
        .send(Job::Simulate {
            structure_key,
            code: code.to_string(),
            model_name: model_name.to_string(),
            library_paths: library_paths.to_vec(),
            sim: sim.clone(),
            param_overrides,
            enable_fast,
            control,
            reply: reply_tx,
        })
        .map_err(|e| StudioReport {
            success: false,
            stage: "fast_restart".into(),
            errors: vec![StudioDiagnostic {
                message: format!("fast-restart worker unavailable: {e}"),
                path: None,
                line: None,
                column: None,
            }],
            warnings: Vec::new(),
        })?;
    reply_rx.recv().map_err(|e| StudioReport {
        success: false,
        stage: "fast_restart".into(),
        errors: vec![StudioDiagnostic {
            message: format!("fast-restart worker disconnected: {e}"),
            path: None,
            line: None,
            column: None,
        }],
        warnings: Vec::new(),
    })?
}

pub fn invalidate_cache() {
    let (reply_tx, reply_rx) = mpsc::channel();
    if worker_tx()
        .send(Job::Invalidate { reply: reply_tx })
        .is_ok()
    {
        let _ = reply_rx.recv();
    }
}

/// Structural fingerprint: topology + types + param *names* (not values).
pub fn structure_key(
    model_name: &str,
    diagram: Option<&DiagramDocument>,
    subsystems: &HashMap<String, DiagramDocument>,
    library_paths: &[String],
    code_fallback: &str,
) -> u64 {
    let mut h = DefaultHasher::new();
    model_name.hash(&mut h);
    for p in library_paths {
        p.hash(&mut h);
    }
    if let Some(d) = diagram {
        hash_diagram(d, subsystems, &mut h);
    } else {
        // Without a diagram, any source change (incl. params) forces full compile.
        code_fallback.hash(&mut h);
    }
    h.finish()
}

fn hash_diagram(
    d: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    h: &mut DefaultHasher,
) {
    d.model_name.hash(h);
    d.diagram_id.hash(h);
    let mut nodes: Vec<&DiagramNode> = d.nodes.iter().collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    for n in nodes {
        n.id.hash(h);
        n.instance_name.hash(h);
        n.type_name.hash(h);
        n.node_kind.hash(h);
        n.subsystem_ref.hash(h);
        let mut pnames: Vec<&str> = n.params.iter().map(|p| p.name.as_str()).collect();
        pnames.sort_unstable();
        for name in pnames {
            name.hash(h);
        }
        for p in &n.ports {
            p.id.hash(h);
            p.kind.hash(h);
            p.direction.hash(h);
            p.type_name.hash(h);
        }
    }
    let mut edges: Vec<_> = d.edges.iter().collect();
    edges.sort_by(|a, b| a.id.cmp(&b.id));
    for e in edges {
        e.id.hash(h);
        e.source_node_id.hash(h);
        e.source_port_id.hash(h);
        e.target_node_id.hash(h);
        e.target_port_id.hash(h);
    }
    let mut sub_keys: Vec<_> = subsystems.keys().collect();
    sub_keys.sort();
    for k in sub_keys {
        k.hash(h);
        if let Some(child) = subsystems.get(k) {
            hash_diagram(child, subsystems, h);
        }
    }
}

/// Collect `instance.param -> f64` overrides from the diagram tree.
/// Dictionary resolves `$Name` references before parsing numbers.
pub fn param_overrides_from_diagram(
    diagram: Option<&DiagramDocument>,
    subsystems: &HashMap<String, DiagramDocument>,
) -> HashMap<String, f64> {
    param_overrides_from_diagram_with_dict(diagram, subsystems, &HashMap::new())
}

pub fn param_overrides_from_diagram_with_dict(
    diagram: Option<&DiagramDocument>,
    subsystems: &HashMap<String, DiagramDocument>,
    dict: &HashMap<String, String>,
) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    if let Some(d) = diagram {
        collect_params(d, "", subsystems, dict, &mut out);
    }
    out
}

fn collect_params(
    diagram: &DiagramDocument,
    prefix: &str,
    subsystems: &HashMap<String, DiagramDocument>,
    dict: &HashMap<String, String>,
    out: &mut HashMap<String, f64>,
) {
    for node in &diagram.nodes {
        if node.node_kind == "subsystem" {
            if let Some(ref_id) = &node.subsystem_ref {
                if let Some(child) = subsystems.get(ref_id) {
                    let child_prefix = if prefix.is_empty() {
                        format!("{}_", node.instance_name)
                    } else {
                        format!("{prefix}{}_", node.instance_name)
                    };
                    collect_params(child, &child_prefix, subsystems, dict, out);
                }
            }
            continue;
        }
        if node.node_kind != "component" && node.node_kind != "fmu" {
            continue;
        }
        let inst = format!("{prefix}{}", node.instance_name);
        for p in &node.params {
            let v = crate::emit_mo::resolve_dict_value(&p.value, dict);
            if v.is_empty() {
                continue;
            }
            if let Ok(num) = v.parse::<f64>() {
                out.insert(format!("{inst}.{}", p.name), num);
            }
        }
    }
}
