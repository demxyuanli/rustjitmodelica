# Single-file regression for three historically failing cases:
#   1) Modelica.Math.Random.Examples.GenerateRandomNumbers
#      -> expect simulation success (exit 0) after MSL xorshift JIT multi-assign.
#   2) ModelicaTest.RedeclareSmoke.ConstrainedByCoarseFalse
#   3) ModelicaTest.RedeclareSmoke.ConstrainedByIllegal
#      -> expect flatten/snapshot failure (exit != 0); negative constrainedby tests.
#
# Usage (from repo root or anywhere):
#   pwsh -File jit-compiler/scripts/run_three_failures_single.ps1
#
# Requires: target/release/rustmodlica.exe (cargo build -p rustmodlica --release)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $root "target\release\rustmodlica.exe"
$jit = Join-Path $root "jit-compiler"
$libPath = $jit

if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Build release first: cargo build -p rustmodlica --release"
    exit 1
}

Push-Location $jit
$oldEa = $ErrorActionPreference
$ErrorActionPreference = "Continue"

$failed = 0

Write-Host ""
Write-Host "=== 1/3 Modelica.Math.Random.Examples.GenerateRandomNumbers (expect exit 0) ==="
$csv = Join-Path $env:TEMP "rustmodlica_GenerateRandomNumbers.csv"
Remove-Item -LiteralPath $csv -ErrorAction SilentlyContinue
$out1 = & $exe `
    "--lib-path=$libPath" `
    "--t-end=2" `
    "--dt=0.05" `
    "--result-file=$csv" `
    "Modelica.Math.Random.Examples.GenerateRandomNumbers" 2>&1
$e1 = $LASTEXITCODE
$out1 | Write-Host
if ($e1 -ne 0) {
    Write-Host "FAIL: expected exit 0, got $e1"
    $failed++
} else {
    Write-Host "OK: exit 0"
}

Write-Host ""
Write-Host "=== 2/3 ModelicaTest.RedeclareSmoke.ConstrainedByCoarseFalse (expect exit != 0) ==="
$snap2 = Join-Path $env:TEMP "rustmodlica_neg_ConstrainedByCoarseFalse.json"
$out2 = & $exe `
    "--lib-path=$libPath" `
    "--flat-snapshot-only" `
    "--emit-flat-snapshot=$snap2" `
    "ModelicaTest.RedeclareSmoke.ConstrainedByCoarseFalse" 2>&1
$e2 = $LASTEXITCODE
$out2 | Write-Host
if ($e2 -eq 0) {
    Write-Host "FAIL: expected non-zero exit (invalid redeclare), got 0"
    $failed++
} else {
    Write-Host "OK: exit $e2 (expected failure)"
}

Write-Host ""
Write-Host "=== 3/3 ModelicaTest.RedeclareSmoke.ConstrainedByIllegal (expect exit != 0) ==="
$snap3 = Join-Path $env:TEMP "rustmodlica_neg_ConstrainedByIllegal.json"
$out3 = & $exe `
    "--lib-path=$libPath" `
    "--flat-snapshot-only" `
    "--emit-flat-snapshot=$snap3" `
    "ModelicaTest.RedeclareSmoke.ConstrainedByIllegal" 2>&1
$e3 = $LASTEXITCODE
$out3 | Write-Host
if ($e3 -eq 0) {
    Write-Host "FAIL: expected non-zero exit (invalid redeclare), got 0"
    $failed++
} else {
    Write-Host "OK: exit $e3 (expected failure)"
}

Pop-Location
$ErrorActionPreference = $oldEa

Write-Host ""
if ($failed -eq 0) {
    Write-Host "All three checks passed."
    exit 0
} else {
    Write-Host "Finished with $failed failing check(s)."
    exit 1
}
