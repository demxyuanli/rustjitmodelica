# Two sequential runs of one model with COFF / exec-buffer tracing on stderr.
# Use after a warm-cache AV to compare jit-disk-object vs aot-native load lines.
#
# Example:
#   rtk powershell -NoProfile -ExecutionPolicy Bypass -File scripts/repro_coff_reloc_trace.ps1 TestLib/AlgTest
#
param(
    [string]$Model = "TestLib/AlgTest"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $repoRoot

$jitRoot = Join-Path $repoRoot "jit-compiler"
$testLibRoot = Join-Path $jitRoot "TestLib"
# Main `rustmodlica` sim CLI only accepts `--lib-path=<dir>` (equals form), not `--lib-path <dir>`.
$extra = @()
if (Test-Path -LiteralPath (Join-Path $jitRoot "Modelica\package.mo")) {
    $extra += ("--lib-path=" + $jitRoot)
}
$extra += ("--lib-path=" + $testLibRoot)

$env:RUSTMODLICA_COFF_RELOC_TRACE = "1"

foreach ($i in 1, 2) {
    Write-Host "========== run $i =========="
    rtk cargo run -p rustmodlica --bin rustmodlica --release -- @extra $Model
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
