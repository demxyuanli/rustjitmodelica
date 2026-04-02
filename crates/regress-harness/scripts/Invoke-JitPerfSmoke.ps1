param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [Parameter(Mandatory = $true)][string]$CargoTargetDir,
    [Parameter(Mandatory = $true)][string]$Model,
    [Parameter(Mandatory = $true)][double]$TEnd,
    [Parameter(Mandatory = $true)][double]$Dt,
    [Parameter(Mandatory = $true)][double]$OutputInterval,
    [Parameter(Mandatory = $true)][int]$CompileMsMax,
    [Parameter(Mandatory = $true)][int]$SimMsMax,
    [Parameter(Mandatory = $true)][int]$EventIterMax,
    [Parameter(Mandatory = $true)][int]$ClockDispatchMax,
    [string]$ArtifactsDir = ""
)

$ErrorActionPreference = "Stop"

function Test-Truthy([string]$v) {
    if ($null -eq $v) { return $false }
    $t = $v.Trim().ToLowerInvariant()
    return -not ($t -eq "" -or $t -eq "0" -or $t -eq "false" -or $t -eq "off" -or $t -eq "no")
}

if (-not (Test-Truthy $env:RUSTMODLICA_PERF_SMOKE)) {
    Write-Host ("[perf-smoke] skipped (RUSTMODLICA_PERF_SMOKE not enabled) model=" + $Model)
    exit 0
}

if ([string]::IsNullOrWhiteSpace($ArtifactsDir)) {
    $ArtifactsDir = Join-Path $RepoRoot "build/regression_data_jit_phase1/artifacts"
}
New-Item -ItemType Directory -Path $ArtifactsDir -Force | Out-Null

$perfBaselinePath = Join-Path $ArtifactsDir "perf_smoke_baseline.json"
$perfSnapshotPath = Join-Path $ArtifactsDir ("perf_smoke_snapshot_{0:yyyyMMdd_HHmmss}.json" -f (Get-Date))

$perfModeRaw = [string]$env:RUSTMODLICA_PERF_BASELINE_MODE
if ([string]::IsNullOrWhiteSpace($perfModeRaw)) { $perfModeRaw = "compare" }
$perfMode = $perfModeRaw.Trim().ToLowerInvariant()
if (@("compare", "record", "update") -notcontains $perfMode) { $perfMode = "compare" }

$ratioRaw = [string]$env:RUSTMODLICA_PERF_DEGRADE_RATIO
$perfDegradeRatio = 0.2
if (-not [string]::IsNullOrWhiteSpace($ratioRaw)) {
    try { $perfDegradeRatio = [double]$ratioRaw } catch { $perfDegradeRatio = 0.2 }
    if ($perfDegradeRatio -lt 0.0) { $perfDegradeRatio = 0.0 }
}

function Get-PerfBaselineMap([string]$path) {
    $m = @{}
    if (Test-Path -LiteralPath $path) {
        try {
            $obj = (Get-Content -LiteralPath $path -Raw) | ConvertFrom-Json
            if ($null -ne $obj) {
                $obj.psobject.Properties | ForEach-Object { $m[$_.Name] = $_.Value }
            }
        } catch {
            $m = @{}
        }
    }
    return $m
}

function Get-PerfBaselineEntry([hashtable]$map, [string]$model) {
    if ($map.ContainsKey($model)) { return $map[$model] }
    return $null
}

function Get-LimitFromBaseline([int]$baseValue, [double]$ratio) {
    if ($baseValue -lt 0) { return [int]::MaxValue }
    $raw = [math]::Ceiling([double]$baseValue * (1.0 + $ratio))
    $v = [int]$raw
    if ($v -lt 0) { $v = [int]::MaxValue }
    return $v
}

$baselineMap = Get-PerfBaselineMap -path $perfBaselinePath
$base = Get-PerfBaselineEntry -map $baselineMap -model $Model
$hasBase = ($null -ne $base)

$compileLimit = $CompileMsMax
$simLimit = $SimMsMax
$eventIterLimit = $EventIterMax
$clockDispatchLimit = $ClockDispatchMax

