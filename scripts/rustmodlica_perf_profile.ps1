# Apply multibody A/B "default perf" profiles (same env matrix as regression benchmarks).
# Usage (current PowerShell session):
#   . D:\source\repos\rustmodlica\scripts\rustmodlica_perf_profile.ps1
#   Set-RustmodlicaPerfProfile -Profile AllOff
#   Set-RustmodlicaPerfProfile -Profile ConstDceOnly
#   Set-RustmodlicaPerfProfile -Profile Clear

param(
    [string]$Profile = ''
)

$script:RustmodlicaPerfProfileVars = @(
    'RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE',
    'RUSTMODLICA_CONST_FOLD',
    'RUSTMODLICA_EQ_DCE',
    'RUSTMODLICA_JIT_INLINE_BUILTINS',
    'RUSTMODLICA_SIMD_STEP',
    'RUSTMODLICA_JIT_TYPE_SPECIALIZATION',
    'RUSTMODLICA_JIT_STACK_SCRATCH',
    'RUSTMODLICA_RUNTIME_BOUNDARY_EPOCH'
)

function Clear-RustmodlicaPerfProfile {
    foreach ($k in $script:RustmodlicaPerfProfileVars) {
        Remove-Item "Env:$k" -ErrorAction SilentlyContinue
    }
}

function Set-RustmodlicaPerfProfile {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('AllOff', 'ConstDceOnly', 'Clear')]
        [string]$Profile
    )
    Clear-RustmodlicaPerfProfile
    if ($Profile -eq 'Clear') {
        Write-Host '[rustmodlica_perf_profile] cleared tracked env vars (process defaults apply)'
        return
    }
    $env:RUSTMODLICA_RUNTIME_BOUNDARY_EPOCH = '1'
    $env:RUSTMODLICA_JIT_INLINE_BUILTINS = '0'
    $env:RUSTMODLICA_SIMD_STEP = '0'
    $env:RUSTMODLICA_JIT_TYPE_SPECIALIZATION = '0'
    $env:RUSTMODLICA_JIT_STACK_SCRATCH = '0'
    if ($Profile -eq 'AllOff') {
        $env:RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE = '0'
        $env:RUSTMODLICA_CONST_FOLD = '0'
        $env:RUSTMODLICA_EQ_DCE = '0'
        Write-Host '[rustmodlica_perf_profile] AllOff (primary default from multibody A/B)'
        return
    }
    if ($Profile -eq 'ConstDceOnly') {
        $env:RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE = '0'
        $env:RUSTMODLICA_CONST_FOLD = '1'
        $env:RUSTMODLICA_EQ_DCE = '1'
        Write-Host '[rustmodlica_perf_profile] ConstDceOnly (alternate, cold-first)'
        return
    }
}

if ($Profile) {
    Set-RustmodlicaPerfProfile -Profile $Profile
}
