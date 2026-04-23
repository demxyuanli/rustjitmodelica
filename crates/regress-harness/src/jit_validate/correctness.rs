//! TestLib correctness baseline: which models pass `jit validate-perf` per scenario, and negative fixtures.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use super::artifacts::ValidatePerfReport;
use super::runner::parse_validate_success;

/// Default models for correctness recording: `run_regression.ps1` `$cases` where `pass`, path starts with `TestLib/`, excluding `TestLib/Pendulum` (matches `JIT_VALIDATE_PERF` filter).
pub fn default_correctness_validate_perf_models() -> Vec<String> {
    vec![
        "TestLib/InitDummy",
        "TestLib/InitWithParam",
        "TestLib/InitAlg",
        "TestLib/InitWhen",
        "TestLib/InitTwoVars",
        "TestLib/JacobianTest",
        "TestLib/AlgebraicLoop2Eq",
        "TestLib/SolvableBlock4Res",
        "TestLib/AlgebraicLoopWarn",
        "TestLib/SolvableBlockMultiRes",
        "TestLib/NoEventTest",
        "TestLib/NoEventInWhen",
        "TestLib/NoEventInAlg",
        "TestLib/TerminalWhen",
        "TestLib/SimpleFunctionDef",
        "TestLib/FuncInline",
        "TestLib/RecursiveFunc",
        "TestLib/AdaptiveRKTest",
        "TestLib/SmallFor",
        "TestLib/ForBound1",
        "TestLib/BigFor",
        "TestLib/AliasRemoval",
        "TestLib/BackendDaeInfo",
        "TestLib/ConstraintEq",
        "TestLib/MathBuiltins",
        "TestLib/NestedDerTest",
        "TestLib/AnnotationParse",
        "TestLib/SimpleTest",
        "TestLib/MathTest",
        "TestLib/ForTest",
        "TestLib/WhenTest",
        "TestLib/BouncingBall",
        "TestLib/BLTTest",
        "TestLib/TearingTest",
        "TestLib/ArrayTest",
        "TestLib/ArrayLoopTest",
        "TestLib/DiscreteTest",
        "TestLib/IfTest",
        "TestLib/WhileTest",
        "TestLib/AlgTest",
        "TestLib/LoopTest",
        "TestLib/LibraryTest",
        "TestLib/MSLBlocksTest",
        "TestLib/MSLTransferFunctionTest",
        "TestLib/SIunitsTest",
        "TestLib/HierarchicalMod",
        "TestLib/NestedConnect",
        "TestLib/LoopConnect",
        "TestLib/ArrayConnect",
        "TestLib/Circuit",
        "TestLib/Sub",
        "TestLib/Parent",
        "TestLib/Child",
        "TestLib/Base",
        "TestLib/Component",
        "TestLib/Container",
        "TestLib/ChildWithMod",
        "TestLib/MainPin",
        "TestLib/Pin",
        "TestLib/SubPin",
        "TestLib/VoltageSource",
        "TestLib/Resistor",
        "TestLib/TwoPin",
        "TestLib/Ground",
        "TestLib/OverdeterminedIndex2Warn",
        "TestLib/SimpleRecord",
        "TestLib/SimpleBlockTest",
        "TestLib/SimpleBlock",
        "TestLib/RecordEqTest",
        "TestLib/ConnectInWhen",
        "TestLib/MultiOutputFunc",
        "TestLib/MultiOutputNestedExpr",
        "TestLib/MultiOutputMixedArrayScalar",
        "TestLib/MultiAssignRecord",
        "TestLib/MultiAssignComprehension",
        "TestLib/MatrixOuterProduct",
        "TestLib/MatrixIdentity",
        "TestLib/MatrixSkew",
        "TestLib/MixedMultiTargetSafePass",
        "TestLib/MultiOutputShapeMismatch",
        "TestLib/MultiOutputRecordShapeMismatch",
        "TestLib/MultiOutput2DArrayShapeMismatch",
        "TestLib/MultiOutputComprehensionShapeMismatch",
        "TestLib/MultiOutputRecordNestedArrayMismatch",
        "TestLib/MultiOutputCrossLayerComprehensionMismatch",
        "TestLib/MultiOutputComplexLhsFieldStore",
        "TestLib/DeepRecordNestedMismatch",
        "TestLib/MixedNestedLhsFieldStoreMismatch",
        "TestLib/MixedMultiTargetFieldStoreFail",
        "TestLib/CrossModuleComprehensionMismatch",
        "TestLib/CrossModuleRecordCompositeMismatch",
        "TestLib/AliasChainTypeMismatch",
        "TestLib/MultiTopCombined",
        "TestLib/PreEdgeChange",
        "TestLib/IfEqTest",
        "TestLib/AssertTerminateTest",
        "TestLib/PkgA.PkgB.Inner",
        "TestLib/TypeAliasTest",
        "TestLib/ReplaceableTest",
        "TestLib/OperatorFunctionShortClassDecl",
        "TestLib/RedeclareOperatorFunctionExtendsDecl",
        "TestLib/ExpandableConnectorAliasUse",
        "TestLib/ClockedPartitionTest",
        "TestLib/ClockedTwoRates",
        "TestLib/HoldPreviousTest",
        "TestLib/IntervalClockTest",
        "TestLib/DefaultArgTest",
        "TestLib/ReinitTest",
        "TestLib/ExtLibAnnotationTest",
        "TestLib/ArrayArgTest",
        "TestLib/ExtFuncArrayArgTest",
        "TestLib/ExtFuncStringArgTest",
        "TestLib/SubSuperShiftSampleTest",
        "TestLib/BackSampleClockTest",
        "TestLib/ClockedStartAndSubSampleTest",
        "TestLib/ClockedStartAndBackSampleTest",
        "TestLib/ClockedStartShiftThenBackSampleTest",
        "TestLib/ClockedStartShiftThenSuperSampleTest",
        "TestLib/ClockedStartAndSuperSampleTest",
        "TestLib/ClockedStartShiftThenSubSampleTest",
        "TestLib/ClockedInvalidFactorClampTest",
        "TestLib/ElseWhenPriorityTest",
        "TestLib/ReinitInWhenTest",
        "TestLib/RestParamTest",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// Negative `.mo` stems under `jit-compiler/TestLib/negative/` expected to fail validation.
pub fn default_negative_fixture_stems() -> Vec<&'static str> {
    vec!["BadSyntax", "BadConnect", "UnknownTypeError"]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectnessBaseline {
    pub schema_version: u32,
    pub generated_at: String,
    pub git_head: Option<String>,
    pub validate_tier: String,
    pub validation_mode: String,
    /// Scenario id -> models that succeeded for every run in that scenario.
    pub ok: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectFailBaseline {
    pub schema_version: u32,
    pub generated_at: String,
    /// Stems validated with `TestLib/negative` as extra `--lib-path` (e.g. `BadSyntax`).
    pub negative_stems: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CorrectnessVerdict {
    Pass,
    Warn,
    Fail,
}

pub fn merge_correctness_verdict(a: CorrectnessVerdict, b: CorrectnessVerdict) -> CorrectnessVerdict {
    use CorrectnessVerdict::*;
    match (a, b) {
        (Fail, _) | (_, Fail) => Fail,
        (Warn, _) | (_, Warn) => Warn,
        _ => Pass,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectnessCompareResult {
    pub overall_verdict: CorrectnessVerdict,
    pub missing_ok: Vec<String>,
    pub new_ok: Vec<String>,
    pub expect_fail_now_pass: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullCorrectnessCompare {
    pub overall_verdict: CorrectnessVerdict,
    pub validate_perf: CorrectnessCompareResult,
    pub negative_fixtures: CorrectnessCompareResult,
}

pub fn compare_correctness_full(
    baseline_ok: &CorrectnessBaseline,
    current_report: &ValidatePerfReport,
    baseline_expect: &ExpectFailBaseline,
    unexpected_negative_passes: &[String],
) -> FullCorrectnessCompare {
    let current_ok = record_correctness_from_report(
        current_report,
        None,
        baseline_ok.validate_tier.clone(),
        baseline_ok.validation_mode.clone(),
    );
    let validate_perf = compare_correctness(baseline_ok, &current_ok);
    let negative_fixtures = compare_expect_fail(&baseline_expect.negative_stems, unexpected_negative_passes);
    let overall_verdict = merge_correctness_verdict(
        validate_perf.overall_verdict,
        negative_fixtures.overall_verdict,
    );
    FullCorrectnessCompare {
        overall_verdict,
        validate_perf,
        negative_fixtures,
    }
}

pub fn write_expect_fail_baseline(path: &std::path::Path) -> Result<()> {
    let bl = ExpectFailBaseline {
        schema_version: 1,
        generated_at: chrono::Utc::now().to_rfc3339(),
        negative_stems: default_negative_fixture_stems()
            .into_iter()
            .map(String::from)
            .collect(),
    };
    let t = serde_json::to_string_pretty(&bl)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, t).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Build `ok` map: model is listed for scenario `s` iff every case `(s, model, *)` has `success`.
pub fn ok_map_from_report(report: &ValidatePerfReport) -> BTreeMap<String, Vec<String>> {
    let mut failed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for c in &report.cases {
        if !c.success {
            failed
                .entry(c.scenario.clone())
                .or_default()
                .insert(c.model.clone());
        }
    }
    let mut scenarios: BTreeSet<String> = BTreeSet::new();
    for c in &report.cases {
        scenarios.insert(c.scenario.clone());
    }
    let mut ok: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for sc in scenarios {
        let mut models: BTreeSet<String> = BTreeSet::new();
        for c in &report.cases {
            if c.scenario == sc {
                models.insert(c.model.clone());
            }
        }
        let bad = failed.get(&sc).cloned().unwrap_or_default();
        let passed: Vec<String> = models
            .into_iter()
            .filter(|m| !bad.contains(m))
            .collect();
        ok.insert(sc, passed);
    }
    ok
}

pub fn record_correctness_from_report(
    report: &ValidatePerfReport,
    git_head: Option<String>,
    validate_tier: impl Into<String>,
    validation_mode: impl Into<String>,
) -> CorrectnessBaseline {
    CorrectnessBaseline {
        schema_version: 1,
        generated_at: report.generated_at.clone(),
        git_head,
        validate_tier: validate_tier.into(),
        validation_mode: validation_mode.into(),
        ok: ok_map_from_report(report),
    }
}

fn parse_success_from_child_output(stdout: &str, stderr: &str) -> bool {
    parse_validate_success(stdout, stderr)
}

/// Run `--validate --validate-tier=analyze` on each negative stem with `negative/` as extra lib path.
/// Returns stems that **incorrectly** succeeded (exit 0 and JSON `success: true`).
pub fn negative_stems_that_pass_validate(
    exe: &Path,
    lib_paths: &[std::path::PathBuf],
    testlib_root: &Path,
) -> Result<Vec<String>> {
    let neg_dir = testlib_root.join("negative");
    if !neg_dir.is_dir() {
        bail!("missing {}", neg_dir.display());
    }
    let mut unexpected_pass = Vec::new();
    for stem in default_negative_fixture_stems() {
        let mut cmd = Command::new(exe);
        for lp in lib_paths {
            cmd.arg(format!("--lib-path={}", lp.display()));
        }
        cmd.arg(format!("--lib-path={}", neg_dir.display()));
        cmd.arg("--validate");
        cmd.arg("--validate-tier=analyze");
        cmd.arg(stem);
        let out = cmd.output().with_context(|| format!("negative validate {}", stem))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let ok_json = parse_success_from_child_output(&stdout, &stderr);
        if out.status.success() && ok_json {
            unexpected_pass.push(stem.to_string());
        }
    }
    Ok(unexpected_pass)
}

pub fn ensure_negative_fixtures_fail(
    exe: &Path,
    lib_paths: &[std::path::PathBuf],
    testlib_root: &Path,
) -> Result<()> {
    let bad = negative_stems_that_pass_validate(exe, lib_paths, testlib_root)?;
    if !bad.is_empty() {
        bail!(
            "negative fixtures unexpectedly passed validation (should fail): {}",
            bad.join(", ")
        );
    }
    Ok(())
}

pub fn compare_correctness(
    baseline: &CorrectnessBaseline,
    current: &CorrectnessBaseline,
) -> CorrectnessCompareResult {
    let mut missing_ok: Vec<String> = Vec::new();
    let mut new_ok: Vec<String> = Vec::new();

    for (sc, b_models) in &baseline.ok {
        let cur_set: BTreeSet<String> = current
            .ok
            .get(sc)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();
        let base_set: BTreeSet<String> = b_models.iter().cloned().collect();
        for m in &base_set {
            if !cur_set.contains(m) {
                missing_ok.push(format!("{sc}/{m}"));
            }
        }
        for m in &cur_set {
            if !base_set.contains(m) {
                new_ok.push(format!("{sc}/{m}"));
            }
        }
    }
    for (sc, c_models) in &current.ok {
        if !baseline.ok.contains_key(sc) {
            for m in c_models {
                new_ok.push(format!("{sc}/{m}"));
            }
        }
    }

    let mut verdict = CorrectnessVerdict::Pass;
    if !new_ok.is_empty() {
        verdict = CorrectnessVerdict::Warn;
    }
    if !missing_ok.is_empty() {
        verdict = CorrectnessVerdict::Fail;
    }

    let summary = format!(
        "missing_ok={} new_ok={} expect_fail_now_pass=0",
        missing_ok.len(),
        new_ok.len()
    );

    CorrectnessCompareResult {
        overall_verdict: verdict,
        missing_ok,
        new_ok,
        expect_fail_now_pass: Vec::new(),
        summary,
    }
}

/// Compare negative stems: baseline lists stems that must still fail; current run must not report success.
pub fn compare_expect_fail(
    baseline_stems: &[String],
    unexpected_pass: &[String],
) -> CorrectnessCompareResult {
    let expect_fail_now_pass: Vec<String> = unexpected_pass.to_vec();
    let mut verdict = CorrectnessVerdict::Pass;
    if !expect_fail_now_pass.is_empty() {
        verdict = CorrectnessVerdict::Fail;
    }
    let summary = format!(
        "negative_stems_baseline={} unexpected_pass={}",
        baseline_stems.len(),
        expect_fail_now_pass.len()
    );
    CorrectnessCompareResult {
        overall_verdict: verdict,
        missing_ok: Vec::new(),
        new_ok: Vec::new(),
        expect_fail_now_pass,
        summary,
    }
}

pub fn save_correctness_baseline(path: &Path, b: &CorrectnessBaseline) -> Result<()> {
    let t = serde_json::to_string_pretty(b)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, t).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn load_correctness_baseline(path: &Path) -> Result<CorrectnessBaseline> {
    let t = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(serde_json::from_str(&t)?)
}

pub fn load_expect_fail_baseline(path: &Path) -> Result<ExpectFailBaseline> {
    let t = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(serde_json::from_str(&t)?)
}
