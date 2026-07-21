# REG-2: Compare rustmodlica simulation output with OMC (or two CSV files).
# Usage:
#   .\compare_omc.ps1 [Model] [T_END] [DT]
#   .\compare_omc.ps1 -Model TestLib/InitDummy -TEnd 10 -Dt 0.01 -OmcOut omc_out.csv
#   .\compare_omc.ps1 -Models @("TestLib/InitDummy","TestLib/ClockedTwoRates") -TEnd 1.2 -Dt 0.05 -JsonSummary summary.json
#   .\compare_omc.ps1 -Models $chopper -LibPath <MSL> -OutDir build/em_omc_compare -CrossSolverSelfGate
# If -OmcOut is not set, only rustmodlica is run and rust_out.csv is written.
# If -OmcOut is set and the file exists, last row (final state) is compared and max diff reported.
# If -CrossSolverSelfGate and no OMC CSV: rk4 vs rk45 last-row maxDiff (self-consistency gate).
param(
    [string]$Model = "TestLib/InitDummy",
    [string[]]$Models = @(),
    [double]$TEnd = 10.0,
    [double]$Dt = 0.01,
    [string]$RustOut = "rust_out.csv",
    [string]$OmcOut = "",
    [string[]]$RustArgs = @(),
    [string]$JsonSummary = "",
    [string]$LibPath = "",
    [string]$OutDir = "",
    [string]$Solver = "rk4",
    [switch]$CrossSolverSelfGate
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Continue"

$exe = Join-Path $PSScriptRoot "target\release\rustmodlica.exe"
if (-not (Test-Path $exe)) {
    Write-Error "Build first: cargo build --release -p rustmodlica"
    exit 1
}

function Get-CsvLastRowMaxDiff {
    param([string]$PathA, [string]$PathB)
    if (-not (Test-Path $PathA) -or -not (Test-Path $PathB)) {
        return $null
    }
    $aLines = Get-Content $PathA
    $bLines = Get-Content $PathB
    if ($aLines.Count -lt 2 -or $bLines.Count -lt 2) {
        return 0.0
    }
    $aLast = ($aLines[-1] -split ",").Trim()
    $bLast = ($bLines[-1] -split ",").Trim()
    $n = [Math]::Min($aLast.Count, $bLast.Count)
    $maxDiff = 0.0
    for ($j = 1; $j -lt $n; $j++) {
        $a = 0.0; $b = 0.0
        [void][double]::TryParse($aLast[$j], [ref]$a)
        [void][double]::TryParse($bLast[$j], [ref]$b)
        $diff = [Math]::Abs($a - $b)
        if ($diff -gt $maxDiff) { $maxDiff = $diff }
    }
    return $maxDiff
}

function Run-OneCompare {
    param(
        [string]$M,
        [string]$OutCsv,
        [string]$OmcPath
    )
    $argList = @()
    if ($LibPath -ne "") {
        $argList += "--lib-path=$LibPath"
    }
    $argList += $RustArgs
    $argList += @("--solver=$Solver", "--dt=$Dt", "--t-end=$TEnd", "--result-file=$OutCsv", $M)

    Write-Host "Running rustmodlica: $M solver=$Solver t_end=$TEnd dt=$Dt -> $OutCsv"
    $errFile = "$OutCsv.stderr.txt"
    $outFile = "$OutCsv.stdout.txt"
    $p = Start-Process -FilePath $exe -ArgumentList $argList -NoNewWindow -Wait -PassThru `
        -RedirectStandardError $errFile -RedirectStandardOutput $outFile
    $code = $p.ExitCode
    if ($code -ne 0) {
        return [pscustomobject]@{ ok = $false; model = $M; exit = $code; maxDiff = $null; note = "sim_failed" }
    }

    if ($OmcPath -ne "" -and (Test-Path $OmcPath)) {
        $maxDiff = Get-CsvLastRowMaxDiff -PathA $OutCsv -PathB $OmcPath
        Write-Host "Comparison (last row): $M vs $OmcPath -> max abs diff = $maxDiff"
        return [pscustomobject]@{ ok = $true; model = $M; exit = 0; maxDiff = $maxDiff; note = "omc_csv" }
    }

    if ($CrossSolverSelfGate) {
        $altCsv = "$OutCsv.rk45.csv"
        $altArgs = @()
        if ($LibPath -ne "") { $altArgs += "--lib-path=$LibPath" }
        $altArgs += $RustArgs
        $altArgs += @("--solver=rk45", "--dt=$Dt", "--t-end=$TEnd", "--result-file=$altCsv", $M)
        $p2 = Start-Process -FilePath $exe -ArgumentList $altArgs -NoNewWindow -Wait -PassThru `
            -RedirectStandardError "$altCsv.stderr.txt" -RedirectStandardOutput "$altCsv.stdout.txt"
        if ($p2.ExitCode -ne 0) {
            return [pscustomobject]@{ ok = $false; model = $M; exit = $p2.ExitCode; maxDiff = $null; note = "rk45_failed" }
        }
        $maxDiff = Get-CsvLastRowMaxDiff -PathA $OutCsv -PathB $altCsv
        Write-Host "Cross-solver (rk4 vs rk45 last row): $M -> maxDiff=$maxDiff"
        return [pscustomobject]@{ ok = $true; model = $M; exit = 0; maxDiff = $maxDiff; note = "cross_solver" }
    }

    if ($OmcPath -eq "") {
        return [pscustomobject]@{ ok = $true; model = $M; exit = 0; maxDiff = $null; note = "no OMC csv" }
    }
    return [pscustomobject]@{ ok = $true; model = $M; exit = 0; maxDiff = $null; note = "OMC file missing: $OmcPath" }
}

