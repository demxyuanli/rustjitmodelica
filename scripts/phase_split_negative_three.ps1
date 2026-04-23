param(
    [double]$TEnd = 2.0,
    [double]$Dt = 0.01,
    [string]$Solver = "rk45",
    [string]$OutDir = "build_regression_logs/phase_split_negative_three",
    [int]$Rounds = 10,
    [switch]$StrictV2
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repo = (Resolve-Path ".").Path
$exe = Join-Path $repo "target\release\rustmodlica.exe"
if (-not (Test-Path -LiteralPath $exe)) {
    throw "Build release first: cargo build -p rustmodlica --release"
}

if (-not [System.IO.Path]::IsPathRooted($OutDir)) {
    $OutDir = Join-Path $repo $OutDir
}

$models = @(
    "Modelica.Electrical.Machines.Examples.SynchronousMachines.SMEE_Generator",
    "Modelica.Magnetic.FundamentalWave.Examples.BasicMachines.SynchronousMachines.SMEE_Generator",
    "Modelica.Mechanics.MultiBody.Examples.Elementary.DoublePendulum"
)

if (-not (Test-Path -LiteralPath $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

function Invoke-One {
    param(
        [string]$Model,
        [string]$Label,
        [string]$Fold,
        [string]$Dce,
        [int]$Round
    )
    $safe = ($Model -replace '[^A-Za-z0-9_.-]', '_')
    $path = Join-Path $OutDir ("perf_{0}_{1}_r{2}.json" -f $safe, $Label, $Round)

    $env:RUSTMODLICA_JIT_CODEGEN_CACHE = "0"
    $env:RUSTMODLICA_CACHE_SQLITE = "0"
    $env:RUSTMODLICA_QUERY_CACHE = "0"
    $env:RUSTMODLICA_TIERED_COMPILATION = "0"
    $env:RUSTMODLICA_WARMUP_ENABLED = "0"
    $env:RUSTMODLICA_PERF_TRACE = "1"
    $env:RUSTMODLICA_CONST_FOLD = $Fold
    $env:RUSTMODLICA_EQ_DCE = $Dce

    $args = @(
        "--lib-path=$repo\jit-compiler\Modelica",
        "--lib-path=$repo\jit-compiler\ModelicaTest",
        "--solver=$Solver",
        "--dt=$Dt",
        "--t-end=$TEnd",
        "--perf-json=$path",
        $Model
    )

    Push-Location (Join-Path $repo "jit-compiler")
    try {
        & $exe @args | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "run failed model=$Model label=$Label r=$Round exit=$LASTEXITCODE" }
    }
    finally {
        Pop-Location
    }
    return $path
}

function Read-Phases([string]$JsonPath) {
    $j = Get-Content -LiteralPath $JsonPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $c = $j.compile_perf
    $flatWall = [double]$c.flatten_wall_ms
    $flatInline = [double]$c.flatten_inline_ms
    $flat = $flatWall + $flatInline
    $jit = [double]$c.jit_ms + [double]$c.codegen_wall_ms
    $sim = 0.0
    if ($null -ne $j.sim_perf -and $null -ne $j.sim_perf.sim_ms) {
        $sim = [double]$j.sim_perf.sim_ms
    }
    $skipped = $false
    $skipProp = $c.PSObject.Properties["const_fold_skipped_by_policy"]
    if ($null -ne $skipProp -and $null -ne $skipProp.Value) { $skipped = [bool]$skipProp.Value }
    return [pscustomobject]@{
        flat_ms               = [math]::Round($flat, 2)
        flat_wall_ms          = [math]::Round($flatWall, 2)
        flat_inline_ms        = [math]::Round($flatInline, 2)
        jit_ms                = [math]::Round($jit, 2)
        sim_ms                = [math]::Round($sim, 2)
        analyze_ms            = [math]::Round([double]$c.analyze_ms, 2)
        backend_dae_ms        = [math]::Round([double]$c.backend_dae_ms, 2)
        external_resolve_ms   = [math]::Round([double]$c.external_resolve_ms, 2)
        load_model_ms         = [math]::Round([double]$c.load_model_ms, 2)
        fold_count            = [int64]$c.const_fold_count
        dce_removed           = [int64]$c.eq_dce_removed
        const_fold_skipped    = $skipped
    }
}

function Get-Median([double[]]$arr) {
    if ($arr.Count -eq 0) { return 0.0 }
    $s = @($arr | Sort-Object)
    $n = $s.Count
    if (($n % 2) -eq 1) { return [double]$s[[int]($n / 2)] }
    return ([double]$s[$n / 2 - 1] + [double]$s[$n / 2]) / 2.0
}

$allRows = @()
for ($r = 1; $r -le $Rounds; $r++) {
    foreach ($m in $models) {
        $pOpt = Invoke-One -Model $m -Label "opt" -Fold "1" -Dce "1" -Round $r
        $pNo = Invoke-One -Model $m -Label "noopt" -Fold "0" -Dce "0" -Round $r
        $o = Read-Phases $pOpt
        $n = Read-Phases $pNo
        $allRows += [pscustomobject]@{
            round               = $r
            model               = $m
            opt_flat_ms         = $o.flat_ms
            noopt_flat_ms       = $n.flat_ms
            d_flat_ms           = [math]::Round($n.flat_ms - $o.flat_ms, 2)
            opt_jit_ms          = $o.jit_ms
            noopt_jit_ms        = $n.jit_ms
            d_jit_ms            = [math]::Round($n.jit_ms - $o.jit_ms, 2)
            opt_sim_ms          = $o.sim_ms
            noopt_sim_ms        = $n.sim_ms
            d_sim_ms            = [math]::Round($n.sim_ms - $o.sim_ms, 2)
            opt_external_ms     = $o.external_resolve_ms
            noopt_external_ms    = $n.external_resolve_ms
            d_external_ms       = [math]::Round($n.external_resolve_ms - $o.external_resolve_ms, 2)
            opt_fold_count      = $o.fold_count
            noopt_fold_count    = $n.fold_count
            const_fold_skipped_opt = $o.const_fold_skipped
        }
    }
}

$allRows | Export-Csv -NoTypeInformation -Encoding UTF8 -Path (Join-Path $OutDir "phase_rounds.csv")

$summaryModels = foreach ($m in $models) {
    $sub = @($allRows | Where-Object { $_.model -eq $m })
    $dFlat = @($sub | ForEach-Object { [double]$_.d_flat_ms })
    $dExt = @($sub | ForEach-Object { [double]$_.d_external_ms })
    [pscustomobject]@{
        model                 = $m
        median_d_flat_ms      = [math]::Round((Get-Median $dFlat), 2)
        median_d_external_ms  = [math]::Round((Get-Median $dExt), 2)
        skipped_policy_rounds = @($sub | Where-Object { $_.const_fold_skipped_opt }).Count
    }
}

$summary = [pscustomobject]@{
    rounds         = $Rounds
    generated_iso  = (Get-Date).ToString("s")
    models         = $summaryModels
}
$summary | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $OutDir "summary.json")

Write-Host "Wrote:"
Write-Host (Join-Path $OutDir "phase_rounds.csv")
Write-Host (Join-Path $OutDir "summary.json")
$summaryModels | Format-Table -AutoSize

if ($StrictV2) {
    foreach ($row in $summaryModels) {
        if ([math]::Abs($row.median_d_flat_ms) -gt 15000) {
            throw "V2: abs(median d_flat_ms) > 15000 for $($row.model): $($row.median_d_flat_ms)"
        }
    }
}
