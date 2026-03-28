# Batch JIT validate for TestLib.
#
# - Top-level TestLib/*.mo: each basename is validated with --validate; must succeed (success:true).
# - TestLib/negative/*.mo: must fail validation (success:false); unexpected success fails the script.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File jit-compiler/scripts/run_testlib_validate.ps1
#   powershell ... -File ... -CargoTargetSubdir target_regression
#
# Requires: rustmodlica binary (workspace target/, or jit-compiler/<CargoTargetSubdir>/).

param(
    [string]$CargoTargetSubdir = ""
)

$ErrorActionPreference = "Stop"
$jit = Split-Path -Parent $PSScriptRoot
Set-Location $jit

$root = Split-Path -Parent $jit
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

$testLibDir = Join-Path $jit "TestLib"
$negDir = Join-Path $testLibDir "negative"

function Invoke-ValidateRoot {
    param([string]$ModelName)
    $oldEa = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $raw = & $exe --lib-path=$testLibDir --validate $ModelName 2>&1 | Out-String
    $ErrorActionPreference = $oldEa
    return $raw
}

function Invoke-ValidateNegative {
    param([string]$ModelName)
    $oldEa = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    # negative/*.mo plus library connectors/types at TestLib root (e.g. ConnA for BadConnect)
    $raw = & $exe --lib-path=$testLibDir --lib-path=$negDir --validate $ModelName 2>&1 | Out-String
    $ErrorActionPreference = $oldEa
    return $raw
}

$mosRoot = @(Get-ChildItem -LiteralPath $testLibDir -Filter "*.mo" -File | Sort-Object Name)
$mosNeg = @()
if (Test-Path -LiteralPath $negDir) {
    $mosNeg = @(Get-ChildItem -LiteralPath $negDir -Filter "*.mo" -File | Sort-Object Name)
}

$ok = 0
$rootFail = New-Object System.Collections.Generic.List[string]
foreach ($f in $mosRoot) {
    $n = $f.BaseName
    $raw = Invoke-ValidateRoot $n
    if ($raw -match '"success"\s*:\s*true') {
        $ok++
    }
    else {
        [void]$rootFail.Add($n)
    }
}

$negOk = 0
$negUnexpectedPass = New-Object System.Collections.Generic.List[string]
foreach ($f in $mosNeg) {
    $n = $f.BaseName
    $raw = Invoke-ValidateNegative $n
    if ($raw -match '"success"\s*:\s*true') {
        [void]$negUnexpectedPass.Add($n)
    }
    else {
        $negOk++
    }
}

Write-Host "rustmodlica: $exe"
Write-Host "TestLib root .mo (expect PASS): $($mosRoot.Count)"
Write-Host "TestLib/negative .mo (expect FAIL): $($mosNeg.Count)"
Write-Host "PASS (root): $ok"
if ($rootFail.Count -gt 0) {
    Write-Host "FAIL (root, unexpected): $($rootFail.Count)"
    $rootFail | ForEach-Object { Write-Host "  $_" }
}
Write-Host "FAIL-as-expected (negative): $negOk"
if ($negUnexpectedPass.Count -gt 0) {
    Write-Host "PASS (negative, unexpected -- should have failed): $($negUnexpectedPass.Count)"
    $negUnexpectedPass | ForEach-Object { Write-Host "  $_" }
}

$exitBad = ($rootFail.Count -gt 0) -or ($negUnexpectedPass.Count -gt 0)
exit $(if ($exitBad) { 1 } else { 0 })
