param(
    [Parameter(Mandatory = $true)]
    [string]$NdjsonPath,
    [string]$OutList = "",
    [string]$OutSimFailedSkips = ""
)

if (-not (Test-Path -LiteralPath $NdjsonPath)) {
    Write-Error "ndjson not found: $NdjsonPath"
    exit 2
}

$rows = @()
Get-Content -LiteralPath $NdjsonPath | ForEach-Object {
    try {
        $o = $_ | ConvertFrom-Json
        if ($null -ne $o -and $o.case_type -eq "DIR_MODEL" -and $o.status -eq "FAILED") {
            $rows += $o
        }
    } catch {}
}

Write-Output ("failed_total=" + $rows.Count)
Write-Output "--- reasons ---"
$rows | Group-Object reason | Sort-Object Count -Descending | ForEach-Object {
    Write-Output ("{0}`t{1}" -f $_.Count, $_.Name)
}

if (-not [string]::IsNullOrWhiteSpace($OutList)) {
    $dir = Split-Path -Parent $OutList
    if (-not [string]::IsNullOrWhiteSpace($dir) -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    }
    $rows | ForEach-Object { $_.case_name } | Sort-Object -Unique | Set-Content -LiteralPath $OutList -Encoding UTF8
    Write-Output ("wrote_models=" + $OutList)
}

if (-not [string]::IsNullOrWhiteSpace($OutSimFailedSkips)) {
    $dir2 = Split-Path -Parent $OutSimFailedSkips
    if (-not [string]::IsNullOrWhiteSpace($dir2) -and -not (Test-Path -LiteralPath $dir2)) {
        New-Item -ItemType Directory -Force -Path $dir2 | Out-Null
    }
    $rows |
        Where-Object { $_.reason -eq "sim_failed" } |
        ForEach-Object { "-- " + $_.case_name } |
        Sort-Object -Unique |
        Set-Content -LiteralPath $OutSimFailedSkips -Encoding UTF8
    Write-Output ("wrote_sim_failed_skips=" + $OutSimFailedSkips)
}