if ($perfMode -eq "compare" -and $hasBase) {
    $compileLimit = Get-LimitFromBaseline -baseValue ([int]$base.compile_ms) -ratio $perfDegradeRatio
    $simLimit = Get-LimitFromBaseline -baseValue ([int]$base.sim_ms) -ratio $perfDegradeRatio
    $eventIterLimit = Get-LimitFromBaseline -baseValue ([int]$base.event_iter_total) -ratio $perfDegradeRatio
    $clockDispatchLimit = Get-LimitFromBaseline -baseValue ([int]$base.clock_dispatch_total) -ratio $perfDegradeRatio
}

$safeName = $Model.Replace("/", "_").Replace(".", "_")
$csv = Join-Path $ArtifactsDir ("perf_{0}.csv" -f $safeName)

$jitDir = Join-Path $RepoRoot "jit-compiler"
if (-not (Test-Path -LiteralPath $jitDir)) { throw ("missing dir: " + $jitDir) }

$oldPerf = $env:RUSTMODLICA_PERF_TRACE
$env:RUSTMODLICA_PERF_TRACE = "1"
try {
    $out = & cargo --target-dir $CargoTargetDir run -- `
        --solver=rk4 `
        --t-end=$TEnd `
        --dt=$Dt `
        --output-interval=$OutputInterval `
        --result-file=$csv `
        $Model 2>&1 | Out-String
    $exit = $LASTEXITCODE
} finally {
    $env:RUSTMODLICA_PERF_TRACE = $oldPerf
}

$compileMs = -1
$simMs = -1
$eventIter = -1
$clockDispatch = -1
if ($out -match '\[perf\] compile_ms=(\d+)') { $compileMs = [int]$Matches[1] }
if ($out -match '\[perf\] sim_ms=(\d+)') { $simMs = [int]$Matches[1] }
if ($out -match '\[perf\] event_iter_total=(\d+) clock_dispatch_total=(\d+)') {
    $eventIter = [int]$Matches[1]
    $clockDispatch = [int]$Matches[2]
}

$perfOk = ($exit -eq 0) -and (Test-Path -LiteralPath $csv) `
    -and ($compileMs -ge 0) -and ($compileMs -le $compileLimit) `
    -and ($simMs -ge 0) -and ($simMs -le $simLimit) `
    -and ($eventIter -ge 0) -and ($eventIter -le $eventIterLimit) `
    -and ($clockDispatch -ge 0) -and ($clockDispatch -le $clockDispatchLimit)

$current = @{}
$current[$Model] = @{
    compile_ms = $compileMs
    sim_ms = $simMs
    event_iter_total = $eventIter
    clock_dispatch_total = $clockDispatch
}
try { ($current | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $perfSnapshotPath -Encoding UTF8 } catch { }

if ($perfMode -eq "record") {
    ($current | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $perfBaselinePath -Encoding UTF8
} elseif ($perfMode -eq "update") {
    $merged = @{}
    foreach ($k in $baselineMap.Keys) { $merged[$k] = $baselineMap[$k] }
    foreach ($k in $current.Keys) { $merged[$k] = $current[$k] }
    ($merged | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $perfBaselinePath -Encoding UTF8
} elseif ($perfMode -eq "compare" -and -not (Test-Path -LiteralPath $perfBaselinePath)) {
    ($current | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $perfBaselinePath -Encoding UTF8
}

Write-Host ("[perf-smoke] model={0} ok={1} mode={2} has_baseline={3} degrade_ratio={4} compile_ms={5} compile_limit={6} sim_ms={7} sim_limit={8} event_iter_total={9} event_iter_limit={10} clock_dispatch_total={11} clock_dispatch_limit={12} csv={13}" -f `
    $Model, $perfOk, $perfMode, $hasBase, $perfDegradeRatio, $compileMs, $compileLimit, $simMs, $simLimit, $eventIter, $eventIterLimit, $clockDispatch, $clockDispatchLimit, $csv)

if ($perfOk) { exit 0 } else { exit 1 }

