# REG-2: Compare rustmodlica simulation output with OMC (or two CSV files).
# Usage:
#   .\compare_omc.ps1 [Model] [T_END] [DT]
#   .\compare_omc.ps1 -Model TestLib/InitDummy -TEnd 10 -Dt 0.01 -OmcOut omc_out.csv
# If -OmcOut is not set, only rustmodlica is run and rust_out.csv is written.
# If -OmcOut is set and the file exists, last row (final state) is compared and max diff reported.
param(
    [string]$Model = "TestLib/InitDummy",
    [double]$TEnd = 10.0,
    [double]$Dt = 0.01,
    [string]$RustOut = "rust_out.csv",
    [string]$OmcOut = ""
)

$exe = ".\target\release\rustmodlica.exe"
if (-not (Test-Path $exe)) {
    Write-Error "Build first: cargo build --release"
    exit 1
}

Write-Host "Running rustmodlica: $Model t_end=$TEnd dt=$Dt -> $RustOut"
& $exe --solver=rk4 --dt=$Dt --t-end=$TEnd --result-file=$RustOut $Model
if ($LASTEXITCODE -ne 0) {
    Write-Error "rustmodlica failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

if ($OmcOut -eq "" -or -not (Test-Path $OmcOut)) {
    if ($OmcOut -eq "") {
        Write-Host "Done. To compare with OMC: run OMC, export CSV to a file, then:"
        Write-Host "  .\compare_omc.ps1 -Model $Model -TEnd $TEnd -Dt $Dt -OmcOut <path_to_omc_csv>"
    } else {
        Write-Host "OMC file not found: $OmcOut. Skipping comparison."
    }
    exit 0
}

# Compare last row of rust_out.csv and omc_out.csv (numeric columns only)
$rustLines = Get-Content $RustOut
$omcLines = Get-Content $OmcOut
if ($rustLines.Count -lt 2 -or $omcLines.Count -lt 2) {
    Write-Host "One or both CSVs have no data rows. Skipping comparison."
    exit 0
}
$rustLast = ($rustLines[-1] -split ",").Trim()
$omcLast = ($omcLines[-1] -split ",").Trim()
# Assume first column is time; rest are numeric
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
Write-Host "Comparison (last row): rust vs $OmcOut -> max absolute diff = $maxDiff (column index $maxIdx)"
exit 0
