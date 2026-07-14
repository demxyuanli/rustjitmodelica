param([string]$Mode = "full")

# Clear cache
Remove-Item -Recurse -Force .jit-cache/flatten -ErrorAction SilentlyContinue

$env:RUSTMODLICA_PERF_TRACE = "1"
$env:RUSTMODLICA_FLATTEN_FULL_CACHE = "0"
$env:RUSTMODLICA_CACHE_SQLITE = "0"
$env:RUSTMODLICA_CACHE_SHM = "0"

$sw = [System.Diagnostics.Stopwatch]::StartNew()

$output = & powershell -NoProfile -ExecutionPolicy Bypass -File ./run_modelica_dir_regression.ps1 `
    -IncludePattern "DoublePendulum" `
    -MaxCases 1 `
    -TEnd 0.1 `
    -ExtraArgs @("--validation-mode=$Mode") 2>&1

$sw.Stop()

Write-Host ""
Write-Host "=== Mode: $Mode ==="
Write-Host "Total wall time: $($sw.ElapsedMilliseconds) ms"

# Show flatten log lines
$output | Select-String "\[flatten\]" | ForEach-Object { Write-Host $_ }
$output | Select-String "\[cache\] FLAT" | ForEach-Object { Write-Host $_ }
