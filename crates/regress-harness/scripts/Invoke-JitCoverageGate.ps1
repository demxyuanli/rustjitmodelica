param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [string]$JitRoot = "",
    [string]$GeneratorScript = "",
    [string]$StatusJson = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($JitRoot)) {
    $JitRoot = Join-Path $RepoRoot "jit-compiler"
}
if ([string]::IsNullOrWhiteSpace($GeneratorScript)) {
    $GeneratorScript = Join-Path $JitRoot "scripts/generate_coverage_status.ps1"
}
if ([string]::IsNullOrWhiteSpace($StatusJson)) {
    $StatusJson = Join-Path $JitRoot "scripts/coverage_status.json"
}

if (-not (Test-Path -LiteralPath $GeneratorScript)) {
    Write-Host ("[coverage-gate] missing generator: " + $GeneratorScript)
    exit 2
}

& powershell -NoProfile -ExecutionPolicy Bypass -File $GeneratorScript 2>&1 | Out-String | Out-Null
$genExit = $LASTEXITCODE

$ok = $false
$detail = "coverage_status_missing"
if ($genExit -eq 0 -and (Test-Path -LiteralPath $StatusJson)) {
    try {
        $coverage = Get-Content -LiteralPath $StatusJson -Raw | ConvertFrom-Json
        $semanticTarget = [double]$coverage.semantic_target_percent
        $semanticCurrent = [double]$coverage.semantic_current_percent
        $modelicaTarget = [double]$coverage.modelica34_target_percent
        $modelicaCurrent = [double]$coverage.modelica34_current_percent
        $gaps = @($coverage.gaps)
        $ok = ($semanticCurrent -ge $semanticTarget) -and ($modelicaCurrent -ge $modelicaTarget) -and ($gaps.Count -eq 0)
        $detail = ("semantic={0}/{1};modelica34={2}/{3};gaps={4}" -f `
            $semanticCurrent, $semanticTarget, $modelicaCurrent, $modelicaTarget, ($gaps -join "|"))
    } catch {
        $ok = $false
        $detail = ("coverage_status_parse_failed;" + $_.Exception.Message)
    }
} else {
    $ok = $false
    $detail = ("coverage_generator_failed_exit=" + $genExit)
}

Write-Host ("[coverage-gate] ok={0} detail={1} status_json={2}" -f $ok, $detail, $StatusJson)
if ($ok) { exit 0 } else { exit 1 }

