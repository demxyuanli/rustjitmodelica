param(
    [string]$CargoTargetSubdir = "",
    [string]$ValidateTier = "analyze",
    [string]$OutDir = "",
    [int]$HotRuns = 2,
    [string[]]$Models = @("ComponentLibraryCoverage", "SolvableBlock64Sparse", "SimpleTest")
)

$ErrorActionPreference = "Stop"
$jit = Split-Path -Parent $PSScriptRoot
Set-Location $jit

$root = Split-Path -Parent $jit

function Try-RunEngineeringRunner {
    param(
        [string]$RepoRoot,
        [string]$OutDir,
        [string]$ValidateTier,
        [int]$HotRuns,
        [string[]]$Models
    )
    $rhCandidates = @(
        (Join-Path $RepoRoot "target/release/regress-harness.exe"),
        (Join-Path $RepoRoot "target/debug/regress-harness.exe")
    )
    $rh = $rhCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $rh) { return $false }

    $libPath = Join-Path $RepoRoot "jit-compiler"
    $resolvedOut = (Resolve-Path -LiteralPath $OutDir).Path
    $modelList = $Models -join ","
    & $rh jit validate-perf --out-dir $resolvedOut --validate-tier $ValidateTier --validation-mode full --hot-runs $HotRuns --lib-path $libPath --models $modelList
    exit $LASTEXITCODE
}
$exe = $null
$candidates = New-Object System.Collections.Generic.List[string]

