param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [Parameter(Mandatory = $true)][string]$CargoTargetDir,
    [Parameter(Mandatory = $true)][string]$Model,
    [double]$OutputInterval = 0.001,
    [string]$ArtifactsDir = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($ArtifactsDir)) {
    $ArtifactsDir = Join-Path $RepoRoot "build/regression_data_jit_phase1/artifacts"
}
New-Item -ItemType Directory -Path $ArtifactsDir -Force | Out-Null

$safeName = $Model.Replace("/", "_").Replace(".", "_")
$csvA = Join-Path $ArtifactsDir ("clocked_{0}_a.csv" -f $safeName)
$csvB = Join-Path $ArtifactsDir ("clocked_{0}_b.csv" -f $safeName)

$jitDir = Join-Path $RepoRoot "jit-compiler"
if (-not (Test-Path -LiteralPath $jitDir)) { throw ("missing dir: " + $jitDir) }

& cargo --target-dir $CargoTargetDir run -- `
    --solver=rk4 `
    --output-interval=$OutputInterval `
    --result-file=$csvA `
    $Model 2>&1 | Out-String | Out-Null
$e1 = $LASTEXITCODE

& cargo --target-dir $CargoTargetDir run -- `
    --solver=rk4 `
    --output-interval=$OutputInterval `
    --result-file=$csvB `
    $Model 2>&1 | Out-String | Out-Null
$e2 = $LASTEXITCODE

$same = $false
if ($e1 -eq 0 -and $e2 -eq 0 -and (Test-Path -LiteralPath $csvA) -and (Test-Path -LiteralPath $csvB)) {
    $h1 = (Get-FileHash -Algorithm SHA256 -LiteralPath $csvA).Hash
    $h2 = (Get-FileHash -Algorithm SHA256 -LiteralPath $csvB).Hash
    $same = ($h1 -eq $h2)
}

Write-Host ("[sync-det] model={0} ok={1} exit_a={2} exit_b={3} csv_a={4} csv_b={5}" -f $Model, $same, $e1, $e2, $csvA, $csvB)

if ($same) { exit 0 } else { exit 1 }

