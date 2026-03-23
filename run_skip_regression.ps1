param(
    [string]$Root = "",
    [string]$SummaryPath = "",
    [string]$OutDir = "build_modelica_skip_regress"
)
$repoRoot = if ($Root -ne "") { $Root } else { $PSScriptRoot }
$summary = if ($SummaryPath -ne "") { $SummaryPath } else { Join-Path $repoRoot "build_modelica_dir_regress\summary.txt" }
if (-not [System.IO.Path]::IsPathRooted($summary)) {
    $summary = Join-Path $repoRoot $summary
}
if (-not (Test-Path -LiteralPath $summary)) {
    Write-Error "Summary not found: $summary (run a directory regression first, or pass -SummaryPath)."
    exit 2
}
$script = Join-Path $repoRoot "run_modelica_dir_regression.ps1"
& powershell -NoProfile -ExecutionPolicy Bypass -File $script `
    -Root $repoRoot `
    -OnlySkipsFromSummary $summary `
    -OutDir $OutDir `
    -NewtonCountsAsFailed
exit $LASTEXITCODE
