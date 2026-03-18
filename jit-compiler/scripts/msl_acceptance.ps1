param(
  [string]$MslRoot = "",
  [string]$CasesFile = "scripts\msl_acceptance_cases.txt",
  [switch]$Simulate,
  [double]$TEnd = 0.2,
  [double]$Dt = 0.001
)

$ErrorActionPreference = "Stop"

function Resolve-MslRoot {
  param([string]$Root)
  if (-not [string]::IsNullOrWhiteSpace($Root)) { return $Root }
  if ($env:MSL_ROOT -and -not [string]::IsNullOrWhiteSpace($env:MSL_ROOT)) { return $env:MSL_ROOT }
  $default = "C:\Users\85332\AppData\Local\modai-ide\data\libraries\2e8f486bf49f3718cdaf60de"
  return $default
}

$msl = Resolve-MslRoot -Root $MslRoot
if (-not (Test-Path (Join-Path $msl "Modelica\package.mo"))) {
  throw "MSL root does not contain Modelica\package.mo: $msl"
}
if (-not (Test-Path $CasesFile)) {
  throw "Cases file not found: $CasesFile"
}

# Prefer direct exe for stable validate loop (plan: driver-loop-stabilize)
$exePath = Join-Path $PSScriptRoot "..\..\target\release\rustmodlica.exe"
$exe = $null
if (Test-Path $exePath) {
  $exe = (Resolve-Path $exePath).Path
} else {
  $exe = "cargo"
}

$cases = Get-Content $CasesFile | Where-Object { $_ -and -not $_.Trim().StartsWith("#") } | ForEach-Object { $_.Trim() }

$ok = 0
$fail = 0
$firstFailure = $null
$results = @()

function Test-ValidateSuccess {
  param([string]$Out, [int]$ExitCode)
  if ($Out -match '"success"\s*:\s*true\b') { return $true }
  if ($Out -match '"success"\s*:\s*false\b') { return $false }
  return ($ExitCode -eq 0)
}

foreach ($line in $cases) {
  $parts = @($line.Split(";") | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" })
  $name = $parts[0]
  $mode = "validate"
  foreach ($p in $parts) {
    if ($p.StartsWith("mode=", [System.StringComparison]::OrdinalIgnoreCase)) {
      $mode = $p.Substring(5)
    }
  }
  if ($Simulate) { $mode = "simulate" }

  $args = @("--lib-path=$msl")
  if ($mode -eq "validate") {
    $args += "--validate"
    $args += $name
  } else {
    $args += "--t-end=$TEnd"
    $args += "--dt=$Dt"
    $args += $name
  }

  Write-Host "=== $mode $name"
  $out = ""
  $exit = 0
  try {
    if ($exe -eq "cargo") {
      $outLines = & cargo run --release -- @args 2>&1
      $out = ($outLines -join "`n")
      $exit = $LASTEXITCODE
    } else {
      $outLines = & $exe @args 2>&1
      $out = ($outLines -join "`n")
      $exit = $LASTEXITCODE
    }
  } catch {
    $out = $_.ToString()
    $exit = 1
  }

  $success = $false
  if ($mode -eq "validate") {
    $success = Test-ValidateSuccess -Out $out -ExitCode $exit
  } else {
    $success = ($exit -eq 0)
  }

  if ($success) {
    $ok++
    Write-Host "PASS"
  } else {
    $fail++
    if ($null -eq $firstFailure) { $firstFailure = $name }
    Write-Host "FAIL"
    Write-Host ("exit=" + $exit)
    Write-Host $out
  }

  $results += [pscustomobject]@{
    name = $name
    mode = $mode
    success = $success
    exitCode = $exit
  }
}

Write-Host ""
Write-Host ("Summary: pass={0} fail={1}" -f $ok, $fail)
if ($null -ne $firstFailure) {
  Write-Host ("First failure: " + $firstFailure)
}
if ($fail -gt 0) { exit 1 }
exit 0

