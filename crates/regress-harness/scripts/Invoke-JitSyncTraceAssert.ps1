param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [Parameter(Mandatory = $true)][string]$CargoTargetDir,
    [Parameter(Mandatory = $true)][string]$Model,
    [Parameter(Mandatory = $true)][string]$ExpectSubstr,
    [Parameter(Mandatory = $true)][double]$TEnd,
    [string]$ExpectTimes = "",
    [string]$DisallowTimes = "",
    [string]$ArtifactsDir = ""
)

$ErrorActionPreference = "Stop"

function ConvertTo-DoubleList([string]$s) {
    $out = @()
    if ([string]::IsNullOrWhiteSpace($s)) { return $out }
    foreach ($part in $s.Split(",") ) {
        $p = $part.Trim()
        if ($p -eq "") { continue }
        $out += [double]$p
    }
    return $out
}

if ([string]::IsNullOrWhiteSpace($ArtifactsDir)) {
    $ArtifactsDir = Join-Path $RepoRoot "build/regression_data_jit_phase1/artifacts"
}
New-Item -ItemType Directory -Path $ArtifactsDir -Force | Out-Null

$safeName = $Model.Replace("/", "_").Replace(".", "_")
$tracePath = Join-Path $ArtifactsDir ("trace_clocked_{0}.txt" -f $safeName)
$csvPath = Join-Path $ArtifactsDir ("trace_clocked_{0}.csv" -f $safeName)

$jitDir = Join-Path $RepoRoot "jit-compiler"
if (-not (Test-Path -LiteralPath $jitDir)) { throw ("missing dir: " + $jitDir) }

$expectTimesArr = ConvertTo-DoubleList -s $ExpectTimes
$disallowTimesArr = ConvertTo-DoubleList -s $DisallowTimes

$oldTrace = $env:RUSTMODLICA_EVENT_TRACE
$env:RUSTMODLICA_EVENT_TRACE = "1"
try {
    $out = & cargo --target-dir $CargoTargetDir run -- `
        --solver=rk4 `
        --dt=0.01 `
        --t-end=$TEnd `
        --output-interval=0.25 `
        --result-file=$csvPath `
        $Model 2>&1 | Out-String
    $exit = $LASTEXITCODE
} finally {
    $env:RUSTMODLICA_EVENT_TRACE = $oldTrace
}

$out | Set-Content -LiteralPath $tracePath -Encoding UTF8
$text = [string]$out

$substrEsc = [regex]::Escape($ExpectSubstr)
$ok = ($exit -eq 0)

foreach ($t in $expectTimesArr) {
    $tStr = [string]::Format("{0:F6}", [double]$t)
    $pattern = "\\[event-trace\\] t=$tStr active_clock_partitions=.*$substrEsc"
    if ($text -notmatch $pattern) { $ok = $false }
}
foreach ($t in $disallowTimesArr) {
    $tStr = [string]::Format("{0:F6}", [double]$t)
    $pattern = "\\[event-trace\\] t=$tStr active_clock_partitions=.*$substrEsc"
    if ($text -match $pattern) { $ok = $false }
}

Write-Host ("[sync-trace-assert] model={0} ok={1} trace={2} csv={3} expectSubstr={4} expectTimes={5} disallowTimes={6}" -f `
    $Model, $ok, $tracePath, $csvPath, $ExpectSubstr, $ExpectTimes, $DisallowTimes)

if ($ok) { exit 0 } else { exit 1 }

