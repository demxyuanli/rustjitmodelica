param(
    [string]$MainSummary = "build_modelica_dir_regress\summary.txt",
    [string]$RerunSummary = "build_modelica_dir_regress_rerun_tmp\summary.txt",
    [string]$MainLogs = "build_modelica_dir_regress\logs",
    [string]$RerunLogs = "build_modelica_dir_regress_rerun_tmp\logs",
    [string]$Root = ""
)
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repoRoot = if ($Root -ne "") { $Root } else { $PSScriptRoot }
$main = Join-Path $repoRoot $MainSummary
$tmpSum = Join-Path $repoRoot $RerunSummary
$mainLogs = Join-Path $repoRoot $MainLogs
$tmpLogs = Join-Path $repoRoot $RerunLogs
if (-not (Test-Path -LiteralPath $main)) { Write-Error "Missing $main"; exit 2 }
if (-not (Test-Path -LiteralPath $tmpSum)) { Write-Error "Missing $tmpSum"; exit 2 }
$old = [System.IO.File]::ReadAllLines($main)
$unresolved = @{}
foreach ($ln in $old) {
    $t = $ln.Trim()
    if ($t.StartsWith("!!")) {
        $r = $t.Substring(2).TrimStart()
        if ($r -ne "") {
            $n = (($r -split "\s+", 2)[0]).Trim()
            $unresolved[$n] = $true
        }
    }
    elseif ($t.StartsWith("--")) {
        $r = $t.Substring(2).TrimStart()
        if ($r -ne "") {
            $n = (($r -split "\s+", 2)[0]).Trim()
            $unresolved[$n] = $true
        }
    }
}
$newLines = [System.IO.File]::ReadAllLines($tmpSum)
$newMap = @{}
foreach ($nl in $newLines) {
    if ($nl -match "^(OK|!!|--)\s+(\S+)") {
        $newMap[$matches[2]] = $nl
    }
}
$out = New-Object System.Collections.Generic.List[string]
foreach ($ol in $old) {
    if ($ol -match "^(OK|!!|--)\s+(\S+)") {
        $name = $matches[2]
        if ($unresolved.ContainsKey($name) -and $newMap.ContainsKey($name)) {
            $out.Add($newMap[$name])
        }
        else {
            $out.Add($ol)
        }
    }
    else {
        $out.Add($ol)
    }
}
[System.IO.File]::WriteAllLines($main, $out)
if (Test-Path -LiteralPath $tmpLogs) {
    if (-not (Test-Path -LiteralPath $mainLogs)) {
        New-Item -ItemType Directory -Path $mainLogs -Force | Out-Null
    }
    Copy-Item (Join-Path $tmpLogs "*") -Destination $mainLogs -Force
}
Write-Host "Merged: main=$main unresolved_in_old=$($unresolved.Count) rerun_entries=$($newMap.Count) lines_out=$($out.Count)"
