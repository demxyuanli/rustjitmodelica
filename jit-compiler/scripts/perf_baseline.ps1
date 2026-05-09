# Performance regression baseline for JIT compiler
# Captures compile time, memory, and simulation step counts for key models.
# Run: pwsh -File jit-compiler/scripts/perf_baseline.ps1 [-BaselineDir baseline/YYYYMMDD]
param(
    [string]$BaselineDir = "",
    [switch]$Compare
)

$ErrorActionPreference = "Stop"
$jitRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $jitRoot "target\release\rustmodlica.exe"
if (-not (Test-Path $exe)) {
    Write-Warning "No release build found, building..."
    Push-Location $jitRoot
    cargo build -p rustmodlica --release
    Pop-Location
}
if (-not (Test-Path $exe)) {
    throw "rustmodlica.exe not found at $exe"
}

# Key benchmark models from TestLib
$benchmarkModels = @(
    "AlgTest", "AlgebraicLoop2Eq", "SimpleTest", "BigFor",
    "AdaptiveRKTest", "DirectionSwitch", "AlgorithmElseWhen",
    "CoupledClutches", "EngineV6", "TwoBitAdder"
)

# if we have MSL, add a few more
$mslRoot = Join-Path $jitRoot "Modelica"
if (Test-Path (Join-Path $mslRoot "package.mo")) {
    $benchmarkModels += @("BouncingBall", "HelloWorld")
}

$dateStr = if ($BaselineDir) { $BaselineDir } else { "baseline/$(Get-Date -Format 'yyyyMMdd')_perf" }
$outDir = Join-Path $jitRoot $dateStr
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$results = @()
$testLibDir = Join-Path $jitRoot "jit-compiler\TestLib"
$libPaths = @($testLibDir)
if (Test-Path $mslRoot) { $libPaths += $mslRoot }

foreach ($model in $benchmarkModels) {
    Write-Host "Benchmarking: $model"
    $libPathArgs = ($libPaths | ForEach-Object { "--lib-path=$_" }) -join " "
    $sw = [System.Diagnostics.Stopwatch]::StartNew()

    # Validate + compile timing
    $validateOut = & $exe --lib-path=$testLibDir --validate-tier=analyze --validate $model 2>&1 | Out-String
    $sw.Stop()
    $compileMs = $sw.ElapsedMilliseconds

    $success = $validateOut -match '"success"\s*:\s*true'
    $outputVars = if ($validateOut -match '"output_vars"\s*:\s*\[(.*?)\]') { $Matches[1].Split(',').Count } else { 0 }
    $stateVars = if ($validateOut -match '"state_vars"\s*:\s*\[(.*?)\]') { $Matches[1].Split(',').Count } else { 0 }

    $result = [PSCustomObject]@{
        model = $model
        success = $success
        compileMs = $compileMs
        outputVars = $outputVars
        stateVars = $stateVars
    }
    $results += $result

    Write-Host "  $model : compile=${compileMs}ms success=${success} outputs=${outputVars} states=${stateVars}"
}

$resultsFile = Join-Path $outDir "jit_perf_baseline.json"
$results | ConvertTo-Json -Depth 3 | Set-Content $resultsFile

# Summary
$passed = ($results | Where-Object { $_.success }).Count
$total = $results.Count
$avgMs = if ($results.Count -gt 0) { ($results | Measure-Object -Property compileMs -Average).Average } else { 0 }

Write-Host ""
Write-Host "=== Perf Baseline ==="
Write-Host "Models: $passed / $total passed"
Write-Host "Average compile time: $([math]::Round($avgMs, 0)) ms"
Write-Host "Results: $resultsFile"

if ($Compare) {
    $prevDirs = Get-ChildItem -Path (Join-Path $jitRoot "baseline") -Directory |
        Where-Object { $_.Name -like "*_perf" -and $_.Name -ne (Split-Path $dateStr -Leaf) } |
        Sort-Object Name -Descending |
        Select-Object -First 1

    if ($prevDirs) {
        $prevFile = Join-Path $prevDirs.FullName "jit_perf_baseline.json"
        if (Test-Path $prevFile) {
            $prev = Get-Content $prevFile | ConvertFrom-Json
            Write-Host ""
            Write-Host "=== Comparison vs $($prevDirs.Name) ==="
            foreach ($r in $results) {
                $p = $prev | Where-Object { $_.model -eq $r.model } | Select-Object -First 1
                if ($p -and $p.success -and $r.success) {
                    $delta = $r.compileMs - $p.compileMs
                    $pct = if ($p.compileMs -gt 0) { [math]::Round(100.0 * $delta / $p.compileMs, 1) } else { 0 }
                    $flag = if ($delta -gt $p.compileMs * 0.2) { " [REGRESSION]" } else { "" }
                    Write-Host "  $($r.model): $($p.compileMs)ms -> $($r.compileMs)ms ($([math]::Round($delta,0))ms, ${pct}%)$flag"
                }
            }
        }
    }
}
