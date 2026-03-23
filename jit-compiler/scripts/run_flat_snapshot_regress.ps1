# Tier S: emit flat snapshots and diff against jit-compiler/golden_flat_snapshots/*.json
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $root "target\release\rustmodlica.exe"
$goldenDir = Join-Path $PSScriptRoot "..\golden_flat_snapshots"
$jit = Join-Path $root "jit-compiler"
if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Build release first: cargo build --release --manifest-path $root\jit-compiler\Cargo.toml"
    exit 1
}
$models = @(
    "ModelicaTest.RedeclareSmoke.RedeclareExtendsExample",
    "ModelicaTest.RedeclareSmoke.ConstrainedUsage",
    "ModelicaTest.RedeclareSmoke.RedeclareViaExtendsSmoke",
    "ModelicaTest.RedeclareSmoke.InnerOuterModifierSmoke",
    "ModelicaTest.RedeclareSmoke.PublicModifierSmoke",
    "ModelicaTest.RedeclareSmoke.EachModifierSmoke"
)
$fail = 0
Push-Location $jit
foreach ($m in $models) {
    $safe = $m.Replace(".", "_") + ".json"
    $golden = Join-Path $goldenDir $safe
    $tmp = Join-Path $env:TEMP "flat_snap_$safe"
    Write-Host "=== $m ==="
    & $exe --flat-snapshot-only --emit-flat-snapshot=$tmp $m
    if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
    if (-not (Test-Path -LiteralPath $golden)) {
        Write-Error "Missing golden: $golden"
        $fail = 1
        continue
    }
    & powershell -NoProfile -File (Join-Path $PSScriptRoot "diff_flat_snapshots.ps1") -A $golden -B $tmp
    if ($LASTEXITCODE -ne 0) { $fail = 1 }
}
Pop-Location
if ($fail -ne 0) { exit 1 }
Write-Host "Tier S flat snapshot regress OK"
