param()

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$mosMatrix = Join-Path $scriptDir "mos_signal_coverage_matrix.txt"
$coreMatrix = Join-Path $scriptDir "modelica34_core_coverage_matrix.txt"
$semanticMatrix = Join-Path $scriptDir "semantic_coverage_matrix.md"
$statusJson = Join-Path $scriptDir "coverage_status.json"

if (-not (Test-Path $mosMatrix)) { throw "missing file: $mosMatrix" }
if (-not (Test-Path $coreMatrix)) { throw "missing file: $coreMatrix" }

function Get-TargetPercent([string]$path, [string]$key, [double]$defaultValue) {
    $lines = Get-Content -Path $path
    foreach ($line in $lines) {
        $trimmed = $line.Trim()
        if ($trimmed -match "^{0},([0-9]+(?:\.[0-9]+)?)$" -f [regex]::Escape($key)) {
            return [double]$Matches[1]
        }
    }
    return $defaultValue
}

function Get-CoverageFromCsvMatrix([string]$path, [string]$idPrefix) {
    $lines = Get-Content -Path $path
    $headerCols = $null
    $statusIdx = -1
    $total = 0
    $passed = 0
    $active = $false

    foreach ($line in $lines) {
        $trimmed = $line.Trim()
        if ($trimmed -eq "" -or $trimmed.StartsWith("#")) { continue }
        if ($trimmed -match "^[^,]+,.*") {
            $cols = @($trimmed.Split(",") | ForEach-Object { $_.Trim() })
            if (-not $active -and ($cols -contains "status")) {
                $headerCols = $cols
                $statusIdx = [Array]::IndexOf($headerCols, "status")
                if ($statusIdx -lt 0) { throw "matrix '$path' has header but no status column" }
                $active = $true
                continue
            }
            if (-not $active) { continue }
            if ($statusIdx -ge $cols.Count) { continue }
            $status = $cols[$statusIdx].ToLowerInvariant()
            if ($status -in @("pass", "fail", "pending")) {
                $total++
                if ($status -eq "pass") { $passed++ }
            }
        }
    }
    if ($total -eq 0) {
        throw "matrix '$path' has no coverage rows with status values"
    }
    $current = [math]::Round(100.0 * $passed / $total, 2)
    return [PSCustomObject]@{
        id = $idPrefix
        total = $total
        passed = $passed
        current = $current
    }
}

$semanticTarget = Get-TargetPercent -path $mosMatrix -key "target_semantic_coverage_percent" -defaultValue 98.0
$modelicaTarget = Get-TargetPercent -path $coreMatrix -key "target_modelica34_percent" -defaultValue 100.0

$semantic = Get-CoverageFromCsvMatrix -path $mosMatrix -idPrefix "semantic"
$modelica = Get-CoverageFromCsvMatrix -path $coreMatrix -idPrefix "modelica34"

$gaps = New-Object System.Collections.Generic.List[string]
if ($semantic.current -lt $semanticTarget) {
    $gaps.Add("semantic coverage below 98%")
}
if ($modelica.current -lt $modelicaTarget) {
    $gaps.Add("Modelica 3.4 core below 100%")
}

$payload = [PSCustomObject]@{
    semantic_target_percent = $semanticTarget
    semantic_current_percent = $semantic.current
    semantic_passed_items = $semantic.passed
    semantic_total_items = $semantic.total
    modelica34_target_percent = $modelicaTarget
    modelica34_current_percent = $modelica.current
    modelica34_passed_items = $modelica.passed
    modelica34_total_items = $modelica.total
    gaps = $gaps
}

$json = $payload | ConvertTo-Json -Depth 4
Set-Content -Path $statusJson -Value $json -NoNewline

Write-Host ("[coverage-status] semantic={0}% ({1}/{2}) modelica34={3}% ({4}/{5}) gaps={6}" -f `
    $semantic.current, $semantic.passed, $semantic.total, `
    $modelica.current, $modelica.passed, $modelica.total, $gaps.Count)
if (Test-Path $semanticMatrix) {
    Write-Host "[coverage-status] source matrix: scripts/semantic_coverage_matrix.md"
}
Write-Host "[coverage-status] source matrix: scripts/modelica34_core_coverage_matrix.txt"
