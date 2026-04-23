//! Performance baseline comparison and update tooling for JIT validate-perf.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use super::artifacts::ValidatePerfReport;

/// Default `jit compare-baseline --baseline` when omitted (repo-relative).
pub const DEFAULT_JIT_COMPARE_BASELINE_REL: &str =
    "baseline/20260418_three_tier_devloop/jit_perf_baseline.json";

/// Full **TestLib** matrix (`jit validate-perf`, 172 models x five scenarios) with compare thresholds
/// tuned for **analyze-tier** runs: no L0/L1 tier gates, `speedup_min_ratio=0` (cold/hot not enforced).
/// Use: `jit compare-baseline --report <report.json> --baseline <this path>`.
pub const TESTLIB_VALIDATE_PERF_V1_BASELINE_REL: &str =
    "baseline/testlib_validate_perf_v1/jit_perf_baseline.json";

/// Large-matrix perf baseline (same model matrix as v1 + optional JitStress when recorded with `--include-jitstress-probe`).
pub const LARGE_SCALE_VALIDATE_PERF_V2_BASELINE_REL: &str =
    "baseline/large_scale_jit_validate_perf_v2/jit_perf_baseline.json";

// ---------------------------------------------------------------------------
// Baseline schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineFile {
    pub schema_version: u32,
    pub generated_at: String,
    pub git_head: Option<String>,
    pub host: BaselineHost,
    pub thresholds: BaselineThresholds,
    pub benchmarks: BTreeMap<String, BenchmarkEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineHost {
    pub os: Option<String>,
    pub arch: Option<String>,
}

