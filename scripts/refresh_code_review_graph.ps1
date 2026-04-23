param(
    [string]$Repo = "",
    [ValidateSet("build", "update")]
    [string]$Mode = "update",
    [switch]$SkipFlows,
    [switch]$SkipPostprocess
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($Repo)) {
    $Repo = Split-Path -Parent $here
}

$pyArgs = @("-m", "code_review_graph", $Mode, "--repo", $Repo)
if ($SkipFlows) {
    $pyArgs += "--skip-flows"
}
if ($SkipPostprocess) {
    $pyArgs += "--skip-postprocess"
}

Write-Host ("[code-review-graph] python " + ($pyArgs -join " "))
& python @pyArgs
if ($LASTEXITCODE -ne 0) {
    Write-Error ("code_review_graph " + $Mode + " failed with exit " + $LASTEXITCODE)
    exit $LASTEXITCODE
}

Write-Host "[code-review-graph] status:"
& python -m code_review_graph status --repo $Repo
exit 0
