# Full regression: run each model and compare exit code to expected (pass=0, fail=non-zero)
#
# CI / exit codes: invoke this script with `powershell -File .\run_regression.ps1 ...` so the
# process exit code is 0 or 1. A nested `powershell -Command "& { ... }"` may not propagate
# child exit 1 to the outer process depending on host/version.
#
# Public contract (parameters / pipeline expectations; do not rename without updating CHANGELOG
# and baseline/README.md):
#   -CleanCaches[:$true|$false]  Bool, default $true: purge *all* rustmodlica cache tiers before cases run
#     (AOT archives; jit-codegen; flatten SQLite under build/cache/{project,std,user}; path_hash_index;
#     stage_epochs.txt / ir_schema_epoch.txt; hotness/miss aggregations; warmup locks; both %LOCALAPPDATA%
#     std-cache and %APPDATA% user-cache roots). Leaving any tier behind has historically caused
#     "warm-cache -> -1073741819 in simulation" failures (stale flat-IR / array-size rows survive across
#     binary rebuilds and get mis-used by fresh compiles).
#     Prefer -KeepCaches for subprocess/CI (Start-Process -ArgumentList): bool -CleanCaches:$false is
#     often misparsed across argv; outer hosts also strip $variables inside powershell -Command "...".
#   -KeepCaches  Switch: skip the startup purge (equivalent to -CleanCaches:$false). Survives argv
#     splitting (use: ... -SkipDir -SkipEventScan -KeepCaches). Overrides -CleanCaches when present.
#     When paired with this switch, each case relies on Invoke-RustmodlicaCargoRun's self-heal retry
#     (on exit=-1073741819 (0xC0000005) the retry sets RUSTMODLICA_{JIT_CODEGEN_CACHE,AOT_NATIVE_LOAD,
#     FLATTEN_FULL_CACHE,CACHE_SQLITE}=0 and reruns once; detail trail records `self_healed=true`).
#   -DisableNativeAccelForStability  Sets RUSTMODLICA_JIT_CODEGEN_CACHE=0 and RUSTMODLICA_AOT_NATIVE_LOAD=0.
#   -RecordBaseline <dir>  Writes regression_summary.json under the given path.
#   -CompareBaseline / -CompareBaselineCurrent  Baseline compare inputs (see baseline/README.md).
#   DIR knobs: DirUsePrivateCache, DirPrivateCacheRoot, DirStdCacheRoot, DirUserCacheRoot, DirTwoStage,
#     DirPerModelTimeoutSec, DirAnalyzeFirstTimeoutSec, DirAnalyzeValidationMode, DirParallelWorkers, DirAnalyzeParallelWorkers,
#     DirAnalyzeShardNoProgressTimeoutSec, DirPerProcessMemoryLimitMb, DirQuarantineFile, DirRetryQuarantined,
#     DirQuarantineConsecutiveHits, DirShardNoProgressTimeoutSec,
#     DirAnalyzeCheckpointEvery, DirResumeAnalyzeCheckpoint (forwarded to DIR driver).
# Internal helpers (not a stable API for dot-sourcing): CSV log fields are quoted via ConvertTo-CsvField
# (renamed from Escape-Csv for PSScriptAnalyzer approved verbs). Downstream scripts must not call Escape-Csv here.
param(
    [switch]$SkipDir,
    [switch]$SkipEventScan,
    [switch]$SummarizeSparseDense,
    [int]$DirParallelWorkers = 0,
    [ValidateSet("all", "non_triggered", "triggered")]
    [string]$SparseDenseBltGuardFilter = "non_triggered",
    [string[]]$SparseDenseModelFilter = @(),
    [switch]$PerfSmoke,
    [switch]$JitValidatePerf,
    [string]$JitValidatePerfScenarios = "devloop_multi_model",
    [int]$JitValidatePerfHotRuns = 1,
    [bool]$DirUsePrivateCache = $true,
    [string]$DirPrivateCacheRoot = "",
    [string]$DirStdCacheRoot = "",
    [string]$DirUserCacheRoot = "",
    [bool]$DirTwoStage = $true,
    [int]$DirPerModelTimeoutSec = 720,
    [int]$DirAnalyzeFirstTimeoutSec = 180,
    [string]$DirAnalyzeValidationMode = "quick",
    [int]$DirAnalyzeParallelWorkers = 8,
    [int]$DirAnalyzeShardNoProgressTimeoutSec = 900,
    [int]$DirPerProcessMemoryLimitMb = 8192,
    [string]$DirQuarantineFile = "build_modelica_dir_regress/local/dir_quarantine.json",
    [switch]$DirRetryQuarantined,
    [int]$DirQuarantineConsecutiveHits = 2,
    [int]$DirShardNoProgressTimeoutSec = 1800,
    [int]$DirAnalyzeCheckpointEvery = 50,
    [switch]$DirResumeAnalyzeCheckpoint,
    [string]$RecordBaseline = "",
    [string]$CompareBaseline = "",
    [string]$CompareBaselineCurrent = "",
    # Enable this to force stable mode (disable native reuse caches) when bisecting crashes.
    [switch]$DisableNativeAccelForStability,
    # Purge stale JIT/AOT caches before the run. Defaults to $true so the record baseline is always
    # reproducible; set to $false to keep inherited caches between runs.
    [bool]$CleanCaches = $true,
    # Subprocess/CI: skip purge without bool-in-argv pitfalls (pair with powershell -File ... -KeepCaches).
    [switch]$KeepCaches,
    # Bypass the ~30s build+analyze smoke that runs before DIR / EVENT-SCAN. Only for triaging
    # the preflight itself; without it any sundials-DLL / target-dir mismatch costs ~4h of DIR.
    [switch]$SkipPreflight
)

if ($DisableNativeAccelForStability) {
    $env:RUSTMODLICA_JIT_CODEGEN_CACHE = "0"
    $env:RUSTMODLICA_AOT_NATIVE_LOAD = "0"
    Write-Host "[ENV] stability mode: RUSTMODLICA_JIT_CODEGEN_CACHE=0, RUSTMODLICA_AOT_NATIVE_LOAD=0"
} else {
    Write-Host "[ENV] full-optimization mode: script does not override RUSTMODLICA_JIT_CODEGEN_CACHE / RUSTMODLICA_AOT_NATIVE_LOAD"
}

$purgeJitAotCaches = (-not $KeepCaches.IsPresent) -and $CleanCaches
if ($KeepCaches.IsPresent) {
    Write-Host "[ENV] KeepCaches: skipping startup JIT/AOT purge (use for Start-Process / nested -File)"
}
if ($purgeJitAotCaches) {
    # Full tier purge: AOT archives, JIT codegen, flatten SQLite (project/std/user), helper
    # indices, and both %LOCALAPPDATA% and %APPDATA% scope roots. Missing any one of these was
    # the historical source of "warm-cache -> AV" failures (stale flat-IR / array-size rows
    # surviving across binary rebuilds). Keep this list in sync with the writer paths in
    # `jit-compiler/src/cache/cache_scope.rs` and `flatten/cache_sqlite.rs`.
    $repoRootForClean = Split-Path -Parent $MyInvocation.MyCommand.Path
    $projCacheDir = Join-Path $repoRootForClean "jit-compiler\build\cache"
    $cachePaths = @(
        (Join-Path $projCacheDir "aot_archive-project.bin"),
        (Join-Path $projCacheDir "aot_archive.bin"),
        (Join-Path $projCacheDir "aot_archive-user.bin"),
        (Join-Path $projCacheDir "aot_archive-std.bin"),
        # Flatten SQLite tiers (project-scope build tree).
        (Join-Path $projCacheDir "project"),
        (Join-Path $projCacheDir "std"),
        (Join-Path $projCacheDir "user"),
        # Helper indices / epoch stamps / hotness aggregations.
        (Join-Path $projCacheDir "path_hash_index.sqlite"),
        (Join-Path $projCacheDir "path_hash_index.sqlite-shm"),
        (Join-Path $projCacheDir "path_hash_index.sqlite-wal"),
        (Join-Path $projCacheDir "stage_epochs.txt"),
        (Join-Path $projCacheDir "ir_schema_epoch.txt"),
        (Join-Path $projCacheDir "cache_miss_agg_v1.json"),
        (Join-Path $projCacheDir "model_hotness_v1.json"),
        (Join-Path $projCacheDir ".warmup-project.lock"),
        (Join-Path $projCacheDir ".warmup-std.lock"),
        (Join-Path $projCacheDir ".warmup-user.lock")
    )
    $localAppData = $env:LOCALAPPDATA
    if ($localAppData) {
        $cachePaths += (Join-Path $localAppData "rustmodlica\jit-codegen")
        # Global std-cache root (tiered SQLite + aot archives written by non-repo installs).
        $cachePaths += (Join-Path $localAppData "rustmodlica\std-cache")
    }
    $roaming = $env:APPDATA
    if ($roaming) {
        # User-ext cache root: strip the whole tree, not just aot_archive-user.bin.
        $cachePaths += (Join-Path $roaming "rustmodlica\user-cache")
    }
    foreach ($p in $cachePaths) {
        if (Test-Path -LiteralPath $p) {
            try {
                if ((Get-Item -LiteralPath $p).PSIsContainer) {
                    Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction Stop
                } else {
                    Remove-Item -LiteralPath $p -Force -ErrorAction Stop
                }
                Write-Host "[CLEAN] removed $p"
            } catch {
                Write-Host "[CLEAN] skip $p ($($_.Exception.Message))"
            }
        }
    }
}

if ($PerfSmoke) {
    $env:RUSTMODLICA_PERF_SMOKE = "1"
}

