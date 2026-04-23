param(
    [double]$TEnd = 2.0,
    [double]$Dt = 0.02,
    [string]$Solver = "rk45",
    [string]$OutDir = "build_regression_logs/cache_warm_kpi",
    [string]$Candidates = "build_regression_logs/msl_complex_candidates.txt"
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
if (-not (Test-Path -LiteralPath $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

$three = @(
    "Modelica.Electrical.Machines.Examples.SynchronousMachines.SMEE_Generator",
    "Modelica.Magnetic.FundamentalWave.Examples.BasicMachines.SynchronousMachines.SMEE_Generator",
    "Modelica.Mechanics.MultiBody.Examples.Elementary.DoublePendulum"
)
$extra = @()
if (Test-Path -LiteralPath $Candidates) {
    $extra = @(
        Get-Content -LiteralPath $Candidates |
        Where-Object { $_.Trim().Length -gt 0 } |
        ForEach-Object {
            $t = $_.Trim()
            if ($t.StartsWith("--")) { $t = $t.Substring(2).Trim() }
            $t
        } |
        Select-Object -First 3
    )
}
$models = @($three + $extra | Select-Object -Unique)

function Run-One([string]$Model, [string]$Label, [string]$CacheSqlite) {
    $safe = ($Model -replace '[^A-Za-z0-9_.-]', '_')
    $path = Join-Path $OutDir ("perf_{0}_{1}.json" -f $safe, $Label)
    $env:RUSTMODLICA_JIT_CODEGEN_CACHE = "0"
    $env:RUSTMODLICA_CACHE_SQLITE = $CacheSqlite
    $env:RUSTMODLICA_QUERY_CACHE = "1"
    $env:RUSTMODLICA_FLATTEN_FULL_CACHE = "1"
    $env:RUSTMODLICA_EXTERNAL_RESOLVE_CACHE = "1"
    $env:RUSTMODLICA_PERF_TRACE = "1"
    $env:RUSTMODLICA_TIERED_COMPILATION = "0"
    $env:RUSTMODLICA_WARMUP_ENABLED = "0"
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
        if ($LASTEXITCODE -ne 0) { throw "run failed $Model $Label exit=$LASTEXITCODE" }
    }
    finally { Pop-Location }
    $j = Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
    $c = $j.compile_perf
    [pscustomobject]@{
        model                   = $Model
        label                   = $Label
        flatten_inline_ms       = [double]$c.flatten_inline_ms
        flat_full_cache_hits    = [double]$c.flat_full_cache_hits
        ext_resolve_cache       = [string]$c.external_resolve_cache_status
    }
}

$rows = @()
foreach ($m in $models) {
    $rows += Run-One -Model $m -Label "cold" -CacheSqlite "0"
    $rows += Run-One -Model $m -Label "hot" -CacheSqlite "1"
}

$rows | Export-Csv -NoTypeInformation -Encoding UTF8 -Path (Join-Path $OutDir "runs.csv")

$kpis = foreach ($m in $models) {
    $cold = $rows | Where-Object { $_.model -eq $m -and $_.label -eq "cold" } | Select-Object -First 1
    $hot = $rows | Where-Object { $_.model -eq $m -and $_.label -eq "hot" } | Select-Object -First 1
    if ($null -eq $cold -or $null -eq $hot) { continue }
    $ratio = if ($cold.flatten_inline_ms -gt 0) { $hot.flatten_inline_ms / $cold.flatten_inline_ms } else { 1.0 }
    $pct = [math]::Round((1.0 - $ratio) * 100.0, 1)
    [pscustomobject]@{
        model                      = $m
        cold_flatten_inline_ms     = $cold.flatten_inline_ms
        hot_flatten_inline_ms      = $hot.flatten_inline_ms
        flatten_reduction_pct      = $pct
        hot_ext_resolve_status     = $hot.ext_resolve_cache
        hot_flat_full_hits         = $hot.flat_full_cache_hits
    }
}

$kpis | Export-Csv -NoTypeInformation -Encoding UTF8 -Path (Join-Path $OutDir "kpi.csv")
$kpis | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 (Join-Path $OutDir "kpi.json")
Write-Host "Wrote $OutDir\kpi.csv and kpi.json"
$kpis | Format-Table -AutoSize
