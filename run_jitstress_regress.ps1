# Runs only models under jit-compiler/ModelicaTest/**/JitStress/*.mo (ComplexJitRegression, MslBroadCoverage, RobotElectricalControl, ...).
# Matches the scope used for build_modelica_dir_regress_jitstress goldens.
param(
    [string]$Root = ".",
    [string]$ExePath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$regress = Join-Path $here "run_modelica_dir_regression.ps1"
if (-not (Test-Path -LiteralPath $regress)) {
    Write-Error "Missing run_modelica_dir_regression.ps1 next to this script."
    exit 2
}

$splat = @{
    Root           = $Root
    OutDir         = "build_modelica_dir_regress_jitstress"
    IncludePattern = "JitStress"
    TEnd           = 10.0
    Dt             = 0.01
    Solver         = "rk4"
}
if ($ExePath -ne "") {
    $splat["ExePath"] = $ExePath
}

& $regress @splat
exit $LASTEXITCODE
