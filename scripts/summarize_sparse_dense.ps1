param(
    [string]$InputDir = "jit-compiler/build_sparse_dense_bench",
    [string]$OutputDir = "build_sparse_dense_summary",
    [ValidateSet("all", "non_triggered", "triggered")]
    [string]$BltGuardFilter = "non_triggered",
    [string[]]$ModelFilter = @()
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $InputDir)) {
    throw "InputDir not found: $InputDir"
}
if (-not (Test-Path -LiteralPath $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

function Get-Median([double[]]$arr) {
    if ($null -eq $arr -or $arr.Count -eq 0) { return $null }
    $s = $arr | Sort-Object
    $n = $s.Count
    if ($n % 2 -eq 1) {
        return [double]$s[[int]($n / 2)]
    }
    return ([double]$s[$n / 2 - 1] + [double]$s[$n / 2]) / 2.0
}

function Should-IncludeByGuard([object]$guard, [string]$filterMode) {
    if ($filterMode -eq "all") { return $true }
    if ($null -eq $guard) { return $false }
    if ($filterMode -eq "non_triggered") { return (-not [bool]$guard) }
    if ($filterMode -eq "triggered") { return [bool]$guard }
    return $true
}

$csvFiles = Get-ChildItem -LiteralPath $InputDir -Filter "sparse_dense_*.csv" | Sort-Object Name
if ($csvFiles.Count -eq 0) {
    throw "No sparse_dense_*.csv files found in $InputDir"
}

$rows = New-Object System.Collections.Generic.List[object]

foreach ($csvFile in $csvFiles) {
    $csvRows = Import-Csv -LiteralPath $csvFile.FullName
    foreach ($r in $csvRows) {
        if ($ModelFilter.Count -gt 0 -and ($ModelFilter -notcontains $r.model)) { continue }
        if ($r.status -ne "OK") { continue }

        $perfRel = $r.perf_json
        $perfPath = $null
        if ([System.IO.Path]::IsPathRooted($perfRel)) {
            $perfPath = $perfRel
        } else {
            $cand1 = Join-Path (Get-Location) $perfRel
            $cand2 = Join-Path $InputDir $perfRel
            $cand3 = Join-Path $csvFile.DirectoryName $perfRel
            $cand4 = Join-Path (Split-Path -Parent $InputDir) $perfRel
            if (Test-Path -LiteralPath $cand1) {
                $perfPath = $cand1
            } elseif (Test-Path -LiteralPath $cand2) {
                $perfPath = $cand2
            } elseif (Test-Path -LiteralPath $cand4) {
                $perfPath = $cand4
            } else {
                $perfPath = $cand3
            }
        }
        if (-not (Test-Path -LiteralPath $perfPath)) { continue }

        $perfRaw = Get-Content -Raw -LiteralPath $perfPath
        $perfObj = $perfRaw | ConvertFrom-Json
        $cp = $perfObj.compile_perf
        if ($null -eq $cp) { continue }

        $guardTriggered = $cp.blt_degrade_guard_triggered
        if (-not (Should-IncludeByGuard -guard $guardTriggered -filterMode $BltGuardFilter)) {
            continue
        }

        $rows.Add([pscustomobject]@{
            source_csv = $csvFile.Name
            model = $r.model
            path_preference = $r.path_preference
            wall_ms = [double]$r.wall_ms
            compile_jit_ms = [double]$r.compile_jit_ms
            blt_degrade_guard_triggered = $guardTriggered
            blt_degrade_guard_limit = $cp.blt_degrade_guard_limit
            blt_degrade_guard_equation_count = $cp.blt_degrade_guard_equation_count
            perf_json = $r.perf_json
        })
    }
}

$grouped = $rows | Group-Object model, path_preference
$summary = New-Object System.Collections.Generic.List[object]

foreach ($g in $grouped) {
    $first = $g.Group[0]
    $wallVals = @($g.Group | ForEach-Object { [double]$_.wall_ms })
    $jitVals = @($g.Group | ForEach-Object { [double]$_.compile_jit_ms })
    $summary.Add([pscustomobject]@{
        model = $first.model
        path_preference = $first.path_preference
        sample_count = $g.Count
        wall_ms_median = [math]::Round((Get-Median $wallVals), 3)
        compile_jit_ms_median = [math]::Round((Get-Median $jitVals), 3)
        blt_guard_filter = $BltGuardFilter
    })
}

$paired = $summary | Group-Object model
$compare = New-Object System.Collections.Generic.List[object]
foreach ($p in $paired) {
    $dense = $p.Group | Where-Object { $_.path_preference -eq "dense" } | Select-Object -First 1
    $sparse = $p.Group | Where-Object { $_.path_preference -eq "sparse" } | Select-Object -First 1
    if ($null -eq $dense -or $null -eq $sparse) { continue }
    $compare.Add([pscustomobject]@{
        model = $p.Name
        sample_count_dense = $dense.sample_count
        sample_count_sparse = $sparse.sample_count
        wall_ms_median_dense = $dense.wall_ms_median
        wall_ms_median_sparse = $sparse.wall_ms_median
        wall_dense_over_sparse = if ($sparse.wall_ms_median -gt 0) { [math]::Round($dense.wall_ms_median / $sparse.wall_ms_median, 3) } else { $null }
        compile_jit_ms_median_dense = $dense.compile_jit_ms_median
        compile_jit_ms_median_sparse = $sparse.compile_jit_ms_median
        compile_jit_dense_over_sparse = if ($sparse.compile_jit_ms_median -gt 0) { [math]::Round($dense.compile_jit_ms_median / $sparse.compile_jit_ms_median, 3) } else { $null }
        blt_guard_filter = $BltGuardFilter
    })
}

$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$rawCsv = Join-Path $OutputDir ("sparse_dense_rows_{0}.csv" -f $stamp)
$summaryCsv = Join-Path $OutputDir ("sparse_dense_summary_{0}.csv" -f $stamp)
$compareCsv = Join-Path $OutputDir ("sparse_dense_compare_{0}.csv" -f $stamp)
$jsonOut = Join-Path $OutputDir ("sparse_dense_compare_{0}.json" -f $stamp)

$rows | Export-Csv -Path $rawCsv -NoTypeInformation -Encoding UTF8
$summary | Sort-Object model, path_preference | Export-Csv -Path $summaryCsv -NoTypeInformation -Encoding UTF8
$compare | Sort-Object model | Export-Csv -Path $compareCsv -NoTypeInformation -Encoding UTF8

$payload = [pscustomobject]@{
    generated_at = (Get-Date).ToString("s")
    input_dir = $InputDir
    blt_guard_filter = $BltGuardFilter
    model_filter = $ModelFilter
    row_count = $rows.Count
    summary_count = $summary.Count
    compare_count = $compare.Count
    compare = $compare
}
($payload | ConvertTo-Json -Depth 6) | Set-Content -Path $jsonOut -Encoding UTF8

Write-Host "Rows CSV: $rawCsv"
Write-Host "Summary CSV: $summaryCsv"
Write-Host "Compare CSV: $compareCsv"
Write-Host "Compare JSON: $jsonOut"
