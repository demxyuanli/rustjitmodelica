use crate::diagram::{
    DiagramDocument, OpenProjectResult, SimulationDefaults, StudioProjectManifest, StudioUiPrefs,
    Viewport,
};
use crate::emit_mo::emit_modelica_with_dict;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn empty_diagram(model_name: &str) -> DiagramDocument {
    DiagramDocument {
        schema_version: 1,
        model_name: model_name.to_string(),
        diagram_id: "root".to_string(),
        parent_diagram_id: None,
        nodes: Vec::new(),
        edges: Vec::new(),
        annotations: Vec::new(),
        viewport: Some(Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }),
    }
}

pub fn default_manifest(model_name: &str) -> StudioProjectManifest {
    StudioProjectManifest {
        schema_version: 1,
        product: "modai-studio".to_string(),
        model_name: model_name.to_string(),
        library_paths: Vec::new(),
        simulation_defaults: SimulationDefaults::default(),
        ui: StudioUiPrefs::default(),
        param_dictionary: Vec::new(),
    }
}

fn load_subsystems(root: &Path) -> HashMap<String, DiagramDocument> {
    let mut map = HashMap::new();
    let dir = root.join("subsystems");
    let Ok(entries) = fs::read_dir(&dir) else {
        return map;
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(doc) = serde_json::from_str::<DiagramDocument>(&text) else {
            continue;
        };
        map.insert(doc.diagram_id.clone(), doc);
    }
    map
}

