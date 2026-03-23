# Expect flatten to fail (exit != 0) for invalid replaceable redeclares.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $root "target\release\rustmodlica.exe"
$jit = Join-Path $root "jit-compiler"
if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Build release first."
    exit 1
}
$models = @(
    "ModelicaTest.RedeclareSmoke.ConstrainedByCoarseFalse",
    "ModelicaTest.RedeclareSmoke.ConstrainedByIllegal"
)
Push-Location $jit
$oldEa = $ErrorActionPreference
$ErrorActionPreference = "Continue"
foreach ($m in $models) {
    Write-Host "=== expect fail: $m ==="
    & $exe --flat-snapshot-only --emit-flat-snapshot=$env:TEMP\neg.json $m 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Error "Expected failure for $m"
        Pop-Location
        exit 2
    }
}
Pop-Location
$ErrorActionPreference = $oldEa
Write-Host "Constrainedby negative checks OK"
