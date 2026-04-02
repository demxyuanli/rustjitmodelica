//! Baseline merge and incremental scheduling.

use crate::config::{CaseDef, Defaults, IncrementalStrategy};
use crate::report::{CaseResult, CaseStatus, Report};
use crate::runner::hash_case_inputs;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum PlanEntry {
    Run(CaseDef),
    SkippedUnchanged(CaseResult),
    SkippedScope(CaseResult),
}

/// Build execution plan from incremental strategy.
pub fn plan_runs(
    cases: Vec<CaseDef>,
    baseline: Option<&Report>,
    strategy: IncrementalStrategy,
    repo_root: &Path,
    defaults: &Defaults,
) -> Vec<PlanEntry> {
    let mut out = Vec::new();
    let base_map: HashMap<String, CaseResult> = baseline
        .map(|r| r.cases.iter().map(|c| (c.case_id.clone(), c.clone())).collect())
        .unwrap_or_default();

    for case in cases {
        match strategy {
            IncrementalStrategy::None => out.push(PlanEntry::Run(case)),
            IncrementalStrategy::LastStructure | IncrementalStrategy::LastStructureRerunFailed => {
                unreachable!("call effective_plan_strategy() before plan_runs")
            }
            IncrementalStrategy::RerunFailed => {
                if let Some(prev) = base_map.get(case.id.as_str()) {
                    if prev.status == CaseStatus::Pass {
                        let mut copy = prev.clone();
                        copy.duration_ms = 0;
                        out.push(PlanEntry::SkippedScope(copy));
                    } else {
                        out.push(PlanEntry::Run(case));
                    }
                } else {
                    out.push(PlanEntry::Run(case));
                }
            }
            IncrementalStrategy::SkipUnchanged => {
                if let Some(prev) = base_map.get(case.id.as_str()) {
                    let h = hash_case_inputs(repo_root, defaults, &case);
                    if prev.status == CaseStatus::Pass && prev.input_hash.as_deref() == Some(h.as_str())
                    {
                        let mut copy = prev.clone();
                        copy.status = CaseStatus::SkippedUnchanged;
                        copy.duration_ms = 0;
                        out.push(PlanEntry::SkippedUnchanged(copy));
                    } else {
                        out.push(PlanEntry::Run(case));
                    }
                } else {
                    out.push(PlanEntry::Run(case));
                }
            }
        }
    }
    out
}

/// Map `last_structure*` to the inner strategy used by `plan_runs` after manifest filtering.
pub fn effective_plan_strategy(strategy: IncrementalStrategy) -> IncrementalStrategy {
    match strategy {
        IncrementalStrategy::LastStructure => IncrementalStrategy::None,
        IncrementalStrategy::LastStructureRerunFailed => IncrementalStrategy::RerunFailed,
        s => s,
    }
}

pub fn needs_last_manifest(strategy: IncrementalStrategy) -> bool {
    matches!(
        strategy,
        IncrementalStrategy::LastStructure | IncrementalStrategy::LastStructureRerunFailed
    )
}

pub fn needs_baseline_report(strategy: IncrementalStrategy) -> bool {
    matches!(
        strategy,
        IncrementalStrategy::RerunFailed
            | IncrementalStrategy::SkipUnchanged
            | IncrementalStrategy::LastStructureRerunFailed
    )
}

/// Merge baseline with newly executed results, preserving `ordered_ids` order.
pub fn merge_ordered(
    baseline: Option<&Report>,
    new_results: HashMap<String, CaseResult>,
    ordered_ids: &[String],
) -> Vec<CaseResult> {
    let base_map: HashMap<String, CaseResult> = baseline
        .map(|r| r.cases.iter().map(|c| (c.case_id.clone(), c.clone())).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for id in ordered_ids {
        if let Some(c) = new_results.get(id) {
            out.push(c.clone());
        } else if let Some(c) = base_map.get(id) {
            out.push(c.clone());
        }
    }
    out
}