fn write_subsystems(root: &Path, subsystems: &HashMap<String, DiagramDocument>) -> Result<(), String> {
    let dir = root.join("subsystems");
    if subsystems.is_empty() {
        if dir.exists() {
            // leave existing files; overwrite known ids below
        }
        return Ok(());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (id, doc) in subsystems {
        let path = dir.join(format!("{id}.diagram.json"));
        let json = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_fresh_project_dir(root: &Path) -> Result<(), String> {
    if root.is_file() {
        return Err(format!("not a directory: {}", root.display()));
    }
    if root.join("studio-project.json").is_file() {
        return Err(format!("already a Studio project: {}", root.display()));
    }
    if root.is_dir() {
        let mut entries = fs::read_dir(root).map_err(|e| e.to_string())?;
        if entries.next().is_some() {
            return Err(format!("folder is not empty: {}", root.display()));
        }
    }
    Ok(())
}

pub fn create_project(root: &Path, model_name: &str) -> Result<OpenProjectResult, String> {
    ensure_fresh_project_dir(root)?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let manifest = default_manifest(model_name);
    let diagram = empty_diagram(model_name);
    let subsystems = HashMap::new();
    let mo = emit_modelica_with_dict(
        &diagram,
        &subsystems,
        Some(&manifest.simulation_defaults),
        &manifest.param_dictionary,
        Some(root),
    );
    write_project(root, &manifest, &diagram, &subsystems, &mo)?;
    Ok(OpenProjectResult {
        root_dir: root.display().to_string(),
        manifest,
        diagram,
        model_source: mo,
        subsystems,
    })
}

pub fn open_project(root: &Path) -> Result<OpenProjectResult, String> {
    let manifest_path = root.join("studio-project.json");
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|e| format!("read studio-project.json: {e}"))?;
    let manifest: StudioProjectManifest =
        serde_json::from_str(&manifest_text).map_err(|e| format!("parse studio-project.json: {e}"))?;
    if manifest.product != "modai-studio" {
        return Err("not a modai-studio project".into());
    }
    let diagram_path = root.join(format!("{}.diagram.json", manifest.model_name));
    let diagram_text =
        fs::read_to_string(&diagram_path).map_err(|e| format!("read diagram: {e}"))?;
    let diagram: DiagramDocument =
        serde_json::from_str(&diagram_text).map_err(|e| format!("parse diagram: {e}"))?;
    let subsystems = load_subsystems(root);
    let mo_path = root.join(format!("{}.mo", manifest.model_name));
    let model_source = fs::read_to_string(&mo_path).unwrap_or_else(|_| {
        emit_modelica_with_dict(&diagram, &subsystems, Some(&manifest.simulation_defaults), &manifest.param_dictionary, Some(root))
    });
    Ok(OpenProjectResult {
        root_dir: root.display().to_string(),
        manifest,
        diagram,
        model_source,
        subsystems,
    })
}

pub fn write_project(
    root: &Path,
    manifest: &StudioProjectManifest,
    diagram: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
    model_source: &str,
) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let manifest_json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(root.join("studio-project.json"), manifest_json).map_err(|e| e.to_string())?;
    let diagram_json = serde_json::to_string_pretty(diagram).map_err(|e| e.to_string())?;
    fs::write(
        root.join(format!("{}.diagram.json", manifest.model_name)),
        diagram_json,
    )
    .map_err(|e| e.to_string())?;
    write_subsystems(root, subsystems)?;
    fs::write(root.join(format!("{}.mo", manifest.model_name)), model_source)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn save_project(
    root: &Path,
    manifest: &StudioProjectManifest,
    diagram: &DiagramDocument,
    subsystems: &HashMap<String, DiagramDocument>,
) -> Result<String, String> {
    let mo = emit_modelica_with_dict(
        diagram,
        subsystems,
        Some(&manifest.simulation_defaults),
        &manifest.param_dictionary,
        Some(root),
    );
    write_project(root, manifest, diagram, subsystems, &mo)?;
    Ok(mo)
}

pub fn resolve_default_msl_root() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("jit-compiler"),
        PathBuf::from("../jit-compiler"),
        PathBuf::from("../../jit-compiler"),
    ];
    for c in candidates {
        if c.join("Modelica").exists() {
            return Some(c);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().take(6) {
            let p = ancestor.join("jit-compiler");
            if p.join("Modelica").exists() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::StudioSimOptions;
    use crate::examples::{list_examples, resolve_example_dir};
    use crate::sim_bridge::validate_model;
    use crate::startup::example_signal_feedback_dir;

    #[test]
    fn open_signal_feedback_example() {
        let root = example_signal_feedback_dir();
        assert!(
            root.join("studio-project.json").is_file(),
            "missing example at {}",
            root.display()
        );
        let opened = open_project(&root).expect("open signal_feedback");
        assert_eq!(opened.manifest.model_name, "SignalFeedback");
        assert!(!opened.diagram.nodes.is_empty());
        assert!(!opened.diagram.edges.is_empty());
    }

    #[test]
    fn bundled_examples_open_with_diagram() {
        let examples = list_examples();
        assert!(
            examples.len() >= 3,
            "expected at least 3 bundled examples, got {}",
            examples.len()
        );
        for ex in examples {
            let opened = open_project(std::path::Path::new(&ex.root_dir))
                .unwrap_or_else(|e| panic!("open {}: {e}", ex.id));
            assert!(
                !opened.diagram.nodes.is_empty(),
                "{}: empty diagram nodes",
                ex.id
            );
            assert!(
                !opened.diagram.edges.is_empty(),
                "{}: empty diagram edges",
                ex.id
            );
            assert_eq!(opened.manifest.model_name, ex.model_name);
        }
    }

    #[test]
    fn signal_feedback_validates() {
        let root = resolve_example_dir("signal_feedback").expect("resolve");
        let opened = open_project(&root).expect("open");
        let sim = StudioSimOptions {
            stop_time: 1.0,
            number_of_intervals: 50,
            solver: "rk45".into(),
            tolerance: 1e-6,
        };
        let report = validate_model(
            &opened.model_source,
            &opened.manifest.model_name,
            &opened.manifest.library_paths,
            &sim,
            false,
        );
        assert!(
            report.success,
            "validate SignalFeedback failed: {:?}",
            report.errors
        );
    }

    #[test]
    fn rc_circuit_validates() {
        let root = resolve_example_dir("rc_circuit").expect("resolve");
        let opened = open_project(&root).expect("open");
        let sim = StudioSimOptions {
            stop_time: 0.5,
            number_of_intervals: 50,
            solver: "rk45".into(),
            tolerance: 1e-6,
        };
        let report = validate_model(
            &opened.model_source,
            &opened.manifest.model_name,
            &opened.manifest.library_paths,
            &sim,
            false,
        );
        assert!(
            report.success,
            "validate RcCircuit failed: {:?}",
            report.errors
        );
    }
}
