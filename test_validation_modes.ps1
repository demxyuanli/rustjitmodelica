# Test validation modes with MultiBody DoublePendulum
param([string]$Model = "DoublePendulum")

$ErrorActionPreference = "Stop"

function Test-Mode {
    param([string]$Mode)
    
    # Clear cache
    Remove-Item -Recurse -Force .jit-cache/flatten -ErrorAction SilentlyContinue
    
    $env:RUSTMODLICA_PERF_TRACE = "1"
    $env:RUSTMODLICA_FLATTEN_FULL_CACHE = "0"
    $env:RUSTMODLICA_CACHE_SQLITE = "0"
    
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    
    $output = & powershell -NoProfile -ExecutionPolicy Bypass -File ./run_modelica_dir_regression.ps1 `
        -IncludePattern $Model `
        -MaxCases 1 `
        -TEnd 0.1 `
        -ExtraArgs @("--validation-mode=$Mode") 2>&1
    
    $sw.Stop()
    
    # Extract timing from output
    $flattenUs = ($output | Select-String "\[flatten\] END.*us=(\d+)" | ForEach-Object { $_.Matches.Groups[1].Value })
    $cacheHit = ($output | Select-String "\[cache\] FLAT_HIT" | Measure-Object).Count
    $cacheMiss = ($output | Select-String "\[cache\] FLAT_MISS" | ForEach-Object { $_.Matches.Groups[1].Value })
    
    Write-Host "=== Mode: $Mode ==="
    Write-Host "  Wall time: $($sw.ElapsedMilliseconds) ms"
    if ($flattenUs) { Write-Host "  Flatten us: $flattenUs" }
    Write-Host "  Cache hits: $cacheHit"
    Write-Host ""
    
    # Show relevant log lines
    $output | Select-String "\[flatten\]|\[cache\] FLAT" | ForEach-Object { Write-Host "  $_" }
    Write-Host ""
}

Write-Host "Testing validation modes with model: $Model"
Write-Host "========================================`n"

Test-Mode "superfast"
Test-Mode "quick"
Test-Mode "full"

Write-Host "Done!"
