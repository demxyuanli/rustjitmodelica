# Modai Studio: open + visualize + validate/simulate bundled examples.
#
# Usage:
#   pwsh -File ./test_studio_autoload.ps1
#   pwsh -File ./test_studio_autoload.ps1 -NoGui
#   pwsh -File ./test_studio_autoload.ps1 -All
#   pwsh -File ./test_studio_autoload.ps1 -Example rc_circuit

param(
    [switch]$NoGui,
    [switch]$All,
    [ValidateSet("signal_feedback", "rc_circuit", "multibody_preview")]
    [string]$Example = "signal_feedback"
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $MyInvocation.MyCommand.Path
$exampleDir = Join-Path $repo "modai-studio\examples\$Example"
if (-not $All -and -not (Test-Path (Join-Path $exampleDir "studio-project.json"))) {
    throw "Example project missing: $exampleDir"
}

Write-Host "== Studio example open / visualize / test =="
if ($All) {
    Write-Host "Mode: all bundled examples"
} else {
    Write-Host "Example: $exampleDir"
}

Write-Host "`n[1/3] cargo test bundled_examples + validate"
Push-Location $repo
try {
    cargo test -p modai-studio bundled_examples_open_with_diagram -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw "diagram unit test failed exit=$LASTEXITCODE" }
    cargo test -p modai-studio signal_feedback_validates -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw "signal_feedback validate failed exit=$LASTEXITCODE" }
    cargo test -p modai-studio rc_circuit_validates -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw "rc_circuit validate failed exit=$LASTEXITCODE" }
} finally {
    Pop-Location
}

Write-Host "`n[2/3] headless smoke (open + diagram + validate + simulate)"
$smokeLog = Join-Path $repo "target\studio_smoke_open.log"
Remove-Item $smokeLog -ErrorAction SilentlyContinue
Push-Location $repo
try {
    if ($All) {
        cargo run -p modai-studio -- --smoke-all *> $smokeLog
    } else {
        cargo run -p modai-studio -- --smoke-example="$Example" *> $smokeLog
    }
    $code = $LASTEXITCODE
} finally {
    Pop-Location
}
$smokeText = if (Test-Path $smokeLog) { Get-Content $smokeLog -Raw } else { "" }
Write-Host $smokeText
if ($code -ne 0) { throw "smoke failed exit=$code" }
if ($smokeText -notmatch '"ok"\s*:\s*true') {
    throw "smoke: expected ok:true in output"
}
if (-not $All -and $smokeText -notmatch '"validated"\s*:\s*true') {
    throw "smoke: expected validated:true"
}
if (-not $All -and $smokeText -notmatch '"simulated"\s*:\s*true') {
    throw "smoke: expected simulated:true"
}

if ($NoGui) {
    Write-Host "`n[3/3] skip GUI (-NoGui)"
    Write-Host "PASS"
    exit 0
}

Write-Host "`n[3/3] launch Studio GUI with MODAI_STUDIO_OPEN"
$env:MODAI_STUDIO_OPEN = $exampleDir
Push-Location (Join-Path $repo "modai-studio")
try {
    npm run tauri -- dev
} finally {
    Pop-Location
}
