param(
    [string]$Exe = "D:\source\repos\rustmodlica\jit-compiler\target_regression\release\rustmodlica.exe",
    [string]$JitRoot = "D:\source\repos\rustmodlica\jit-compiler",
    [string]$Model = "Modelica.Blocks.Sources.Sine",
    [int]$TimeoutSec = 90
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Exe)) {
    Write-Error "exe missing: $Exe"
    exit 2
}

# Inject sundials DLL dir from <profile>/build/sundials-sys-*/out/lib
$exeResolved = (Resolve-Path -LiteralPath $Exe).Path
$profileDir = Split-Path -Parent $exeResolved
$buildDir = Join-Path $profileDir "build"
if (Test-Path -LiteralPath $buildDir) {
    $dll = @(Get-ChildItem -LiteralPath $buildDir -Directory -Filter "sundials-sys-*" |
        Sort-Object LastWriteTime -Descending |
        ForEach-Object { Join-Path $_.FullName "out\lib" } |
        Where-Object { Test-Path -LiteralPath $_ }) | Select-Object -First 1
    if ($dll) {
        $env:PATH = "$dll;$env:PATH"
        Write-Host "[smoke] PATH+= $dll"
    }
}

$cliArgs = @(
    "--lib-path=$JitRoot\Modelica",
    "--lib-path=$JitRoot\ModelicaTest",
    "--validate",
    "--validate-tier=analyze",
    "--validation-mode=full",
    $Model
)

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $Exe
$psi.WorkingDirectory = $JitRoot
$psi.UseShellExecute = $false
$psi.Arguments = ($cliArgs -join ' ')
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$p = New-Object System.Diagnostics.Process
$p.StartInfo = $psi
[void]$p.Start()
$so = $p.StandardOutput.ReadToEnd()
$se = $p.StandardError.ReadToEnd()
if (-not $p.WaitForExit([Math]::Max(1, $TimeoutSec) * 1000)) {
    try { $p.Kill() } catch {}
    Write-Host "[smoke] TIMEOUT model=$Model"
    Write-Host "STDOUT:"; Write-Host $so
    Write-Host "STDERR:"; Write-Host $se
    exit 124
}
Write-Host ("[smoke] ExitCode=" + $p.ExitCode + " model=" + $Model)
if ($p.ExitCode -ne 0) {
    Write-Host "STDOUT:"; Write-Host $so
    Write-Host "STDERR:"; Write-Host $se
}
exit $p.ExitCode
