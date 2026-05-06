# Cold-cache comparison: query flatten (SALSA=1, default for validate) vs legacy flatten (SALSA=0).
# Forces cold flat_full by disabling SQLite + SHM tiers for this process (see env below).
# Requires release rustmodlica at repo root target/release/rustmodlica.exe.
param(
    [string]$RepoRoot = "",
    [string]$Model = "Modelica.Electrical.Machines.Examples.ControlledDCDrives.SpeedControlledDCPM"
)
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
}
$exe = Join-Path $RepoRoot "target/release/rustmodlica.exe"
if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Missing $exe (cargo build --release -p rustmodlica)"
    exit 1
}
$outDir = Join-Path $RepoRoot "build_modelica_dir_regress/salsa_flatten_compare"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$jitRoot = Join-Path $RepoRoot "jit-compiler"
$libM = Join-Path $jitRoot "Modelica"
$libT = Join-Path $jitRoot "ModelicaTest"

function Invoke-FlattenTier {
    param([string]$SalsaVal, [string]$CacheDir, [string]$PerfJson, [string]$LogPath)
    if (Test-Path -LiteralPath $CacheDir) {
        Remove-Item -Recurse -Force -LiteralPath $CacheDir
    }
    New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null
    $env:RUSTMODLICA_FLATTEN_CACHE_DIR = $CacheDir
    $env:RUSTMODLICA_SALSA = $SalsaVal
    $env:RUSTMODLICA_STAGE_TRACE = "0"
    $env:RUSTMODLICA_PERF_TRACE = "1"
    # Avoid SHM/SQLite tier hits so timing reflects real decl_expand/eq_expand work.
    $env:RUSTMODLICA_CACHE_SQLITE = "0"
    $env:RUSTMODLICA_CACHE_SHM = "0"
    Remove-Item Env:RUSTMODLICA_ANALYZE_ONLY -ErrorAction SilentlyContinue
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $exitCode = 0
    Push-Location $jitRoot
    try {
        $lines = & $exe `
            "--lib-path=$libM" `
            "--lib-path=$libT" `
            "--index-reduction-method=dummyDerivative" `
            "--validate" `
            "--validate-tier=flatten" `
            "--validation-mode=full" `
            "--perf-json=$PerfJson" `
            $Model 2>&1
        $exitCode = $LASTEXITCODE
        Set-Content -LiteralPath $LogPath -Value $lines -Encoding UTF8
    } finally {
        Pop-Location
    }
    $sw.Stop()
    return @{
        WallMs = [int]$sw.Elapsed.TotalMilliseconds
        ExitCode = $exitCode
    }
}

$salsa1Cache = Join-Path $outDir "cache_salsa1"
$salsa0Cache = Join-Path $outDir "cache_salsa0"
$perf1 = Join-Path $outDir "perf_salsa1.json"
$perf0 = Join-Path $outDir "perf_salsa0.json"
$log1 = Join-Path $outDir "log_salsa1.txt"
$log0 = Join-Path $outDir "log_salsa0.txt"

Write-Host "[compare] cold run SALSA=1 ..."
$r1 = Invoke-FlattenTier -SalsaVal "1" -CacheDir $salsa1Cache -PerfJson $perf1 -LogPath $log1
Write-Host "[compare] cold run SALSA=0 ..."
$r0 = Invoke-FlattenTier -SalsaVal "0" -CacheDir $salsa0Cache -PerfJson $perf0 -LogPath $log0
$wallMs1 = $r1.WallMs
$wallMs0 = $r0.WallMs

function Read-PerfNums([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return $null }
    try {
        $j = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
        $cp = $j.compile_perf
        return [ordered]@{
            flatten_inline_ms = [int]$cp.flatten_inline_ms
            decl_expand_ms    = [int]$cp.decl_expand_ms
            eq_expand_ms      = [int]$cp.eq_expand_ms
        }
    } catch {
        return $null
    }
}

$p1 = Read-PerfNums $perf1
$p0 = Read-PerfNums $perf0
if ($null -eq $p1) { $p1 = [ordered]@{ flatten_inline_ms = -1; decl_expand_ms = -1; eq_expand_ms = -1 } }
if ($null -eq $p0) { $p0 = [ordered]@{ flatten_inline_ms = -1; decl_expand_ms = -1; eq_expand_ms = -1 } }
$summary = [ordered]@{
    model                      = $Model
    wall_ms_salsa1             = $wallMs1
    wall_ms_salsa0             = $wallMs0
    exit_salsa1                = $r1.ExitCode
    exit_salsa0                = $r0.ExitCode
    flatten_inline_ms_salsa1   = $p1.flatten_inline_ms
    flatten_inline_ms_salsa0   = $p0.flatten_inline_ms
    decl_expand_ms_salsa1      = $p1.decl_expand_ms
    decl_expand_ms_salsa0      = $p0.decl_expand_ms
    eq_expand_ms_salsa1        = $p1.eq_expand_ms
    eq_expand_ms_salsa0        = $p0.eq_expand_ms
    perf_salsa1                = $perf1
    perf_salsa0                = $perf0
    env_note                   = "RUSTMODLICA_CACHE_SQLITE=0 RUSTMODLICA_CACHE_SHM=0 for cold flat_full"
    perf_note                  = "When SALSA=0, compile_perf.decl_expand_ms/eq_expand_ms are often 0 (legacy path); compare flatten_inline_ms."
}
$sumPath = Join-Path $outDir "summary.json"
$summary | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $sumPath -Encoding UTF8
Write-Host "[compare] wrote $sumPath"
Write-Host ($summary | ConvertTo-Json -Compress)
if ($r1.ExitCode -ne 0) { exit $r1.ExitCode }
if ($r0.ExitCode -ne 0) { exit $r0.ExitCode }
exit 0
