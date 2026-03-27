use std::fs;
use std::path::{Path, PathBuf};

pub fn categorize_case(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("init") {
        "initialization".to_string()
    } else if lower.contains("array") || lower.contains("for") || lower.contains("loop") {
        "array".to_string()
    } else if lower.contains("connect") || lower.contains("pin") || lower.contains("circuit") {
        "connect".to_string()
    } else if lower.contains("when") || lower.contains("discrete") || lower.contains("reinit") {
        "discrete".to_string()
    } else if lower.contains("algebraic") || lower.contains("solvable") || lower.contains("tearing") {
        "algebraic".to_string()
    } else if lower.contains("msl") || lower.contains("library") || lower.contains("siunits") {
        "msl".to_string()
    } else if lower.contains("func") {
        "function".to_string()
    } else if lower.contains("record") || lower.contains("block") {
        "structure".to_string()
    } else if lower.contains("bad") || lower.contains("error") || lower.contains("unknown") {
        "error".to_string()
    } else if lower.contains("adaptive") || lower.contains("bouncing") || lower.contains("pendulum") {
        "solver".to_string()
    } else {
        "basic".to_string()
    }
}

fn collect_mo_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_mo_files(&path, out)?;
        } else if path.extension().is_some_and(|x| x == "mo") {
            out.push(path);
        }
    }
    Ok(())
}

fn model_name_from_file(prefix: &str, root: &Path, file: &Path) -> Option<String> {
    if file.file_name().and_then(|n| n.to_str()) == Some("package.mo") {
        return None;
    }
    let rel = file.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
    let stem = rel.strip_suffix(".mo")?;
    if stem.is_empty() {
        return None;
    }
    Some(format!("{prefix}.{}", stem.replace('/', ".")))
}

pub fn discover_large_full(repo_root: &Path, include_examples: bool, include_modelica_test: bool) -> Result<Vec<String>, String> {
    let mut models = Vec::new();
    if include_examples {
        let modelica_root = repo_root.join("jit-compiler").join("Modelica");
        if modelica_root.is_dir() {
            let mut files = Vec::new();
            collect_mo_files(&modelica_root, &mut files)?;
            for f in files {
                let rel = f
                    .strip_prefix(&modelica_root)
                    .ok()
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                if rel.contains("/Examples/") {
                    if let Some(name) = model_name_from_file("Modelica", &modelica_root, &f) {
                        models.push(name);
                    }
                }
            }
        }
    }
    if include_modelica_test {
        let test_root = repo_root.join("jit-compiler").join("ModelicaTest");
        if test_root.is_dir() {
            let mut files = Vec::new();
            collect_mo_files(&test_root, &mut files)?;
            for f in files {
                if let Some(name) = model_name_from_file("ModelicaTest", &test_root, &f) {
                    models.push(name);
                }
            }
        }
    }
    models.sort();
    models.dedup();
    Ok(models)
}
