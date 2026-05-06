# Buckets build_modelica_dir_regress/summary.txt lines by trailing reason=<token>.
# Usage: pwsh -File bucket_dir_summary.ps1 [path\to\summary.txt]
param(
    [string]$SummaryPath = (Join-Path (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent) "build_modelica_dir_regress\summary.txt")
)
if (-not (Test-Path -LiteralPath $SummaryPath)) {
    Write-Error "Summary not found: $SummaryPath"
    exit 2
}
$byReason = @{}
Get-Content -LiteralPath $SummaryPath | ForEach-Object {
    if ($_ -match 'reason=(\S+)\s*$') {
        $r = $Matches[1]
        if (-not $byReason.ContainsKey($r)) { $byReason[$r] = 0 }
        $byReason[$r]++
    }
}
$total = ($byReason.Values | Measure-Object -Sum).Sum
Write-Output "total_reason_lines=$total"
$byReason.GetEnumerator() | Sort-Object Value -Descending | ForEach-Object {
    $pct = if ($total -gt 0) { [math]::Round(100.0 * $_.Value / $total, 1) } else { 0 }
    Write-Output ("{0}={1} ({2}%)" -f $_.Key, $_.Value, $pct)
}
