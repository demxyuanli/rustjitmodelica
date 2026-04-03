# Smoke test: DIR private cache is reused across two regression out-dirs and by a standalone --validate run.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run_dir_cache_shared_smoke.ps1
# Requires: rustmodlica.exe under target/release or jit-compiler/target_regression/release.

param(
    [string]$Root = "",
    [int]$MaxCases = 2
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = if (-not [string]::IsNullOrWhiteSpace($Root)) {
    if ([System.IO.Path]::IsPathRooted($Root)) { $Root } else { (Resolve-Path -LiteralPath (Join-Path (Join-Path $PSScriptRoot "..") $Root)).Path }
} else {
    (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
}

$dirScript = Join-Path $repoRoot "run_modelica_dir_regression.ps1"
if (-not (Test-Path -LiteralPath $dirScript)) {
    Write-Error "run_modelica_dir_regression.ps1 not found: $dirScript"
    exit 1
}

$sharedRoot = Join-Path $repoRoot "build\dir_cache_shared_test"
$envScript = Join-Path $repoRoot "build\dir_cache_shared_env.ps1"
New-Item -ItemType Directory -Force -Path $sharedRoot | Out-Null

function Find-RustmodlicaExe {
    param([string]$RepoRoot)
    $candidates = @(
        (Join-Path $RepoRoot "target\release\rustmodlica.exe"),
        (Join-Path $RepoRoot "jit-compiler\target_regression\release\rustmodlica.exe"),
        (Join-Path $RepoRoot "jit-compiler\target\release\rustmodlica.exe")
    )
    foreach ($c in $candidates) {
        if (Test-Path -LiteralPath $c) { return $c }
    }
    return $null
}

$exe = Find-RustmodlicaExe -RepoRoot $repoRoot
if (-not $exe) {
    Write-Error "rustmodlica.exe not found. Build: cargo build -p rustmodlica --release (jit-compiler)."
    exit 1
}
$exeRel = $exe.Substring($repoRoot.Length).TrimStart('\', '/')

Write-Host "repo_root=$repoRoot"
Write-Host "shared_cache_root=$sharedRoot"
Write-Host "rustmodlica=$exeRel"

Write-Host ""
Write-Host "=== Round 1: DIR regression (writes cache + env script) ==="
$sw1 = [System.Diagnostics.Stopwatch]::StartNew()
& powershell -NoProfile -ExecutionPolicy Bypass -File $dirScript `
    -Root $repoRoot `
    -MaxCases $MaxCases `
    -ParallelWorkers 1 `
    -UsePrivateCache `
    -PrivateCacheRoot $sharedRoot `
    -OutDir "build_dir_cache_round1" `
    -ExePath $exeRel `
    -WriteDirCacheEnvScript "build\dir_cache_shared_env.ps1"
$exit1 = $LASTEXITCODE
$sw1.Stop()
Write-Host ("Round1 exit={0} wall_ms={1}" -f $exit1, $sw1.ElapsedMilliseconds)

Write-Host ""
Write-Host "=== Round 2: same PrivateCacheRoot (cache shared), different OutDir ==="
$sw2 = [System.Diagnostics.Stopwatch]::StartNew()
& powershell -NoProfile -ExecutionPolicy Bypass -File $dirScript `
    -Root $repoRoot `
    -MaxCases $MaxCases `
    -ParallelWorkers 1 `
    -UsePrivateCache `
    -PrivateCacheRoot $sharedRoot `
    -OutDir "build_dir_cache_round2" `
    -ExePath $exeRel `
    -WriteDirCacheEnvScript "build\dir_cache_shared_env.ps1"
$exit2 = $LASTEXITCODE
$sw2.Stop()
Write-Host ("Round2 exit={0} wall_ms={1}" -f $exit2, $sw2.ElapsedMilliseconds)

if ($exit1 -ne 0 -or $exit2 -ne 0) {
    Write-Error "DIR round failed (exit1=$exit1 exit2=$exit2)"
    exit 1
}

if (-not (Test-Path -LiteralPath $envScript)) {
    Write-Error "Env script not written: $envScript"
    exit 1
}

Write-Host ""
Write-Host "=== Round 3: dot-source env script + --validate (shares query/flatten cache) ==="
$jitRoot = Join-Path $repoRoot "jit-compiler"
$libM = Join-Path $jitRoot "Modelica"
$libT = Join-Path $jitRoot "ModelicaTest"
. $envScript
Push-Location $jitRoot
try {
    $sw3 = [System.Diagnostics.Stopwatch]::StartNew()
    $vOut = & $exe --validate --validate-tier=analyze --lib-path=$libM --lib-path=$libT "Modelica.Blocks.Sources.Sine" 2>&1
    $vExit = $LASTEXITCODE
    $sw3.Stop()
    Write-Host ("Validate exit={0} wall_ms={1}" -f $vExit, $sw3.ElapsedMilliseconds)
    if ($vExit -ne 0) {
        $vOut | ForEach-Object { Write-Host $_ }
        Write-Error "Standalone validate failed"
        exit 1
    }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "OK: shared cache used across round1/round2 out dirs; validate reused exported env."
Write-Host "Compare wall_ms: round2 should often be <= round1 for same MaxCases (not guaranteed if OS cold)."
exit 0
