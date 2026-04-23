# Bakes an MSL pre-pack into modai-ide/src-tauri/resources/msl-cache/<version>-<tree12>/ via msl_pack_bake.
# Requires MODELICA_STDLIB_ROOT (Modelica root containing Modelica/package.mo) or -MslRoot.
# Optional -HotJson: path to msl-hotness.json (merged leaf list during bake).

param(
    [string]$MslRoot = $env:MODELICA_STDLIB_ROOT,
    [string]$HotJson = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if ([string]::IsNullOrWhiteSpace($MslRoot)) {
    Write-Error "Set MODELICA_STDLIB_ROOT or pass -MslRoot <path-to-Modelica-library-root>"
    exit 1
}

$msl = (Resolve-Path $MslRoot).Path
$pkg = Join-Path $msl "Modelica\package.mo"
if (-not (Test-Path -LiteralPath $pkg)) {
    Write-Error "Invalid MSL root: missing Modelica/package.mo under $msl"
    exit 1
}

$outBase = Join-Path $RepoRoot "modai-ide\src-tauri\resources\msl-cache"
New-Item -ItemType Directory -Force -Path $outBase | Out-Null
$staging = Join-Path $outBase "_msl_pack_staging"
if (Test-Path -LiteralPath $staging) {
    Remove-Item -Recurse -Force -LiteralPath $staging
}

Push-Location $RepoRoot
try {
    if ($HotJson -and (Test-Path -LiteralPath $HotJson)) {
        $hotPath = (Resolve-Path $HotJson).Path
        rtk cargo run -p rustmodlica --release --bin msl_pack_bake -- --msl $msl --out $staging --hot $hotPath
    }
    else {
        rtk cargo run -p rustmodlica --release --bin msl_pack_bake -- --msl $msl --out $staging
    }
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
finally {
    Pop-Location
}

$manPath = Join-Path $staging "manifest.json"
if (-not (Test-Path -LiteralPath $manPath)) {
    Write-Error "Bake did not write manifest.json under staging"
    exit 1
}

$j = Get-Content -LiteralPath $manPath -Raw | ConvertFrom-Json
$ver = [regex]::Replace([string]$j.msl_version, "[\\/]+", "_")
$ver = [regex]::Replace($ver, "\s+", "_")
$td = [string]$j.tree_digest
if ($td.Length -lt 12) {
    Write-Error "manifest tree_digest too short"
    exit 1
}
$short = $td.Substring(0, 12)
$dirName = "$ver-$short"
$dest = Join-Path $outBase $dirName
if (Test-Path -LiteralPath $dest) {
    Remove-Item -Recurse -Force -LiteralPath $dest
}
Move-Item -LiteralPath $staging -Destination $dest
Write-Host "OK -> $dest"
