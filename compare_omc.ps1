# REG-2: Compare rustmodlica simulation output with OMC (or two CSV files).
# Usage:
#   .\compare_omc.ps1 [Model] [T_END] [DT]
#   .\compare_omc.ps1 -Model TestLib/InitDummy -TEnd 10 -Dt 0.01 -OmcOut omc_out.csv
#   .\compare_omc.ps1 -Models @("TestLib/InitDummy","TestLib/ClockedTwoRates") -TEnd 1.2 -Dt 0.05 -JsonSummary summary.json
# If -OmcOut is not set, only rustmodlica is run and rust_out.csv is written.
# If -OmcOut is set and the file exists, last row (final state) is compared and max diff reported.
param(
    [string]$Model = "TestLib/InitDummy",
    [string[]]$Models = @(),
    [double]$TEnd = 10.0,
    [double]$Dt = 0.01,
    [string]$RustOut = "rust_out.csv",
    [string]$OmcOut = "",
    [string[]]$RustArgs = @(),
    [string]$JsonSummary = ""
)

$exe = Join-Path $PSScriptRoot "target\\release\\rustmodlica.exe"
if (-not (Test-Path $exe)) {
    Write-Error "Build first: cargo build --release"
    exit 1
}

function Run-OneCompare {
    param(
        [string]$M,
        [string]$OutCsv,
        [string]$OmcPath
    )
    Push-Location (Join-Path $PSScriptRoot "jit-compiler")
    Write-Host "Running rustmodlica: $M t_end=$TEnd dt=$Dt -> $OutCsv"
    & $exe @RustArgs --solver=rk4 --dt=$Dt --t-end=$TEnd --result-file=$OutCsv $M
    $code = $LASTEXITCODE
    Pop-Location
    if ($code -ne 0) {
        return @{ ok = $false; model = $M; exit = $code; maxDiff = $null }
    }
    if ($OmcPath -eq "" -or -not (Test-Path $OmcPath)) {
        return @{ ok = $true; model = $M; exit = 0; maxDiff = $null; note = "no OMC csv" }
    }
    $rustLines = Get-Content $OutCsv
    $omcLines = Get-Content $OmcPath
    if ($rustLines.Count -lt 2 -or $omcLines.Count -lt 2) {
        return @{ ok = $true; model = $M; exit = 0; maxDiff = 0; note = "short csv" }
    }
    $rustLast = ($rustLines[-1] -split ",").Trim()
    $omcLast = ($omcLines[-1] -split ",").Trim()
    $n = [Math]::Min($rustLast.Count, $omcLast.Count)
    $maxDiff = 0.0
    $maxIdx = -1
    for ($j = 1; $j -lt $n; $j++) {
        $a = 0.0; $b = 0.0
        [double]::TryParse($rustLast[$j], [ref]$a) | Out-Null
        [double]::TryParse($omcLast[$j], [ref]$b) | Out-Null
        $diff = [Math]::Abs($a - $b)
        if ($diff -gt $maxDiff) {
            $maxDiff = $diff
            $maxIdx = $j
        }
    }
    Write-Host "Comparison (last row): $M vs $OmcPath -> max abs diff = $maxDiff (col $maxIdx)"
    return @{ ok = $true; model = $M; exit = 0; maxDiff = $maxDiff; maxCol = $maxIdx }
}

$results = @()
if ($Models.Count -gt 0) {
    $i = 0
    foreach ($m in $Models) {
        $suffix = if ($i -eq 0) { "" } else { "_$i" }
        $out = Join-Path $PSScriptRoot ("rust_out{0}.csv" -f $suffix)
        $results += Run-OneCompare -M $m -OutCsv $out -OmcPath $OmcOut
        $i++
    }
} else {
    $results += Run-OneCompare -M $Model -OutCsv (Join-Path $PSScriptRoot $RustOut) -OmcPath $OmcOut
}

if ($JsonSummary -ne "") {
    $results | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 $JsonSummary
}

if ($Models.Count -eq 0 -and ($OmcOut -eq "" -or -not (Test-Path $OmcOut))) {
    if ($OmcOut -eq "") {
        Write-Host "Done. To compare with OMC: run OMC, export CSV to a file, then:"
        Write-Host "  .\compare_omc.ps1 -Model $Model -TEnd $TEnd -Dt $Dt -OmcOut <path_to_omc_csv>"
    } else {
        Write-Host "OMC file not found: $OmcOut. Skipping comparison."
    }
}

$bad = @($results | Where-Object { -not $_.ok })
if ($bad.Count -gt 0) { exit 1 }
exit 0
