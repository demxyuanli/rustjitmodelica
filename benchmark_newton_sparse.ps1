# Compare Newton linear solve: dense vs sparse policy (same model, wall-clock rough timing).
param(
    [string]$Model = "TestLib/SolvableBlockMultiRes",
    [string]$ExePath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = if ($ExePath -ne "") { $ExePath } else { Join-Path $here "target\release\rustmodlica.exe" }
if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Build first: cargo build --release"
    exit 1
}

function Run-Case {
    param([string]$Policy, [string]$OutCsv)
    $env:RUSTMODLICA_NEWTON_SPARSE_POLICY = $Policy
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    Push-Location (Join-Path $here "jit-compiler")
    $prevEa = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $null = & $exe --solver=rk4 --dt=0.01 --t-end=5.0 --result-file=$OutCsv $Model 2>&1
    $ErrorActionPreference = $prevEa
    $code = $LASTEXITCODE
    $sw.Stop()
    Pop-Location
    Remove-Item Env:RUSTMODLICA_NEWTON_SPARSE_POLICY -ErrorAction SilentlyContinue
    [pscustomobject]@{ Policy = $Policy; ExitCode = $code; Ms = $sw.ElapsedMilliseconds }
}

$out1 = Join-Path $here "build_newton_bench_dense.csv"
$out2 = Join-Path $here "build_newton_bench_sparse.csv"
$r1 = Run-Case -Policy "dense" -OutCsv $out1
$r2 = Run-Case -Policy "sparse" -OutCsv $out2
Write-Host ($r1 | ConvertTo-Json -Compress)
Write-Host ($r2 | ConvertTo-Json -Compress)
if ($r1.ExitCode -ne 0 -or $r2.ExitCode -ne 0) { exit 1 }
exit 0
