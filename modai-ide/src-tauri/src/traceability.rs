// Traceability engine: load/save jit_traceability.json, impact/coverage analysis,
// sync check, validation, git-driven impact, feature dependencies.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitFeature {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionCase {
    pub name: String,
    pub expected: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceModuleInfo {
    pub features: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceabilityConfig {
    pub features: Vec<JitFeature>,
    pub cases: Vec<RegressionCase>,
    pub feature_to_cases: HashMap<String, Vec<String>>,
    pub source_modules: HashMap<String, SourceModuleInfo>,
    pub case_to_source_files: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub feature_dependencies: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceabilityMatrix {
    pub features: Vec<JitFeature>,
    pub cases: Vec<RegressionCase>,
    pub feature_to_cases: HashMap<String, Vec<String>>,
    pub case_to_features: HashMap<String, Vec<String>>,
    pub source_modules: HashMap<String, SourceModuleInfo>,
    pub case_to_source_files: HashMap<String, Vec<String>>,
    pub source_to_features: HashMap<String, Vec<String>>,
    pub feature_to_sources: HashMap<String, Vec<String>>,
    pub feature_dependencies: HashMap<String, Vec<String>>,
    pub feature_dependents: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactAnalysisResult {
    pub changed_files: Vec<String>,
    pub affected_features: Vec<String>,
    pub indirectly_affected_features: Vec<String>,
    pub affected_cases: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageAnalysisResult {
    pub untested_features: Vec<String>,
    pub uncovered_sources: Vec<String>,
    pub total_features: usize,
    pub covered_features: usize,
    pub total_sources: usize,
    pub covered_sources: usize,
    pub total_cases: usize,
}

fn config_path(repo_root: &Path) -> PathBuf {
    repo_root.join("jit_traceability.json")
}

pub fn load_config(repo_root: &Path) -> Result<TraceabilityConfig, String> {
    let path = config_path(repo_root);
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse traceability config: {}", e))
}

pub fn save_config(repo_root: &Path, config: &TraceabilityConfig) -> Result<(), String> {
    let path = config_path(repo_root);
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

pub fn get_traceability_matrix(repo_root: &Path) -> Result<TraceabilityMatrix, String> {
    let config = load_config(repo_root)?;

    let mut case_to_features: HashMap<String, Vec<String>> = HashMap::new();
    for (fid, case_list) in &config.feature_to_cases {
        for c in case_list {
            case_to_features
                .entry(c.clone())
                .or_default()
                .push(fid.clone());
        }
    }

    let mut source_to_features: HashMap<String, Vec<String>> = HashMap::new();
    let mut feature_to_sources: HashMap<String, Vec<String>> = HashMap::new();
    for (src, info) in &config.source_modules {
        for fid in &info.features {
            source_to_features
                .entry(src.clone())
                .or_default()
                .push(fid.clone());
            feature_to_sources
                .entry(fid.clone())
                .or_default()
                .push(src.clone());
        }
    }

    let mut feature_dependents: HashMap<String, Vec<String>> = HashMap::new();
    for (fid, deps) in &config.feature_dependencies {
        for dep in deps {
            feature_dependents
                .entry(dep.clone())
                .or_default()
                .push(fid.clone());
        }
    }

    Ok(TraceabilityMatrix {
        features: config.features,
        cases: config.cases,
        feature_to_cases: config.feature_to_cases,
        case_to_features,
        source_modules: config.source_modules,
        case_to_source_files: config.case_to_source_files,
        source_to_features,
        feature_to_sources,
        feature_dependencies: config.feature_dependencies,
        feature_dependents,
    })
}

pub fn impact_analysis(
    repo_root: &Path,
    changed_files: &[String],
) -> Result<ImpactAnalysisResult, String> {
    let config = load_config(repo_root)?;

    let mut affected_features: HashSet<String> = HashSet::new();
    for f in changed_files {
        let normalized = f.replace('\\', "/");
        if let Some(info) = config.source_modules.get(&normalized) {
            for fid in &info.features {
                affected_features.insert(fid.clone());
            }
        }
    }

    let mut feature_dependents: HashMap<String, Vec<String>> = HashMap::new();
    for (fid, deps) in &config.feature_dependencies {
        for dep in deps {
            feature_dependents
                .entry(dep.clone())
                .or_default()
                .push(fid.clone());
        }
    }

    let mut indirect: HashSet<String> = HashSet::new();
    let mut queue: Vec<String> = affected_features.iter().cloned().collect();
    while let Some(fid) = queue.pop() {
        if let Some(dependents) = feature_dependents.get(&fid) {
            for dep in dependents {
                if !affected_features.contains(dep) && indirect.insert(dep.clone()) {
                    queue.push(dep.clone());
                }
            }
        }
    }

    let all_affected: HashSet<String> = affected_features.iter().chain(indirect.iter()).cloned().collect();
    let mut affected_cases: HashSet<String> = HashSet::new();
    for fid in &all_affected {
        if let Some(case_list) = config.feature_to_cases.get(fid) {
            for c in case_list {
                affected_cases.insert(c.clone());
            }
        }
    }

    for f in changed_files {
        let normalized = f.replace('\\', "/");
        for (case_name, sources) in &config.case_to_source_files {
            if sources.iter().any(|s| s == &normalized) {
                affected_cases.insert(case_name.clone());
            }
        }
    }

    let mut af: Vec<String> = affected_features.into_iter().collect();
    af.sort();
    let mut iaf: Vec<String> = indirect.into_iter().collect();
    iaf.sort();
    let mut ac: Vec<String> = affected_cases.into_iter().collect();
    ac.sort();

    Ok(ImpactAnalysisResult {
        changed_files: changed_files.to_vec(),
        affected_features: af,
        indirectly_affected_features: iaf,
        affected_cases: ac,
    })
}

pub fn coverage_analysis(repo_root: &Path) -> Result<CoverageAnalysisResult, String> {
    let config = load_config(repo_root)?;

    let mut features_with_cases: HashSet<String> = HashSet::new();
    for (fid, case_list) in &config.feature_to_cases {
        if !case_list.is_empty() {
            features_with_cases.insert(fid.clone());
        }
    }

    let untested_features: Vec<String> = config
        .features
        .iter()
        .filter(|f| !features_with_cases.contains(&f.id))
        .map(|f| f.id.clone())
        .collect();

    let mut sources_with_tests: HashSet<String> = HashSet::new();
    for sources in config.case_to_source_files.values() {
        for s in sources {
            sources_with_tests.insert(s.clone());
        }
    }

    let uncovered_sources: Vec<String> = config
        .source_modules
        .keys()
        .filter(|s| !sources_with_tests.contains(*s))
        .cloned()
        .collect();

    Ok(CoverageAnalysisResult {
        total_features: config.features.len(),
        covered_features: features_with_cases.len(),
        untested_features,
        total_sources: config.source_modules.len(),
        covered_sources: sources_with_tests.len(),
        uncovered_sources,
        total_cases: config.cases.len(),
    })
}

pub fn update_traceability_link(
    repo_root: &Path,
    link_type: &str,
    source: &str,
    target: &str,
    add: bool,
) -> Result<(), String> {
    let mut config = load_config(repo_root)?;

    match link_type {
        "featureToCase" => {
            let entry = config.feature_to_cases.entry(source.to_string()).or_default();
            if add {
                if !entry.contains(&target.to_string()) {
                    entry.push(target.to_string());
                }
            } else {
                entry.retain(|c| c != target);
            }
        }
        "sourceToFeature" => {
            let entry = config
                .source_modules
                .entry(source.to_string())
                .or_insert_with(|| SourceModuleInfo {
                    features: vec![],
                    description: String::new(),
                });
            if add {
                if !entry.features.contains(&target.to_string()) {
                    entry.features.push(target.to_string());
                }
            } else {
                entry.features.retain(|f| f != target);
            }
        }
        "caseToSource" => {
            let entry = config
                .case_to_source_files
                .entry(source.to_string())
                .or_default();
            if add {
                if !entry.contains(&target.to_string()) {
                    entry.push(target.to_string());
                }
            } else {
                entry.retain(|s| s != target);
            }
        }
        _ => return Err(format!("Unknown link type: {}", link_type)),
    }

    save_config(repo_root, &config)
}

// --- Sync check: compare filesystem with config ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCheckResult {
    pub new_sources: Vec<String>,
    pub removed_sources: Vec<String>,
    pub new_cases: Vec<String>,
    pub removed_cases: Vec<String>,
}

fn collect_rs_files(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let read_dir = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in read_dir.filter_map(|e| e.ok()) {
        let p = entry.path();
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        if p.is_dir() {
            collect_rs_files(&p, &rel, out);
        } else if name.ends_with(".rs") {
            out.push(rel);
        }
    }
}

fn collect_mo_cases(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let read_dir = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in read_dir.filter_map(|e| e.ok()) {
        let p = entry.path();
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.starts_with('.') {
            continue;
        }
        let rel = format!("{}/{}", prefix, name);
        if p.is_dir() {
            collect_mo_cases(&p, &rel, out);
        } else if name.ends_with(".mo") {
            let case_name = rel.trim_end_matches(".mo").to_string();
            out.push(case_name);
        }
    }
}

pub fn sync_check(repo_root: &Path) -> Result<SyncCheckResult, String> {
    let config = load_config(repo_root)?;

    let mut disk_sources: Vec<String> = Vec::new();
    let src_dir = repo_root.join("src");
    if src_dir.is_dir() {
        collect_rs_files(&src_dir, "src", &mut disk_sources);
    }
    let config_sources: HashSet<String> = config.source_modules.keys().cloned().collect();
    let disk_set: HashSet<String> = disk_sources.iter().cloned().collect();

    let mut new_sources: Vec<String> = disk_set.difference(&config_sources).cloned().collect();
    new_sources.sort();
    let mut removed_sources: Vec<String> = config_sources.difference(&disk_set).cloned().collect();
    removed_sources.sort();

    let mut disk_cases: Vec<String> = Vec::new();
    let test_dir = repo_root.join("TestLib");
    if test_dir.is_dir() {
        collect_mo_cases(&test_dir, "TestLib", &mut disk_cases);
    }
    let config_cases: HashSet<String> = config.cases.iter().map(|c| c.name.clone()).collect();
    let disk_case_set: HashSet<String> = disk_cases.iter().cloned().collect();

    let mut new_cases: Vec<String> = disk_case_set.difference(&config_cases).cloned().collect();
    new_cases.sort();
    let mut removed_cases: Vec<String> = config_cases.difference(&disk_case_set).cloned().collect();
    removed_cases.sort();

    Ok(SyncCheckResult {
        new_sources,
        removed_sources,
        new_cases,
        removed_cases,
    })
}

// --- Validation: check internal reference integrity ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub kind: String,
    pub message: String,
    pub related: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
}

pub fn validate_config(repo_root: &Path) -> Result<ValidationResult, String> {
    let config = load_config(repo_root)?;
    let mut errors: Vec<ValidationError> = Vec::new();

    let feature_ids: HashSet<String> = config.features.iter().map(|f| f.id.clone()).collect();
    let case_names: HashSet<String> = config.cases.iter().map(|c| c.name.clone()).collect();
    let source_keys: HashSet<String> = config.source_modules.keys().cloned().collect();

    for (fid, case_list) in &config.feature_to_cases {
        if !feature_ids.contains(fid) {
            errors.push(ValidationError {
                kind: "dangling_feature".to_string(),
                message: format!("featureToCases references unknown feature '{}'", fid),
                related: vec![fid.clone()],
            });
        }
        for c in case_list {
            if !case_names.contains(c) {
                errors.push(ValidationError {
                    kind: "dangling_case".to_string(),
                    message: format!("featureToCases[{}] references unknown case '{}'", fid, c),
                    related: vec![fid.clone(), c.clone()],
                });
            }
        }
    }

    for (src, info) in &config.source_modules {
        for fid in &info.features {
            if !feature_ids.contains(fid) {
                errors.push(ValidationError {
                    kind: "dangling_feature".to_string(),
                    message: format!("sourceModules[{}].features references unknown feature '{}'", src, fid),
                    related: vec![src.clone(), fid.clone()],
                });
            }
        }
    }

    for (case_name, sources) in &config.case_to_source_files {
        if !case_names.contains(case_name) {
            errors.push(ValidationError {
                kind: "dangling_case".to_string(),
                message: format!("caseToSourceFiles references unknown case '{}'", case_name),
                related: vec![case_name.clone()],
            });
        }
        for src in sources {
            if !source_keys.contains(src) {
                errors.push(ValidationError {
                    kind: "dangling_source".to_string(),
                    message: format!("caseToSourceFiles[{}] references unknown source '{}'", case_name, src),
                    related: vec![case_name.clone(), src.clone()],
                });
            }
        }
    }

    for (fid, deps) in &config.feature_dependencies {
        if !feature_ids.contains(fid) {
            errors.push(ValidationError {
                kind: "dangling_feature".to_string(),
                message: format!("featureDependencies references unknown feature '{}'", fid),
                related: vec![fid.clone()],
            });
        }
        for dep in deps {
            if !feature_ids.contains(dep) {
                errors.push(ValidationError {
                    kind: "dangling_feature".to_string(),
                    message: format!("featureDependencies[{}] references unknown dependency '{}'", fid, dep),
                    related: vec![fid.clone(), dep.clone()],
                });
            }
        }
    }

    let mut features_referenced: HashSet<String> = HashSet::new();
    for info in config.source_modules.values() {
        for fid in &info.features {
            features_referenced.insert(fid.clone());
        }
    }
    for fid in &feature_ids {
        if !features_referenced.contains(fid) {
            errors.push(ValidationError {
                kind: "orphan_feature".to_string(),
                message: format!("Feature '{}' is not referenced by any source module", fid),
                related: vec![fid.clone()],
            });
        }
    }

    Ok(ValidationResult { errors })
}

// --- Apply sync: batch register/remove entries ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplySyncRequest {
    #[serde(default)]
    pub add_sources: Vec<String>,
    #[serde(default)]
    pub remove_sources: Vec<String>,
    #[serde(default)]
    pub add_cases: Vec<String>,
    #[serde(default)]
    pub remove_cases: Vec<String>,
}

pub fn apply_sync(repo_root: &Path, req: &ApplySyncRequest) -> Result<(), String> {
    let mut config = load_config(repo_root)?;

    for src in &req.add_sources {
        if !config.source_modules.contains_key(src) {
            config.source_modules.insert(
                src.clone(),
                SourceModuleInfo {
                    features: vec![],
                    description: String::new(),
                },
            );
        }
    }

    for src in &req.remove_sources {
        config.source_modules.remove(src);
        for sources in config.case_to_source_files.values_mut() {
            sources.retain(|s| s != src);
        }
    }

    for case_name in &req.add_cases {
        if !config.cases.iter().any(|c| &c.name == case_name) {
            config.cases.push(RegressionCase {
                name: case_name.clone(),
                expected: "pass".to_string(),
                notes: None,
            });
        }
    }

    for case_name in &req.remove_cases {
        config.cases.retain(|c| &c.name != case_name);
        config.case_to_source_files.remove(case_name);
        for case_list in config.feature_to_cases.values_mut() {
            case_list.retain(|c| c != case_name);
        }
    }

    save_config(repo_root, &config)
}

// --- Git-driven impact analysis ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitImpactResult {
    pub impact: ImpactAnalysisResult,
    pub suggested_cases: Vec<String>,
    pub unregistered_changed_sources: Vec<String>,
}

pub fn git_changed_impact(repo_root: &Path) -> Result<GitImpactResult, String> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Failed to run git status: {}", e))?;

    if !output.status.success() {
        return Err("git status failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut changed_sources: Vec<String> = Vec::new();
    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let path = line[3..].trim().replace('\\', "/");
        let path = if path.contains(" -> ") {
            path.split(" -> ").last().unwrap_or(&path).to_string()
        } else {
            path
        };
        if path.starts_with("src/") && path.ends_with(".rs") {
            changed_sources.push(path);
        }
    }

    changed_sources.sort();
    changed_sources.dedup();

    let config = load_config(repo_root)?;
    let source_keys: HashSet<String> = config.source_modules.keys().cloned().collect();
    let mut unregistered: Vec<String> = changed_sources
        .iter()
        .filter(|s| !source_keys.contains(*s))
        .cloned()
        .collect();
    unregistered.sort();

    let impact = impact_analysis(repo_root, &changed_sources)?;

    let suggested = impact.affected_cases.clone();

    Ok(GitImpactResult {
        impact,
        suggested_cases: suggested,
        unregistered_changed_sources: unregistered,
    })
}
