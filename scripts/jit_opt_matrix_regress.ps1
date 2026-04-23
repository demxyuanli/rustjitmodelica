# Runs jit validate-perf under several env "profiles", compares each report to the
# default JIT perf baseline, and writes per-profile artifacts plus a summary JSON.
# Usage (from repo root):
#   rtk pwsh -File scripts/jit_opt_matrix_regress.ps1
#   rtk pwsh -File scripts/jit_opt_matrix_regress.ps1 -Quick
# Requires: target/release/regress-harness.exe, target/release/rustmodlica.exe

param(
    [string]$RepoRoot = "",
    [string]$OutParent = "build/jit_opt_matrix",
    [switch]$Quick,
    [string[]]$ProfileFilter = @()
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-RepoRoot {
    if (-not [string]::IsNullOrWhiteSpace($RepoRoot)) {
        if ([System.IO.Path]::IsPathRooted($RepoRoot)) { return $RepoRoot.TrimEnd('\', '/') }
        return (Resolve-Path -LiteralPath (Join-Path (Get-Location) $RepoRoot)).Path
    }
    $here = $PSScriptRoot
    return (Resolve-Path -LiteralPath (Join-Path $here "..")).Path
}

$root = Resolve-RepoRoot
$rh = Join-Path $root "target/release/regress-harness.exe"
$exe = Join-Path $root "target/release/rustmodlica.exe"
$lib = Join-Path $root "jit-compiler"
$baseline = Join-Path $root "baseline/20260418_three_tier_devloop/jit_perf_baseline.json"

foreach ($p in @($rh, $exe, $lib, $baseline)) {
    if (-not (Test-Path -LiteralPath $p)) {
        Write-Error "Missing required path: $p"
    }
}

$models = @(
    "ModelicaTest.JitStress.ComplexJitRegression",
    "ModelicaTest.JitStress.MslBroadCoverage",
    "ModelicaTest.JitStress.RobotElectricalControl",
    "TestLib.BigFor",
    "TestLib.MSLBlocksTest",
    "TestLib.MultiTopCombined"
) -join ","

$profiles = @(
    @{ id = "cranelift_none"; label = "Cranelift opt none (matches default baseline)"; args = @("--set-env", "RUSTMODLICA_CRANELIFT_OPT_LEVEL=none") },
    @{ id = "cranelift_speed"; label = "Cranelift opt speed (default-ish JIT IR)"; args = @("--set-env", "RUSTMODLICA_CRANELIFT_OPT_LEVEL=speed") },
    @{ id = "adaptive_fold"; label = "Adaptive const-fold policy"; args = @("--set-env", "RUSTMODLICA_ADAPTIVE_FOLD_POLICY=1") },
    @{ id = "speed_plus_adaptive"; label = "Cranelift speed + adaptive fold"; args = @("--set-env", "RUSTMODLICA_CRANELIFT_OPT_LEVEL=speed", "--set-env", "RUSTMODLICA_ADAPTIVE_FOLD_POLICY=1") },
    @{ id = "flatten_eq_parallel"; label = "Flatten equation parallel"; args = @("--set-env", "RUSTMODLICA_FLATTEN_EQ_PARALLEL=1") },
    @{ id = "jit_stub_parallel"; label = "JIT stub parallel"; args = @("--set-env", "RUSTMODLICA_JIT_STUB_PARALLEL=1") }
)

if ($Quick) {
    $profiles = @($profiles[0], $profiles[1], $profiles[2])
}

if ($ProfileFilter.Count -gt 0) {
    $set = [System.Collections.Generic.HashSet[string]]::new([string[]]$ProfileFilter, [StringComparer]::OrdinalIgnoreCase)
    $profiles = @($profiles | Where-Object { $set.Contains($_.id) })
}

$outRoot = if ([System.IO.Path]::IsPathRooted($OutParent)) { $OutParent } else { Join-Path $root $OutParent }
New-Item -ItemType Directory -Force -Path $outRoot | Out-Null

$matrix = [ordered]@{ generated_at = (Get-Date).ToString("o"); repo_root = $root; profiles = @() }

foreach ($pr in $profiles) {
    $id = [string]$pr.id
    $dir = Join-Path $outRoot $id
    New-Item -ItemType Directory -Force -Path $dir | Out-Null

    $common = @(
        "jit", "validate-perf",
        "--exe", $exe,
        "--lib-path", $lib,
        "--out-dir", $dir,
        "--validate-tier", "full",
        "--validation-mode", "full",
        "--models", $models,
        "--hot-runs", "2",
        "--stage-trace",
        "--perf-trace",
        "--scenarios", "stdlib_bake,devloop_multi_model",
        "--purge-scenario-caches"
    ) + [string[]]$pr.args

    Write-Host "=== validate-perf profile=$id ==="
    & $rh @common
    if ($LASTEXITCODE -ne 0) {
        Write-Error "validate-perf failed for profile $id (exit $LASTEXITCODE)"
    }

    $report = Join-Path $dir "report.json"
    $cmpOut = Join-Path $dir "compare_baseline.json"
    $cmpErr = Join-Path $dir "compare_baseline.stderr.txt"
    Write-Host "=== compare-baseline profile=$id ==="
    $oldEa = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    # NativeCommandHandler writes to stderr; merge breaks JSON — capture stdout only.
    $p = Start-Process -FilePath $rh -ArgumentList @(
        "jit", "compare-baseline", "--report", $report, "--baseline", $baseline
    ) -NoNewWindow -Wait -PassThru -RedirectStandardOutput $cmpOut -RedirectStandardError $cmpErr
    $cmpExit = $p.ExitCode
    $ErrorActionPreference = $oldEa
    $cmpText = [System.IO.File]::ReadAllText($cmpOut)

    $verdict = $null
    try {
        $obj = $cmpText | ConvertFrom-Json
        $verdict = $obj.overall_verdict
    }
    catch {
        $verdict = "parse_error"
    }

    $failedKeys = @()
    $warnKeys = @()
    try {
        $obj = $cmpText | ConvertFrom-Json
        foreach ($c in $obj.comparisons) {
            if ($c.verdict -eq "Fail") { $failedKeys += $c.key }
            if ($c.verdict -eq "Warn") { $warnKeys += $c.key }
        }
    }
    catch { }

    $speedFails = @()
    $speedWarns = @()
    try {
        $obj = $cmpText | ConvertFrom-Json
        foreach ($s in $obj.speedup_checks) {
            if ($s.verdict -eq "Fail") { $speedFails += $s.model }
            if ($s.verdict -eq "Warn") { $speedWarns += $s.model }
        }
    }
    catch { }

    $matrix.profiles += [ordered]@{
        id                     = $id
        label                  = $pr.label
        out_dir                = $dir
        compare_exit           = $cmpExit
        overall_verdict        = $verdict
        failed_benchmark_keys  = @($failedKeys)
        warned_benchmark_keys  = @($warnKeys)
        failed_speedup_models  = @($speedFails)
        warned_speedup_models  = @($speedWarns)
    }

    Write-Host "profile=$id verdict=$verdict compare_exit=$cmpExit"
}

$summaryPath = Join-Path $outRoot "matrix_summary.json"
$matrix | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Host "Wrote $summaryPath"

# Short interpretation for CI / logs
foreach ($row in $matrix.profiles) {
    if ($row.compare_exit -ne 0 -or $row.overall_verdict -eq "Fail") {
        Write-Host "[analysis] $($row.id): FAIL — see compare_baseline.json (duration regression vs baseline, or speedup_checks Fail)."
    }
    elseif ($row.failed_benchmark_keys.Count -gt 0) {
        Write-Host "[analysis] $($row.id): benchmark Fail entries: $($row.failed_benchmark_keys -join ', ')"
    }
    elseif ($row.warned_benchmark_keys.Count -gt 0 -or $row.warned_speedup_models.Count -gt 0) {
        Write-Host "[analysis] $($row.id): WARN — wall/codegen marginal deltas or cold/hot speedup marginal; models: $($row.warned_speedup_models -join ', ')"
    }
    else {
        Write-Host "[analysis] $($row.id): PASS (within baseline gates)."
    }
}
