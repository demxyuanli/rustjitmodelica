param(
    [int]$Rounds = 10,
    [string]$OutRoot = "build_regression_logs/leyden_repeat_compare"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-Median([double[]]$arr) {
    if ($arr.Count -eq 0) { return 0.0 }
    $s = $arr | Sort-Object
    $n = $s.Count
    if (($n % 2) -eq 1) {
        return [double]$s[[int]($n / 2)]
    }
    return ([double]$s[$n / 2 - 1] + [double]$s[$n / 2]) / 2.0
}

function Get-P95([double[]]$arr) {
    if ($arr.Count -eq 0) { return 0.0 }
    $s = $arr | Sort-Object
    $idx = [int][math]::Ceiling($s.Count * 0.95) - 1
    if ($idx -lt 0) { $idx = 0 }
    return [double]$s[$idx]
}

if (-not (Test-Path -LiteralPath $OutRoot)) {
    New-Item -ItemType Directory -Path $OutRoot | Out-Null
}

$optAll = New-Object System.Collections.Generic.List[object]
$noAll = New-Object System.Collections.Generic.List[object]
$totals = New-Object System.Collections.Generic.List[object]

for ($i = 1; $i -le $Rounds; $i++) {
    $optDir = "build_modelica_dir_regress_jitstress_opt_r$i"
    $noDir = "build_modelica_dir_regress_jitstress_noopt_r$i"

    $env:RUSTMODLICA_JIT_CODEGEN_CACHE = "0"
    $env:RUSTMODLICA_CONST_FOLD = "1"
    $env:RUSTMODLICA_EQ_DCE = "1"
    $env:RUSTMODLICA_TIERED_COMPILATION = "0"
    $env:RUSTMODLICA_WARMUP_ENABLED = "0"
    $env:RUSTMODLICA_CACHE_SQLITE = "0"
    $env:RUSTMODLICA_FLATTEN_FULL_CACHE = "0"
    $env:RUSTMODLICA_QUERY_CACHE = "0"

    powershell -NoProfile -File run_modelica_dir_regression.ps1 `
        -OutDir $optDir -IncludePattern JitStress -TEnd 10 -Dt 0.01 -Solver rk4 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "opt run failed round $i" }

    $env:RUSTMODLICA_JIT_CODEGEN_CACHE = "0"
    $env:RUSTMODLICA_CONST_FOLD = "0"
    $env:RUSTMODLICA_EQ_DCE = "0"
    $env:RUSTMODLICA_TIERED_COMPILATION = "0"
    $env:RUSTMODLICA_WARMUP_ENABLED = "0"
    $env:RUSTMODLICA_CACHE_SQLITE = "0"
    $env:RUSTMODLICA_FLATTEN_FULL_CACHE = "0"
    $env:RUSTMODLICA_QUERY_CACHE = "0"

    powershell -NoProfile -File run_modelica_dir_regression.ps1 `
        -OutDir $noDir -IncludePattern JitStress -TEnd 10 -Dt 0.01 -Solver rk4 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "noopt run failed round $i" }

    $optCsv = Get-ChildItem "$optDir/runlog_*.csv" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $noCsv = Get-ChildItem "$noDir/runlog_*.csv" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $optRows = Import-Csv $optCsv.FullName
    $noRows = Import-Csv $noCsv.FullName

    foreach ($r in $optRows) {
        $optAll.Add([pscustomobject]@{
                round = $i
                model = $r.case_name
                ms    = [double]$r.duration_ms
            }) | Out-Null
    }
    foreach ($r in $noRows) {
        $noAll.Add([pscustomobject]@{
                round = $i
                model = $r.case_name
                ms    = [double]$r.duration_ms
            }) | Out-Null
    }

    $optTotal = ($optRows | Measure-Object duration_ms -Sum).Sum
    $noTotal = ($noRows | Measure-Object duration_ms -Sum).Sum
    $totals.Add([pscustomobject]@{
            round         = $i
            opt_total_ms  = [double]$optTotal
            noopt_total_ms = [double]$noTotal
            delta_ms      = [double]$noTotal - [double]$optTotal
        }) | Out-Null

    Write-Host ("round {0} done: opt={1} noopt={2}" -f $i, $optTotal, $noTotal)
}

$models = ($optAll | Select-Object -ExpandProperty model -Unique | Sort-Object)

$modelStats = foreach ($m in $models) {
    $o = @($optAll | Where-Object { $_.model -eq $m } | Select-Object -ExpandProperty ms)
    $n = @($noAll | Where-Object { $_.model -eq $m } | Select-Object -ExpandProperty ms)
    $oMean = (($o | Measure-Object -Average).Average)
    $nMean = (($n | Measure-Object -Average).Average)
    [pscustomobject]@{
        model           = $m
        opt_mean_ms     = [math]::Round($oMean, 2)
        opt_median_ms   = [math]::Round((Get-Median $o), 2)
        opt_p95_ms      = [math]::Round((Get-P95 $o), 2)
        noopt_mean_ms   = [math]::Round($nMean, 2)
        noopt_median_ms = [math]::Round((Get-Median $n), 2)
        noopt_p95_ms    = [math]::Round((Get-P95 $n), 2)
        delta_mean_ms   = [math]::Round(($nMean - $oMean), 2)
        speedup_mean_pct = if ($nMean -gt 0) {
            [math]::Round((($nMean - $oMean) / $nMean) * 100, 2)
        } else { 0.0 }
    }
}

$ot = @($totals | Select-Object -ExpandProperty opt_total_ms)
$nt = @($totals | Select-Object -ExpandProperty noopt_total_ms)
$otMean = (($ot | Measure-Object -Average).Average)
$ntMean = (($nt | Measure-Object -Average).Average)

$totalStats = [pscustomobject]@{
    rounds                = $Rounds
    opt_total_mean_ms     = [math]::Round($otMean, 2)
    opt_total_median_ms   = [math]::Round((Get-Median $ot), 2)
    opt_total_p95_ms      = [math]::Round((Get-P95 $ot), 2)
    noopt_total_mean_ms   = [math]::Round($ntMean, 2)
    noopt_total_median_ms = [math]::Round((Get-Median $nt), 2)
    noopt_total_p95_ms    = [math]::Round((Get-P95 $nt), 2)
    total_delta_mean_ms   = [math]::Round(($ntMean - $otMean), 2)
    total_speedup_mean_pct = if ($ntMean -gt 0) {
        [math]::Round((($ntMean - $otMean) / $ntMean) * 100, 2)
    } else { 0.0 }
}

$modelStats | Export-Csv -NoTypeInformation -Encoding UTF8 -Path (Join-Path $OutRoot "model_stats.csv")
$totals | Export-Csv -NoTypeInformation -Encoding UTF8 -Path (Join-Path $OutRoot "round_totals.csv")

$result = [pscustomobject]@{
    total_stats = $totalStats
    model_stats = $modelStats
}
$result | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $OutRoot "summary.json")

Write-Host "=== TOTAL ==="
$totalStats | Format-List
Write-Host "=== MODEL ==="
$modelStats | Sort-Object model | Format-Table -AutoSize
