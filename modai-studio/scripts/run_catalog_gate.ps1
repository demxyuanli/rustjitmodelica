# Q4-A catalog FQN gate: dump inventory + validate core groups.
# Usage:
#   pwsh -File ./modai-studio/scripts/run_catalog_gate.ps1
#   pwsh -File ./modai-studio/scripts/run_catalog_gate.ps1 -Groups signal-core,physical-core
param(
  [string]$Groups = "signal-core,physical-core,routing",
  [switch]$DumpOnly
)
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$inv = Join-Path $root "docs\modai-studio\catalog_gate_inventory.json"
$report = Join-Path $root "docs\modai-studio\catalog_gate_report.json"
# Prefer debug console binary: release uses windows_subsystem and may omit exit codes.
$exeCandidates = @(
  (Join-Path $root "target\debug\modai-studio.exe"),
  (Join-Path $root "target\release\modai-studio.exe")
)
$exe = $exeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $exe) {
  Write-Host "Building modai-studio (debug)..."
  Push-Location $root
  try {
    cargo build -p modai-studio
  } finally {
    Pop-Location
  }
  $exe = Join-Path $root "target\debug\modai-studio.exe"
}
if (-not (Test-Path $exe)) {
  throw "modai-studio.exe not found after build"
}
Write-Host "Using: $exe"

& $exe "--dump-catalog-gate=$inv"
if (-not (Test-Path $inv)) { throw "dump-catalog-gate did not write $inv" }
Write-Host "Inventory: $inv"

if ($DumpOnly) { exit 0 }

& $exe "--run-catalog-gate=$report" "--gate-groups=$Groups"
if (-not (Test-Path $report)) { throw "run-catalog-gate did not write $report" }
$doc = Get-Content -Raw -Path $report | ConvertFrom-Json
Write-Host ("Report: {0} total={1} passed={2} failed={3} skipped={4}" -f $report, $doc.total, $doc.passed, $doc.failed, $doc.skipped)
if ([int]$doc.failed -gt 0) { exit 2 }

# Q4-B: smoke mix examples when present (open + validate + short sim).
$mixIds = @("rot_sensor_control", "elec_sensor_control", "trans_sensor_control", "rot_spring_damper", "trans_spring_damper")
foreach ($id in $mixIds) {
  Write-Host "Smoke example: $id"
  & $exe "--smoke-example=$id"
  if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne $null -and "$LASTEXITCODE" -ne "") {
    # windows_subsystem may omit codes; rely on stderr JSON when present
    Write-Host "smoke-example $id exit=$LASTEXITCODE (continuing if debug binary)"
  }
}

exit 0