if (-not [string]::IsNullOrWhiteSpace($CargoTargetSubdir)) {
    $sub = $CargoTargetSubdir.Trim().TrimStart('\', '/')
    foreach ($rel in @(
            (Join-Path $sub "release/rustmodlica"),
            (Join-Path $sub "release/rustmodlica.exe"),
            (Join-Path $sub "debug/rustmodlica"),
            (Join-Path $sub "debug/rustmodlica.exe")
        )) {
        [void]$candidates.Add((Join-Path $jit $rel))
    }
}

foreach ($c in @(
        (Join-Path $root "target/release/rustmodlica"),
        (Join-Path $root "target/release/rustmodlica.exe"),
        (Join-Path $root "target/debug/rustmodlica"),
        (Join-Path $root "target/debug/rustmodlica.exe")
    )) {
    [void]$candidates.Add($c)
}

foreach ($c in $candidates) {
    if (Test-Path -LiteralPath $c) {
        $exe = $c
        break
    }
}

if (-not $exe) {
    Write-Error "rustmodlica binary not found. Tried CargoTargetSubdir='$CargoTargetSubdir' under jit-compiler and workspace target/release|debug."
    exit 1
}

if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Join-Path $root "build/perf_bench"
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# Prefer engineered runner when available; fall back to legacy PS1 logic otherwise.
Try-RunEngineeringRunner -RepoRoot $root -OutDir $OutDir -ValidateTier $ValidateTier -HotRuns $HotRuns -Models $Models | Out-Null

$testLibDir = Join-Path $jit "TestLib"
$mslParent = $jit

function Get-LibArgs {
    $args = New-Object System.Collections.Generic.List[string]
    if (Test-Path -LiteralPath (Join-Path $mslParent "Modelica/package.mo")) {
        [void]$args.Add("--lib-path=$mslParent")
    }
    [void]$args.Add("--lib-path=$testLibDir")
    [void]$args.Add("--lib-path=$jit")
    return $args
}

function Invoke-ValidatePerf {
    param(
        [string]$ModelName,
        [string]$PerfPath,
        [string]$StatsPath
    )
    $libArgs = Get-LibArgs
    $cmd = @(
        "--validate",
        "--validate-tier=$ValidateTier",
        "--perf-json=$PerfPath"
    ) + $libArgs + @($ModelName)

    $oldEa = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $oldStats = $env:RUSTMODLICA_CACHE_STATS_JSON
    if (-not [string]::IsNullOrWhiteSpace($StatsPath)) {
        $env:RUSTMODLICA_CACHE_STATS_JSON = $StatsPath
    }
    $raw = & $exe @cmd 2>&1 | Out-String
    $code = $LASTEXITCODE
    if ($null -ne $oldStats) {
        $env:RUSTMODLICA_CACHE_STATS_JSON = $oldStats
    }
    else {
        Remove-Item Env:RUSTMODLICA_CACHE_STATS_JSON -ErrorAction SilentlyContinue
    }
    $ErrorActionPreference = $oldEa

    if ($code -ne 0) {
        Write-Error "validate failed: model=$ModelName exit=$code perf_json=$PerfPath`n$raw"
        exit $code
    }
}

function Run-Scenario {
    param(
        [string]$ScenarioName,
        [scriptblock]$SetupEnv,
        [int]$Runs
    )
    & $SetupEnv
    foreach ($m in $Models) {
        for ($i = 1; $i -le $Runs; $i++) {
            $out = Join-Path $OutDir ("perf_{0}_{1}_{2}.json" -f $ScenarioName, $m, $i)
            $stats = Join-Path $OutDir ("cache_stats_{0}_{1}_{2}.json" -f $ScenarioName, $m, $i)
            Invoke-ValidatePerf -ModelName $m -PerfPath $out -StatsPath $stats
        }
    }
}

Write-Host ("rustmodlica: {0}" -f $exe)
Write-Host ("out_dir: {0}" -f $OutDir)
Write-Host ("validate_tier: {0}" -f $ValidateTier)
Write-Host ("models: {0}" -f ($Models -join ","))

Run-Scenario -ScenarioName "cold_empty_nsCOLD" -Runs 1 -SetupEnv {
    $cacheDir = Join-Path $OutDir "cache_cold_empty_nsCOLD"
    Remove-Item -LiteralPath $cacheDir -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null
    Remove-Item Env:RUSTMODLICA_QUERY_CACHE -ErrorAction SilentlyContinue
    $env:RUSTMODLICA_QUERY_CACHE_NAMESPACE = "COLD"
    Remove-Item Env:RUSTMODLICA_SALSA -ErrorAction SilentlyContinue
    $env:RUSTMODLICA_CACHE_SQLITE = "1"
    $env:RUSTMODLICA_FLATTEN_CACHE_DIR = $cacheDir
}

Run-Scenario -ScenarioName "cold_qcache0" -Runs 1 -SetupEnv {
    $cacheDir = Join-Path $OutDir "cache_cold_qcache0"
    New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null
    $env:RUSTMODLICA_QUERY_CACHE = "0"
    Remove-Item Env:RUSTMODLICA_QUERY_CACHE_NAMESPACE -ErrorAction SilentlyContinue
    Remove-Item Env:RUSTMODLICA_SALSA -ErrorAction SilentlyContinue
    $env:RUSTMODLICA_CACHE_SQLITE = "1"
    $env:RUSTMODLICA_FLATTEN_CACHE_DIR = $cacheDir
}

Run-Scenario -ScenarioName "hot_nsA" -Runs $HotRuns -SetupEnv {
    $cacheDir = Join-Path $OutDir "cache_hot_nsA"
    New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null
    Remove-Item Env:RUSTMODLICA_QUERY_CACHE -ErrorAction SilentlyContinue
    $env:RUSTMODLICA_QUERY_CACHE_NAMESPACE = "A"
    Remove-Item Env:RUSTMODLICA_SALSA -ErrorAction SilentlyContinue
    $env:RUSTMODLICA_CACHE_SQLITE = "1"
    $env:RUSTMODLICA_FLATTEN_CACHE_DIR = $cacheDir
}

Run-Scenario -ScenarioName "legacy_salsa0" -Runs 1 -SetupEnv {
    $cacheDir = Join-Path $OutDir "cache_legacy_salsa0"
    New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null
    Remove-Item Env:RUSTMODLICA_QUERY_CACHE -ErrorAction SilentlyContinue
    Remove-Item Env:RUSTMODLICA_QUERY_CACHE_NAMESPACE -ErrorAction SilentlyContinue
    $env:RUSTMODLICA_SALSA = "0"
    $env:RUSTMODLICA_CACHE_SQLITE = "1"
    $env:RUSTMODLICA_FLATTEN_CACHE_DIR = $cacheDir
    $env:RUSTMODLICA_FLATTEN_FULL_CACHE = "1"
}

Write-Host "done"