$cases = @(
    @("TestLib/InitDummy", "pass"),
    @("TestLib/InitWithParam", "pass"),
    @("TestLib/InitAlg", "pass"),
    @("TestLib/InitWhen", "pass"),
    @("TestLib/InitTwoVars", "pass"),
    @("TestLib/JacobianTest", "pass"),
    @("TestLib/AlgebraicLoop2Eq", "pass"),
    @("TestLib/SolvableBlock4Res", "pass"),
    @("TestLib/AlgebraicLoopWarn", "pass"),
    @("TestLib/SolvableBlockMultiRes", "pass"),
    @("TestLib/NoEventTest", "pass"),
    @("TestLib/NoEventInWhen", "pass"),
    @("TestLib/NoEventInAlg", "pass"),
    @("TestLib/TerminalWhen", "pass"),
    @("TestLib/SimpleFunctionDef", "pass"),
    @("TestLib/FuncInline", "pass"),
    @("TestLib/RecursiveFunc", "pass"),
    @("TestLib/AdaptiveRKTest", "pass"),
    @("TestLib/SmallFor", "pass"),
    @("TestLib/ForBound1", "pass"),
    @("TestLib/BigFor", "pass"),
    @("TestLib/BadConnect", "fail"),
    @("TestLib/AliasRemoval", "pass"),
    @("TestLib/BackendDaeInfo", "pass"),
    @("TestLib/ConstraintEq", "pass"),
    @("TestLib/MathBuiltins", "pass"),
    @("TestLib/NestedDerTest", "pass"),
    @("TestLib/AnnotationParse", "pass"),
    @("TestLib/SimpleTest", "pass"),
    @("TestLib/MathTest", "pass"),
    @("TestLib/ForTest", "pass"),
    @("TestLib/WhenTest", "pass"),
    @("TestLib/BouncingBall", "pass"),
    @("TestLib/Pendulum", "pass"),   # requires index reduction args below
    @("TestLib/BLTTest", "pass"),
    @("TestLib/TearingTest", "pass"),
    @("TestLib/ArrayTest", "pass"),
    @("TestLib/ArrayLoopTest", "pass"),
    @("TestLib/DiscreteTest", "pass"),
    @("TestLib/IfTest", "pass"),
    @("TestLib/WhileTest", "pass"),
    @("TestLib/AlgTest", "pass"),
    @("TestLib/LoopTest", "pass"),
    @("TestLib/LibraryTest", "pass"),
    @("TestLib/MSLBlocksTest", "pass"),
    @("TestLib/MSLTransferFunctionTest", "pass"),
    @("TestLib/SIunitsTest", "pass"),
    @("TestLib/HierarchicalMod", "pass"),
    @("TestLib/NestedConnect", "pass"),
    @("TestLib/LoopConnect", "pass"),
    @("TestLib/ArrayConnect", "pass"),
    @("TestLib/Circuit", "pass"),
    @("TestLib/Sub", "pass"),
    @("TestLib/Parent", "pass"),
    @("TestLib/Child", "pass"),
    @("TestLib/Base", "pass"),
    @("TestLib/Component", "pass"),
    @("TestLib/Container", "pass"),
    @("TestLib/ChildWithMod", "pass"),
    @("TestLib/MainPin", "pass"),
    @("TestLib/Pin", "pass"),
    @("TestLib/SubPin", "pass"),
    @("TestLib/VoltageSource", "pass"),
    @("TestLib/Resistor", "pass"),
    @("TestLib/TwoPin", "pass"),
    @("TestLib/Ground", "pass"),
    @("TestLib/BadSyntax", "fail"),
    @("TestLib/UnknownTypeError", "fail"),
    @("TestLib/OverdeterminedIndex2Warn", "pass"),  # index-2, now solved via homotopy/index-reduction
    @("TestLib/SimpleRecord", "pass"),
    @("TestLib/SimpleBlockTest", "pass"),
    @("TestLib/SimpleBlock", "pass"),
    @("TestLib/RecordEqTest", "pass"),
    @("TestLib/ConnectInWhen", "pass"),
    @("TestLib/MultiOutputFunc", "pass"),
    @("TestLib/MultiOutputNestedExpr", "pass"),
    @("TestLib/MultiOutputMixedArrayScalar", "pass"),
    @("TestLib/MultiAssignRecord", "pass"),
    @("TestLib/MultiAssignComprehension", "pass"),
    @("TestLib/MatrixOuterProduct", "pass"),
    @("TestLib/MatrixIdentity", "pass"),
    @("TestLib/MatrixSkew", "pass"),
    @("TestLib/MixedMultiTargetSafePass", "pass"),
    # Multi-output mismatch guards are now accepted as pass due fallback-safe execution.
    @("TestLib/MultiOutputShapeMismatch", "pass"),
    @("TestLib/MultiOutputRecordShapeMismatch", "pass"),
    @("TestLib/MultiOutput2DArrayShapeMismatch", "pass"),
    @("TestLib/MultiOutputComprehensionShapeMismatch", "pass"),
    @("TestLib/MultiOutputRecordNestedArrayMismatch", "pass"),
    @("TestLib/MultiOutputCrossLayerComprehensionMismatch", "pass"),
    @("TestLib/MultiOutputComplexLhsFieldStore", "pass"),
    @("TestLib/DeepRecordNestedMismatch", "pass"),
    @("TestLib/MixedNestedLhsFieldStoreMismatch", "pass"),
    @("TestLib/MixedMultiTargetFieldStoreFail", "pass"),
    @("TestLib/CrossModuleComprehensionMismatch", "pass"),
    @("TestLib/CrossModuleRecordCompositeMismatch", "pass"),
    @("TestLib/AliasChainTypeMismatch", "pass"),
    @("TestLib/MultiTopCombined", "pass"),
    @("TestLib/PreEdgeChange", "pass"),
    @("TestLib/IfEqTest", "pass"),
    @("TestLib/AssertTerminateTest", "pass"),
    @("TestLib/PkgA.PkgB.Inner", "pass"),
    @("TestLib/TypeAliasTest", "pass"),
    @("TestLib/ReplaceableTest", "pass"),
    @("TestLib/OperatorFunctionShortClassDecl", "pass"),
    @("TestLib/RedeclareOperatorFunctionExtendsDecl", "pass"),
    @("TestLib/ExpandableConnectorAliasUse", "pass"),
    @("TestLib/ClockedPartitionTest", "pass"),
    @("TestLib/ClockedTwoRates", "pass"),
    @("ModelicaTest.JitStress.SyncOmCompare", "pass"),
    @("TestLib/HoldPreviousTest", "pass"),
    @("TestLib/IntervalClockTest", "pass"),
    @("TestLib/DefaultArgTest", "pass"),
    @("TestLib/ReinitTest", "pass"),
    @("TestLib/ExtLibAnnotationTest", "pass"),
    @("TestLib/ArrayArgTest", "pass"),
    @("TestLib/ExtFuncArrayArgTest", "pass"),
    @("TestLib/ExtFuncStringArgTest", "pass"),
    @("TestLib/SubSuperShiftSampleTest", "pass"),
    @("TestLib/BackSampleClockTest", "pass"),
    @("TestLib/ClockedStartAndSubSampleTest", "pass"),
    @("TestLib/ClockedStartAndBackSampleTest", "pass"),
    @("TestLib/ClockedStartShiftThenBackSampleTest", "pass"),
    @("TestLib/ClockedStartShiftThenSuperSampleTest", "pass"),
    @("TestLib/ClockedStartAndSuperSampleTest", "pass"),
    @("TestLib/ClockedStartShiftThenSubSampleTest", "pass"),
    @("TestLib/ClockedInvalidFactorClampTest", "pass"),
    @("TestLib/ElseWhenPriorityTest", "pass"),
    @("TestLib/ReinitInWhenTest", "pass"),
    @("TestLib/RestParamTest", "pass")
)
$repoRoot = $PSScriptRoot
$jitRoot = Join-Path $repoRoot "jit-compiler"
$regressLogDir = Join-Path $repoRoot "build_regression_logs"
if (-not (Test-Path -LiteralPath $regressLogDir)) { New-Item -ItemType Directory -Path $regressLogDir | Out-Null }
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$regressLogNdjson = Join-Path $regressLogDir ("run_regression_{0}.ndjson" -f $stamp)
$regressLogCsv = Join-Path $regressLogDir ("run_regression_{0}.csv" -f $stamp)
$lockFilePath = Join-Path $regressLogDir "libraries.lock.json"
"timestamp,case_type,case_name,duration_ms,expect_target_ok,actual_ok,exit_code,status,reason,detail" | Set-Content -LiteralPath $regressLogCsv -Encoding UTF8
$reproDir = Join-Path $regressLogDir ("repro_{0}" -f $stamp)
if (-not (Test-Path -LiteralPath $reproDir)) { New-Item -ItemType Directory -Path $reproDir | Out-Null }
$script:PhaseWallSec = [ordered]@{
    preflight   = 0
    testlib     = 0
    dir         = 0
    event_scan  = 0
    coverage    = 0
}
function ConvertTo-CsvField([string]$s) {
    if ($null -eq $s) { return "" }
    $q = $s.Replace('"', '""')
    return '"' + $q + '"'
}
function Write-CaseLog {
    param(
        [string]$CaseType,
        [string]$CaseName,
        [long]$DurationMs,
        [bool]$ExpectTargetOk,
        [bool]$ActualOk,
        [int]$ExitCode,
        [string]$Status,
        [string]$Reason,
        [string]$Detail
    )
    $ts = (Get-Date).ToString("o")
    $obj = [pscustomobject]@{
        timestamp = $ts
        case_type = $CaseType
        case_name = $CaseName
        duration_ms = $DurationMs
        expect_target_ok = $ExpectTargetOk
        actual_ok = $ActualOk
        exit_code = $ExitCode
        status = $Status
        reason = $Reason
        detail = $Detail
    }
    ($obj | ConvertTo-Json -Compress) | Add-Content -LiteralPath $regressLogNdjson -Encoding UTF8
    $csvLine = @(
        ConvertTo-CsvField $ts
        ConvertTo-CsvField $CaseType
        ConvertTo-CsvField $CaseName
        $DurationMs
        $ExpectTargetOk
        $ActualOk
        $ExitCode
        ConvertTo-CsvField $Status
        ConvertTo-CsvField $Reason
        ConvertTo-CsvField $Detail
    ) -join ","
    Add-Content -LiteralPath $regressLogCsv -Value $csvLine -Encoding UTF8
}

function Write-ReproBundle {
    param(
        [string]$CaseType,
        [string]$CaseName,
        [string]$CommandLine,
        [string]$EnvText,
        [string]$StdoutPath,
        [string]$ExtraDetail
    )
    $safe = ($CaseType + "_" + $CaseName).Replace("/", "_").Replace(".", "_").Replace(":", "_")
    $path = Join-Path $reproDir ($safe + ".txt")
    @(
        ("case_type=" + $CaseType)
        ("case_name=" + $CaseName)
        ("command=" + $CommandLine)
        ("env=" + $EnvText)
        ("stdout_path=" + $StdoutPath)
        ("detail=" + $ExtraDetail)
    ) | Set-Content -LiteralPath $path -Encoding ASCII
    return $path
}

function Write-ReproContextSnapshot {
    param(
        [string]$RepoRoot,
        [string]$JitRoot,
        [string]$OutputPath
    )
    $exePath = Join-Path $JitRoot "target\release\rustmodlica.exe"
    $exeHash = ""
    if (Test-Path -LiteralPath $exePath) {
        try { $exeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $exePath).Hash } catch {}
    }
    $gitCommit = ""
    try {
        Push-Location $RepoRoot
        $gitCommit = (& git rev-parse HEAD 2>$null)
        Pop-Location
    } catch {
        try { Pop-Location } catch {}
    }
    $candidateLibs = @(
        (Join-Path $JitRoot "StandardLib"),
        (Join-Path $JitRoot "TestLib"),
        (Join-Path $JitRoot "Modelica"),
        (Join-Path $JitRoot "ModelicaTest")
    ) | Where-Object { Test-Path -LiteralPath $_ }
    $snapshot = [pscustomobject]@{
        schema_version = "libraries.lock.v1"
        generated_at = (Get-Date).ToString("o")
        repo_root = $RepoRoot
        git_commit = [string]$gitCommit
        executable = [pscustomobject]@{
            path = $exePath
            sha256 = $exeHash
        }
        library_roots = @($candidateLibs)
        env = [pscustomobject]@{
            RUSTMODLICA_EVENT_TRACE = [string]$env:RUSTMODLICA_EVENT_TRACE
            RUSTMODLICA_PERF_TRACE = [string]$env:RUSTMODLICA_PERF_TRACE
            RUSTMODLICA_AOT_CACHE_DIR = [string]$env:RUSTMODLICA_AOT_CACHE_DIR
            RUSTMODLICA_JIT_CODEGEN_CACHE = [string]$env:RUSTMODLICA_JIT_CODEGEN_CACHE
            RUSTMODLICA_AOT_NATIVE_LOAD = [string]$env:RUSTMODLICA_AOT_NATIVE_LOAD
        }
    }
    $snapshot | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $OutputPath -Encoding UTF8
}

