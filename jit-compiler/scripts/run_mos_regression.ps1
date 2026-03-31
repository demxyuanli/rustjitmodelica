param(
    [string]$CargoCmd = "cargo"
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDir = Resolve-Path (Join-Path $scriptDir "..")

$mosCases = @(
    "scripts/omc_regression_named_simulate.mos",
    "scripts/omc_regression_if_for.mos",
    "scripts/omc_regression_for_range.mos",
    "scripts/omc_regression_simulate_named_combo.mos",
    "scripts/omc_regression_elseif_nested_for.mos",
    "scripts/omc_regression_reverse_range.mos",
    "scripts/omc_regression_mixed_simulate_args.mos",
    "scripts/omc_regression_sync_signal.mos",
    "scripts/omc_regression_sync_super_shift.mos",
    "scripts/omc_regression_newton_symbolic_dense.mos",
    "scripts/omc_regression_newton_symbolic_sparse.mos",
    "scripts/omc_regression_stream_semantics.mos",
    "scripts/omc_regression_algorithm_elsewhen.mos",
    "scripts/omc_regression_direction_switch_stream.mos"
)

Push-Location $projectDir
try {
    & $CargoCmd check
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    foreach ($case in $mosCases) {
        Write-Host "[mos-regression] running $case"
        & $CargoCmd run -- --script="$case"
        if ($LASTEXITCODE -ne 0) {
            Write-Error "[mos-regression] FAILED: $case"
            exit $LASTEXITCODE
        }
    }

    & powershell -ExecutionPolicy Bypass -File "scripts/generate_coverage_status.ps1"
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $coveragePath = Join-Path $projectDir "scripts/coverage_status.json"
    if (-not (Test-Path -LiteralPath $coveragePath)) {
        Write-Error "[mos-regression] coverage status json missing: $coveragePath"
        exit 2
    }
    $coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
    $semanticTarget = [double]$coverage.semantic_target_percent
    $semanticCurrent = [double]$coverage.semantic_current_percent
    $modelicaTarget = [double]$coverage.modelica34_target_percent
    $modelicaCurrent = [double]$coverage.modelica34_current_percent
    $gaps = @($coverage.gaps)
    if (($semanticCurrent -lt $semanticTarget) -or ($modelicaCurrent -lt $modelicaTarget) -or ($gaps.Count -gt 0)) {
        Write-Error ("[mos-regression] coverage gate failed: semantic={0}/{1}, modelica34={2}/{3}, gaps={4}" -f `
            $semanticCurrent, $semanticTarget, $modelicaCurrent, $modelicaTarget, ($gaps -join "; "))
        exit 3
    }

    Write-Host "[mos-regression] all cases passed: $($mosCases.Count)"
}
finally {
    Pop-Location
}