impl Default for BaselineHost {
    fn default() -> Self {
        Self {
            os: Some(std::env::consts::OS.to_string()),
            arch: Some(std::env::consts::ARCH.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineThresholds {
    /// Duration regression threshold (%). Default 20.
    pub duration_ms_regression_pct: f64,
    /// Codegen regression threshold (%). Default 15.
    pub codegen_wall_us_regression_pct: f64,
    /// Minimum cache hit rate for hot scenarios (%). Default 85.
    pub cache_hit_rate_min_pct: f64,
    /// Minimum hot/cold speedup ratio. Default 1.5.
    pub speedup_min_ratio: f64,
    /// Optional L0 hit-rate floor for tiered cache (0–100). When absent, not enforced.
    #[serde(default)]
    pub std_hit_rate_min_pct: Option<f64>,
    /// Optional L1 hit-rate floor (0–100).
    #[serde(default)]
    pub user_hit_rate_min_pct: Option<f64>,
    /// Optional AOT TOC hit-rate floor (0–100).
    #[serde(default)]
    pub aot_hit_rate_min_pct: Option<f64>,
    /// Optional max regression % for project-tier rebuild wall time.
    #[serde(default)]
    pub project_rebuild_regression_pct: Option<f64>,
}

impl Default for BaselineThresholds {
    fn default() -> Self {
        Self {
            duration_ms_regression_pct: 20.0,
            codegen_wall_us_regression_pct: 15.0,
            cache_hit_rate_min_pct: 85.0,
            speedup_min_ratio: 1.5,
            std_hit_rate_min_pct: None,
            user_hit_rate_min_pct: None,
            aot_hit_rate_min_pct: None,
            project_rebuild_regression_pct: None,
        }
    }
}

/// Thresholds for `large-scale-v2` preset (analyze-tier friendly, slightly looser duration/codegen gates).
pub fn thresholds_large_scale_v2() -> BaselineThresholds {
    BaselineThresholds {
        duration_ms_regression_pct: 25.0,
        codegen_wall_us_regression_pct: 20.0,
        cache_hit_rate_min_pct: 85.0,
        speedup_min_ratio: 0.0,
        std_hit_rate_min_pct: None,
        user_hit_rate_min_pct: None,
        aot_hit_rate_min_pct: None,
        project_rebuild_regression_pct: Some(30.0),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEntry {
    pub duration_ms_p50: u64,
    pub codegen_wall_us_p50: Option<u64>,
    pub flatten_wall_us_p50: Option<u64>,
    pub cache_l0_hits: Option<u64>,
    pub cache_l0_writes: Option<u64>,
    pub cache_l1_hits: Option<u64>,
    pub cache_l1_writes: Option<u64>,
    pub cache_l2_hits: Option<u64>,
    pub cache_l2_writes: Option<u64>,
    pub sample_count: usize,
    #[serde(default)]
    pub project_rebuild_wall_ms_p50: Option<u64>,
    #[serde(default)]
    pub std_flat_full_hit_rate_p50: Option<f64>,
    #[serde(default)]
    pub user_flat_full_hit_rate_p50: Option<f64>,
    #[serde(default)]
    pub aot_hit_rate_p50: Option<f64>,
}

// ---------------------------------------------------------------------------
// Comparison result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub key: String,
    pub verdict: Verdict,
    pub details: Vec<String>,
    pub duration_ms_baseline: Option<u64>,
    pub duration_ms_current: Option<u64>,
    pub duration_ms_delta_pct: Option<f64>,
    pub codegen_wall_us_baseline: Option<u64>,
    pub codegen_wall_us_current: Option<u64>,
    pub codegen_wall_us_delta_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierMetricCheck {
    pub key: String,
    pub metric: String,
    pub current: Option<f64>,
    pub threshold: f64,
    pub verdict: Verdict,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareResult {
    pub overall_verdict: Verdict,
    pub comparisons: Vec<BenchmarkComparison>,
    pub speedup_checks: Vec<SpeedupCheck>,
    #[serde(default)]
    pub tier_checks: Vec<TierMetricCheck>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedupCheck {
    pub model: String,
    pub cold_duration_ms: Option<u64>,
    pub hot_duration_ms: Option<u64>,
    pub speedup_ratio: Option<f64>,
    pub verdict: Verdict,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Update baseline from report
// ---------------------------------------------------------------------------

pub fn update_baseline_from_report(
    report: &ValidatePerfReport,
    thresholds: BaselineThresholds,
    git_head: Option<String>,
) -> BaselineFile {
    let mut benchmarks = BTreeMap::new();

    for (scenario, models) in &report.stats.by_scenario {
        for (model, stats) in models {
            let key = format!("{}/{}", scenario, model);
            let p50 = stats.duration_ms_min.unwrap_or(0);

            let entry = BenchmarkEntry {
                duration_ms_p50: p50,
                codegen_wall_us_p50: stats.codegen_wall_us_min,
                flatten_wall_us_p50: stats.flatten_wall_us_min,
                cache_l0_hits: None,
                cache_l0_writes: None,
                cache_l1_hits: None,
                cache_l1_writes: None,
                cache_l2_hits: None,
                cache_l2_writes: None,
                sample_count: stats.runs,
                project_rebuild_wall_ms_p50: stats.project_rebuild_wall_ms_min,
                std_flat_full_hit_rate_p50: stats.std_flat_full_hit_rate_max,
                user_flat_full_hit_rate_p50: stats.user_flat_full_hit_rate_max,
                aot_hit_rate_p50: stats.aot_hit_rate_max,
            };

            if let Some(layer_stats) = &stats.cache_layer_stats {
                let mut entry = entry;
                for (scope, ls) in layer_stats {
                    match scope.as_str() {
                        s if s.contains("L0") || s.contains("l0") => {
                            entry.cache_l0_hits = Some(entry.cache_l0_hits.unwrap_or(0) + ls.hits);
                            entry.cache_l0_writes =
                                Some(entry.cache_l0_writes.unwrap_or(0) + ls.writes);
                        }
                        s if s.contains("L1") || s.contains("l1") => {
                            entry.cache_l1_hits = Some(entry.cache_l1_hits.unwrap_or(0) + ls.hits);
                            entry.cache_l1_writes =
                                Some(entry.cache_l1_writes.unwrap_or(0) + ls.writes);
                        }
                        s if s.contains("L2") || s.contains("l2") => {
                            entry.cache_l2_hits = Some(entry.cache_l2_hits.unwrap_or(0) + ls.hits);
                            entry.cache_l2_writes =
                                Some(entry.cache_l2_writes.unwrap_or(0) + ls.writes);
                        }
                        _ => {}
                    }
                }
                benchmarks.insert(key, entry);
            } else {
                benchmarks.insert(key, entry);
            }
        }
    }

    let now = chrono::Utc::now().to_rfc3339();
    BaselineFile {
        schema_version: 1,
        generated_at: now,
        git_head,
        host: BaselineHost::default(),
        thresholds,
        benchmarks,
    }
}

pub fn save_baseline(baseline: &BaselineFile, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(baseline)?;
    std::fs::write(path, json)
        .with_context(|| format!("write baseline to {}", path.display()))?;
    Ok(())
}

pub fn load_baseline(path: &Path) -> Result<BaselineFile> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read baseline from {}", path.display()))?;
    let baseline: BaselineFile = serde_json::from_str(&text)?;
    if baseline.schema_version != 1 {
        bail!(
            "unsupported baseline schema version: {}",
            baseline.schema_version
        );
    }
    Ok(baseline)
}

// ---------------------------------------------------------------------------
// Compare report against baseline
// ---------------------------------------------------------------------------

fn pct_delta(current: u64, baseline: u64) -> f64 {
    if baseline == 0 {
        return 0.0;
    }
    ((current as f64 - baseline as f64) / baseline as f64) * 100.0
}

fn bump_worst(worst: &mut Verdict, v: Verdict) {
    match v {
        Verdict::Fail => *worst = Verdict::Fail,
        Verdict::Warn if *worst == Verdict::Pass => *worst = Verdict::Warn,
        _ => {}
    }
}

pub fn compare_report_to_baseline(
    report: &ValidatePerfReport,
    baseline: &BaselineFile,
) -> CompareResult {
    let thresholds = &baseline.thresholds;
    let mut comparisons = Vec::new();
    let mut speedup_checks = Vec::new();
    let mut tier_checks = Vec::new();
    let mut worst_verdict = Verdict::Pass;

    // Collect all (scenario, model) pairs from current report
    let mut current_entries: BTreeMap<String, u64> = BTreeMap::new(); // key -> duration_ms_min
    let mut current_codegen: BTreeMap<String, u64> = BTreeMap::new();
    let mut models_by_scenario: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (scenario, models) in &report.stats.by_scenario {
        for (model, stats) in models {
            let key = format!("{}/{}", scenario, model);
            if let Some(d) = stats.duration_ms_min {
                current_entries.insert(key.clone(), d);
            }
            if let Some(c) = stats.codegen_wall_us_min {
                current_codegen.insert(key.clone(), c);
            }
            models_by_scenario
                .entry(scenario.clone())
                .or_default()
                .push(model.clone());
        }
    }

    // Compare each baseline entry against current
    for (key, base_entry) in &baseline.benchmarks {
        let mut details = Vec::new();
        let mut verdict = Verdict::Pass;

        let current_dur = current_entries.get(key);
        let current_cg = current_codegen.get(key);

        let dur_delta_pct = match current_dur {
            Some(cur) => {
                let delta = pct_delta(*cur, base_entry.duration_ms_p50);
                if delta > thresholds.duration_ms_regression_pct {
                    verdict = Verdict::Fail;
                    details.push(format!(
                        "duration regressed: {}ms vs {}ms baseline ({:+.1}%)",
                        cur, base_entry.duration_ms_p50, delta
                    ));
                } else if delta > thresholds.duration_ms_regression_pct * 0.5 {
                    if verdict != Verdict::Fail {
                        verdict = Verdict::Warn;
                    }
                    details.push(format!(
                        "duration warning: {}ms vs {}ms baseline ({:+.1}%)",
                        cur, base_entry.duration_ms_p50, delta
                    ));
                }
                Some(delta)
            }
            None => {
                details.push("not found in current report".to_string());
                verdict = Verdict::Warn;
                None
            }
        };

        let cg_delta_pct = match (current_cg, base_entry.codegen_wall_us_p50) {
            (Some(cur), Some(base)) => {
                let delta = pct_delta(*cur, base);
                if delta > thresholds.codegen_wall_us_regression_pct {
                    if verdict == Verdict::Pass {
                        verdict = Verdict::Warn;
                    }
                    details.push(format!(
                        "codegen regressed: {}us vs {}us baseline ({:+.1}%)",
                        cur, base, delta
                    ));
                }
                Some(delta)
            }
            _ => None,
        };

        if verdict == Verdict::Fail {
            worst_verdict = Verdict::Fail;
        } else if verdict == Verdict::Warn && worst_verdict != Verdict::Fail {
            worst_verdict = Verdict::Warn;
        }

        comparisons.push(BenchmarkComparison {
            key: key.clone(),
            verdict,
            details,
            duration_ms_baseline: Some(base_entry.duration_ms_p50),
            duration_ms_current: current_dur.copied(),
            duration_ms_delta_pct: dur_delta_pct,
            codegen_wall_us_baseline: base_entry.codegen_wall_us_p50,
            codegen_wall_us_current: current_cg.copied(),
            codegen_wall_us_delta_pct: cg_delta_pct,
        });
    }

    // Speedup checks: compare cold vs hot scenario durations for same model
    let cold_scenario = "stdlib_bake";
    let hot_scenario = "devloop_multi_model";
    if let (Some(cold_models), Some(hot_models)) = (
        models_by_scenario.get(cold_scenario),
        models_by_scenario.get(hot_scenario),
    ) {
        let cold_stats: BTreeMap<&str, u64> = report
            .stats
            .by_scenario
            .get(cold_scenario)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.duration_ms_min.map(|d| (k.as_str(), d)))
                    .collect()
            })
            .unwrap_or_default();
        let hot_stats: BTreeMap<&str, u64> = report
            .stats
            .by_scenario
            .get(hot_scenario)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.duration_ms_min.map(|d| (k.as_str(), d)))
                    .collect()
            })
            .unwrap_or_default();

        let all_models: Vec<&str> = cold_models
            .iter()
            .chain(hot_models.iter())
            .map(|s| s.as_str())
            .collect();
        let mut seen = std::collections::HashSet::new();
        for model in all_models {
            if !seen.insert(model) {
                continue;
            }
            let cold_dur = cold_stats.get(model).copied();
            let hot_dur = hot_stats.get(model).copied();
            let (ratio, sv, detail) = match (cold_dur, hot_dur) {
                (Some(c), Some(h)) if h > 0 => {
                    let r = c as f64 / h as f64;
                    if r < thresholds.speedup_min_ratio {
                        if worst_verdict != Verdict::Fail {
                            worst_verdict = Verdict::Fail;
                        }
                        (
                            Some(r),
                            Verdict::Fail,
                            format!(
                                "speedup {:.2}x < min {:.1}x (cold={}ms, hot={}ms)",
                                r, thresholds.speedup_min_ratio, c, h
                            ),
                        )
                    } else if r < thresholds.speedup_min_ratio + 0.5 {
                        if worst_verdict == Verdict::Pass {
                            worst_verdict = Verdict::Warn;
                        }
                        (
                            Some(r),
                            Verdict::Warn,
                            format!(
                                "speedup {:.2}x marginal (cold={}ms, hot={}ms)",
                                r, c, h
                            ),
                        )
                    } else {
                        (
                            Some(r),
                            Verdict::Pass,
                            format!("speedup {:.2}x OK (cold={}ms, hot={}ms)", r, c, h),
                        )
                    }
                }
                _ => (
                    None,
                    Verdict::Warn,
                    "missing cold or hot data".to_string(),
                ),
            };
            speedup_checks.push(SpeedupCheck {
                model: model.to_string(),
                cold_duration_ms: cold_dur,
                hot_duration_ms: hot_dur,
                speedup_ratio: ratio,
                verdict: sv,
                detail,
            });
        }
    }

    for (scenario, models) in &report.stats.by_scenario {
        for (model, st) in models {
            let key = format!("{}/{}", scenario, model);
            let base_entry = baseline.benchmarks.get(&key);

            if let Some(th) = thresholds.std_hit_rate_min_pct {
                // Hot devloop only: `stdlib_bake` is a deliberate cold scenario; enforcing L0 flat_full
                // floor there would false-fail once real scope/stage counters exist.
                if scenario == "devloop_multi_model" {
                    let cur = st.std_flat_full_hit_rate_max;
                    let (verdict, detail) = match cur {
                        Some(c) if c + 1e-9 < th => (
                            Verdict::Fail,
                            format!(
                                "L0 flat_full hit rate {:.2}% below floor {:.2}%",
                                c, th
                            ),
                        ),
                        Some(c) => (
                            Verdict::Pass,
                            format!("L0 flat_full hit rate {:.2}% >= {:.2}%", c, th),
                        ),
                        None => (
                            Verdict::Warn,
                            "L0 flat_full hit rate missing (no compile_perf scope/stage data)"
                                .to_string(),
                        ),
                    };
                    bump_worst(&mut worst_verdict, verdict);
                    tier_checks.push(TierMetricCheck {
                        key: key.clone(),
                        metric: "std_flat_full_hit_rate_pct".to_string(),
                        current: cur,
                        threshold: th,
                        verdict,
                        detail,
                    });
                }
            }

            if let Some(th) = thresholds.user_hit_rate_min_pct {
                if scenario == "devloop_multi_model" {
                    let cur = st.user_flat_full_hit_rate_max;
                    let (verdict, detail) = match cur {
                        Some(c) if c + 1e-9 < th => (
                            Verdict::Fail,
                            format!(
                                "L1 flat_full hit rate {:.2}% below floor {:.2}%",
                                c, th
                            ),
                        ),
                        Some(c) => (
                            Verdict::Pass,
                            format!("L1 flat_full hit rate {:.2}% >= {:.2}%", c, th),
                        ),
                        None => (
                            Verdict::Warn,
                            "L1 flat_full hit rate missing (no compile_perf scope/stage data)"
                                .to_string(),
                        ),
                    };
                    bump_worst(&mut worst_verdict, verdict);
                    tier_checks.push(TierMetricCheck {
                        key: key.clone(),
                        metric: "user_flat_full_hit_rate_pct".to_string(),
                        current: cur,
                        threshold: th,
                        verdict,
                        detail,
                    });
                }
            }

            if let Some(th) = thresholds.aot_hit_rate_min_pct {
                if scenario == "devloop_multi_model" {
                    let Some(cur_v) = st.aot_hit_rate_max else {
                        continue;
                    };
                    let (verdict, detail) = if cur_v + 1e-9 < th {
                        (
                            Verdict::Fail,
                            format!("AOT native load proxy {:.2}% below floor {:.2}%", cur_v, th),
                        )
                    } else {
                        (
                            Verdict::Pass,
                            format!("AOT native load proxy {:.2}% >= {:.2}%", cur_v, th),
                        )
                    };
                    bump_worst(&mut worst_verdict, verdict);
                    tier_checks.push(TierMetricCheck {
                        key: key.clone(),
                        metric: "aot_hit_rate_pct".to_string(),
                        current: Some(cur_v),
                        threshold: th,
                        verdict,
                        detail,
                    });
                }
            }

            if let Some(th_pct) = thresholds.project_rebuild_regression_pct {
                if scenario == "devloop_multi_model" {
                    let Some(base_ms) = base_entry
                        .and_then(|e| e.project_rebuild_wall_ms_p50)
                        .filter(|b| *b > 0)
                    else {
                        continue;
                    };
                    let cur_ms = st.project_rebuild_wall_ms_min;
                    let (verdict, detail, current_as_f) = match cur_ms {
                        Some(cur) => {
                            let delta = pct_delta(cur, base_ms);
                            if delta > th_pct {
                                (
                                    Verdict::Fail,
                                    format!(
                                        "project rebuild wall {}ms vs baseline {}ms ({:+.1}% > {}% cap)",
                                        cur, base_ms, delta, th_pct
                                    ),
                                    Some(cur as f64),
                                )
                            } else if delta > th_pct * 0.5 {
                                (
                                    Verdict::Warn,
                                    format!(
                                        "project rebuild wall {}ms vs baseline {}ms ({:+.1}%)",
                                        cur, base_ms, delta
                                    ),
                                    Some(cur as f64),
                                )
                            } else {
                                (
                                    Verdict::Pass,
                                    format!(
                                        "project rebuild wall {}ms vs baseline {}ms ({:+.1}%)",
                                        cur, base_ms, delta
                                    ),
                                    Some(cur as f64),
                                )
                            }
                        }
                        None => (
                            Verdict::Warn,
                            "project rebuild wall missing in current report".to_string(),
                            None,
                        ),
                    };
                    bump_worst(&mut worst_verdict, verdict);
                    tier_checks.push(TierMetricCheck {
                        key: key.clone(),
                        metric: "project_rebuild_wall_ms".to_string(),
                        current: current_as_f,
                        threshold: th_pct,
                        verdict,
                        detail,
                    });
                }
            }
        }
    }

    let pass_count = comparisons
        .iter()
        .filter(|c| c.verdict == Verdict::Pass)
        .count();
    let warn_count = comparisons
        .iter()
        .filter(|c| c.verdict == Verdict::Warn)
        .count();
    let fail_count = comparisons
        .iter()
        .filter(|c| c.verdict == Verdict::Fail)
        .count();

    let tier_fail = tier_checks
        .iter()
        .filter(|t| t.verdict == Verdict::Fail)
        .count();
    let tier_warn = tier_checks
        .iter()
        .filter(|t| t.verdict == Verdict::Warn)
        .count();

    let summary = format!(
        "Compared {} benchmarks: {} pass, {} warn, {} fail. Speedup checks: {}. Tier metric checks: {} ({} fail, {} warn).",
        comparisons.len(),
        pass_count,
        warn_count,
        fail_count,
        speedup_checks.len(),
        tier_checks.len(),
        tier_fail,
        tier_warn,
    );

    CompareResult {
        overall_verdict: worst_verdict,
        comparisons,
        speedup_checks,
        tier_checks,
        summary,
    }
}
