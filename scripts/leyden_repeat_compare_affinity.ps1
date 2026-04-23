param(
    [int]$Rounds = 10,
    [string]$OutRoot = "build_regression_logs/leyden_repeat_compare_affinity",
    [string]$AffinityMaskHex = "0x0000000F"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $OutRoot)) {
    New-Item -ItemType Directory -Path $OutRoot | Out-Null
}

$orig = powercfg /getactivescheme
$orig | Set-Content -Encoding UTF8 (Join-Path $OutRoot "power_plan_before.txt")
$origGuid = ([regex]::Match($orig, "([A-Fa-f0-9\\-]{36})")).Groups[1].Value

# High Performance scheme GUID
$hpGuid = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"

try {
    powercfg /setactive $hpGuid | Out-Null
} catch {}

try {
    powercfg /setacvalueindex scheme_current sub_processor PROCTHROTTLEMIN 100 | Out-Null
    powercfg /setacvalueindex scheme_current sub_processor PROCTHROTTLEMAX 100 | Out-Null
    powercfg /setdcvalueindex scheme_current sub_processor PROCTHROTTLEMIN 100 | Out-Null
    powercfg /setdcvalueindex scheme_current sub_processor PROCTHROTTLEMAX 100 | Out-Null
    powercfg /setactive scheme_current | Out-Null
} catch {}

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "powershell"
$psi.Arguments = "-NoProfile -File ""scripts/leyden_repeat_compare.ps1"" -Rounds $Rounds -OutRoot ""$OutRoot"""
$psi.WorkingDirectory = (Resolve-Path ".").Path
$psi.UseShellExecute = $false

$p = [System.Diagnostics.Process]::Start($psi)
$p.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::High
$p.ProcessorAffinity = [intptr]::new([convert]::ToInt64($AffinityMaskHex, 16))
$p.WaitForExit()
$code = $p.ExitCode

if ($origGuid) {
    try { powercfg /setactive $origGuid | Out-Null } catch {}
}
powercfg /getactivescheme | Set-Content -Encoding UTF8 (Join-Path $OutRoot "power_plan_after.txt")

exit $code
