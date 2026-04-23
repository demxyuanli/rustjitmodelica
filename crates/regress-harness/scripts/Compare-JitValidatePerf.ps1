# Compare two jit validate-perf output directories (report.json + perf_*.json).
#
# Interpretation: baseline and compare use separate out_dir trees; each has its own
# cache_* SQLite + SHM state. Scenario devloop_multi_model uses PreserveBetweenScenarios,
# so an L2 flat_full hit in baseline may be leftover from an earlier run into that folder.
# Large d_flat_us rows usually pair with cache_l2_hits=0 vs 1 (miss vs hit), not codegen regressions.
# For fair A/B: run both benches with the same flags and add
#   --purge-scenario-caches
# to regress-harness `jit validate-perf` so each run starts without leftover out_dir/cache_*.
#
# Usage (from repo root):
#   powershell -NoProfile -File crates/regress-harness/scripts/Compare-JitValidatePerf.ps1 `
#     -BaselineDir build/jit_heavy_validate_perf_six `
#     -CompareDir build/jit_heavy_validate_perf_rerun
param(
    [Parameter(Mandatory = $true)][string]$BaselineDir,
    [Parameter(Mandatory = $true)][string]$CompareDir,
    [string]$RepoRoot = "",
    [string]$OutCsv = "",
    [double]$OutlierFlatDeltaUs = 5000000
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
}

function Resolve-Dir([string]$d) {
    if ([System.IO.Path]::IsPathRooted($d)) { return $d }
    return (Join-Path $RepoRoot $d)
}

$baseRoot = Resolve-Dir $BaselineDir
$cmpRoot = Resolve-Dir $CompareDir
$baseReport = Join-Path $baseRoot "report.json"
$cmpReport = Join-Path $cmpRoot "report.json"

foreach ($p in @($baseReport, $cmpReport)) {
    if (-not (Test-Path -LiteralPath $p)) {
        Write-Error "Missing report.json: $p"
    }
}

$docBaseline = Get-Content -LiteralPath $baseReport -Raw -Encoding UTF8 | ConvertFrom-Json
$docCompare = Get-Content -LiteralPath $cmpReport -Raw -Encoding UTF8 | ConvertFrom-Json

function Test-L2FlatFullMiss([pscustomobject]$c) {
    $m = $c.cache_scope_stage_misses
    if ($null -eq $m) { return $false }
    if ($m.PSObject.Properties.Match('L2').Count -eq 0) { return $false }
    $l2 = $m.L2
    if ($null -eq $l2) { return $false }
    return ($l2.PSObject.Properties.Match('flat_full').Count -gt 0)
}

