# SYNC-*: Run clocked/sample models with rustmodlica; writes build_sync_omc_compare.json summary.
# For OMC last-row compare, run OpenModelica separately and pass -OmcOut to compare_omc.ps1 per model.
param(
    [double]$TEnd = 1.2,
    [double]$Dt = 0.05
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$compare = Join-Path $here "compare_omc.ps1"
$models = @(
    "TestLib/ClockedTwoRates",
    "ModelicaTest.JitStress.SyncOmCompare"
)
$splat = @{
    Models      = $models
    TEnd        = $TEnd
    Dt          = $Dt
    JsonSummary = (Join-Path $here "build_sync_omc_compare.json")
}
& $compare @splat
exit $LASTEXITCODE
