param(
    [string[]]$Models = @("TestLib/SolvableBlock4Res", "TestLib/ClockedPartitionTest"),
    [double]$TEnd = 1.0,
    [double]$Dt = 0.01,
    [string]$OutputDir = "build_sparse_dense_bench",
    [string]$Warnings = "none",
    [switch]$UseRelease
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$csvPath = Join-Path $OutputDir ("sparse_dense_{0}.csv" -f $stamp)
$jsonPath = Join-Path $OutputDir ("sparse_dense_{0}.json" -f $stamp)

$rows = New-Object System.Collections.Generic.List[object]

function Run-OneCase {
    param(
        [string]$Model,
        [string]$PathPref
    )

    $perfPath = Join-Path $OutputDir ("perf_{0}_{1}_{2}.json" -f ($Model -replace "[^A-Za-z0-9_\-]", "_"), $PathPref, $stamp)
    $resultPath = Join-Path $OutputDir ("result_{0}_{1}_{2}.csv" -f ($Model -replace "[^A-Za-z0-9_\-]", "_"), $PathPref, $stamp)
    $targetDir = "target_bench_$PathPref"

    $oldPref = $env:RUSTMODLICA_NEWTON_PATH
    $oldTrace = $env:RUSTMODLICA_NEWTON_PATH_TRACE
    $env:RUSTMODLICA_NEWTON_PATH = $PathPref
    $env:RUSTMODLICA_NEWTON_PATH_TRACE = "1"

    try {
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        $cmdArgs = @("run", "-p", "rustmodlica")
        $cmdArgs += @("--target-dir", $targetDir)
        if ($UseRelease) { $cmdArgs += "--release" }
        $cmdArgs += @(
            "--",
            "--warnings=$Warnings",
            "--perf-json=$perfPath",
            "--t-end=$TEnd",
            "--dt=$Dt",
            "--result-file=$resultPath",
            $Model
        )
        & cargo @cmdArgs | Out-Host
        $exitCode = $LASTEXITCODE
        $sw.Stop()
    }
    finally {
        if ($null -eq $oldPref) { Remove-Item Env:RUSTMODLICA_NEWTON_PATH -ErrorAction SilentlyContinue } else { $env:RUSTMODLICA_NEWTON_PATH = $oldPref }
        if ($null -eq $oldTrace) { Remove-Item Env:RUSTMODLICA_NEWTON_PATH_TRACE -ErrorAction SilentlyContinue } else { $env:RUSTMODLICA_NEWTON_PATH_TRACE = $oldTrace }
    }

    $compilePerf = $null
    if (Test-Path $perfPath) {
        try {
            $perfRaw = Get-Content -Raw -Path $perfPath
            $perfJson = $perfRaw | ConvertFrom-Json
            $compilePerf = $perfJson.compile_perf
        } catch {
            $compilePerf = $null
        }
    }

    [pscustomobject]@{
        model = $Model
        path_preference = $PathPref
        exit_code = $exitCode
        status = if ($exitCode -eq 0) { "OK" } else { "BAD" }
        wall_ms = [int64]$sw.ElapsedMilliseconds
        compile_load_ms = if ($compilePerf) { [int64]$compilePerf.load_model_ms } else { $null }
        compile_flatten_ms = if ($compilePerf) { [int64]$compilePerf.flatten_inline_ms } else { $null }
        compile_analyze_ms = if ($compilePerf) { [int64]$compilePerf.analyze_ms } else { $null }
        compile_backend_dae_ms = if ($compilePerf) { [int64]$compilePerf.backend_dae_ms } else { $null }
        compile_jit_ms = if ($compilePerf) { [int64]$compilePerf.jit_ms } else { $null }
        state_count = if ($compilePerf) { [int64]$compilePerf.state_count } else { $null }
        alg_eq_count = if ($compilePerf) { [int64]$compilePerf.alg_eq_count } else { $null }
        diff_eq_count = if ($compilePerf) { [int64]$compilePerf.diff_eq_count } else { $null }
        peak_mem_mb = $null
        perf_json = $perfPath
        result_csv = $resultPath
    }
}

foreach ($m in $Models) {
    $rows.Add((Run-OneCase -Model $m -PathPref "dense"))
    $rows.Add((Run-OneCase -Model $m -PathPref "sparse"))
}

$rows | Export-Csv -Path $csvPath -NoTypeInformation -Encoding UTF8
$rows | ConvertTo-Json -Depth 6 | Set-Content -Path $jsonPath -Encoding UTF8

Write-Host "Benchmark CSV: $csvPath"
Write-Host "Benchmark JSON: $jsonPath"
exit 0
