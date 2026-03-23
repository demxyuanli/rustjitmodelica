param(
  [string]$MslRoot = "",
  [string]$ModelicaTestRoot = "",
  [switch]$IncludeResources,
  [bool]$IncludeModelicaExamples = $true,
  [string]$ModelicaExamplesSubdirs = "Fluid/Examples;Mechanics/MultiBody/Examples"
)

$ErrorActionPreference = "Stop"

function Resolve-MslRoot {
  param([string]$Root)
  if (-not [string]::IsNullOrWhiteSpace($Root)) { return $Root }
  if ($env:MSL_ROOT -and -not [string]::IsNullOrWhiteSpace($env:MSL_ROOT)) { return $env:MSL_ROOT }
  $default = "C:\Users\85332\AppData\Local\modai-ide\data\libraries\2e8f486bf49f3718cdaf60de"
  return $default
}

function Resolve-ModelicaTestRoot {
  param([string]$Root)
  if (-not [string]::IsNullOrWhiteSpace($Root)) { return $Root }
  $default = Join-Path $PSScriptRoot "..\ModelicaTest"
  return $default
}

function Test-ValidateSuccess {
  param([string]$Out, [int]$ExitCode)
  if ($Out -match '"success"\s*:\s*true\b') { return $true }
  if ($Out -match '"success"\s*:\s*false\b') { return $false }
  return ($ExitCode -eq 0)
}

$msl = Resolve-MslRoot -Root $MslRoot
if (-not (Test-Path (Join-Path $msl "Modelica\package.mo"))) {
  throw "MSL root does not contain Modelica\package.mo: $msl"
}

$mt = Resolve-ModelicaTestRoot -Root $ModelicaTestRoot
if (-not (Test-Path (Join-Path $mt "package.mo"))) {
  throw "ModelicaTest root does not contain package.mo: $mt"
}

# Prefer direct exe for stable validate loop
$exePath = Join-Path $PSScriptRoot "..\..\target\release\rustmodlica.exe"
$exe = $null
if (Test-Path $exePath) {
  $exe = (Resolve-Path $exePath).Path
} else {
  $exe = "cargo"
}

function PathToQualifiedName {
  param([string]$FullPath, [string]$RootDir, [string]$PackagePrefix)
  $rel = $FullPath.Substring($RootDir.Length).TrimStart("\","/")
  $rel = $rel -replace "\\", "/"
  if ($rel.EndsWith(".mo")) { $rel = $rel.Substring(0, $rel.Length - 3) }
  $rel = $rel.TrimEnd("/")
  $parts = $rel.Split("/") | Where-Object { $_ -ne "" }
  if ($parts.Length -eq 0) { return $null }
  # Drop trailing "package" if caller accidentally passes package.mo
  if ($parts[$parts.Length - 1] -eq "package") { return $null }
  return ($PackagePrefix + ($parts -join "."))
}

$cases = @()
$mtAbs = (Resolve-Path $mt).Path
$mtFiles = Get-ChildItem -Path $mt -Recurse -Filter "*.mo" -File | Where-Object { $_.Name -ne "package.mo" }
if (-not $IncludeResources) {
  $mtFiles = $mtFiles | Where-Object { $_.FullName -notmatch "\\Resources\\|/Resources/" }
}
foreach ($f in $mtFiles) {
  $q = PathToQualifiedName -FullPath $f.FullName -RootDir $mtAbs -PackagePrefix "ModelicaTest."
  if ($q) { $cases += $q }
}

if ($IncludeModelicaExamples) {
  $mRoot = Join-Path $PSScriptRoot "..\Modelica"
  if (-not (Test-Path (Join-Path $mRoot "package.mo"))) {
    throw "Modelica root does not contain package.mo: $mRoot"
  }

  $mRootAbs = (Resolve-Path $mRoot).Path
  $subdirs = @()
  if (-not [string]::IsNullOrWhiteSpace($ModelicaExamplesSubdirs)) {
    $subdirs = $ModelicaExamplesSubdirs.Split(";") | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" }
  }

  foreach ($sub in $subdirs) {
    $subPath = Join-Path $mRootAbs $sub
    if (-not (Test-Path $subPath)) {
      Write-Host ("Skip missing Modelica examples subdir: " + $subPath)
      continue
    }

    $subFiles = Get-ChildItem -Path $subPath -Recurse -Filter "*.mo" -File | Where-Object { $_.Name -ne "package.mo" }
    if (-not $IncludeResources) {
      $subFiles = $subFiles | Where-Object { $_.FullName -notmatch "\\Resources\\|/Resources/" }
    }

    foreach ($f in $subFiles) {
      $q = PathToQualifiedName -FullPath $f.FullName -RootDir $mRootAbs -PackagePrefix "Modelica."
      if ($q) { $cases += $q }
    }
  }
}

$cases = $cases | Sort-Object -Unique

Write-Host ("Validation cases: " + $cases.Count)

$ok = 0
$fail = 0
$firstFailure = $null

foreach ($name in $cases) {
  Write-Host "=== validate $name"
  $runArgs = @("--lib-path=$msl", "--validate", $name)
  $out = ""
  $exit = 0
  try {
    if ($exe -eq "cargo") {
      $outLines = & cargo run --release -- @runArgs 2>&1
      $out = ($outLines -join "`n")
      $exit = $LASTEXITCODE
    } else {
      $outLines = & $exe @runArgs 2>&1
      $out = ($outLines -join "`n")
      $exit = $LASTEXITCODE
    }
  } catch {
    $out = $_.ToString()
    $exit = 1
  }

  $success = Test-ValidateSuccess -Out $out -ExitCode $exit
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
}

Write-Host ""
Write-Host ("Summary: pass={0} fail={1}" -f $ok, $fail)
if ($null -ne $firstFailure) {
  Write-Host ("First failure: " + $firstFailure)
}
if ($fail -gt 0) { exit 1 }
exit 0

