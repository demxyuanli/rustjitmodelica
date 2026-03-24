param(
    [string]$CargoCmd = "cargo"
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDir = Resolve-Path (Join-Path $scriptDir "..")
$denseCase = "scripts/omc_regression_newton_symbolic_dense.mos"
$sparseCase = "scripts/omc_regression_newton_symbolic_sparse.mos"

function Invoke-Case([string]$casePath, [string]$modeName, [string]$symbolicFlag) {
    $old = $env:RUSTMODLICA_NEWTON_SYMBOLIC_JACOBIAN
    try {
        $env:RUSTMODLICA_NEWTON_SYMBOLIC_JACOBIAN = $symbolicFlag
        $elapsed = Measure-Command {
            & $CargoCmd run -- --script="$casePath"
            if ($LASTEXITCODE -ne 0) { throw "case failed: $casePath ($modeName)" }
        }
        return [PSCustomObject]@{
            Case = $casePath
            Mode = $modeName
            Symbolic = $symbolicFlag
            Milliseconds = [math]::Round($elapsed.TotalMilliseconds, 2)
        }
    }
    finally {
        $env:RUSTMODLICA_NEWTON_SYMBOLIC_JACOBIAN = $old
    }
}

Push-Location $projectDir
try {
    & $CargoCmd check
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $results = @()
    $results += Invoke-Case -casePath $denseCase -modeName "symbolic_on" -symbolicFlag "1"
    $results += Invoke-Case -casePath $denseCase -modeName "symbolic_off" -symbolicFlag "0"
    $results += Invoke-Case -casePath $sparseCase -modeName "symbolic_on" -symbolicFlag "1"
    $results += Invoke-Case -casePath $sparseCase -modeName "symbolic_off" -symbolicFlag "0"

    $results | Format-Table -AutoSize | Out-String | Write-Host
}
finally {
    Pop-Location
}