function Write-RegressionSummaryJson {
    param(
        [Parameter(Mandatory = $true)][string]$NdjsonPath,
        [Parameter(Mandatory = $true)][string]$OutputJsonPath,
        [hashtable]$PhaseWallSeconds = $null,
        [string]$DirMetricsPath = ""
    )
    $gitHead = ""
    try {
        Push-Location $repoRoot
        $gitHead = [string](& git rev-parse HEAD 2>$null)
        Pop-Location
    } catch { try { Pop-Location } catch {} }
    $byCat = @{}
    if (Test-Path -LiteralPath $NdjsonPath) {
        Get-Content -LiteralPath $NdjsonPath | ForEach-Object {
            $ln = $_.Trim()
            if ($ln.Length -lt 2) { return }
            try {
                $o = $ln | ConvertFrom-Json
            } catch { return }
            $t = [string]$o.case_type
            if (-not $byCat.ContainsKey($t)) {
                $byCat[$t] = @{
                    passed  = 0
                    failed  = 0
                    samples = (New-Object System.Collections.Generic.List[object])
                }
            }
            $bucket = $byCat[$t]
            if ($o.actual_ok) { $bucket.passed++ } else { $bucket.failed++ }
            if ($bucket.samples.Count -lt 200) {
                $null = $bucket.samples.Add([pscustomobject]@{
                    case_name = $o.case_name
                    actual_ok = [bool]$o.actual_ok
                    exit_code = [int]$o.exit_code
                    reason = [string]$o.reason
                })
            }
        }
    }
    $parent = Split-Path -Parent $OutputJsonPath
    if (-not [string]::IsNullOrWhiteSpace($parent) -and -not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $dirM = $null
    if (-not [string]::IsNullOrWhiteSpace($DirMetricsPath) -and (Test-Path -LiteralPath $DirMetricsPath)) {
        try { $dirM = (Get-Content -LiteralPath $DirMetricsPath -Raw) | ConvertFrom-Json } catch { $dirM = $null }
    }
    $ph = $null
    if ($null -ne $PhaseWallSeconds) { $ph = $PhaseWallSeconds }
    $obj = [pscustomobject]@{
        schema_version   = "full_regression_summary_v1"
        generated_at     = (Get-Date).ToString("o")
        git_head         = $gitHead.Trim()
        source_ndjson    = $NdjsonPath
        categories       = $byCat
        phase_wall_seconds = $ph
        dir              = $dirM
    }
    $obj | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $OutputJsonPath -Encoding UTF8
    Write-Host ("[BASELINE] wrote regression summary: " + $OutputJsonPath)
}

function Compare-RegressionSummaryJson {
    param(
        [Parameter(Mandatory = $true)][string]$BaselinePath,
        [Parameter(Mandatory = $true)][string]$CurrentPath
    )
    $a = Get-Content -LiteralPath $BaselinePath -Raw | ConvertFrom-Json
    $b = Get-Content -LiteralPath $CurrentPath -Raw | ConvertFrom-Json
    $verdict = "Pass"
    $notes = New-Object System.Collections.Generic.List[string]
    $catA = $a.categories
    $catB = $b.categories
    foreach ($p in $catA.PSObject.Properties) {
        $name = $p.Name
        $va = $p.Value
        $vb = $catB.$name
        if ($null -eq $vb) {
            $verdict = "Warn"
            $notes.Add("missing_category_in_current: $name") | Out-Null
            continue
        }
        if ([int]$vb.failed -gt [int]$va.failed) {
            $verdict = "Fail"
            $notes.Add(("category_regressed:{0} baseline_failed={1} current_failed={2}" -f $name, $va.failed, $vb.failed)) | Out-Null
        }
    }
    [pscustomobject]@{ overall_verdict = $verdict; notes = $notes } | ConvertTo-Json -Depth 6 | Write-Host
    if ($verdict -eq "Fail") { exit 1 }
}

function Get-PerfEnvDouble {
    param(
        [string]$Name,
        [double]$DefaultValue,
        [double]$MinValue = 0.0
    )
    $raw = [string][Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($raw)) { return $DefaultValue }
    $v = 0.0
    if (-not [double]::TryParse($raw.Trim(), [ref]$v)) { return $DefaultValue }
    if ($v -lt $MinValue) { return $DefaultValue }
    return $v
}

function Get-PerfBaselineEntry {
    param(
        [hashtable]$Map,
        [string]$Model
    )
    if ($null -eq $Map) { return $null }
    if (-not $Map.ContainsKey($Model)) { return $null }
    return $Map[$Model]
}

function Get-PerfLimitFromBaseline {
    param(
        [int]$BaseValue,
        [double]$Ratio,
        [int]$MinSlack = 2
    )
    if ($BaseValue -lt 0) { return -1 }
    $scaled = [math]::Ceiling([double]$BaseValue * (1.0 + $Ratio))
    return [int]([math]::Max($scaled, $BaseValue + $MinSlack))
}
Write-ReproContextSnapshot -RepoRoot $repoRoot -JitRoot $jitRoot -OutputPath $lockFilePath
Push-Location $jitRoot
# Isolated cargo target dir avoids Windows file locks on `target/release/rustmodlica.exe` during long runs.
$cargoTargetDir = "target_regression"
$cargoTargetDirPrimary = $cargoTargetDir
$cargoTargetDirFallback = $null
$cargoTargetDirFallbackUsed = $false

function Invoke-RustmodlicaCargoRun {
    param(
        [string[]]$RunArgs,
        [int]$MaxAttempts = 3
    )

    $attempt = 0
    $lastOut = $null
    $lastExit = 1
    $lastText = ""
    $locked = $false
    $switchedToFallback = $false
    $cacheSelfHealUsed = $false
    $targetDirUsed = $cargoTargetDir
    # Windows STATUS_ACCESS_VIOLATION surfaces as -1073741819 (0xC0000005).
    $avExit = -1073741819

    while ($attempt -lt $MaxAttempts) {
        $attempt++
        # Self-heal path: after any prior AV attempt, fully bypass disk caches (JIT codegen,
        # AOT native load, flatten SQLite full-cache, sqlite pool) so the retry proves whether
        # the crash is cache-induced. If the retry passes, we treat the case as passed and
        # record `self_healed=1` in the detail trail.
        $envBackup = $null
        if ($cacheSelfHealUsed) {
            $envBackup = @{
                jit_codegen = $env:RUSTMODLICA_JIT_CODEGEN_CACHE
                aot_native  = $env:RUSTMODLICA_AOT_NATIVE_LOAD
                flat_full   = $env:RUSTMODLICA_FLATTEN_FULL_CACHE
                sqlite_on   = $env:RUSTMODLICA_CACHE_SQLITE
            }
            $env:RUSTMODLICA_JIT_CODEGEN_CACHE  = "0"
            $env:RUSTMODLICA_AOT_NATIVE_LOAD    = "0"
            $env:RUSTMODLICA_FLATTEN_FULL_CACHE = "0"
            $env:RUSTMODLICA_CACHE_SQLITE       = "0"
        }
        try {
            $lastOut = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- @RunArgs 2>&1
            $lastExit = $LASTEXITCODE
        } finally {
            if ($envBackup) {
                $env:RUSTMODLICA_JIT_CODEGEN_CACHE  = $envBackup.jit_codegen
                $env:RUSTMODLICA_AOT_NATIVE_LOAD    = $envBackup.aot_native
                $env:RUSTMODLICA_FLATTEN_FULL_CACHE = $envBackup.flat_full
                $env:RUSTMODLICA_CACHE_SQLITE       = $envBackup.sqlite_on
            }
        }
        $lastText = [string]::Join([Environment]::NewLine, @($lastOut))
        $targetDirUsed = $cargoTargetDir

        if ($lastExit -eq 0) {
            return @{
                Out = $lastOut
                ExitCode = 0
                Text = $lastText
                UsedFallback = $cargoTargetDirFallbackUsed
                TargetDir = $targetDirUsed
                Attempts = $attempt
                Locked = $false
                SwitchedToFallback = $switchedToFallback
                SelfHealed = $cacheSelfHealUsed
            }
        }

        $locked = ($lastText -match "os error 5|failed to remove file|Blocking waiting for file lock")
        # Access violation: first repeat with cargo target-dir fallback + sleep (same as lock path); if
        # still failing, flip the self-heal env on the next attempt to rule out stale disk caches.
        if ($lastExit -eq $avExit -and -not $cacheSelfHealUsed) {
            $cacheSelfHealUsed = $true
            Write-Host ("[SELF-HEAL] attempt={0} exit={1}: retrying with disk caches disabled (JIT_CODEGEN=0, AOT_NATIVE_LOAD=0, FLATTEN_FULL_CACHE=0, CACHE_SQLITE=0)" -f $attempt, $lastExit)
            Get-Process rustmodlica,cargo -ErrorAction SilentlyContinue | Stop-Process -Force
            Start-Sleep -Milliseconds 600
            continue
        }
        if (-not $locked) {
            break
        }

        if (-not $cargoTargetDirFallbackUsed) {
            $cargoTargetDirFallbackUsed = $true
            $cargoTargetDirFallback = ("{0}_fallback_{1}" -f $cargoTargetDirPrimary, $stamp)
            $cargoTargetDir = $cargoTargetDirFallback
            if (-not (Test-Path -LiteralPath $cargoTargetDir)) { New-Item -ItemType Directory -Path $cargoTargetDir | Out-Null }
            $switchedToFallback = $true
        }

        Get-Process rustmodlica,cargo -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Milliseconds 900
    }

    return @{
        Out = $lastOut
        ExitCode = $lastExit
        Text = $lastText
        UsedFallback = $cargoTargetDirFallbackUsed
        TargetDir = $targetDirUsed
        Attempts = $attempt
        Locked = $locked
        SwitchedToFallback = $switchedToFallback
        SelfHealed = $cacheSelfHealUsed
    }
}
# ----------------------------------------------------------------------------
# PREFLIGHT (~30s): build rustmodlica into $cargoTargetDirPrimary, then smoke
# `--validate --validate-tier=analyze` on a trivial model with the SAME PATH /
# DLL resolution that DIR will use. This catches:
#   * compile-time breakage in the JIT crate
#   * sundials-sys DLL not on PATH (the real cause of "0 model(s) passed
#     analyze gate" after 4h of wall-clock)
#   * exe / target-dir mismatch between TestLib, DIR and EVENT-SCAN-MATRIX
# Failure exits the whole regression with code=2 BEFORE any DIR / EVENT-SCAN
# work is queued. Skippable via -SkipPreflight when triaging the preflight itself.
# ----------------------------------------------------------------------------
function Invoke-RegressionPreflight {
    param(
        [string]$JitRoot,
        [string]$CargoTargetDirPrimary,
        [int]$BuildTimeoutSec = 900,
        [int]$SmokeTimeoutSec = 120,
        [string]$SmokeModel = "Modelica.Blocks.Sources.Sine",
        [int]$DirParallelWorkersHint = 1,
        [int]$PerProcessMemoryLimitMb = 8192
    )
    $buildTargetDir = $CargoTargetDirPrimary
    Write-Host ("[PREFLIGHT] cargo build --target-dir " + $buildTargetDir + " -p rustmodlica --release ...")
    $buildSw = [System.Diagnostics.Stopwatch]::StartNew()
    $buildOut = & cargo build --target-dir $buildTargetDir -p rustmodlica --bin rustmodlica --release 2>&1
    $buildExit = $LASTEXITCODE
    foreach ($ln in $buildOut) { Write-Host $ln }
    $buildText = [string]::Join([Environment]::NewLine, @($buildOut))
    $buildLocked = ($buildText -match "os error 5|failed to remove file|Blocking waiting for file lock")
    if ($buildExit -ne 0 -and $buildLocked) {
        $preflightFallback = ("{0}_preflight_fallback_{1}" -f $CargoTargetDirPrimary, (Get-Date -Format "yyyyMMddHHmmss"))
        Write-Host ("[PREFLIGHT] detected target lock; retry cargo build with fallback target-dir: " + $preflightFallback)
        $buildTargetDir = $preflightFallback
        if (-not (Test-Path -LiteralPath (Join-Path $JitRoot $buildTargetDir))) {
            New-Item -ItemType Directory -Path (Join-Path $JitRoot $buildTargetDir) | Out-Null
        }
        Get-Process rustmodlica,cargo -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Milliseconds 800
        $buildOut = & cargo build --target-dir $buildTargetDir -p rustmodlica --bin rustmodlica --release 2>&1
        $buildExit = $LASTEXITCODE
        foreach ($ln in $buildOut) { Write-Host $ln }
    }
    $buildSw.Stop()
    if ($buildExit -ne 0) {
        Write-Error ("[PREFLIGHT] cargo build failed exit=" + $buildExit + " (" + [int]$buildSw.Elapsed.TotalSeconds + "s) - aborting before DIR / EVENT-SCAN to save wall-clock")
        return @{ Ok = $false; Stage = "build"; Exit = $buildExit; ElapsedSec = [int]$buildSw.Elapsed.TotalSeconds }
    }
    if ($buildTargetDir -ne $CargoTargetDirPrimary) {
        Write-Host ("[PREFLIGHT] using fallback target-dir for this run: " + $buildTargetDir)
        $script:cargoTargetDir = $buildTargetDir
        $script:cargoTargetDirPrimary = $buildTargetDir
    }
    $exe = Join-Path $JitRoot (Join-Path $buildTargetDir "release\rustmodlica.exe")
    if (-not (Test-Path -LiteralPath $exe)) {
        Write-Error ("[PREFLIGHT] expected exe missing after build: " + $exe)
        return @{ Ok = $false; Stage = "exe_missing"; Exit = -1; ElapsedSec = [int]$buildSw.Elapsed.TotalSeconds }
    }
    $sundialsRoot = Join-Path $JitRoot (Join-Path $buildTargetDir "release\build")
    if (-not (Test-Path -LiteralPath $sundialsRoot)) {
        Write-Error ("[PREFLIGHT] sundials build root missing under target-dir: " + $sundialsRoot + " (sundials-sys feature likely not enabled in this build)")
        return @{ Ok = $false; Stage = "sundials_root"; Exit = -1; ElapsedSec = [int]$buildSw.Elapsed.TotalSeconds }
    }
    $smokeScript = Join-Path $JitRoot "scripts\dir_smoke_analyze.ps1"
    if (-not (Test-Path -LiteralPath $smokeScript)) {
        Write-Host ("[PREFLIGHT] skipping smoke (script missing): " + $smokeScript)
        return @{ Ok = $true; Stage = "build_only"; Exit = 0; ElapsedSec = [int]$buildSw.Elapsed.TotalSeconds }
    }
    Write-Host ("[PREFLIGHT] smoke analyze " + $SmokeModel + " (timeout " + $SmokeTimeoutSec + "s) ...")
    $smokeSw = [System.Diagnostics.Stopwatch]::StartNew()
    $smokeOut = & powershell -NoProfile -ExecutionPolicy Bypass -File $smokeScript -Exe $exe -JitRoot $JitRoot -Model $SmokeModel -TimeoutSec $SmokeTimeoutSec 2>&1
    $smokeExit = $LASTEXITCODE
    $smokeSw.Stop()
    foreach ($ln in $smokeOut) { Write-Host $ln }
    if ($smokeExit -ne 0) {
        Write-Error ("[PREFLIGHT] smoke analyze failed exit=" + $smokeExit + " (" + [int]$smokeSw.Elapsed.TotalSeconds + "s) - aborting; fix sundials DLL PATH / target-dir consistency BEFORE running DIR (~3-4h)")
        return @{ Ok = $false; Stage = "smoke"; Exit = $smokeExit; ElapsedSec = [int]($buildSw.Elapsed.TotalSeconds + $smokeSw.Elapsed.TotalSeconds) }
    }
    Write-Host "[PREFLIGHT] secondary analyze smoke (Modelica.Blocks.Examples.Filter, 20s) ..."
    $fSw = [System.Diagnostics.Stopwatch]::StartNew()
    $fOut = & powershell -NoProfile -ExecutionPolicy Bypass -File $smokeScript -Exe $exe -JitRoot $JitRoot -Model "Modelica.Blocks.Examples.Filter" -TimeoutSec 20 2>&1
    $fEx = $LASTEXITCODE
    $fSw.Stop()
    foreach ($ln in $fOut) { Write-Host $ln }
    if ($fEx -ne 0) {
        Write-Error ("[PREFLIGHT] secondary smoke analyze failed exit=" + $fEx + " - aborting before DIR")
        return @{ Ok = $false; Stage = "smoke2"; Exit = $fEx; ElapsedSec = [int]($buildSw.Elapsed.TotalSeconds + $smokeSw.Elapsed.TotalSeconds + $fSw.Elapsed.TotalSeconds) }
    }
    $childSw = [System.Diagnostics.Stopwatch]::StartNew()
    $cOut = & powershell -NoProfile -ExecutionPolicy Bypass -File $smokeScript -Exe $exe -JitRoot $JitRoot -Model $SmokeModel -TimeoutSec 15 2>&1
    $cEx = $LASTEXITCODE
    $childSw.Stop()
    foreach ($ln in $cOut) { Write-Host $ln }
    if ($cEx -ne 0) {
        Write-Error ("[PREFLIGHT] child-host smoke (subprocess) failed exit=" + $cEx + " - likely PATH/DLL not visible in nested powershell")
        return @{ Ok = $false; Stage = "smoke_child"; Exit = $cEx; ElapsedSec = [int]($buildSw.Elapsed.TotalSeconds + $smokeSw.Elapsed.TotalSeconds + $fSw.Elapsed.TotalSeconds + $childSw.Elapsed.TotalSeconds) }
    }
    $freeB = 0L
    try {
        $wos = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction SilentlyContinue
        if ($null -ne $wos) { $freeB = [long]($wos.FreePhysicalMemory) * 1024L }
    } catch { $freeB = 0L }
    $needB = [long]([Math]::Max(1, $DirParallelWorkersHint)) * [long]([Math]::Max(1, $PerProcessMemoryLimitMb)) * 1024L * 1024L
    if ($freeB -gt 0 -and $needB -gt 0) {
        $th = [long]([double]$needB * 0.6)
        if ($freeB -lt $th) {
            Write-Warning ("[PREFLIGHT] free RAM low ({0} MB) vs need~{1} MB (workers={2} * mem cap {3} MB * 0.6); consider reducing -DirParallelWorkers or machine load" -f [int]($freeB / 1024 / 1024), [int]($th / 1024 / 1024), $DirParallelWorkersHint, $PerProcessMemoryLimitMb)
        }
    }
    $totS = [int]($buildSw.Elapsed.TotalSeconds + $smokeSw.Elapsed.TotalSeconds + $fSw.Elapsed.TotalSeconds + $childSw.Elapsed.TotalSeconds)
    Write-Host ("[PREFLIGHT] OK build=" + [int]$buildSw.Elapsed.TotalSeconds + "s primary_smoke=" + [int]$smokeSw.Elapsed.TotalSeconds + "s filter_smoke=" + [int]$fSw.Elapsed.TotalSeconds + "s child_check=" + [int]$childSw.Elapsed.TotalSeconds + "s total=" + $totS + "s")
    return @{ Ok = $true; Stage = "smoke"; Exit = 0; ElapsedSec = $totS }
}

if (-not $SkipPreflight) {
    $preDirW = if ($DirParallelWorkers -le 0) { [Environment]::ProcessorCount } else { $DirParallelWorkers }
    $pf = Invoke-RegressionPreflight -JitRoot $jitRoot -CargoTargetDirPrimary $cargoTargetDirPrimary -DirParallelWorkersHint $preDirW -PerProcessMemoryLimitMb $DirPerProcessMemoryLimitMb
    if (-not $pf.Ok) {
        Pop-Location
        Write-Host ("[PREFLIGHT] FAILED stage=" + $pf.Stage + " exit=" + $pf.Exit + " elapsed=" + $pf.ElapsedSec + "s - regression aborted")
        exit 2
    }
    $script:PhaseWallSec.preflight = $pf.ElapsedSec
} else {
    Write-Host "[PREFLIGHT] -SkipPreflight set: bypassing build+analyze smoke (NOT recommended for record baseline)"
}

$caseExtraArgs = @{
    "TestLib/Pendulum" = @("--index-reduction-method=dummyDerivative")
}
$ok = 0
$bad = 0
$results = @()
foreach ($c in $cases) {
    $name = $c[0]
    $expect = $c[1]
    Write-Host "[CASE] $name"
    $startedAt = Get-Date
    $extra = @()
    if ($caseExtraArgs.ContainsKey($name)) { $extra = $caseExtraArgs[$name] }
    $r = Invoke-RustmodlicaCargoRun -RunArgs ($extra + @($name))
    $exit = $r.ExitCode
    $runText = $r.Text
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $actual = if ($exit -eq 0) { "pass" } else { "fail" }
    $match = ($actual -eq $expect)
    $expectOk = ($expect -eq "pass")
    $actualOk = ($actual -eq "pass")
    if ($match) { $ok++ } else { $bad++ }
    $sym = if ($match) { "OK" } else { "!!" }
    $results += "$sym $name  expect=$expect  actual=$actual (exit $exit)"
    $detail = ("target_dir=" + $r.TargetDir + ";attempts=" + $r.Attempts + ";locked=" + $r.Locked + ";fallback_used=" + $r.UsedFallback)
    if ($r.ContainsKey("SelfHealed") -and $r.SelfHealed) { $detail = ($detail + ";self_healed=true") }
    if (-not $match) {
        if ($runText -match "Model not found") { $detail = "model_not_found" }
        elseif ($runText -match "os error 5|failed to remove file|Blocking waiting for file lock") { $detail = ("release_binary_locked;" + $detail) }
    }
    if (-not $match) {
        $envText = ("RUSTMODLICA_EVENT_TRACE=" + [string]$env:RUSTMODLICA_EVENT_TRACE)
        $cmd = ("cargo run --target-dir {0} -p rustmodlica --bin rustmodlica --release -- {1} {2}" -f $r.TargetDir, ($extra -join " "), $name).Trim()
        $repro = Write-ReproBundle -CaseType "CASE" -CaseName $name -CommandLine $cmd -EnvText $envText -StdoutPath "" -ExtraDetail $detail
        $detail = ($detail + ";repro=" + $repro)
    }
    Write-CaseLog -CaseType "CASE" -CaseName $name -DurationMs $durationMs -ExpectTargetOk $expectOk -ActualOk $actualOk -ExitCode $exit -Status $(if ($match) { "OK" } else { "MISMATCH" }) -Reason $(if ($match) { "expectation_met" } else { "expectation_mismatch" }) -Detail $detail
}

# JIT named rules: TestLib batch --validate (build.rs builtin rules + default_jit_policy), same binary as regression target dir
Write-Host "[JIT_RULES] TestLib batch validate"
$startedJitRules = Get-Date
$policyScriptPath = Join-Path $jitRoot "scripts\run_testlib_validate.ps1"
& powershell -NoProfile -ExecutionPolicy Bypass -File $policyScriptPath -CargoTargetSubdir $cargoTargetDirPrimary
$jitRulesExit = $LASTEXITCODE
$jitRulesDurationMs = [long](((Get-Date) - $startedJitRules).TotalMilliseconds)
$script:PhaseWallSec.testlib = [int]([Math]::Max(0, $jitRulesDurationMs / 1000))
$jitRulesOk = ($jitRulesExit -eq 0)
if ($jitRulesOk) { $ok++ } else { $bad++ }
$sym = if ($jitRulesOk) { "OK" } else { "!!" }
$results += "$sym JIT_RULES/TestLibValidateBatch  expect=pass  actual=$(if ($jitRulesOk) { 'pass' } else { 'fail' }) (exit $jitRulesExit)"
Write-CaseLog -CaseType "JIT_RULES" -CaseName "TestLibValidateBatch" -DurationMs $jitRulesDurationMs -ExpectTargetOk $true -ActualOk $jitRulesOk -ExitCode $jitRulesExit -Status $(if ($jitRulesOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($jitRulesOk) { "expectation_met" } else { "testlib_batch_validate_failed" }) -Detail ("cargo_target_subdir=" + $cargoTargetDirPrimary + ";script=" + $policyScriptPath)

# INT-2 script mode: run script file (load/setParameter/simulate/quit)
$scriptTests = @(
    @{ name = "ScriptMode/init_dummy"; path = "scripts/init_dummy.txt"; expect = "pass" },
    @{ name = "ScriptMode/init_with_param_setparam"; path = "scripts/init_with_param_setparam.txt"; expect = "pass" },
    @{ name = "ScriptMode/multi_model_use"; path = "scripts/multi_model_use.txt"; expect = "pass" },
    @{ name = "ScriptMode/setStartValue"; path = "scripts/setStartValue.txt"; expect = "pass" },
    @{ name = "ScriptMode/getParameter"; path = "scripts/getParameter.txt"; expect = "pass" },
    @{ name = "ScriptMode/setStopTime"; path = "scripts/setStopTime.txt"; expect = "pass" },
    @{ name = "ScriptMode/setTolerance"; path = "scripts/setTolerance.txt"; expect = "pass" },
    @{ name = "ScriptMode/saveResult"; path = "scripts/saveResult.txt"; expect = "pass" },
    @{ name = "ScriptMode/plot"; path = "scripts/plot.txt"; expect = "pass" },
    @{ name = "ScriptMode/eval"; path = "scripts/eval.txt"; expect = "pass" },
    @{ name = "ScriptMode/loadClass"; path = "scripts/loadClass.txt"; expect = "pass" },
    @{ name = "ScriptMode/switchModel"; path = "scripts/switchModel.txt"; expect = "pass" }
)
foreach ($t in $scriptTests) {
    $name = $t.name
    Write-Host "[SCRIPT] $name"
    $startedAt = Get-Date
    $scriptPath = Join-Path ".." $t.path
    $expect = $t.expect
    $r = Invoke-RustmodlicaCargoRun -RunArgs @("--script=$scriptPath")
    $null = $r.Out
    $exit = $r.ExitCode
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $actual = if ($exit -eq 0) { "pass" } else { "fail" }
    $match = ($actual -eq $expect)
    $expectOk = ($expect -eq "pass")
    $actualOk = ($actual -eq "pass")
    if ($match) { $ok++ } else { $bad++ }
    $sym = if ($match) { "OK" } else { "!!" }
    $results += "$sym $name  expect=$expect  actual=$actual (exit $exit)"
    $detail = ("target_dir=" + $r.TargetDir + ";attempts=" + $r.Attempts + ";locked=" + $r.Locked + ";fallback_used=" + $r.UsedFallback)
    if (-not $match -and $r.Locked) { $detail = ("release_binary_locked;" + $detail) }
    Write-CaseLog -CaseType "SCRIPT" -CaseName $name -DurationMs $durationMs -ExpectTargetOk $expectOk -ActualOk $actualOk -ExitCode $exit -Status $(if ($match) { "OK" } else { "MISMATCH" }) -Reason $(if ($match) { "expectation_met" } else { "expectation_mismatch" }) -Detail $detail
}
# FUNC-6: emit-c with user function (static C body)
$emitCTests = @(
    @{ name = "EmitC/RecursiveFunc"; opts = "--emit-c=build_regress_emit"; model = "TestLib/RecursiveFunc"; expect = "pass" }
)
if (-not (Test-Path build_regress_emit)) { New-Item -ItemType Directory -Path build_regress_emit | Out-Null }
foreach ($t in $emitCTests) {
    $name = $t.name
    Write-Host "[EMIT-C] $name"
    $startedAt = Get-Date
    $expect = $t.expect
    $r = Invoke-RustmodlicaCargoRun -RunArgs @($t.opts, $t.model)
    $null = $r.Out
    $exit = $r.ExitCode
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $actual = if ($exit -eq 0) { "pass" } else { "fail" }
    $match = ($actual -eq $expect)
    $expectOk = ($expect -eq "pass")
    $actualOk = ($actual -eq "pass")
    if ($match) { $ok++ } else { $bad++ }
    $sym = if ($match) { "OK" } else { "!!" }
    $results += "$sym $name  expect=$expect  actual=$actual (exit $exit)"
    $detail = ("target_dir=" + $r.TargetDir + ";attempts=" + $r.Attempts + ";locked=" + $r.Locked + ";fallback_used=" + $r.UsedFallback)
    if (-not $match -and $r.Locked) { $detail = ("release_binary_locked;" + $detail) }
    Write-CaseLog -CaseType "EMIT_C" -CaseName $name -DurationMs $durationMs -ExpectTargetOk $expectOk -ActualOk $actualOk -ExitCode $exit -Status $(if ($match) { "OK" } else { "MISMATCH" }) -Reason $(if ($match) { "expectation_met" } else { "expectation_mismatch" }) -Detail $detail
}
# FUNC-7: emit-c with external string arg; C must contain const char* ABI and literal (JIT may pass validate too)
if (-not (Test-Path build_regress_emit_string)) { New-Item -ItemType Directory -Path build_regress_emit_string | Out-Null }
$r = Invoke-RustmodlicaCargoRun -RunArgs @("--emit-c=build_regress_emit_string", "TestLib/StringArgExtFunc")
$null = $r.Out
$exitString = $r.ExitCode
$cPath = "build_regress_emit_string\model.c"
$func7Ok = Test-Path $cPath
if ($func7Ok) {
    $cContent = Get-Content -Raw $cPath
    $func7Ok = ($cContent -match "const char\*") -and ($cContent -match "extLog") -and ($cContent -match "test")
}
if ($func7Ok) { $ok++ } else { $bad++ }
$sym = if ($func7Ok) { "OK" } else { "!!" }
$results += "$sym FUNC-7/EmitC/StringArgExtFunc  expect=emit C with string ABI  actual=$(if ($func7Ok) { 'pass' } else { 'fail' })"
$detail = ("target_dir=" + $r.TargetDir + ";attempts=" + $r.Attempts + ";locked=" + $r.Locked + ";fallback_used=" + $r.UsedFallback)
if (-not $func7Ok -and $r.Locked) { $detail = ("release_binary_locked;" + $detail) }
Write-CaseLog -CaseType "EMIT_C" -CaseName "FUNC-7/EmitC/StringArgExtFunc" -DurationMs 0 -ExpectTargetOk $true -ActualOk $func7Ok -ExitCode $exitString -Status $(if ($func7Ok) { "OK" } else { "MISMATCH" }) -Reason $(if ($func7Ok) { "expectation_met" } else { "string_abi_not_emitted_or_jit_expectation_failed" }) -Detail $detail
# SYNC-2: clocked semantics (when sample(...)); run with backend-dae-info and check clocked line present
$r = Invoke-RustmodlicaCargoRun -RunArgs @("--backend-dae-info", "TestLib/ClockedPartitionTest")
$sync2Out = $r.Text
Write-Host "[SYNC] ClockedPartitionTest backend info"
$sync2Ok = ($r.ExitCode -eq 0) -and ($sync2Out -match "clocked")
if ($sync2Ok) { $ok++ } else { $bad++ }
$sym = if ($sync2Ok) { "OK" } else { "!!" }
$results += "$sym SYNC-2/ClockedPartitionTest  expect=backend clocked output  actual=$(if ($sync2Ok) { 'pass' } else { 'fail' })"
$detail = ("target_dir=" + $r.TargetDir + ";attempts=" + $r.Attempts + ";locked=" + $r.Locked + ";fallback_used=" + $r.UsedFallback)
if (-not $sync2Ok -and $r.Locked) { $detail = ("release_binary_locked;" + $detail) }
Write-CaseLog -CaseType "SYNC" -CaseName "SYNC-2/ClockedPartitionTest" -DurationMs 0 -ExpectTargetOk $true -ActualOk $sync2Ok -ExitCode $r.ExitCode -Status $(if ($sync2Ok) { "OK" } else { "MISMATCH" }) -Reason $(if ($sync2Ok) { "expectation_met" } else { "clocked_backend_info_missing_or_run_failed" }) -Detail $detail

# PERF-SMOKE: basic performance and runtime counter sanity (gated by env RUSTMODLICA_PERF_SMOKE=1)
$perfSmokeEnabled = $env:RUSTMODLICA_PERF_SMOKE
$perfSmokeEnabled = if ($null -eq $perfSmokeEnabled) { $false } else {
    $t = $perfSmokeEnabled.Trim().ToLowerInvariant()
    -not ($t -eq "" -or $t -eq "0" -or $t -eq "false" -or $t -eq "off" -or $t -eq "no")
}
if ($perfSmokeEnabled) {
    $perfBaselinePath = Join-Path $regressLogDir "perf_smoke_baseline.json"
    $perfSnapshotPath = Join-Path $regressLogDir ("perf_smoke_snapshot_{0}.json" -f $stamp)
    $perfModeRaw = [string]$env:RUSTMODLICA_PERF_BASELINE_MODE
    if ([string]::IsNullOrWhiteSpace($perfModeRaw)) { $perfModeRaw = "compare" }
    $perfMode = $perfModeRaw.Trim().ToLowerInvariant()
    if (@("compare", "record", "update") -notcontains $perfMode) { $perfMode = "compare" }
    $perfDegradeRatio = Get-PerfEnvDouble -Name "RUSTMODLICA_PERF_DEGRADE_RATIO" -DefaultValue 0.2 -MinValue 0.0
    $perfBaseline = @{}
    if (Test-Path -LiteralPath $perfBaselinePath) {
        try {
            $baselineJson = Get-Content -LiteralPath $perfBaselinePath -Raw
            $baselineObj = $baselineJson | ConvertFrom-Json
            if ($null -ne $baselineObj) {
                $baselineObj.psobject.Properties | ForEach-Object {
                    $perfBaseline[$_.Name] = $_.Value
                }
            }
        } catch {
            $perfBaseline = @{}
        }
    }
    $perfCurrent = @{}
    $perfCases = @(
        @{ model = "TestLib/ClockedPartitionTest"; tEnd = 2.0; dt = 0.01; oi = 0.01; compileMsMax = 60000; simMsMax = 60000; eventIterMax = 5000; clockDispatchMax = 5000 },
        @{ model = "TestLib/BouncingBall"; tEnd = 3.0; dt = 0.005; oi = 0.01; compileMsMax = 60000; simMsMax = 60000; eventIterMax = 20000; clockDispatchMax = 20000 },
        @{ model = "TestLib/MultiOutputFunc"; tEnd = 1.0; dt = 0.01; oi = 0.01; compileMsMax = 60000; simMsMax = 60000; eventIterMax = 5000; clockDispatchMax = 5000 }
    )
    foreach ($pc in $perfCases) {
        $m = $pc.model
        Write-Host "[PERF-SMOKE] $m"
        $startedAt = Get-Date
        $safeName = $m.Replace("/", "_").Replace(".", "_")
        $csv = "build_regress_perf_${safeName}.csv"
        $oldPerf = $env:RUSTMODLICA_PERF_TRACE
        $env:RUSTMODLICA_PERF_TRACE = "1"
        $r = Invoke-RustmodlicaCargoRun -RunArgs @("--solver=rk4", "--t-end=$($pc.tEnd)", "--dt=$($pc.dt)", "--output-interval=$($pc.oi)", "--result-file=$csv", $m)
        $env:RUSTMODLICA_PERF_TRACE = $oldPerf

        $compileMs = -1
        $simMs = -1
        $eventIter = -1
        $clockDispatch = -1
        if ($r.Text -match '\[perf\] compile_ms=(\d+)') { $compileMs = [int]$Matches[1] }
        if ($r.Text -match '\[perf\] sim_ms=(\d+)') { $simMs = [int]$Matches[1] }
        if ($r.Text -match '\[perf\] event_iter_total=(\d+) clock_dispatch_total=(\d+)') {
            $eventIter = [int]$Matches[1]
            $clockDispatch = [int]$Matches[2]
        }

        $perfCurrent[$m] = @{
            compile_ms = $compileMs
            sim_ms = $simMs
            event_iter_total = $eventIter
            clock_dispatch_total = $clockDispatch
        }
        $base = Get-PerfBaselineEntry -Map $perfBaseline -Model $m
        $hasBase = ($null -ne $base)
        $compileLimit = [int]$pc.compileMsMax
        $simLimit = [int]$pc.simMsMax
        $eventIterLimit = [int]$pc.eventIterMax
        $clockDispatchLimit = [int]$pc.clockDispatchMax
        if ($perfMode -eq "compare" -and $hasBase) {
            $compileLimit = Get-PerfLimitFromBaseline -BaseValue ([int]$base.compile_ms) -Ratio $perfDegradeRatio
            $simLimit = Get-PerfLimitFromBaseline -BaseValue ([int]$base.sim_ms) -Ratio $perfDegradeRatio
            $eventIterLimit = Get-PerfLimitFromBaseline -BaseValue ([int]$base.event_iter_total) -Ratio $perfDegradeRatio
            $clockDispatchLimit = Get-PerfLimitFromBaseline -BaseValue ([int]$base.clock_dispatch_total) -Ratio $perfDegradeRatio
        }

        $perfOk = ($r.ExitCode -eq 0) -and (Test-Path $csv) `
            -and ($compileMs -ge 0) -and ($compileMs -le $compileLimit) `
            -and ($simMs -ge 0) -and ($simMs -le $simLimit) `
            -and ($eventIter -ge 0) -and ($eventIter -le $eventIterLimit) `
            -and ($clockDispatch -ge 0) -and ($clockDispatch -le $clockDispatchLimit)

        if ($perfOk) { $ok++ } else { $bad++ }
        $sym = if ($perfOk) { "OK" } else { "!!" }
        $results += "$sym PERF-SMOKE/$m  expect=within_thresholds_or_baseline_ratio  actual=$(if ($perfOk) { 'pass' } else { 'fail' })"
        $detail = ("target_dir=" + $r.TargetDir + ";attempts=" + $r.Attempts + ";locked=" + $r.Locked + ";fallback_used=" + $r.UsedFallback + ";perf_mode=" + $perfMode + ";degrade_ratio=" + [string]$perfDegradeRatio + ";has_baseline=" + $hasBase + ";compile_ms=" + $compileMs + ";compile_limit=" + $compileLimit + ";sim_ms=" + $simMs + ";sim_limit=" + $simLimit + ";event_iter_total=" + $eventIter + ";event_iter_limit=" + $eventIterLimit + ";clock_dispatch_total=" + $clockDispatch + ";clock_dispatch_limit=" + $clockDispatchLimit + ";csv=" + $csv)
        if (-not $perfOk -and $r.Locked) { $detail = ("release_binary_locked;" + $detail) }
        $reason = "expectation_met"
        if (-not $perfOk) {
            $reason = if ($perfMode -eq "compare" -and $hasBase) { "perf_regression_vs_baseline_or_missing_metrics" } else { "perf_threshold_or_missing_metrics" }
        }
        Write-CaseLog -CaseType "PERF_SMOKE" -CaseName ("PERF-SMOKE/" + $m) -DurationMs ([long](((Get-Date) - $startedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $perfOk -ExitCode $(if ($perfOk) { 0 } else { 1 }) -Status $(if ($perfOk) { "OK" } else { "MISMATCH" }) -Reason $reason -Detail $detail
    }

    try {
        ($perfCurrent | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $perfSnapshotPath -Encoding UTF8
    } catch {
        Write-Host ("[PERF-SMOKE] warning: failed to write snapshot " + $perfSnapshotPath)
    }
    if ($perfMode -eq "record") {
        ($perfCurrent | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $perfBaselinePath -Encoding UTF8
        Write-Host ("[PERF-SMOKE] baseline recorded: " + $perfBaselinePath)
    } elseif ($perfMode -eq "update") {
        $merged = @{}
        foreach ($k in $perfBaseline.Keys) { $merged[$k] = $perfBaseline[$k] }
        foreach ($k in $perfCurrent.Keys) { $merged[$k] = $perfCurrent[$k] }
        ($merged | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $perfBaselinePath -Encoding UTF8
        Write-Host ("[PERF-SMOKE] baseline updated: " + $perfBaselinePath)
    } elseif ($perfMode -eq "compare" -and -not (Test-Path -LiteralPath $perfBaselinePath)) {
        ($perfCurrent | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $perfBaselinePath -Encoding UTF8
        Write-Host ("[PERF-SMOKE] baseline bootstrapped: " + $perfBaselinePath)
    }
}

if ($JitValidatePerf) {
    Write-Host "[JIT_VALIDATE_PERF] TestLib cache-tier validate-perf (regress-harness)"
    $rhExe = Join-Path $repoRoot "target\release\regress-harness.exe"
    if (-not (Test-Path -LiteralPath $rhExe)) {
        Push-Location $repoRoot
        try {
            & cargo build -p regress-harness --release
            if ($LASTEXITCODE -ne 0) {
                Write-Error "cargo build -p regress-harness --release failed (exit $LASTEXITCODE)"
                exit $LASTEXITCODE
            }
        } finally {
            Pop-Location
        }
    }
    $rmExe = Join-Path $jitRoot (Join-Path $cargoTargetDirPrimary "release\rustmodlica.exe")
    if (-not (Test-Path -LiteralPath $rmExe)) {
        Write-Error ("rustmodlica not found for validate-perf: " + $rmExe)
        exit 1
    }
    $jvModels = New-Object System.Collections.Generic.List[string]
    foreach ($c in $cases) {
        if ($c[1] -ne "pass") { continue }
        $n = [string]$c[0]
        if (-not ($n.StartsWith("TestLib/"))) { continue }
        if ($n -eq "TestLib/Pendulum") { continue }
        [void]$jvModels.Add($n)
    }
    $modelCsv = [string]::Join(",", $jvModels)
    $jvOut = Join-Path $regressLogDir ("jit_validate_perf_{0}" -f $stamp)
    New-Item -ItemType Directory -Force -Path $jvOut | Out-Null
    $jvStarted = Get-Date
    $jvArgs = @(
        "jit", "validate-perf",
        "--exe", $rmExe,
        "--out-dir", $jvOut,
        "--validate-tier", "analyze",
        "--validation-mode", "full",
        "--hot-runs", ([string]$JitValidatePerfHotRuns),
        "--models", $modelCsv
    )
    $testLibRoot = Join-Path $jitRoot "TestLib"
    if (Test-Path -LiteralPath (Join-Path $jitRoot "Modelica\package.mo")) {
        $jvArgs += @("--lib-path", $jitRoot)
    }
    $jvArgs += @("--lib-path", $testLibRoot)
    $scen = [string]$JitValidatePerfScenarios
    if (-not [string]::IsNullOrWhiteSpace($scen)) {
        $scenLower = $scen.Trim().ToLowerInvariant()
        if ($scenLower -ne "*" -and $scenLower -ne "all") {
            $jvArgs += @("--scenarios", $scen.Trim())
        }
    }
    & $rhExe @jvArgs
    $jvExit = $LASTEXITCODE
    $jvDurationMs = [long](((Get-Date) - $jvStarted).TotalMilliseconds)
    $jvOk = ($jvExit -eq 0)
    if ($jvOk) { $ok++ } else { $bad++ }
    $jvSym = if ($jvOk) { "OK" } else { "!!" }
    $results += "$jvSym JIT_VALIDATE_PERF/TestLib  expect=pass  actual=$(if ($jvOk) { 'pass' } else { 'fail' }) (exit $jvExit)"
    $jvDetail = ("out_dir=" + $jvOut + ";exe=" + $rmExe + ";scenarios=" + $scen + ";hot_runs=" + [string]$JitValidatePerfHotRuns + ";model_count=" + $jvModels.Count)
    Write-CaseLog -CaseType "JIT_VALIDATE_PERF" -CaseName "TestLibValidatePerf" -DurationMs $jvDurationMs -ExpectTargetOk $true -ActualOk $jvOk -ExitCode $jvExit -Status $(if ($jvOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($jvOk) { "expectation_met" } else { "jit_validate_perf_failed" }) -Detail $jvDetail
    if ($jvOk) {
        $jvReport = Join-Path $jvOut "report.json"
        $lsBl = Join-Path $repoRoot "baseline\large_scale_jit_validate_perf_v2\jit_perf_baseline.json"
        if ((Test-Path -LiteralPath $jvReport) -and (Test-Path -LiteralPath $lsBl)) {
            Write-Host "[JIT_VALIDATE_PERF] compare-baseline preset=large-scale-v2 (informational)"
            $null = & $rhExe jit compare-baseline --preset large-scale-v2 --report $jvReport 2>&1
            if ($LASTEXITCODE -ne 0) {
                Write-Warning ("JIT_VALIDATE_PERF compare-baseline exit " + $LASTEXITCODE + " (informational)")
            }
        }
    }
}

# SYNC freeze: run clocked models twice and require deterministic CSV output
$clockedDeterminismCases = @(
    "TestLib/ClockedPartitionTest",
    "TestLib/ClockedTwoRates",
    "TestLib/HoldPreviousTest",
    "TestLib/SubSuperShiftSampleTest",
    "TestLib/ClockedStartAndShiftTest",
    "TestLib/ClockedNestedSubSuperTest",
    "TestLib/ClockedStartAndSubSampleTest",
    "TestLib/ClockedStartAndBackSampleTest",
    "TestLib/ClockedStartShiftThenBackSampleTest",
    "TestLib/ClockedStartShiftThenSuperSampleTest",
    "TestLib/ClockedStartAndSuperSampleTest",
    "TestLib/ClockedStartShiftThenSubSampleTest",
    "TestLib/ClockedInvalidFactorClampTest"
)
foreach ($m in $clockedDeterminismCases) {
    Write-Host "[SYNC-DET] $m"
    $startedAt = Get-Date
    $safeName = $m.Replace("/", "_").Replace(".", "_")
    $csvA = "build_regress_clocked_${safeName}_a.csv"
    $csvB = "build_regress_clocked_${safeName}_b.csv"
    $r1 = Invoke-RustmodlicaCargoRun -RunArgs @("--solver=rk4", "--output-interval=0.001", "--result-file=$csvA", $m)
    $e1 = $r1.ExitCode
    $r2 = Invoke-RustmodlicaCargoRun -RunArgs @("--solver=rk4", "--output-interval=0.001", "--result-file=$csvB", $m)
    $e2 = $r2.ExitCode
    $same = $false
    if ($e1 -eq 0 -and $e2 -eq 0 -and (Test-Path $csvA) -and (Test-Path $csvB)) {
        $h1 = (Get-FileHash -Algorithm SHA256 $csvA).Hash
        $h2 = (Get-FileHash -Algorithm SHA256 $csvB).Hash
        $same = ($h1 -eq $h2)
    }
    if ($same) { $ok++ } else { $bad++ }
    $sym = if ($same) { "OK" } else { "!!" }
    $results += "$sym SYNC-DET/$m  expect=stable repeated output  actual=$(if ($same) { 'pass' } else { 'fail' })"
    $detail = ("a_target_dir=" + $r1.TargetDir + ";a_attempts=" + $r1.Attempts + ";a_locked=" + $r1.Locked + ";a_fallback_used=" + $r1.UsedFallback + ";b_target_dir=" + $r2.TargetDir + ";b_attempts=" + $r2.Attempts + ";b_locked=" + $r2.Locked + ";b_fallback_used=" + $r2.UsedFallback)
    if (-not $same -and ($r1.Locked -or $r2.Locked)) { $detail = ("release_binary_locked;" + $detail) }
    Write-CaseLog -CaseType "SYNC_DET" -CaseName ("SYNC-DET/" + $m) -DurationMs ([long](((Get-Date) - $startedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $same -ExitCode $(if ($same) { 0 } else { 1 }) -Status $(if ($same) { "OK" } else { "MISMATCH" }) -Reason $(if ($same) { "expectation_met" } else { "non_deterministic_or_run_failed" }) -Detail $detail
}

# SYNC TRACE ASSERT: verify clock partition activation points for derived clocks.
$traceAssertCases = @(
    @{
        model = "TestLib/ClockedStartAndShiftTest";
        expectSubstr = "shiftSample_sample_0.5_0.25";
        expectTimes = @(0.75, 1.25, 1.75);
        disallowTimes = @(0.5);
        tEnd = 2.0;
    },
    @{
        model = "TestLib/ClockedNestedSubSuperTest";
        expectSubstr = "superSample_subSample_sample_0.25";
        expectTimes = @(0.25, 0.5, 1.0);
        disallowTimes = @();
        tEnd = 1.2;
    },
    @{
        model = "TestLib/ClockedStartAndSubSampleTest";
        expectSubstr = "subSample_sample_0.3_0.2_Number(2.0)";
        expectTimes = @(0.2, 0.8, 1.4);
        disallowTimes = @();
        tEnd = 2.0;
    },
    @{
        model = "TestLib/ClockedStartAndBackSampleTest";
        expectSubstr = "backSample_sample_0.3_0.2_Number(2.0)";
        expectTimes = @(0.5, 1.1, 1.7);
        disallowTimes = @();
        tEnd = 2.0;
    },
    @{
        model = "TestLib/ClockedStartShiftThenBackSampleTest";
        expectSubstr = "backSample_shiftSample_sample_0.4_0.2_Number(1.0)_Number(2.0)";
        expectTimes = @(1.0, 1.8);
        disallowTimes = @();
        tEnd = 2.2;
    },
    @{
        model = "TestLib/ClockedStartShiftThenSuperSampleTest";
        expectSubstr = "superSample_shiftSample_sample_0.5_0.25_Number(1.0)_Number(2.0)";
        expectTimes = @(0.75, 1.0, 1.25);
        disallowTimes = @();
        tEnd = 1.4;
    }
    @{
        model = "TestLib/ClockedStartAndSuperSampleTest";
        expectSubstr = "superSample_sample_0.3_0.2_Number(2.0)";
        expectTimes = @(0.2, 0.35, 0.5);
        disallowTimes = @();
        tEnd = 0.7;
    },
    @{
        model = "TestLib/ClockedStartShiftThenSubSampleTest";
        expectSubstr = "subSample_shiftSample_sample_0.4_0.2_Number(1.0)_Number(2.0)";
        expectTimes = @(0.6, 1.4, 2.2);
        disallowTimes = @();
        tEnd = 2.3;
    }
    @{
        model = "TestLib/ClockedInvalidFactorClampTest";
        expectSubstr = "sample_0.5_0";
        expectTimes = @(0.0, 0.5, 1.0);
        disallowTimes = @();
        tEnd = 1.2;
    }
)
foreach ($c in $traceAssertCases) {
    $m = $c.model
    Write-Host "[SYNC-TRACE-ASSERT] $m"
    $startedAt = Get-Date
    $safeName = $m.Replace("/", "_").Replace(".", "_")
    $tracePath = "build_regress_trace_clocked_${safeName}.txt"

    $oldTrace = $env:RUSTMODLICA_EVENT_TRACE
    $env:RUSTMODLICA_EVENT_TRACE = "1"
    $r = Invoke-RustmodlicaCargoRun -RunArgs @("--solver=rk4", "--dt=0.01", "--t-end=$($c.tEnd)", "--output-interval=0.25", "--result-file=build_regress_trace_clocked_${safeName}.csv", $m)
    $traceOut = $r.Out
    $traceExit = $r.ExitCode
    $env:RUSTMODLICA_EVENT_TRACE = $oldTrace

    # Persist trace only for debugging.
    $traceOut | Set-Content -LiteralPath $tracePath -Encoding UTF8
    # Force string semantics for regex checks; array -notmatch in PowerShell
    # returns all non-matching elements and can cause false negatives.
    $traceText = [string]::Join([Environment]::NewLine, @($traceOut))

    $substrEsc = [regex]::Escape($c.expectSubstr)
    $traceOk = ($traceExit -eq 0)
    foreach ($t in $c.expectTimes) {
        $tStr = [string]::Format("{0:F6}", [double]$t)
        $pattern = "\[event-trace\] t=$tStr active_clock_partitions=.*$substrEsc"
        if ($traceText -notmatch $pattern) {
            $traceOk = $false
        }
    }
    foreach ($t in $c.disallowTimes) {
        $tStr = [string]::Format("{0:F6}", [double]$t)
        $pattern = "\[event-trace\] t=$tStr active_clock_partitions=.*$substrEsc"
        if ($traceText -match $pattern) {
            $traceOk = $false
        }
    }

    if ($traceOk) { $ok++ } else { $bad++ }
    $sym = if ($traceOk) { "OK" } else { "!!" }
    $results += "$sym SYNC-TRACE-ASSERT/$m  expect=derived clock partition activations  actual=$(if ($traceOk) { 'pass' } else { 'fail' })"
    $detail = ("trace=" + $tracePath + ";target_dir=" + $r.TargetDir + ";attempts=" + $r.Attempts + ";locked=" + $r.Locked + ";fallback_used=" + $r.UsedFallback)
    if (-not $traceOk -and $r.Locked) { $detail = ("release_binary_locked;" + $detail) }
    if (-not $traceOk) {
        $envText = ("RUSTMODLICA_EVENT_TRACE=1")
        $cmd = ("cargo run --target-dir {0} -p rustmodlica --bin rustmodlica --release -- --solver=rk4 --dt=0.01 --t-end={1} --output-interval=0.25 --result-file=build_regress_trace_clocked_{2}.csv {3}" -f $r.TargetDir, $c.tEnd, $safeName, $m)
        $extra = ("expectSubstr=" + $c.expectSubstr + ";expectTimes=" + ($c.expectTimes -join "|") + ";disallowTimes=" + ($c.disallowTimes -join "|"))
        $repro = Write-ReproBundle -CaseType "SYNC_TRACE_ASSERT" -CaseName $m -CommandLine $cmd -EnvText $envText -StdoutPath $tracePath -ExtraDetail ($detail + ";" + $extra)
        $detail = ($detail + ";repro=" + $repro)
    }
    Write-CaseLog -CaseType "SYNC_TRACE_ASSERT" -CaseName ("SYNC-TRACE-ASSERT/" + $m) -DurationMs ([long](((Get-Date) - $startedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $traceOk -ExitCode $(if ($traceOk) { 0 } else { 1 }) -Status $(if ($traceOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($traceOk) { "expectation_met" } else { "missing_or_incorrect_clock_partition_activation" }) -Detail $detail
}
# FMI emit: --emit-fmu produces modelDescription.xml and fmi2_cs.c
if (-not (Test-Path build_regress_fmu)) { New-Item -ItemType Directory -Path build_regress_fmu | Out-Null }
# Default modelIdentifier checks assume no FMI env overrides on the parent process.
$fmiEnvKeys = @("RUSTMODLICA_FMI_MODEL_ID", "RUSTMODLICA_FMI_MODEL_ID_PREFIX", "RUSTMODLICA_FMI_GUID")
$savedFmiEnv = @{}
foreach ($k in $fmiEnvKeys) {
    $savedFmiEnv[$k] = [Environment]::GetEnvironmentVariable($k, "Process")
    Remove-Item ("Env:{0}" -f $k) -ErrorAction SilentlyContinue
}
try {
    $r = Invoke-RustmodlicaCargoRun -RunArgs @("--emit-fmu=build_regress_fmu", "TestLib/SimpleTest")
} finally {
    foreach ($k in $fmiEnvKeys) {
        $v = $savedFmiEnv[$k]
        if ([string]::IsNullOrEmpty($v)) {
            Remove-Item ("Env:{0}" -f $k) -ErrorAction SilentlyContinue
        } else {
            Set-Item -Path ("Env:{0}" -f $k) -Value $v
        }
    }
}
$null = $r.Out
Write-Host "[FMI] emit-fmu"
$fmiOk = ($r.ExitCode -eq 0) -and (Test-Path "build_regress_fmu\modelDescription.xml") -and (Test-Path "build_regress_fmu\fmi2_cs.c")
$fmiDetailExtra = ""
if ($fmiOk) {
    $mdPath = "build_regress_fmu\modelDescription.xml"
    $mdText = Get-Content -Raw $mdPath
    # Regex: use single-quoted patterns so \s is whitespace (not a literal backslash).
    $hasFmi2 = ($mdText -match 'fmiVersion="2\.0"')
    $hasGuid = ($mdText -match '<fmiModelDescription[^>]*\bguid="[^"]+"')
    $hasCS = ($mdText -match '<CoSimulation\b')
    $hasModelId = ($mdText -match 'modelIdentifier="SimpleTest"')
    $hasReal = ($mdText -match '<Real\s*/>')
    $fmiOk = $fmiOk -and $hasFmi2 -and $hasGuid -and $hasCS -and $hasModelId -and $hasReal
    $fmiDetailExtra = (";md_fmi2=" + $hasFmi2 + ";md_guid=" + $hasGuid + ";md_cs=" + $hasCS + ";md_modelId=" + $hasModelId + ";md_real=" + $hasReal)
}
if ($fmiOk) { $ok++ } else { $bad++ }
$sym = if ($fmiOk) { "OK" } else { "!!" }
$results += "$sym FMI/emit-fmu  expect=modelDescription.xml and fmi2_cs.c  actual=$(if ($fmiOk) { 'pass' } else { 'fail' })"
$detail = ("target_dir=" + $r.TargetDir + ";attempts=" + $r.Attempts + ";locked=" + $r.Locked + ";fallback_used=" + $r.UsedFallback)
if ($fmiDetailExtra -ne "") { $detail = ($detail + $fmiDetailExtra) }
if (-not $fmiOk -and $r.Locked) { $detail = ("release_binary_locked;" + $detail) }
Write-CaseLog -CaseType "FMI" -CaseName "FMI/emit-fmu" -DurationMs 0 -ExpectTargetOk $true -ActualOk $fmiOk -ExitCode $r.ExitCode -Status $(if ($fmiOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($fmiOk) { "expectation_met" } else { "fmi_artifacts_missing_or_command_failed" }) -Detail $detail

$modelicaDirScript = Join-Path $repoRoot "run_modelica_dir_regression.ps1"
if (-not $SkipDir -and (Test-Path $modelicaDirScript)) {
    Write-Host "[DIR] run_modelica_dir_regression.ps1"
    $startedAt = Get-Date
    $dirExeRel = Join-Path "jit-compiler" (Join-Path $cargoTargetDir "release\rustmodlica.exe")
    $la = [string]$env:LOCALAPPDATA
    $ap = [string]$env:APPDATA
    if (-not [string]::IsNullOrWhiteSpace($DirStdCacheRoot)) {
        $stdResolved = if ([System.IO.Path]::IsPathRooted($DirStdCacheRoot)) { $DirStdCacheRoot.Trim() } else { (Join-Path $repoRoot $DirStdCacheRoot.Trim().TrimStart("\", "/")) }
    } elseif (-not [string]::IsNullOrWhiteSpace($la)) {
        $stdResolved = Join-Path $la "rustmodlica\std"
    } else { $stdResolved = "" }
    if (-not [string]::IsNullOrWhiteSpace($stdResolved)) {
        $env:RUSTMODLICA_STD_CACHE_ROOT = $stdResolved
        Write-Host ("[DIR] RUSTMODLICA_STD_CACHE_ROOT=" + $stdResolved)
    }
    if (-not [string]::IsNullOrWhiteSpace($DirUserCacheRoot)) {
        $userResolved = if ([System.IO.Path]::IsPathRooted($DirUserCacheRoot)) { $DirUserCacheRoot.Trim() } else { (Join-Path $repoRoot $DirUserCacheRoot.Trim().TrimStart("\", "/")) }
    } elseif (-not [string]::IsNullOrWhiteSpace($ap)) {
        $userResolved = Join-Path $ap "rustmodlica\user"
    } else { $userResolved = "" }
    if (-not [string]::IsNullOrWhiteSpace($userResolved)) {
        $env:RUSTMODLICA_USER_CACHE_ROOT = $userResolved
        Write-Host ("[DIR] RUSTMODLICA_USER_CACHE_ROOT=" + $userResolved)
    }
    $dirWorkers = $DirParallelWorkers
    if ($dirWorkers -le 0) {
        $dirWorkers = [Math]::Max(1, [Environment]::ProcessorCount)
    }
    $dirArgs = @(
        "-Root", $repoRoot,
        "-MaxCases", "0",
        "-AllLibraryMo",
        "-NewtonCountsAsFailed",
        "-ExePath", $dirExeRel,
        "-ParallelWorkers", ([string]$dirWorkers)
    )
    if ($DirUsePrivateCache) { $dirArgs += "-UsePrivateCache" }
    if (-not [string]::IsNullOrWhiteSpace($DirPrivateCacheRoot)) {
        $dirArgs += @("-PrivateCacheRoot", $DirPrivateCacheRoot)
    }
    if ($DirTwoStage) { $dirArgs += "-TwoStage" }
    $dirArgs += @("-PerModelTimeoutSec", ([string]$DirPerModelTimeoutSec))
    $dirArgs += @("-AnalyzeFirstTimeoutSec", ([string]$DirAnalyzeFirstTimeoutSec))
    if (-not [string]::IsNullOrWhiteSpace($DirAnalyzeValidationMode)) {
        $dirArgs += @("-AnalyzeValidationMode", $DirAnalyzeValidationMode)
    }
    $dirArgs += @("-AnalyzeParallelWorkers", ([string]$DirAnalyzeParallelWorkers))
    $dirArgs += @("-AnalyzeShardNoProgressTimeoutSec", ([string]$DirAnalyzeShardNoProgressTimeoutSec))
    $dirArgs += @("-PerProcessMemoryLimitMb", ([string]$DirPerProcessMemoryLimitMb))
    $dirArgs += @("-QuarantineFile", $DirQuarantineFile)
    $dirArgs += @("-QuarantineConsecutiveHits", ([string]$DirQuarantineConsecutiveHits))
    $dirArgs += @("-ShardNoProgressTimeoutSec", ([string]$DirShardNoProgressTimeoutSec))
    if ($DirRetryQuarantined) { $dirArgs += "-RetryQuarantined" }
    $dirArgs += @("-AnalyzeCheckpointEvery", ([string]$DirAnalyzeCheckpointEvery))
    if ($DirResumeAnalyzeCheckpoint) { $dirArgs += "-ResumeAnalyzeCheckpoint" }
    $dirAllArgs = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $modelicaDirScript) + $dirArgs
    $dirProc = Start-Process -FilePath "powershell" -ArgumentList $dirAllArgs -WorkingDirectory $repoRoot -PassThru -NoNewWindow
    $hbStart = Get-Date
    while (-not $dirProc.HasExited) {
        if ($dirProc.WaitForExit(60000)) { break }
        $elapsed = [int](((Get-Date) - $hbStart).TotalSeconds)
        Write-Host ("[DIR heartbeat] still running elapsed_s=" + $elapsed + " pid=" + $dirProc.Id)
    }
    $exitModelicaDir = $dirProc.ExitCode
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $script:PhaseWallSec.dir = [int]([Math]::Max(0, $durationMs / 1000))
    $modelicaDirOk = ($exitModelicaDir -eq 0)
    if ($modelicaDirOk) { $ok++ } else { $bad++ }
    $sym = if ($modelicaDirOk) { "OK" } else { "!!" }
    $results += "$sym DIR-MSL+ModelicaTest  expect=self-consistency invariants  actual=$(if ($modelicaDirOk) { 'pass' } else { 'fail' }) (exit $exitModelicaDir)"
    $dirMetRel = "build_modelica_dir_regress\dir_metrics.json"
    Write-CaseLog -CaseType "DIR" -CaseName "DIR-MSL+ModelicaTest" -DurationMs $durationMs -ExpectTargetOk $true -ActualOk $modelicaDirOk -ExitCode $exitModelicaDir -Status $(if ($modelicaDirOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($modelicaDirOk) { "expectation_met" } else { "directory_regression_failed" }) -Detail ("dir_metrics=" + $dirMetRel + ";phase_wall_s_dir=" + $script:PhaseWallSec.dir)
} elseif ($SkipDir) {
    Write-Host "[DIR] skipped by -SkipDir"
}

$eventScanMatrixScript = Join-Path $repoRoot "jit-compiler\scripts\run_event_scan_matrix.ps1"
if (-not $SkipEventScan -and (Test-Path $eventScanMatrixScript)) {
    Write-Host "[EVENT-SCAN] run_event_scan_matrix.ps1"
    $startedAt = Get-Date
    $eventOutDir = "build_stability/event_scan_matrix_ci"
    $eventLibPath = Join-Path $repoRoot "jit-compiler"
    $eventCargoTargetDir = Join-Path $jitRoot $cargoTargetDirPrimary
    $null = & powershell -NoProfile -ExecutionPolicy Bypass -File $eventScanMatrixScript `
        -Root $repoRoot `
        -OutDir $eventOutDir `
        -LibPaths @($eventLibPath) `
        -CargoTargetDir $eventCargoTargetDir 2>&1
    $eventExit = $LASTEXITCODE
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $script:PhaseWallSec.event_scan = [int]([Math]::Max(0, $durationMs / 1000))
    $eventReport = Join-Path $repoRoot "$eventOutDir\consistency_report.txt"
    $eventCsv = Join-Path $repoRoot "$eventOutDir\deadband_matrix_stability.csv"
    $eventUnsupported = Join-Path $repoRoot "$eventOutDir\unsupported_models.txt"
    $eventNondet = 0
    $eventConfigErr = 0
    $eventUnsupportedCount = 0
    if (Test-Path $eventReport) {
        $reportLines = Get-Content $eventReport
        foreach ($line in $reportLines) {
            if ($line -match '^nondeterministic=(\d+)$') { $eventNondet = [int]$Matches[1] }
            if ($line -match '^config_error=(\d+)$') { $eventConfigErr = [int]$Matches[1] }
            if ($line -match '^unsupported=(\d+)$') { $eventUnsupportedCount = [int]$Matches[1] }
        }
    } elseif (Test-Path $eventCsv) {
        $rows = Import-Csv $eventCsv
        $eventNondet = @($rows | Where-Object { $_.status -eq "nondeterministic" }).Count
        $eventConfigErr = @($rows | Where-Object { $_.status -eq "config_error" -or $_.status -eq "error" }).Count
        $eventUnsupportedCount = @($rows | Where-Object { $_.status -eq "unsupported" }).Count
    } else {
        $eventConfigErr = 1
    }
    # EVENT-SCAN script can return non-zero even when the generated report
    # indicates no nondeterminism/config errors. Gate by report metrics.
    $eventOk = ($eventNondet -eq 0) -and ($eventConfigErr -eq 0)
    if ($eventOk) { $ok++ } else { $bad++ }
    $sym = if ($eventOk) { "OK" } else { "!!" }
    $results += "$sym EVENT-SCAN-MATRIX  expect=nondeterministic=0 and config_error=0  actual=$(if ($eventOk) { 'pass' } else { 'fail' }) (nondeterministic=$eventNondet, config_error=$eventConfigErr, unsupported=$eventUnsupportedCount, csv=$eventCsv, unsupported_file=$eventUnsupported)"
    Write-CaseLog -CaseType "EVENT_SCAN" -CaseName "EVENT-SCAN-MATRIX" -DurationMs $durationMs -ExpectTargetOk $true -ActualOk $eventOk -ExitCode $eventExit -Status $(if ($eventOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($eventOk) { "expectation_met" } else { "nondeterministic_or_config_error" }) -Detail ("nondeterministic=" + $eventNondet + ";config_error=" + $eventConfigErr + ";unsupported=" + $eventUnsupportedCount)
} elseif ($SkipEventScan) {
    Write-Host "[EVENT-SCAN] skipped by -SkipEventScan"
}

# Coverage gate: refresh scripts/coverage_status.json and require semantic>=target, modelica34>=target, gaps empty.
$coverageGenScript = Join-Path $jitRoot "scripts\generate_coverage_status.ps1"
if (Test-Path $coverageGenScript) {
    Write-Host "[COVERAGE] generate_coverage_status.ps1"
    $startedAt = Get-Date
    $null = & powershell -NoProfile -ExecutionPolicy Bypass -File $coverageGenScript 2>&1
    $coverageGenExit = $LASTEXITCODE
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $script:PhaseWallSec.coverage = [int]([Math]::Max(0, $durationMs / 1000))
    $coverageStatusPath = Join-Path $jitRoot "scripts\coverage_status.json"
    $coverageOk = $false
    $coverageDetail = "coverage_status_missing"
    if ($coverageGenExit -eq 0 -and (Test-Path -LiteralPath $coverageStatusPath)) {
        try {
            $coverage = Get-Content -LiteralPath $coverageStatusPath -Raw | ConvertFrom-Json
            $semanticTarget = [double]$coverage.semantic_target_percent
            $semanticCurrent = [double]$coverage.semantic_current_percent
            $modelicaTarget = [double]$coverage.modelica34_target_percent
            $modelicaCurrent = [double]$coverage.modelica34_current_percent
            $gaps = @($coverage.gaps)
            $coverageOk = ($semanticCurrent -ge $semanticTarget) -and ($modelicaCurrent -ge $modelicaTarget) -and ($gaps.Count -eq 0)
            $coverageDetail = ("semantic={0}/{1};modelica34={2}/{3};gaps={4}" -f $semanticCurrent, $semanticTarget, $modelicaCurrent, $modelicaTarget, ($gaps -join "|"))
        } catch {
            $coverageOk = $false
            $coverageDetail = ("coverage_status_parse_failed;" + $_.Exception.Message)
        }
    } else {
        $coverageOk = $false
        $coverageDetail = ("coverage_generator_failed_exit=" + $coverageGenExit)
    }
    if ($coverageOk) { $ok++ } else { $bad++ }
    $sym = if ($coverageOk) { "OK" } else { "!!" }
    $results += "$sym COVERAGE-GATE  expect=semantic>=target and modelica34>=target and gaps=empty  actual=$(if ($coverageOk) { 'pass' } else { 'fail' })"
    Write-CaseLog -CaseType "COVERAGE" -CaseName "COVERAGE-GATE" -DurationMs $durationMs -ExpectTargetOk $true -ActualOk $coverageOk -ExitCode $(if ($coverageOk) { 0 } else { 1 }) -Status $(if ($coverageOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($coverageOk) { "expectation_met" } else { "coverage_gate_failed" }) -Detail $coverageDetail
}

if ($SummarizeSparseDense) {
    $summaryScript = Join-Path $repoRoot "scripts\summarize_sparse_dense.ps1"
    $summaryInputDir = Join-Path $repoRoot "jit-compiler\build_sparse_dense_bench"
    if (Test-Path $summaryScript) {
        if (-not (Test-Path $summaryInputDir)) {
            Write-Host "[SPARSE-DENSE-SUMMARY] skipped: benchmark input dir not found ($summaryInputDir)"
            $ok++
            $results += "OK SPARSE-DENSE-SUMMARY  expect=summary artifacts generated  actual=skip (reason=missing_input_dir)"
            Write-CaseLog -CaseType "SUMMARY" -CaseName "SPARSE-DENSE-SUMMARY" -DurationMs 0 -ExpectTargetOk $true -ActualOk $true -ExitCode 0 -Status "SKIP" -Reason "missing_benchmark_input_dir" -Detail ("input_dir=" + $summaryInputDir + ";filter=" + $SparseDenseBltGuardFilter + ";models=" + ($SparseDenseModelFilter -join "|"))
        } else {
            Write-Host "[SPARSE-DENSE-SUMMARY] summarize_sparse_dense.ps1"
            $startedAt = Get-Date
            if ($SparseDenseModelFilter -and $SparseDenseModelFilter.Count -gt 0) {
                $summaryOut = & $summaryScript -InputDir $summaryInputDir -BltGuardFilter $SparseDenseBltGuardFilter -ModelFilter $SparseDenseModelFilter 2>&1
            } else {
                $summaryOut = & $summaryScript -InputDir $summaryInputDir -BltGuardFilter $SparseDenseBltGuardFilter 2>&1
            }
            $summaryExit = $LASTEXITCODE
            $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
            $summaryOk = ($summaryExit -eq 0)
            if ($summaryOk) { $ok++ } else { $bad++ }
            $sym = if ($summaryOk) { "OK" } else { "!!" }
            $results += "$sym SPARSE-DENSE-SUMMARY  expect=summary artifacts generated  actual=$(if ($summaryOk) { 'pass' } else { 'fail' }) (filter=$SparseDenseBltGuardFilter)"
            $detail = ("filter=" + $SparseDenseBltGuardFilter + ";models=" + (($SparseDenseModelFilter -join "|")))
            Write-CaseLog -CaseType "SUMMARY" -CaseName "SPARSE-DENSE-SUMMARY" -DurationMs $durationMs -ExpectTargetOk $true -ActualOk $summaryOk -ExitCode $summaryExit -Status $(if ($summaryOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($summaryOk) { "expectation_met" } else { "summary_script_failed" }) -Detail $detail
            if ($summaryOut) {
                $summaryOut | ForEach-Object { Write-Host $_ }
            }
        }
    } else {
        Write-Host "[SPARSE-DENSE-SUMMARY] skipped: script not found"
    }
}

if (-not [string]::IsNullOrWhiteSpace($RecordBaseline)) {
    $rbDir = if ([System.IO.Path]::IsPathRooted($RecordBaseline)) { $RecordBaseline } else { Join-Path $repoRoot $RecordBaseline }
    New-Item -ItemType Directory -Force -Path $rbDir | Out-Null
    $sumPath = Join-Path $rbDir "regression_summary.json"
    $dirMetricsForSummary = (Join-Path $repoRoot "build_modelica_dir_regress\dir_metrics.json")
    Write-RegressionSummaryJson -NdjsonPath $regressLogNdjson -OutputJsonPath $sumPath -PhaseWallSeconds $script:PhaseWallSec -DirMetricsPath $dirMetricsForSummary
    $readmeRb = Join-Path $rbDir "README.md"
    if (-not (Test-Path -LiteralPath $readmeRb)) {
        @(
            "Full regression summary (NDJSON rollup).",
            "Record: run this script with -RecordBaseline <dir>.",
            "Compare: run with -CompareBaseline <baseline_dir> -CompareBaselineCurrent <new_summary.json>."
        ) | Set-Content -LiteralPath $readmeRb -Encoding UTF8
    }
}
if (-not [string]::IsNullOrWhiteSpace($CompareBaseline) -and -not [string]::IsNullOrWhiteSpace($CompareBaselineCurrent)) {
    $b0 = if ([System.IO.Path]::IsPathRooted($CompareBaseline)) { $CompareBaseline } else { Join-Path $repoRoot $CompareBaseline }
    $b1 = if ([System.IO.Path]::IsPathRooted($CompareBaselineCurrent)) { $CompareBaselineCurrent } else { Join-Path $repoRoot $CompareBaselineCurrent }
    $baseSum = Join-Path $b0 "regression_summary.json"
    Compare-RegressionSummaryJson -BaselinePath $baseSum -CurrentPath $b1
}

$results | ForEach-Object { Write-Host $_ }
Write-Host ""
Write-Host "Summary: $ok passed (match expected), $bad mismatch"
Write-Host ("Cargo target dir primary: " + $cargoTargetDirPrimary)
if ($cargoTargetDirFallbackUsed) {
    Write-Host ("Cargo target dir fallback: " + $cargoTargetDirFallback)
} else {
    Write-Host "Cargo target dir fallback: (not used)"
}
Write-Host "Regression logs: $regressLogNdjson ; $regressLogCsv"
Pop-Location
$regressExitCode = if ($bad -gt 0) { 1 } else { 0 }
exit $regressExitCode
