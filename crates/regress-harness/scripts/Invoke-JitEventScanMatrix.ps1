param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [string]$OutDir = "build_stability/event_scan_matrix_ci",
    [string]$LibPath = "",
    [string]$ScriptPath = "",
    [int]$AllowUnsupported = 1
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($LibPath)) {
    $LibPath = Join-Path $RepoRoot "jit-compiler"
}
if ([string]::IsNullOrWhiteSpace($ScriptPath)) {
    $ScriptPath = Join-Path $RepoRoot "jit-compiler/scripts/run_event_scan_matrix.ps1"
}
if (-not (Test-Path -LiteralPath $ScriptPath)) {
    Write-Host ("[event-scan] missing script: " + $ScriptPath)
    exit 2
}
if (-not (Test-Path -LiteralPath $LibPath)) {
    Write-Host ("[event-scan] missing lib path: " + $LibPath)
    exit 3
}

& powershell -NoProfile -ExecutionPolicy Bypass -File $ScriptPath `
    -Root $RepoRoot `
    -OutDir $OutDir `
    -LibPaths @($LibPath) 2>&1 | Out-String | Out-Null
$eventExit = $LASTEXITCODE

$eventReport = Join-Path $RepoRoot (Join-Path $OutDir "consistency_report.txt")
$eventCsv = Join-Path $RepoRoot (Join-Path $OutDir "deadband_matrix_stability.csv")
$eventUnsupported = Join-Path $RepoRoot (Join-Path $OutDir "unsupported_models.txt")

$eventNondet = 0
$eventConfigErr = 0
$eventUnsupportedCount = 0

if (Test-Path -LiteralPath $eventReport) {
    $reportLines = Get-Content -LiteralPath $eventReport
    foreach ($line in $reportLines) {
        if ($line -match '^nondeterministic=(\d+)$') { $eventNondet = [int]$Matches[1] }
        if ($line -match '^config_error=(\d+)$') { $eventConfigErr = [int]$Matches[1] }
        if ($line -match '^unsupported=(\d+)$') { $eventUnsupportedCount = [int]$Matches[1] }
    }
} elseif (Test-Path -LiteralPath $eventCsv) {
    $rows = Import-Csv -LiteralPath $eventCsv
    $eventNondet = @($rows | Where-Object { $_.status -eq "nondeterministic" }).Count
    $eventConfigErr = @($rows | Where-Object { $_.status -eq "config_error" -or $_.status -eq "error" }).Count
    $eventUnsupportedCount = @($rows | Where-Object { $_.status -eq "unsupported" }).Count
} else {
    $eventConfigErr = 1
}

$ok = ($eventNondet -eq 0) -and ($eventConfigErr -eq 0)
if (-not $ok -and $AllowUnsupported -eq 0 -and $eventUnsupportedCount -gt 0) {
    $ok = $false
}

Write-Host ("[event-scan] ok={0} exit={1} nondeterministic={2} config_error={3} unsupported={4} report={5} csv={6} unsupported_file={7}" -f `
    $ok, $eventExit, $eventNondet, $eventConfigErr, $eventUnsupportedCount, $eventReport, $eventCsv, $eventUnsupported)

if ($ok) { exit 0 } else { exit 1 }

