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
Push-Location $jitRoot
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
    $extra = @()
    if ($caseExtraArgs.ContainsKey($name)) { $extra = $caseExtraArgs[$name] }
    $null = & cargo run --release -- @extra -- $name 2>&1
    $exit = $LASTEXITCODE
    $actual = if ($exit -eq 0) { "pass" } else { "fail" }
    $match = ($actual -eq $expect)
    if ($match) { $ok++ } else { $bad++ }
    $sym = if ($match) { "OK" } else { "!!" }
    $results += "$sym $name  expect=$expect  actual=$actual (exit $exit)"
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
    $scriptPath = Join-Path ".." $t.path
    $expect = $t.expect
    $null = & cargo run --release -- --script=$scriptPath 2>&1
    $exit = $LASTEXITCODE
    $actual = if ($exit -eq 0) { "pass" } else { "fail" }
    $match = ($actual -eq $expect)
    if ($match) { $ok++ } else { $bad++ }
    $sym = if ($match) { "OK" } else { "!!" }
    $results += "$sym $name  expect=$expect  actual=$actual (exit $exit)"
}
# FUNC-6: emit-c with user function (static C body)
$emitCTests = @(
    @{ name = "EmitC/RecursiveFunc"; opts = "--emit-c=build_regress_emit"; model = "TestLib/RecursiveFunc"; expect = "pass" }
)
if (-not (Test-Path build_regress_emit)) { New-Item -ItemType Directory -Path build_regress_emit | Out-Null }
foreach ($t in $emitCTests) {
    $name = $t.name
    Write-Host "[EMIT-C] $name"
    $expect = $t.expect
    $null = & cargo run --release -- $t.opts $t.model 2>&1
    $exit = $LASTEXITCODE
    $actual = if ($exit -eq 0) { "pass" } else { "fail" }
    $match = ($actual -eq $expect)
    if ($match) { $ok++ } else { $bad++ }
    $sym = if ($match) { "OK" } else { "!!" }
    $results += "$sym $name  expect=$expect  actual=$actual (exit $exit)"
}
# FUNC-7: emit-c with external string arg; JIT fails but C must be emitted with const char* and string literal
if (-not (Test-Path build_regress_emit_string)) { New-Item -ItemType Directory -Path build_regress_emit_string | Out-Null }
$null = & cargo run --release -- --emit-c=build_regress_emit_string TestLib/StringArgExtFunc 2>&1
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
# SYNC-2: clocked semantics (when sample(...)); run with backend-dae-info and check clocked line present
$sync2Out = & cargo run --release -- --backend-dae-info TestLib/ClockedPartitionTest 2>&1
Write-Host "[SYNC] ClockedPartitionTest backend info"
$sync2Ok = ($LASTEXITCODE -eq 0) -and ($sync2Out -match "clocked")
if ($sync2Ok) { $ok++ } else { $bad++ }
$sym = if ($sync2Ok) { "OK" } else { "!!" }
$results += "$sym SYNC-2/ClockedPartitionTest  expect=backend clocked output  actual=$(if ($sync2Ok) { 'pass' } else { 'fail' })"
# SYNC freeze: run clocked models twice and require deterministic CSV output
$clockedDeterminismCases = @(
    "TestLib/ClockedPartitionTest",
    "TestLib/ClockedTwoRates",
    "TestLib/HoldPreviousTest",
    "TestLib/SubSuperShiftSampleTest"
)
foreach ($m in $clockedDeterminismCases) {
    Write-Host "[SYNC-DET] $m"
    $safeName = $m.Replace("/", "_").Replace(".", "_")
    $csvA = "build_regress_clocked_${safeName}_a.csv"
    $csvB = "build_regress_clocked_${safeName}_b.csv"
    $null = & cargo run --release -- --solver=rk4 --output-interval=0.001 --result-file=$csvA $m 2>&1
    $e1 = $LASTEXITCODE
    $null = & cargo run --release -- --solver=rk4 --output-interval=0.001 --result-file=$csvB $m 2>&1
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
}
# FMI emit: --emit-fmu produces modelDescription.xml and fmi2_cs.c
if (-not (Test-Path build_regress_fmu)) { New-Item -ItemType Directory -Path build_regress_fmu | Out-Null }
$null = & cargo run --release -- --emit-fmu=build_regress_fmu TestLib/SimpleTest 2>&1
Write-Host "[FMI] emit-fmu"
$fmiOk = ($LASTEXITCODE -eq 0) -and (Test-Path "build_regress_fmu\modelDescription.xml") -and (Test-Path "build_regress_fmu\fmi2_cs.c")
if ($fmiOk) { $ok++ } else { $bad++ }
$sym = if ($fmiOk) { "OK" } else { "!!" }
$results += "$sym FMI/emit-fmu  expect=modelDescription.xml and fmi2_cs.c  actual=$(if ($fmiOk) { 'pass' } else { 'fail' })"

$modelicaDirScript = Join-Path $repoRoot "run_modelica_dir_regression.ps1"
if (Test-Path $modelicaDirScript) {
    Write-Host "[DIR] run_modelica_dir_regression.ps1"
    $null = & powershell -NoProfile -ExecutionPolicy Bypass -File $modelicaDirScript -Root $repoRoot -MaxCases 0 -AllLibraryMo -NewtonCountsAsFailed 2>&1
    $exitModelicaDir = $LASTEXITCODE
    $modelicaDirOk = ($exitModelicaDir -eq 0)
    if ($modelicaDirOk) { $ok++ } else { $bad++ }
    $sym = if ($modelicaDirOk) { "OK" } else { "!!" }
    $results += "$sym DIR-MSL+ModelicaTest  expect=self-consistency invariants  actual=$(if ($modelicaDirOk) { 'pass' } else { 'fail' }) (exit $exitModelicaDir)"
}

$eventScanMatrixScript = Join-Path $repoRoot "jit-compiler\scripts\run_event_scan_matrix.ps1"
if (Test-Path $eventScanMatrixScript) {
    Write-Host "[EVENT-SCAN] run_event_scan_matrix.ps1"
    $eventOutDir = "build_stability/event_scan_matrix_ci"
    $eventLibPath = Join-Path $repoRoot "jit-compiler"
    $null = & powershell -NoProfile -ExecutionPolicy Bypass -File $eventScanMatrixScript `
        -Root $repoRoot `
        -OutDir $eventOutDir `
        -LibPaths @($eventLibPath) 2>&1
    $eventExit = $LASTEXITCODE
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
}

$results | ForEach-Object { Write-Host $_ }
Write-Host ""
Write-Host "Summary: $ok passed (match expected), $bad mismatch"
Pop-Location
if ($bad -gt 0) { exit 1 }
