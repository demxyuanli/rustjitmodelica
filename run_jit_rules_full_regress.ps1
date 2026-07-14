# JIT policy / named-rules regression bundle: unit tests + TestLib batch validate + MOS script suite.
# Does not run the full run_regression.ps1 simulation matrix (use that separately for end-to-end).
#
# Usage (repository root):
#   powershell -NoProfile -ExecutionPolicy Bypass -File .\run_jit_rules_full_regress.ps1

$ErrorActionPreference = "Stop"
$repoRoot = $PSScriptRoot
Set-Location $repoRoot

Write-Host "=== [1/3] cargo test -p rustmodlica ==="
& cargo test -p rustmodlica -- --nocapture
if ($LASTEXITCODE -ne 0) {
    Write-Error "cargo test failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "=== [2/3] TestLib batch JIT --validate (named rules) ==="
$testlibScript = Join-Path $repoRoot "jit-compiler\scripts\run_testlib_validate.ps1"
& powershell -NoProfile -ExecutionPolicy Bypass -File $testlibScript
if ($LASTEXITCODE -ne 0) {
    Write-Error "TestLib validate batch failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "=== [3/3] MOS regression (14 scripts) ==="
$mosScript = Join-Path $repoRoot "jit-compiler\scripts\run_mos_regression.ps1"
Push-Location (Join-Path $repoRoot "jit-compiler")
try {
    & powershell -NoProfile -ExecutionPolicy Bypass -File $mosScript
    $mosExit = $LASTEXITCODE
}
finally {
    Pop-Location
}
if ($mosExit -ne 0) {
    Write-Error "MOS regression failed with exit code $mosExit"
    exit $mosExit
}

Write-Host ""
Write-Host "=== JIT rules full regress: all steps passed ==="
exit 0
