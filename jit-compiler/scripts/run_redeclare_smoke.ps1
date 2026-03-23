# Runs rustmodlica on ModelicaTest.RedeclareSmoke examples (flatten + JIT smoke).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $root "target\release\rustmodlica.exe"
if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Build release first: cargo build --release --manifest-path $root\jit-compiler\Cargo.toml"
    exit 1
}
$jit = Join-Path $root "jit-compiler"
$models = @(
    "ModelicaTest.RedeclareSmoke.RedeclareExtendsExample",
    "ModelicaTest.RedeclareSmoke.ConstrainedUsage",
    "ModelicaTest.RedeclareSmoke.RedeclareViaExtendsSmoke",
    "ModelicaTest.RedeclareSmoke.InnerOuterModifierSmoke",
    "ModelicaTest.RedeclareSmoke.PublicModifierSmoke",
    "ModelicaTest.RedeclareSmoke.EachModifierSmoke"
)
Push-Location $jit
foreach ($m in $models) {
    Write-Host "=== $m ==="
    & $exe --index-reduction-method=dummyDerivative --solver=rk4 --dt=0.01 --t-end=1 --result-file="$env:TEMP\redeclare_smoke.csv" $m
    if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
}
Pop-Location
Write-Host "RedeclareSmoke OK"
