param()

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDir = Resolve-Path (Join-Path $scriptDir "..")
$repoDir = Resolve-Path (Join-Path $projectDir "..")

$mosMatrix = Join-Path $scriptDir "mos_signal_coverage_matrix.txt"
$semanticMatrix = Join-Path $scriptDir "semantic_coverage_matrix.md"
$updateMd = Join-Path $repoDir "update.md"
$statusJson = Join-Path $scriptDir "coverage_status.json"

if (-not (Test-Path $mosMatrix)) { throw "missing file: $mosMatrix" }
if (-not (Test-Path $updateMd)) { throw "missing file: $updateMd" }

$lines = Get-Content -Path $mosMatrix
$cases = @()
foreach ($line in $lines) {
    $trimmed = $line.Trim()
    if ($trimmed -like "*.mos,*") {
        $parts = $trimmed.Split(",")
        if ($parts.Length -ge 3) {
            $cases += [PSCustomObject]@{
                Case = $parts[0]
                Status = $parts[2].Trim()
            }
        }
    }
}

$total = $cases.Count
$passed = ($cases | Where-Object { $_.Status -eq "pass" }).Count
$semanticCurrent = if ($total -gt 0) { [math]::Round(100.0 * $passed / $total, 2) } else { 0.0 }
$semanticTarget = 98.0
$modelicaTarget = 100.0

$modelicaCurrent = 0.0
$um = Get-Content -Path $updateMd
foreach ($line in $um) {
    if ($line -like "*Modelica 3.4核心*") {
        if ($line -match "([0-9]+(?:\\.[0-9]+)?)%") {
            $modelicaCurrent = [double]$matches[1]
        }
        break
    }
}

$gaps = New-Object System.Collections.Generic.List[string]
if ($semanticCurrent -lt $semanticTarget) {
    $gaps.Add("semantic coverage below 98%")
}
if ($modelicaCurrent -lt $modelicaTarget) {
    $gaps.Add("Modelica 3.4 core below 100%")
}

$payload = [PSCustomObject]@{
    semantic_target_percent = $semanticTarget
    semantic_current_percent = $semanticCurrent
    modelica34_target_percent = $modelicaTarget
    modelica34_current_percent = $modelicaCurrent
    gaps = $gaps
}

$json = $payload | ConvertTo-Json -Depth 4
Set-Content -Path $statusJson -Value $json -NoNewline

Write-Host ("[coverage-status] semantic={0}% modelica34={1}% gaps={2}" -f $semanticCurrent, $modelicaCurrent, $gaps.Count)
if (Test-Path $semanticMatrix) {
    Write-Host "[coverage-status] source matrix: scripts/semantic_coverage_matrix.md"
}
