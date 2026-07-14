param(
    [string]$Root = ".",
    [switch]$Interactive,
    [string]$Config = "crates/regress-harness/examples/smoke.json",
    [string]$DataRoot = "build/regression_data",
    [string]$OutDir = "",
    [int]$Workers = 0,
    [string]$Tier = "",
    [string]$Tags = "",
    [string]$Baseline = "",
    [string]$Incremental = "",
    [string]$Manifest = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = if ([System.IO.Path]::IsPathRooted($Root)) { $Root } else { (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot $Root)).Path }
$configPath = if ([System.IO.Path]::IsPathRooted($Config)) { $Config } else { Join-Path $repoRoot $Config }
$exe = Join-Path $repoRoot "target\release\regress-harness.exe"
if (-not (Test-Path -LiteralPath $exe)) {
    Push-Location $repoRoot
    try {
        cargo build -p regress-harness --release --manifest-path (Join-Path $repoRoot "Cargo.toml")
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    } finally {
        Pop-Location
    }
}

if ($Interactive) {
    Push-Location $repoRoot
    try {
        & $exe "interactive"
    } finally {
        Pop-Location
    }
    exit $LASTEXITCODE
}

$argsList = @(
    "run",
    "--config", $configPath,
    "--data-root", (Join-Path $repoRoot $DataRoot)
)
if ($OutDir -ne "") {
    $argsList += @("--out-dir", (Join-Path $repoRoot $OutDir))
}
if ($Workers -gt 0) {
    $argsList += @("--workers", "$Workers")
}
if ($Tier -ne "") {
    $argsList += @("--tier", $Tier)
}
if ($Tags -ne "") {
    $argsList += @("--tags", $Tags)
}
if ($Baseline -ne "") {
    $bp = if ([System.IO.Path]::IsPathRooted($Baseline)) { $Baseline } else { Join-Path $repoRoot $Baseline }
    $argsList += @("--baseline", $bp)
}
if ($Incremental -ne "") {
    $argsList += @("--incremental", $Incremental)
}
if ($Manifest -ne "") {
    $mp = if ([System.IO.Path]::IsPathRooted($Manifest)) { $Manifest } else { Join-Path $repoRoot $Manifest }
    $argsList += @("--manifest", $mp)
}
$argsList += @("--ndjson", "--summary-compat")

& $exe @argsList
exit $LASTEXITCODE