function Get-PerfRow {
    param($case, [string]$rootDir)
    $rel = [string]$case.perf_json
    if ([string]::IsNullOrWhiteSpace($rel)) { return $null }
    $leaf = Split-Path -Leaf ($rel -replace '\\', '\')
    $p = Join-Path $rootDir $leaf
    if (-not (Test-Path -LiteralPath $p)) { return $null }
    $j = Get-Content -LiteralPath $p -Raw -Encoding UTF8 | ConvertFrom-Json
    $c = $j.compile_perf
    return [pscustomobject]@{
        flat_us = [long]$c.flatten_wall_us
        codegen_us = [long]$c.codegen_wall_us
        dur_ms = [long]$case.duration_ms
        l2_hits = [int]$c.cache_l2_hits
        l2_flat_miss = (Test-L2FlatFullMiss $c)
    }
}

$rows = New-Object System.Collections.Generic.List[object]
foreach ($a in $docBaseline.cases) {
    $b = $docCompare.cases | Where-Object {
        $_.scenario -eq $a.scenario -and $_.model -eq $a.model -and $_.run_index -eq $a.run_index
    } | Select-Object -First 1
    if (-not $b) { continue }
    $pa = Get-PerfRow $a $baseRoot
    $pb = Get-PerfRow $b $cmpRoot
    if (-not $pa -or -not $pb) { continue }
    $dFlat = $pb.flat_us - $pa.flat_us
    $dCg = $pb.codegen_us - $pa.codegen_us
    $out = [Math]::Abs($dFlat) -ge $OutlierFlatDeltaUs
    $rows.Add([pscustomobject]@{
        scenario = $a.scenario
        model = $a.model
        run = $a.run_index
        flat_baseline_us = $pa.flat_us
        flat_compare_us = $pb.flat_us
        d_flat_us = $dFlat
        cg_baseline_us = $pa.codegen_us
        cg_compare_us = $pb.codegen_us
        d_codegen_us = $dCg
        wall_baseline_ms = $pa.dur_ms
        wall_compare_ms = $pb.dur_ms
        d_wall_ms = ($pb.dur_ms - $pa.dur_ms)
        l2_hit_b = $pa.l2_hits
        l2_hit_c = $pb.l2_hits
        l2_miss_flat_b = $pa.l2_flat_miss
        l2_miss_flat_c = $pb.l2_flat_miss
        outlier_flat = $out
    }) | Out-Null
}

Write-Host ("[compare-jit-validate-perf] baseline={0}" -f $baseRoot)
Write-Host ("[compare-jit-validate-perf] compare ={0}" -f $cmpRoot)
Write-Host ("[compare-jit-validate-perf] rows={0} outlier_threshold_|d_flat|>={1} us" -f $rows.Count, $OutlierFlatDeltaUs)
Write-Host "[compare-jit-validate-perf] hint: different out_dir => different cache_*; L2 hit/miss explains most flatten deltas between folders."
Write-Host ""

$rows | Sort-Object scenario, model, run | Format-Table scenario, model, run, l2_hit_b, l2_hit_c, l2_miss_flat_b, l2_miss_flat_c, d_flat_us, d_codegen_us, outlier_flat -AutoSize

$df = $rows | ForEach-Object { [long]$_.d_flat_us }
$dc = $rows | ForEach-Object { [long]$_.d_codegen_us }
$dm = $rows | ForEach-Object { [long]$_.d_wall_ms }
if ($df.Count -gt 0) {
    $sortedF = $df | Sort-Object
    $mid = [int]($sortedF.Count / 2)
    $medianF = if (($sortedF.Count % 2) -eq 1) { $sortedF[$mid] } else { [long](($sortedF[$mid - 1] + $sortedF[$mid]) / 2) }
    Write-Host "--- delta (compare minus baseline) ---"
    Write-Host ("d_flat_us:    min={0} max={1} median={2}" -f ($df | Measure-Object -Minimum).Minimum, ($df | Measure-Object -Maximum).Maximum, $medianF)
    Write-Host ("d_codegen_us: min={0} max={1} avg={2:n0}" -f ($dc | Measure-Object -Minimum).Minimum, ($dc | Measure-Object -Maximum).Maximum, (($dc | Measure-Object -Average).Average))
    Write-Host ("d_wall_ms:    min={0} max={1} avg={2:n1}" -f ($dm | Measure-Object -Minimum).Minimum, ($dm | Measure-Object -Maximum).Maximum, (($dm | Measure-Object -Average).Average))
}

$flags = @($rows | Where-Object { $_.outlier_flat })
if ($flags.Count -gt 0) {
    Write-Host ""
    Write-Host "--- flagged large |d_flat_us| (check l2_hit / l2_miss_flat) ---"
    $flags | Format-Table scenario, model, run, l2_hit_b, l2_hit_c, l2_miss_flat_b, l2_miss_flat_c, d_flat_us -AutoSize
}

if (-not [string]::IsNullOrWhiteSpace($OutCsv)) {
    $csvPath = if ([System.IO.Path]::IsPathRooted($OutCsv)) { $OutCsv } else { Join-Path $RepoRoot $OutCsv }
    $rows | Sort-Object scenario, model, run | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding UTF8
    Write-Host ""
    Write-Host ("[compare-jit-validate-perf] wrote {0}" -f $csvPath)
}
