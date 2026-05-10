# Solver accuracy smoke test: compares Radau, QSS, and RK4 on a simple ODE.
# Usage: pwsh -File jit-compiler/scripts/test_solvers.ps1
param(
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"
$jitRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $jitRoot "target\release\rustmodlica.exe"
if (-not (Test-Path $exe)) {
    Write-Host "Building release..."
    Push-Location $jitRoot
    cargo build -p rustmodlica --release
    Pop-Location
}
if (-not (Test-Path $exe)) { throw "rustmodlica.exe not found" }

$testLib = Join-Path $jitRoot "jit-compiler\TestLib"
$model = "qss_test"
$tEnd = "1.0"
$dt = "0.01"

$solvers = @("rk4", "radau", "qss")
$results = @{}

foreach ($solver in $solvers) {
    Write-Host "Testing --solver=$solver ..."
    $tmpFile = Join-Path $env:TEMP "rustmodlica_solver_test_${solver}.csv"
    $args = @(
        "--lib-path=$testLib",
        "--solver=$solver",
        "--t-end=$tEnd",
        "--dt=$dt",
        "--result-file=$tmpFile",
        "--warnings=none",
        $model
    )
    $out = & $exe @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  FAIL: solver $solver exited with code $LASTEXITCODE"
        continue
    }
    if (Test-Path $tmpFile) {
        $csv = Import-Csv $tmpFile
        $results[$solver] = $csv
        $lastRow = $csv[-1]
        Write-Host "  OK: $($csv.Count) rows, final x=$($lastRow.x), y=$($lastRow.y)"
        Remove-Item $tmpFile -Force
    } else {
        Write-Host "  FAIL: no CSV output"
    }
}

# Cross-validate: check that Radau and QSS agree with RK4 within tolerance
if ($results.Count -ge 2) {
    $ref = $results["rk4"]
    if ($ref) {
        Write-Host ""
        Write-Host "=== Cross-validation vs RK4 ==="
        foreach ($s in @("radau", "qss")) {
            $other = $results[$s]
            if (-not $other -or $other.Count -eq 0) { continue }
            $maxErrX = 0.0
            $maxErrY = 0.0
            $n = [Math]::Min($ref.Count, $other.Count)
            for ($i = 0; $i -lt $n; $i++) {
                $dx = [Math]::Abs([double]$ref[$i].x - [double]$other[$i].x)
                $dy = [Math]::Abs([double]$ref[$i].y - [double]$other[$i].y)
                if ($dx -gt $maxErrX) { $maxErrX = $dx }
                if ($dy -gt $maxErrY) { $maxErrY = $dy }
            }
            $pass = ($maxErrX -lt 0.1 -and $maxErrY -lt 0.1)
            $status = if ($pass) { "PASS" } else { "FAIL" }
            Write-Host "  $s vs rk4: ${status} max|x|=$([Math]::Round($maxErrX,6)) max|y|=$([Math]::Round($maxErrY,6))"
        }
    }
}

Write-Host ""
Write-Host "Solver smoke test complete."
