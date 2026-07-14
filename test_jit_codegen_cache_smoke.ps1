# Smoke: RUSTMODLICA_JIT_CODEGEN_CACHE with two fresh rustmodlica processes.
# - Run 1: expect CODEGEN_CACHE_WRITE (JIT compile + persist bytes).
# - Run 2: on Unix, expect CODEGEN_CACHE_HIT if load is supported; on Windows, load is disabled
#   (machine code is not relocatable across processes), so expect FUNC_CACHE_MISS / re-JIT.

# Native stderr from rustmodlica (e.g. [jit]) must not trigger terminating errors.
$ErrorActionPreference = "Continue"
$repo = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = Join-Path $repo "target\release\rustmodlica.exe"
if (-not (Test-Path $exe)) {
    Write-Host "Building rustmodlica release..."
    Push-Location $repo
    cargo build -p rustmodlica --release -j 8
    Pop-Location
}
$lib = Join-Path $repo "jit-compiler\TestLib"
$csv1 = Join-Path $repo "target\jit_codegen_smoke_1.csv"
$csv2 = Join-Path $repo "target\jit_codegen_smoke_2.csv"
$log1 = Join-Path $repo "target\jit_codegen_smoke_run1.log"
$log2 = Join-Path $repo "target\jit_codegen_smoke_run2.log"
$cache = Join-Path $env:LOCALAPPDATA "rustmodlica\jit-codegen"
if (Test-Path $cache) {
    Get-ChildItem $cache -File -ErrorAction SilentlyContinue | Remove-Item -Force
}

$env:RUSTMODLICA_JIT_CODEGEN_CACHE = "1"
Remove-Item $csv1, $csv2, $log1, $log2 -ErrorAction SilentlyContinue

& $exe --lib-path=$lib --t-end=0.5 --result-file=$csv1 BouncingBall *> $log1
if ($LASTEXITCODE -ne 0) {
    throw "run1 failed exit=$LASTEXITCODE"
}
& $exe --lib-path=$lib --t-end=0.5 --result-file=$csv2 BouncingBall *> $log2
if ($LASTEXITCODE -ne 0) {
    throw "run2 failed exit=$LASTEXITCODE"
}

$t1 = Get-Content $log1 -Raw
$t2 = Get-Content $log2 -Raw
if ($t1 -notmatch "CODEGEN_CACHE_WRITE") { throw "run1: expected CODEGEN_CACHE_WRITE in log" }
Write-Host "run1: OK (saw CODEGEN_CACHE_WRITE)"

if ($IsWindows -or $env:OS -match "Windows") {
    if ($t2 -match "CODEGEN_CACHE_HIT") { throw "run2: Windows must not use disk load; unexpected CODEGEN_CACHE_HIT" }
    if ($t2 -notmatch "FUNC_CACHE_MISS") { Write-Host "run2: note — expected FUNC_CACHE_MISS on Windows" }
    Write-Host "run2: OK (Windows: disk load disabled; results compared below)"
} else {
    if ($t2 -notmatch "CODEGEN_CACHE_HIT") { throw "run2: expected CODEGEN_CACHE_HIT on Unix" }
    Write-Host "run2: OK (saw CODEGEN_CACHE_HIT)"
}

$h1 = Get-FileHash $csv1 -Algorithm SHA256
$h2 = Get-FileHash $csv2 -Algorithm SHA256
if ($h1.Hash -ne $h2.Hash) {
    throw "CSV SHA256 mismatch: $($h1.Hash) vs $($h2.Hash)"
}
Write-Host "CSV outputs match (SHA256 $($h1.Hash.Substring(0,16))...)."
