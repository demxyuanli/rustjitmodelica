$models = @("DoublePendulum", "ThreeSprings", "FreeBody", "Pendulum", "SpringMassSystem")

foreach ($mode in @("superfast", "quick", "full")) {
    Write-Host ""
    Write-Host "=== Mode: $mode ===" 
    
    # Clear cache
    Remove-Item -Recurse -Force .jit-cache/flatten -ErrorAction SilentlyContinue
    
    $env:RUSTMODLICA_PERF_TRACE = "1"
    $env:RUSTMODLICA_FLATTEN_FULL_CACHE = "0"
    
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    
    $passed = 0
    $failed = 0
    
    foreach ($m in $models) {
        $result = & powershell -NoProfile -ExecutionPolicy Bypass -File ./run_modelica_dir_regression.ps1 `
            -IncludePattern $m `
            -MaxCases 1 `
            -TEnd 0.1 `
            -ExtraArgs "--validation-mode=$mode" 2>&1
        
        if ($result -match "Summary: (\d+) passed") {
            $passed += $matches[1].Value
        } else {
            $failed++
        }
    }
    
    $sw.Stop()
    
    Write-Host "Passed: $passed, Failed: $failed"
    Write-Host "Total time: $($sw.ElapsedMilliseconds) ms"
    Write-Host ""
}
