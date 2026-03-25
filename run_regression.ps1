# Full regression: run each model and compare exit code to expected (pass=0, fail=non-zero)
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
    @("TestLib/MixedMultiTargetSafePass", "pass"),
    # Expected failures: shape/cardinality mismatches in multi-output assignments
    @("TestLib/MultiOutputShapeMismatch", "fail"),
    @("TestLib/MultiOutputRecordShapeMismatch", "fail"),
    @("TestLib/MultiOutput2DArrayShapeMismatch", "fail"),
    @("TestLib/MultiOutputComprehensionShapeMismatch", "fail"),
    @("TestLib/MultiOutputRecordNestedArrayMismatch", "fail"),
    @("TestLib/MultiOutputCrossLayerComprehensionMismatch", "fail"),
    # Expected failures: invalid nested/field LHS stores that should be rejected
    @("TestLib/MultiOutputComplexLhsFieldStore", "fail"),
    @("TestLib/DeepRecordNestedMismatch", "fail"),
    @("TestLib/MixedNestedLhsFieldStoreMismatch", "fail"),
    @("TestLib/MixedMultiTargetFieldStoreFail", "fail"),
    # Expected failures: cross-module comprehension shape propagation mismatches
    @("TestLib/CrossModuleComprehensionMismatch", "fail"),
    @("TestLib/CrossModuleRecordCompositeMismatch", "fail"),
    @("TestLib/AliasChainTypeMismatch", "fail"),
    @("TestLib/MultiTopCombined", "pass"),
    @("TestLib/PreEdgeChange", "pass"),
    @("TestLib/IfEqTest", "pass"),
    @("TestLib/AssertTerminateTest", "pass"),
    @("TestLib/PkgA.PkgB.Inner", "pass"),
    @("TestLib/TypeAliasTest", "pass"),
    @("TestLib/ReplaceableTest", "pass"),
    @("TestLib/ClockedPartitionTest", "pass"),
    @("TestLib/ClockedTwoRates", "pass"),
    @("ModelicaTest.JitStress.SyncOmCompare", "pass"),
    @("TestLib/HoldPreviousTest", "pass"),
    @("TestLib/IntervalClockTest", "pass"),
    @("TestLib/DefaultArgTest", "pass"),
    @("TestLib/ReinitTest", "pass"),
    @("TestLib/ExtLibAnnotationTest", "pass"),
    @("TestLib/ArrayArgTest", "pass"),
    @("TestLib/SubSuperShiftSampleTest", "pass"),
    @("TestLib/RestParamTest", "pass")
)
$repoRoot = $PSScriptRoot
$jitRoot = Join-Path $repoRoot "jit-compiler"
$regressLogDir = Join-Path $repoRoot "build_regression_logs"
if (-not (Test-Path -LiteralPath $regressLogDir)) { New-Item -ItemType Directory -Path $regressLogDir | Out-Null }
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$regressLogNdjson = Join-Path $regressLogDir ("run_regression_{0}.ndjson" -f $stamp)
$regressLogCsv = Join-Path $regressLogDir ("run_regression_{0}.csv" -f $stamp)
"timestamp,case_type,case_name,duration_ms,expect_target_ok,actual_ok,exit_code,status,reason,detail" | Set-Content -LiteralPath $regressLogCsv -Encoding UTF8
function Escape-Csv([string]$s) {
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
        Escape-Csv $ts
        Escape-Csv $CaseType
        Escape-Csv $CaseName
        $DurationMs
        $ExpectTargetOk
        $ActualOk
        $ExitCode
        Escape-Csv $Status
        Escape-Csv $Reason
        Escape-Csv $Detail
    ) -join ","
    Add-Content -LiteralPath $regressLogCsv -Value $csvLine -Encoding UTF8
}
Push-Location $jitRoot
# Isolated cargo target dir avoids Windows file locks on `target/release/rustmodlica.exe` during long runs.
$cargoTargetDir = "target_regression"
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
    $runOut = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- @extra $name 2>&1
    $exit = $LASTEXITCODE
    $runText = ($runOut | Out-String)
    if ($exit -ne 0 -and ($runText -match "os error 5" -or $runText -match "failed to remove file")) {
        Get-Process rustmodlica,cargo -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Milliseconds 800
        $runOut = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- @extra $name 2>&1
        $exit = $LASTEXITCODE
        $runText = ($runOut | Out-String)
    }
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $actual = if ($exit -eq 0) { "pass" } else { "fail" }
    $match = ($actual -eq $expect)
    $expectOk = ($expect -eq "pass")
    $actualOk = ($actual -eq "pass")
    if ($match) { $ok++ } else { $bad++ }
    $sym = if ($match) { "OK" } else { "!!" }
    $results += "$sym $name  expect=$expect  actual=$actual (exit $exit)"
    $detail = ""
    if (-not $match) {
        if ($runText -match "Model not found") { $detail = "model_not_found" }
        elseif ($runText -match "os error 5|failed to remove file") { $detail = "release_binary_locked" }
    }
    Write-CaseLog -CaseType "CASE" -CaseName $name -DurationMs $durationMs -ExpectTargetOk $expectOk -ActualOk $actualOk -ExitCode $exit -Status $(if ($match) { "OK" } else { "MISMATCH" }) -Reason $(if ($match) { "expectation_met" } else { "expectation_mismatch" }) -Detail $detail
}
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
    $null = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- --script=$scriptPath 2>&1
    $exit = $LASTEXITCODE
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $actual = if ($exit -eq 0) { "pass" } else { "fail" }
    $match = ($actual -eq $expect)
    $expectOk = ($expect -eq "pass")
    $actualOk = ($actual -eq "pass")
    if ($match) { $ok++ } else { $bad++ }
    $sym = if ($match) { "OK" } else { "!!" }
    $results += "$sym $name  expect=$expect  actual=$actual (exit $exit)"
    Write-CaseLog -CaseType "SCRIPT" -CaseName $name -DurationMs $durationMs -ExpectTargetOk $expectOk -ActualOk $actualOk -ExitCode $exit -Status $(if ($match) { "OK" } else { "MISMATCH" }) -Reason $(if ($match) { "expectation_met" } else { "expectation_mismatch" }) -Detail ""
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
    $null = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- $t.opts $t.model 2>&1
    $exit = $LASTEXITCODE
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $actual = if ($exit -eq 0) { "pass" } else { "fail" }
    $match = ($actual -eq $expect)
    $expectOk = ($expect -eq "pass")
    $actualOk = ($actual -eq "pass")
    if ($match) { $ok++ } else { $bad++ }
    $sym = if ($match) { "OK" } else { "!!" }
    $results += "$sym $name  expect=$expect  actual=$actual (exit $exit)"
    Write-CaseLog -CaseType "EMIT_C" -CaseName $name -DurationMs $durationMs -ExpectTargetOk $expectOk -ActualOk $actualOk -ExitCode $exit -Status $(if ($match) { "OK" } else { "MISMATCH" }) -Reason $(if ($match) { "expectation_met" } else { "expectation_mismatch" }) -Detail ""
}
# FUNC-7: emit-c with external string arg; JIT fails but C must be emitted with const char* and string literal
if (-not (Test-Path build_regress_emit_string)) { New-Item -ItemType Directory -Path build_regress_emit_string | Out-Null }
$null = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- --emit-c=build_regress_emit_string TestLib/StringArgExtFunc 2>&1
$exitString = $LASTEXITCODE
$cPath = "build_regress_emit_string\model.c"
$func7Ok = ($exitString -ne 0) -and (Test-Path $cPath)
if ($func7Ok) {
    $cContent = Get-Content -Raw $cPath
    $func7Ok = ($cContent -match "const char\*") -and ($cContent -match "extLog") -and ($cContent -match "test")
}
if ($func7Ok) { $ok++ } else { $bad++ }
$sym = if ($func7Ok) { "OK" } else { "!!" }
$results += "$sym FUNC-7/EmitC/StringArgExtFunc  expect=emit C with string ABI  actual=$(if ($func7Ok) { 'pass' } else { 'fail' })"
Write-CaseLog -CaseType "EMIT_C" -CaseName "FUNC-7/EmitC/StringArgExtFunc" -DurationMs 0 -ExpectTargetOk $true -ActualOk $func7Ok -ExitCode $exitString -Status $(if ($func7Ok) { "OK" } else { "MISMATCH" }) -Reason $(if ($func7Ok) { "expectation_met" } else { "string_abi_not_emitted_or_jit_expectation_failed" }) -Detail ""
# SYNC-2: clocked semantics (when sample(...)); run with backend-dae-info and check clocked line present
$sync2Out = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- --backend-dae-info TestLib/ClockedPartitionTest 2>&1
Write-Host "[SYNC] ClockedPartitionTest backend info"
$sync2Ok = ($LASTEXITCODE -eq 0) -and ($sync2Out -match "clocked")
if ($sync2Ok) { $ok++ } else { $bad++ }
$sym = if ($sync2Ok) { "OK" } else { "!!" }
$results += "$sym SYNC-2/ClockedPartitionTest  expect=backend clocked output  actual=$(if ($sync2Ok) { 'pass' } else { 'fail' })"
Write-CaseLog -CaseType "SYNC" -CaseName "SYNC-2/ClockedPartitionTest" -DurationMs 0 -ExpectTargetOk $true -ActualOk $sync2Ok -ExitCode $LASTEXITCODE -Status $(if ($sync2Ok) { "OK" } else { "MISMATCH" }) -Reason $(if ($sync2Ok) { "expectation_met" } else { "clocked_backend_info_missing_or_run_failed" }) -Detail ""
# SYNC freeze: run clocked models twice and require deterministic CSV output
$clockedDeterminismCases = @(
    "TestLib/ClockedPartitionTest",
    "TestLib/ClockedTwoRates",
    "TestLib/HoldPreviousTest",
    "TestLib/SubSuperShiftSampleTest"
)
foreach ($m in $clockedDeterminismCases) {
    Write-Host "[SYNC-DET] $m"
    $startedAt = Get-Date
    $safeName = $m.Replace("/", "_").Replace(".", "_")
    $csvA = "build_regress_clocked_${safeName}_a.csv"
    $csvB = "build_regress_clocked_${safeName}_b.csv"
    $null = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- --solver=rk4 --output-interval=0.001 --result-file=$csvA $m 2>&1
    $e1 = $LASTEXITCODE
    $null = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- --solver=rk4 --output-interval=0.001 --result-file=$csvB $m 2>&1
    $e2 = $LASTEXITCODE
    $same = $false
    if ($e1 -eq 0 -and $e2 -eq 0 -and (Test-Path $csvA) -and (Test-Path $csvB)) {
        $h1 = (Get-FileHash -Algorithm SHA256 $csvA).Hash
        $h2 = (Get-FileHash -Algorithm SHA256 $csvB).Hash
        $same = ($h1 -eq $h2)
    }
    if ($same) { $ok++ } else { $bad++ }
    $sym = if ($same) { "OK" } else { "!!" }
    $results += "$sym SYNC-DET/$m  expect=stable repeated output  actual=$(if ($same) { 'pass' } else { 'fail' })"
    Write-CaseLog -CaseType "SYNC_DET" -CaseName ("SYNC-DET/" + $m) -DurationMs ([long](((Get-Date) - $startedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $same -ExitCode $(if ($same) { 0 } else { 1 }) -Status $(if ($same) { "OK" } else { "MISMATCH" }) -Reason $(if ($same) { "expectation_met" } else { "non_deterministic_or_run_failed" }) -Detail ""
}
# FMI emit: --emit-fmu produces modelDescription.xml and fmi2_cs.c
if (-not (Test-Path build_regress_fmu)) { New-Item -ItemType Directory -Path build_regress_fmu | Out-Null }
$null = & cargo run --target-dir $cargoTargetDir -p rustmodlica --bin rustmodlica --release -- --emit-fmu=build_regress_fmu TestLib/SimpleTest 2>&1
Write-Host "[FMI] emit-fmu"
$fmiOk = ($LASTEXITCODE -eq 0) -and (Test-Path "build_regress_fmu\modelDescription.xml") -and (Test-Path "build_regress_fmu\fmi2_cs.c")
if ($fmiOk) { $ok++ } else { $bad++ }
$sym = if ($fmiOk) { "OK" } else { "!!" }
$results += "$sym FMI/emit-fmu  expect=modelDescription.xml and fmi2_cs.c  actual=$(if ($fmiOk) { 'pass' } else { 'fail' })"
Write-CaseLog -CaseType "FMI" -CaseName "FMI/emit-fmu" -DurationMs 0 -ExpectTargetOk $true -ActualOk $fmiOk -ExitCode $LASTEXITCODE -Status $(if ($fmiOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($fmiOk) { "expectation_met" } else { "fmi_artifacts_missing_or_command_failed" }) -Detail ""

$modelicaDirScript = Join-Path $repoRoot "run_modelica_dir_regression.ps1"
if (Test-Path $modelicaDirScript) {
    Write-Host "[DIR] run_modelica_dir_regression.ps1"
    $startedAt = Get-Date
    $null = & powershell -NoProfile -ExecutionPolicy Bypass -File $modelicaDirScript -Root $repoRoot -MaxCases 0 -AllLibraryMo -NewtonCountsAsFailed 2>&1
    $exitModelicaDir = $LASTEXITCODE
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
    $modelicaDirOk = ($exitModelicaDir -eq 0)
    if ($modelicaDirOk) { $ok++ } else { $bad++ }
    $sym = if ($modelicaDirOk) { "OK" } else { "!!" }
    $results += "$sym DIR-MSL+ModelicaTest  expect=self-consistency invariants  actual=$(if ($modelicaDirOk) { 'pass' } else { 'fail' }) (exit $exitModelicaDir)"
    Write-CaseLog -CaseType "DIR" -CaseName "DIR-MSL+ModelicaTest" -DurationMs $durationMs -ExpectTargetOk $true -ActualOk $modelicaDirOk -ExitCode $exitModelicaDir -Status $(if ($modelicaDirOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($modelicaDirOk) { "expectation_met" } else { "directory_regression_failed" }) -Detail ""
}

$eventScanMatrixScript = Join-Path $repoRoot "jit-compiler\scripts\run_event_scan_matrix.ps1"
if (Test-Path $eventScanMatrixScript) {
    Write-Host "[EVENT-SCAN] run_event_scan_matrix.ps1"
    $startedAt = Get-Date
    $eventOutDir = "build_stability/event_scan_matrix_ci"
    $eventLibPath = Join-Path $repoRoot "jit-compiler"
    $null = & powershell -NoProfile -ExecutionPolicy Bypass -File $eventScanMatrixScript `
        -Root $repoRoot `
        -OutDir $eventOutDir `
        -LibPaths @($eventLibPath) 2>&1
    $eventExit = $LASTEXITCODE
    $durationMs = [long](((Get-Date) - $startedAt).TotalMilliseconds)
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
    $eventOk = ($eventExit -eq 0) -and ($eventNondet -eq 0) -and ($eventConfigErr -eq 0)
    if ($eventOk) { $ok++ } else { $bad++ }
    $sym = if ($eventOk) { "OK" } else { "!!" }
    $results += "$sym EVENT-SCAN-MATRIX  expect=nondeterministic=0 and config_error=0  actual=$(if ($eventOk) { 'pass' } else { 'fail' }) (nondeterministic=$eventNondet, config_error=$eventConfigErr, unsupported=$eventUnsupportedCount, csv=$eventCsv, unsupported_file=$eventUnsupported)"
    Write-CaseLog -CaseType "EVENT_SCAN" -CaseName "EVENT-SCAN-MATRIX" -DurationMs $durationMs -ExpectTargetOk $true -ActualOk $eventOk -ExitCode $eventExit -Status $(if ($eventOk) { "OK" } else { "MISMATCH" }) -Reason $(if ($eventOk) { "expectation_met" } else { "nondeterministic_or_config_error" }) -Detail ("nondeterministic=" + $eventNondet + ";config_error=" + $eventConfigErr + ";unsupported=" + $eventUnsupportedCount)
}

$results | ForEach-Object { Write-Host $_ }
Write-Host ""
Write-Host "Summary: $ok passed (match expected), $bad mismatch"
Write-Host "Regression logs: $regressLogNdjson ; $regressLogCsv"
Pop-Location
if ($bad -gt 0) { exit 1 }
