param(
    [string]$Root = ".",
    [string]$OutDir = "build_event_scan_matrix",
    [string[]]$Models = @("TestLib/BouncingBall", "TestLib/Pendulum", "ModelicaTest.JitStress.SyncOmCompare"),
    [string[]]$CountValues = @("0.0004", "0.0005", "0.0006", "0.0008"),
    [string[]]$TailVelocityValues = @("0.02", "0.03", "0.04", "0.05"),
    [string[]]$LibPaths = @(),
    [int]$TopN = 3
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($LibPaths.Count -eq 0) {
    Write-Error "run_event_scan_matrix requires explicit -LibPaths (at least one path)."
    exit 2
}

$repoRoot = (Resolve-Path -LiteralPath $Root).Path
$manifest = Join-Path $repoRoot "jit-compiler/Cargo.toml"
$outPath = Join-Path $repoRoot $OutDir
if (-not (Test-Path -LiteralPath $outPath)) {
    New-Item -ItemType Directory -Path $outPath | Out-Null
}

$featuresRaw = [string]$env:RUSTMODLICA_CARGO_FEATURES
if ([string]::IsNullOrWhiteSpace($featuresRaw)) { $featuresRaw = "sundials" }
$features = @()
foreach ($f in $featuresRaw.Split(",")) {
    $t = $f.Trim()
    if ($t -ne "") { $features += $t }
}
$argFeatures = @()
if ($features.Count -gt 0) {
    $argFeatures += "--features"
    $argFeatures += ($features -join ",")
}

$csvPath = Join-Path $outPath "deadband_matrix_stability.csv"
"model,count_deadband,tail_deadband,run1_hash,run2_hash,status,reason" | Set-Content -LiteralPath $csvPath -Encoding UTF8

$unsupported = New-Object System.Collections.Generic.List[string]
$configErrors = New-Object System.Collections.Generic.List[string]

foreach ($m in $Models) {
    foreach ($c in $CountValues) {
        foreach ($tv in $TailVelocityValues) {
            $safe = ($m -replace '[^A-Za-z0-9_.-]', '_')
            $a = Join-Path $outPath ("event_{0}_{1}_{2}_a.json" -f $safe, $c, $tv)
            $b = Join-Path $outPath ("event_{0}_{1}_{2}_b.json" -f $safe, $c, $tv)

            $argLib = @()
            foreach ($lp in $LibPaths) {
                $argLib += "--lib-path=$lp"
            }
            $scanArgsA = @(
                "run", "-p", "rustmodlica", "--manifest-path", $manifest
            ) + $argFeatures + @(
                "--release", "--",
                "event-scan",
                "--model=$m",
                "--count-values=$c",
                "--tail-velocity-values=$tv",
                "--top-n=$TopN",
                "--aggregate-report=full",
                "--output-file=$a"
            ) + $argLib

            $oldEap = $ErrorActionPreference
            $ErrorActionPreference = "Continue"
            $null = & cargo @scanArgsA 2>&1
            $e1 = $LASTEXITCODE
            $scanArgsB = @(
                "run", "-p", "rustmodlica", "--manifest-path", $manifest
            ) + $argFeatures + @(
                "--release", "--",
                "event-scan",
                "--model=$m",
                "--count-values=$c",
                "--tail-velocity-values=$tv",
                "--top-n=$TopN",
                "--aggregate-report=full",
                "--output-file=$b"
            ) + $argLib
            $null = & cargo @scanArgsB 2>&1
            $e2 = $LASTEXITCODE
            $ErrorActionPreference = $oldEap

            if ($e1 -ne 0 -or $e2 -ne 0 -or -not (Test-Path -LiteralPath $a) -or -not (Test-Path -LiteralPath $b)) {
                "$m,$c,$tv,,,error,process_failed" | Add-Content -LiteralPath $csvPath -Encoding UTF8
                $configErrors.Add("$m c=$c tv=$tv reason=process_failed") | Out-Null
                continue
            }

            $contentA = Get-Content -LiteralPath $a -Raw
            $contentB = Get-Content -LiteralPath $b -Raw
            $jsonA = $null
            $jsonB = $null
            try {
                $jsonA = $contentA | ConvertFrom-Json
                $jsonB = $contentB | ConvertFrom-Json
            } catch {
                "$m,$c,$tv,,,error,invalid_json" | Add-Content -LiteralPath $csvPath -Encoding UTF8
                $configErrors.Add("$m c=$c tv=$tv reason=invalid_json") | Out-Null
                continue
            }

            $modelA = $jsonA.models | Select-Object -First 1
            $modelB = $jsonB.models | Select-Object -First 1
            if ($null -eq $modelA -or $null -eq $modelB) {
                "$m,$c,$tv,,,error,missing_model_output" | Add-Content -LiteralPath $csvPath -Encoding UTF8
                $configErrors.Add("$m c=$c tv=$tv reason=missing_model_output") | Out-Null
                continue
            }

            if ($modelA.status -eq "unsupported" -or $modelB.status -eq "unsupported") {
                "$m,$c,$tv,,,unsupported,$($modelA.unsupported_reason)" | Add-Content -LiteralPath $csvPath -Encoding UTF8
                $unsupported.Add("$m c=$c tv=$tv reason=$($modelA.unsupported_reason)") | Out-Null
                continue
            }
            if ($modelA.status -eq "config_error" -or $modelB.status -eq "config_error") {
                "$m,$c,$tv,,,config_error,$($modelA.config_error)" | Add-Content -LiteralPath $csvPath -Encoding UTF8
                $configErrors.Add("$m c=$c tv=$tv reason=$($modelA.config_error)") | Out-Null
                continue
            }

            $ha = (Get-FileHash -LiteralPath $a -Algorithm SHA256).Hash
            $hb = (Get-FileHash -LiteralPath $b -Algorithm SHA256).Hash
            $st = if ($ha -eq $hb) { "stable" } else { "nondeterministic" }
            "$m,$c,$tv,$ha,$hb,$st," | Add-Content -LiteralPath $csvPath -Encoding UTF8
        }
    }
}

$rows = Import-Csv -LiteralPath $csvPath
$stable = @($rows | Where-Object { $_.status -eq "stable" }).Count
$nondeterministic = @($rows | Where-Object { $_.status -eq "nondeterministic" }).Count
$unsupportedCount = @($rows | Where-Object { $_.status -eq "unsupported" }).Count
$configErrorCount = @($rows | Where-Object { $_.status -eq "config_error" -or $_.status -eq "error" }).Count

$unsupportedPath = Join-Path $outPath "unsupported_models.txt"
if ($unsupported.Count -gt 0) {
    $unsupported | Set-Content -LiteralPath $unsupportedPath -Encoding UTF8
} else {
    "none" | Set-Content -LiteralPath $unsupportedPath -Encoding UTF8
}

$reportPath = Join-Path $outPath "consistency_report.txt"
@(
    "stable=$stable",
    "nondeterministic=$nondeterministic",
    "unsupported=$unsupportedCount",
    "config_error=$configErrorCount",
    "csv=$csvPath",
    "unsupported_models=$unsupportedPath"
) | Set-Content -LiteralPath $reportPath -Encoding UTF8

Get-Content -LiteralPath $reportPath
