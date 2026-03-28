# Targeted single runs for the 14 historically failing DIR-MSL models (no full library scan).
# Same CLI shape as run_modelica_dir_regression.ps1 defaults: dummyDerivative, rk4, Newton strict.
# By default skips Modelica.Mechanics.MultiBody.Examples.Loops.EngineV6 (very long JIT). Run all 14: -SkipPattern ""
#
# Usage (repo root; use Windows PowerShell if pwsh is not installed):
#   powershell -NoProfile -ExecutionPolicy Bypass -File jit-compiler/scripts/run_msl_dir_failures_14.ps1 [-ExePath <path>] ...
#   pwsh -File jit-compiler/scripts/run_msl_dir_failures_14.ps1 ...
#
# Requires: rustmodlica.exe built with current sources.

param(
    [string]$Root = "",
    [string]$ExePath = "",
    [string]$OutDir = "build_dir_failures_14",
    [double]$TEnd = 10.0,
    [double]$Dt = 0.01,
    # Regex; matching model names are skipped. Default skips EngineV6 (long JIT). Clear with -SkipPattern "".
    [string]$SkipPattern = "EngineV6"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
}
$repoRoot = (Resolve-Path -LiteralPath $Root).Path
$jitRoot = Join-Path $repoRoot "jit-compiler"
$modelicaRoot = Join-Path $jitRoot "Modelica"
$modelicaTestRoot = Join-Path $jitRoot "ModelicaTest"
$exe = if ($ExePath -ne "") {
    if ([System.IO.Path]::IsPathRooted($ExePath)) { $ExePath } else { Join-Path $repoRoot $ExePath }
} else {
    Join-Path $repoRoot "jit-compiler\target\release\rustmodlica.exe"
}
if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Build first: cargo build -p rustmodlica --release (or pass -ExePath)"
    exit 1
}
if ($SkipPattern -ne "") {
    Write-Host "Active SkipPattern: $SkipPattern (use -SkipPattern '' to run every listed model)"
}

$outPath = Join-Path $repoRoot $OutDir
if (-not (Test-Path -LiteralPath $outPath)) { New-Item -ItemType Directory -Path $outPath | Out-Null }
$logDir = Join-Path $outPath "logs"
if (-not (Test-Path -LiteralPath $logDir)) { New-Item -ItemType Directory -Path $logDir | Out-Null }

$models = @(
    "Modelica.Fluid.Examples.AST_BatchPlant.BatchPlant_StandardWater",
    "ModelicaTest.Fluid.TestPipesAndValves.LumpedPipeInitialization",
    "Modelica.Magnetic.FundamentalWave.Examples.BasicMachines.SynchronousMachines.ComparisonPolyphase.SMEE_Generator_Polyphase",
    "Modelica.Mechanics.MultiBody.Examples.Loops.EngineV6",
    "Modelica.Electrical.PowerConverters.Examples.DCAC.PolyphaseTwoLevel.PolyphaseTwoLevel_RL",
    "ModelicaTest.Electrical.PowerConverters.HalfControlledBridge2mPulse",
    "Modelica.Magnetic.QuasiStatic.FundamentalWave.Examples.BasicMachines.InductionMachines.IMC_Initialize",
    "Modelica.Magnetic.QuasiStatic.FundamentalWave.Examples.BasicMachines.SynchronousMachines.SMPM_FieldWeakening",
    "ModelicaTest.Fluid.TestPipesAndValves.DynamicPipeInitialization",
    "Modelica.Mechanics.MultiBody.Examples.Constraints.PrismaticConstraint",
    "ModelicaTest.Blocks.FirstOrderHold",
    "Modelica.Fluid.Examples.TraceSubstances.RoomCO2WithControls",
    "Modelica.Magnetic.FundamentalWave.Examples.BasicMachines.InductionMachines.IMC_Inverter",
    "ModelicaTest.Blocks.Discrete"
)

$failed = 0
$skipped = 0
$summaryLines = @()

Push-Location $jitRoot
$oldEa = $ErrorActionPreference
$ErrorActionPreference = "Continue"

$idx = 0
foreach ($m in $models) {
    if ($SkipPattern -ne "" -and ($m -match $SkipPattern)) {
        Write-Host ""
        Write-Host "[skip] $m (SkipPattern)"
        $skipped++
        $summaryLines += "-- $m  reason=skipped_by_pattern"
        continue
    }
    $idx++
    $safe = ($m -replace '[^A-Za-z0-9_.-]', '_')
    $csv = Join-Path $outPath "$safe.csv"
    $logPath = Join-Path $logDir "$safe.log"
    Write-Host ""
    Write-Host "[$idx/14] $m"
    Remove-Item -LiteralPath $csv -ErrorAction SilentlyContinue
    $cliArgs = @(
        "--index-reduction-method=dummyDerivative",
        "--lib-path=$modelicaRoot",
        "--lib-path=$modelicaTestRoot",
        "--solver=rk4",
        "--dt=$Dt",
        "--t-end=$TEnd",
        "--result-file=$csv",
        $m
    )
    $outLines = & $exe @cliArgs 2>&1
    $exit = $LASTEXITCODE
    $outLines | Set-Content -LiteralPath $logPath -Encoding UTF8
    $ok = ($exit -eq 0) -and (Test-Path -LiteralPath $csv)
    if ($ok) {
        $line = "OK $m  exit=$exit"
        Write-Host $line
        $summaryLines += $line
    } else {
        $failed++
        $lastErr = ""
        foreach ($ln in $outLines) {
            $s = $ln.ToString()
            if ($s -match 'JIT compilation failed|Variable ''|Newton-Raphson|Simulation failed|error') {
                $lastErr = $s.Trim()
                break
            }
        }
        if ($lastErr -eq "") { $lastErr = "exit=$exit" }
        $line = "!! $m  exit=$exit  hint=$lastErr"
        Write-Host $line
        $summaryLines += $line
    }
}

Pop-Location
$ErrorActionPreference = $oldEa

$summaryPath = Join-Path $outPath "summary.txt"
$summaryLines | Set-Content -LiteralPath $summaryPath -Encoding UTF8
Write-Host ""
$ran = $models.Count - $skipped
Write-Host "Summary: ran=$ran skipped=$skipped ok=$($ran - $failed) failed=$failed (of $($models.Count) listed)"
Write-Host "Details: $summaryPath"
exit $(if ($failed -eq 0) { 0 } else { 1 })
