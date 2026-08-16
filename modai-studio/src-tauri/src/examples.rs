use crate::diagram::OpenProjectResult;
use crate::project::open_project;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleInfo {
    pub id: String,
    pub title: String,
    pub model_name: String,
    pub domain: String,
    /// When true, smoke / GUI Run is expected to complete a short simulation.
    pub can_simulate: bool,
    pub root_dir: String,
}

pub fn examples_root() -> PathBuf {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples"));
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().take(6) {
            candidates.push(ancestor.join("examples"));
            candidates.push(ancestor.join("modai-studio/examples"));
            candidates.push(ancestor.join("resources/examples"));
        }
    }
    candidates.push(PathBuf::from("modai-studio/examples"));
    candidates.push(PathBuf::from("examples"));
    for c in candidates {
        if c.join("signal_feedback/studio-project.json").is_file() {
            return c;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples")
}

fn defs() -> &'static [(&'static str, &'static str, &'static str, bool)] {
    &[
        ("signal_feedback", "Signal Feedback", "signal", true),
        ("first_order_step", "First-Order Step", "signal", true),
        ("pid_plant", "PID Plant", "signal", true),
        ("tf_plant", "Transfer Function", "signal", true),
        ("limiter_integrator", "Limiter Integrator", "signal", true),
        ("switch_select", "Switch Select", "signal", true),
        ("goto_from", "Goto / From", "signal", true),
        ("rate_transition", "Rate Transition", "signal", true),
        ("stategraph_first", "StateGraph First", "signal", true),
        ("rc_circuit", "RC Circuit", "electrical", true),
        ("rl_circuit", "RL Circuit", "electrical", true),
        ("rlc_series", "Series RLC", "electrical", true),
        ("elec_sensor_control", "Electrical + Sensors", "electrical", true),
        ("rot_drive", "Rotational Drive", "mechanical", true),
        ("rot_gear_train", "Gear Train", "mechanical", true),
        ("rot_spring_damper", "Rotational SpringDamper", "mechanical", true),
        ("rot_sensor_control", "Rotational + Sensors", "mechanical", true),
        ("trans_mass_force", "Mass-Damper Force", "mechanical", true),
        ("trans_spring_damper", "Translational SpringDamper", "mechanical", true),
        ("trans_sensor_control", "Translational + Sensors", "mechanical", true),
        ("multibody_preview", "Multibody Preview", "multibody", true),
    ]
}

pub fn resolve_example_dir(id: &str) -> Result<PathBuf, String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("empty example id".into());
    }
    if !defs().iter().any(|(d, ..)| *d == trimmed) {
        return Err(format!("unknown example id: {trimmed}"));
    }
    let root = examples_root().join(trimmed);
    if !root.join("studio-project.json").is_file() {
        return Err(format!("example project missing: {}", root.display()));
    }
    Ok(root)
}

pub fn list_examples() -> Vec<ExampleInfo> {
    defs()
        .iter()
        .filter_map(|(id, title, domain, can_sim)| {
            let root = examples_root().join(id);
            if !root.join("studio-project.json").is_file() {
                return None;
            }
            let opened = open_project(&root).ok();
            let model_name = opened
                .as_ref()
                .map(|o| o.manifest.model_name.clone())
                .unwrap_or_else(|| id.to_string());
            Some(ExampleInfo {
                id: (*id).to_string(),
                title: (*title).to_string(),
                model_name,
                domain: (*domain).to_string(),
                can_simulate: *can_sim,
                root_dir: root.display().to_string(),
            })
        })
        .collect()
}

pub fn open_example(id: &str) -> Result<OpenProjectResult, String> {
    let root = resolve_example_dir(id)?;
    open_project(&root)
}