if ($OutDir -ne "") {
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
}

$results = @()
if ($Models.Count -gt 0) {
    $i = 0
    foreach ($m in $Models) {
        $safe = ($m -replace '[^\w\.-]', '_')
        $out = if ($OutDir -ne "") {
            Join-Path $OutDir "$safe.csv"
        } else {
            Join-Path $PSScriptRoot ("rust_out{0}.csv" -f $(if ($i -eq 0) { "" } else { "_$i" }))
        }
        $omcForModel = $OmcOut
        if ($OutDir -ne "" -and $OmcOut -eq "") {
            $cand = Join-Path $OutDir "$safe.omc.csv"
            if (Test-Path $cand) { $omcForModel = $cand }
        }
        $results += Run-OneCompare -M $m -OutCsv $out -OmcPath $omcForModel
        $i++
    }
} else {
    $out = if ($OutDir -ne "") { Join-Path $OutDir (Split-Path $RustOut -Leaf) } else { Join-Path $PSScriptRoot $RustOut }
    $results += Run-OneCompare -M $Model -OutCsv $out -OmcPath $OmcOut
}

if ($JsonSummary -ne "") {
    $summary = @{
        total = $results.Count
        pass  = @($results | Where-Object { $_.ok }).Count
        fail  = @($results | Where-Object { -not $_.ok }).Count
        cases = $results
        note  = if ($CrossSolverSelfGate) { "cross_solver_self_gate" } elseif ($OmcOut -ne "") { "omc_csv" } else { "rust_only" }
    }
    $summary | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $JsonSummary
    Write-Host ("Summary pass={0} fail={1} -> {2}" -f $summary.pass, $summary.fail, $JsonSummary)
}

if ($Models.Count -eq 0 -and ($OmcOut -eq "" -or -not (Test-Path $OmcOut)) -and -not $CrossSolverSelfGate) {
    if ($OmcOut -eq "") {
        Write-Host "Done. To compare with OMC: export CSV, then:"
        Write-Host "  .\compare_omc.ps1 -Model $Model -TEnd $TEnd -Dt $Dt -OmcOut <path_to_omc_csv>"
        Write-Host "Or self-gate without OMC: -CrossSolverSelfGate"
    } else {
        Write-Host "OMC file not found: $OmcOut. Skipping comparison."
    }
}

$bad = @($results | Where-Object { -not $_.ok })
if ($bad.Count -gt 0) { exit 1 }
exit 0
