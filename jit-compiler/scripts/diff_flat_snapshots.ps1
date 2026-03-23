# Compare two Tier S JSON files (line-oriented diff; UTF-8).
param(
    [Parameter(Mandatory = $true)][string]$A,
    [Parameter(Mandatory = $true)][string]$B
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path -LiteralPath $A)) { Write-Error "Missing A: $A"; exit 2 }
if (-not (Test-Path -LiteralPath $B)) { Write-Error "Missing B: $B"; exit 2 }
$ca = Get-Content -LiteralPath $A -Raw -Encoding utf8
$cb = Get-Content -LiteralPath $B -Raw -Encoding utf8
if ($ca -eq $cb) { exit 0 }
Write-Host "Mismatch: $A vs $B"
$la = $ca -split "`n"
$lb = $cb -split "`n"
$max = [Math]::Max($la.Count, $lb.Count)
$shown = 0
for ($i = 0; $i -lt $max; $i++) {
    $a = if ($i -lt $la.Count) { $la[$i] } else { "" }
    $b = if ($i -lt $lb.Count) { $lb[$i] } else { "" }
    if ($a -ne $b) {
        Write-Host "Line $($i+1):"
        Write-Host "  < $a"
        Write-Host "  > $b"
        $shown++
        if ($shown -ge 40) {
            Write-Host "... (truncated)"
            break
        }
    }
}
exit 1
