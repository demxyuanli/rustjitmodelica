use crate::discover::{categorize_case, discover_large_full};
use modai_protocol::{PlanStrategy, PlannedCase, RegressionExecutionPlan, RegressionPlanRequest};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraceabilityConfig {
    feature_to_cases: HashMap<String, Vec<String>>,
    source_modules: HashMap<String, SourceInfo>,
    case_to_source_files: HashMap<String, Vec<String>>,
    #[serde(default)]
    feature_dependencies: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct SourceInfo {
    features: Vec<String>,
}

fn load_traceability(repo_root: &Path) -> Result<TraceabilityConfig, String> {
    let path = repo_root.join("jit-compiler").join("jit_traceability.json");
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn add_case(out: &mut Vec<PlannedCase>, seen: &mut HashSet<String>, name: String, reason: String, priority: u32) {
    if seen.insert(name.clone()) {
        out.push(PlannedCase {
            category: categorize_case(&name),
            name,
            reason,
            priority,
        });
    }
}

pub fn build_plan(repo_root: &Path, request: &RegressionPlanRequest) -> Result<RegressionExecutionPlan, String> {
    let all_cases =
        discover_large_full(repo_root, request.include_modelica_examples, request.include_modelica_test)?;
    let trace = load_traceability(repo_root)?;
    let mut planned = Vec::new();
    let mut seen = HashSet::new();
    let mut skipped = Vec::new();
    let mut affected_features = Vec::new();
    let changed_sources = request.changed_files.clone();

    match request.strategy {
        PlanStrategy::Category => {
            let wanted: HashSet<String> = request.categories.iter().map(|x| x.to_lowercase()).collect();
            for case in all_cases {
                let c = categorize_case(&case);
                if wanted.is_empty() || wanted.contains(&c) {
                    add_case(&mut planned, &mut seen, case, "category".to_string(), 60);
                } else {
                    skipped.push(case);
                }
            }
        }
        PlanStrategy::Feature => {
            for fid in &request.feature_ids {
                affected_features.push(fid.clone());
                if let Some(cases) = trace.feature_to_cases.get(fid) {
                    for c in cases {
                        add_case(&mut planned, &mut seen, c.clone(), format!("feature:{fid}"), 90);
                    }
                }
                if request.include_indirect {
                    if let Some(deps) = trace.feature_dependencies.get(fid) {
                        for dep in deps {
                            affected_features.push(dep.clone());
                            if let Some(cases) = trace.feature_to_cases.get(dep) {
                                for c in cases {
                                    add_case(
                                        &mut planned,
                                        &mut seen,
                                        c.clone(),
                                        format!("feature-dependency:{dep}"),
                                        70,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            for c in all_cases {
                if !seen.contains(&c) {
                    skipped.push(c);
                }
            }
        }
        PlanStrategy::Relation => {
            let mut direct_features = HashSet::new();
            for changed in &request.changed_files {
                let changed_norm = changed.replace('\\', "/");
                if let Some(src) = trace.source_modules.get(&changed_norm) {
                    for f in &src.features {
                        direct_features.insert(f.clone());
                    }
                }
                for (case, sources) in &trace.case_to_source_files {
                    if sources.iter().any(|s| s == &changed_norm) {
                        add_case(&mut planned, &mut seen, case.clone(), "source-direct".to_string(), 100);
                    }
                }
            }
            for f in &direct_features {
                affected_features.push(f.clone());
                if let Some(cases) = trace.feature_to_cases.get(f) {
                    for c in cases {
                        add_case(&mut planned, &mut seen, c.clone(), format!("source-feature:{f}"), 95);
                    }
                }
            }
            if request.include_indirect {
                for f in direct_features {
                    if let Some(deps) = trace.feature_dependencies.get(&f) {
                        for dep in deps {
                            affected_features.push(dep.clone());
                            if let Some(cases) = trace.feature_to_cases.get(dep) {
                                for c in cases {
                                    add_case(
                                        &mut planned,
                                        &mut seen,
                                        c.clone(),
                                        format!("relation-dependency:{dep}"),
                                        75,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            if seen.is_empty() {
                for c in all_cases {
                    add_case(&mut planned, &mut seen, c, "fallback-large-full".to_string(), 40);
                }
            } else {
                for c in all_cases {
                    if !seen.contains(&c) {
                        skipped.push(c);
                    }
                }
            }
        }
    }

    planned.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.name.cmp(&b.name)));
    if let Some(max_cases) = request.max_cases {
        if max_cases > 0 && planned.len() > max_cases {
            for x in planned.drain(max_cases..) {
                skipped.push(x.name);
            }
        }
    }

    Ok(RegressionExecutionPlan {
        strategy: request.strategy.clone(),
        changed_sources,
        affected_features,
        planned_cases: planned,
        skipped_cases: skipped,
    })
}
