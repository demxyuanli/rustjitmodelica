# Tier O (optional): run OpenModelica instantiateModel and capture stdout (errors + any printed flat).
# Requires omc on PATH. Adjust the generated .mos if your OMC version needs different API for flat export.
param(
    [Parameter(Mandatory = $true)][string]$Model,
    [Parameter(Mandatory = $true)][string]$Out
)
$ErrorActionPreference = "Stop"
if (-not (Get-Command omc -ErrorAction SilentlyContinue)) {
    Write-Error "omc not found on PATH."
    exit 3
}
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$jit = Join-Path $root "jit-compiler"
$mos = Join-Path $env:TEMP "rustmodlica_omc_inst.mos"
$modelDot = $Model.Replace("/", ".")
$relMo = ($Model -replace "\.", "/") + ".mo"
$mosText = @"
setModelicaPath(getModelicaPath() + ";$($jit.Replace('\','/'))");
loadFile("ModelicaTest/package.mo");
loadFile("$($relMo.Replace('\','/'))");
b := instantiateModel($modelDot);
s := getErrorString();
writeFile("$($Out.Replace('\','/'))", "instantiate ok: " + String(b) + "\n" + s);
"@
Set-Content -LiteralPath $mos -Value $mosText -Encoding utf8
Push-Location $jit
try {
    & omc $mos 2>&1 | Out-Null
} finally {
    Pop-Location
}
if (-not (Test-Path -LiteralPath $Out)) {
    Write-Error "OMC did not produce $Out"
    exit 4
}
Write-Host "Wrote $Out"
