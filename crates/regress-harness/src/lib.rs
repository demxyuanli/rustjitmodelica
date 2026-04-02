//! JSON-driven regression harness: config loading, execution planning, reporting.

pub mod compare;
pub mod collection;
pub mod config;
pub mod incremental;
pub mod report;
pub mod runtime;
pub mod runner;
pub mod session_prep;
pub mod tiers;
pub mod jit_validate;

pub use compare::compare_csv_last_row_max_abs_diff;
pub use config::{
    load_config, Defaults, ExpectKind, HarnessConfig, IncrementalConfig, IncrementalStrategy, MosRunMode,
};
pub use incremental::{
    effective_plan_strategy, merge_ordered, needs_baseline_report, needs_last_manifest, plan_runs,
    PlanEntry,
};
pub use report::{
    filter_cases_by_manifest, read_manifest, write_manifest, Artifacts, CaseResult, CaseStatus,
    OmcCompareResult, RegressManifest, Report,
};
pub use runner::{classify_failure, run_case, run_case_with_trace, CaseRunTrace, RunContext};
pub use session_prep::{
    apply_incremental_overrides, case_kind_str, listed_case_json, plan_row_json, prepare_for_list,
    prepare_session, resolve_data_root, resolve_user_path_str, ListPrep, PlanRowJson,
    PreparedSession,
};
pub use tiers::resolve_cases;
