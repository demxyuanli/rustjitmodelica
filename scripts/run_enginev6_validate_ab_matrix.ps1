# EngineV6 A/B matrix: --validate --validate-tier=analyze only (no simulation args).
# Per profile: cold (fresh query-cache namespace) + 2 hot runs (same namespace).
# Requires: target/release/rustmodlica.exe built; cwd jit-compiler for lib-path resolution.

param(
    [string]$ExePath = (Join-Path (Split-Path $PSScriptRoot -Parent) "target/release/rustmodlica.exe"),
    [string]$OutDir = (Join-Path (Split-Path $PSScriptRoot -Parent) "build_enginev6_ab"),
    [string]$Model = "Modelica.Mechanics.MultiBody.Examples.Loops.EngineV6"
)

$ErrorActionPreference = "Stop"
# Keep native stderr as stderr/log content, not PowerShell terminating errors.
if ($null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)) {
    $PSNativeCommandUseErrorActionPreference = $false
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$cacheRoot = Join-Path $OutDir "cache_root"
New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null
$csv = Join-Path $OutDir "enginev6_validate_analyze_ab.csv"
$logDir = Join-Path $OutDir "logs"
New-Item -ItemType Directory -Force -Path $logDir | Out-Null

function Clear-JitAbEnv {
    foreach ($k in @(
            "RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE",
            "RUSTMODLICA_CONST_FOLD",
            "RUSTMODLICA_EQ_DCE",
            "RUSTMODLICA_JIT_TYPE_SPECIALIZATION",
            "RUSTMODLICA_JIT_STACK_SCRATCH")) {
        Remove-Item "Env:$k" -ErrorAction SilentlyContinue
    }
}

function Apply-Profile {
    param([string]$ProfileName)
    Clear-JitAbEnv
    switch ($ProfileName) {
        "baseline" { }
        "all_off" {
            $env:RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE = "0"
            $env:RUSTMODLICA_CONST_FOLD = "0"
            $env:RUSTMODLICA_EQ_DCE = "0"
            $env:RUSTMODLICA_JIT_TYPE_SPECIALIZATION = "0"
            $env:RUSTMODLICA_JIT_STACK_SCRATCH = "0"
        }
        "inc_only" {
            $env:RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE = "1"
            $env:RUSTMODLICA_CONST_FOLD = "0"
            $env:RUSTMODLICA_EQ_DCE = "0"
            $env:RUSTMODLICA_JIT_TYPE_SPECIALIZATION = "0"
            $env:RUSTMODLICA_JIT_STACK_SCRATCH = "0"
        }
        "const_dce_only" {
            $env:RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE = "0"
            $env:RUSTMODLICA_CONST_FOLD = "1"
            $env:RUSTMODLICA_EQ_DCE = "1"
            $env:RUSTMODLICA_JIT_TYPE_SPECIALIZATION = "0"
            $env:RUSTMODLICA_JIT_STACK_SCRATCH = "0"
        }
        "type_spec_only" {
            $env:RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE = "0"
            $env:RUSTMODLICA_CONST_FOLD = "0"
            $env:RUSTMODLICA_EQ_DCE = "0"
            $env:RUSTMODLICA_JIT_TYPE_SPECIALIZATION = "1"
            $env:RUSTMODLICA_JIT_STACK_SCRATCH = "0"
        }
        "stack_scratch_only" {
            $env:RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE = "0"
            $env:RUSTMODLICA_CONST_FOLD = "0"
            $env:RUSTMODLICA_EQ_DCE = "0"
            $env:RUSTMODLICA_JIT_TYPE_SPECIALIZATION = "0"
            $env:RUSTMODLICA_JIT_STACK_SCRATCH = "1"
        }
        "all_on" {
            $env:RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE = "1"
            $env:RUSTMODLICA_CONST_FOLD = "1"
            $env:RUSTMODLICA_EQ_DCE = "1"
            $env:RUSTMODLICA_JIT_TYPE_SPECIALIZATION = "1"
            $env:RUSTMODLICA_JIT_STACK_SCRATCH = "1"
        }
        default { throw "Unknown profile: $ProfileName" }
    }
}

$profiles = @(
    "baseline",
    "all_off",
    "inc_only",
    "const_dce_only",
    "type_spec_only",
    "stack_scratch_only",
    "all_on"
)

$env:RUSTMODLICA_FLATTEN_CACHE_DIR = $cacheRoot
$env:RUSTMODLICA_CACHE_SQLITE = "1"
$env:RUSTMODLICA_QUERY_CACHE = "1"

"profile,phase,elapsed_ms,exitcode,namespace" | Out-File -FilePath $csv -Encoding utf8

$jitRoot = Join-Path (Split-Path $PSScriptRoot -Parent) "jit-compiler"
Push-Location $jitRoot
try {
    foreach ($p in $profiles) {
        Apply-Profile $p
        $ns = $null
        foreach ($phase in @("cold", "hot1", "hot2")) {
            if ($phase -eq "cold") {
                $ns = "ev6_${p}_$([guid]::NewGuid().ToString('N'))"
            }
            $env:RUSTMODLICA_QUERY_CACHE_NAMESPACE = $ns
            $logPath = Join-Path $logDir ("{0}_{1}.log" -f $p, $phase)
            $sw = [System.Diagnostics.Stopwatch]::StartNew()
            $cmdLine = ('"{0}" --validate --validate-tier=analyze --lib-path=Modelica --lib-path=ModelicaTest "{1}" > "{2}" 2>&1' -f $ExePath, $Model, $logPath)
            cmd.exe /d /c $cmdLine | Out-Null
            $code = $LASTEXITCODE
            $sw.Stop()
            $ms = [int]$sw.ElapsedMilliseconds
            '{0},{1},{2},{3},{4}' -f $p, $phase, $ms, $code, $ns | Out-File -FilePath $csv -Append -Encoding utf8
            Write-Host ("ENGINEV6|{0}|{1}|ms={2}|exit={3}" -f $p, $phase, $ms, $code)
        }
    }
}
finally {
    Pop-Location
    Clear-JitAbEnv
    Remove-Item Env:RUSTMODLICA_FLATTEN_CACHE_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:RUSTMODLICA_CACHE_SQLITE -ErrorAction SilentlyContinue
    Remove-Item Env:RUSTMODLICA_QUERY_CACHE -ErrorAction SilentlyContinue
    Remove-Item Env:RUSTMODLICA_QUERY_CACHE_NAMESPACE -ErrorAction SilentlyContinue
}

Write-Host "Wrote $csv"
