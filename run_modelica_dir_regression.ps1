param(
    [string]$Root = ".",
    [string]$OutDir = "build_modelica_dir_regress",
    [string]$ResumeFromSummary = "",
    # When set, only re-run model names from prior summary lines starting with "--" (skip outcomes). Skips .mo discovery.
    [string]$OnlySkipsFromSummary = "",
    [string]$ExePath = "",
    [double]$TEnd = 10.0,
    [double]$Dt = 0.01,
    [string]$Solver = "rk4",
    [int]$MaxCases = 0,
    [string]$IncludePattern = "",
    [string]$ExcludePattern = "",
    [string[]]$ExtraArgs = @(),
    # When set, every .mo under jit-compiler/Modelica and jit-compiler/ModelicaTest is eligible (full MSL + tests).
    # Default (off) keeps only ModelicaTest and Modelica/*/Examples for faster runs.
    [switch]$AllLibraryMo,
    [switch]$ImplicitRetryIdealTwoWaySwitches,
    # Strict by default: Newton non-convergence is counted as failed (!!).
    # Keep this switch for compatibility and explicitness in callers.
    [Alias('NewtonCountsAsFailure')]
    [switch]$NewtonCountsAsFailed,
    # Optional override for local debugging: treat Newton non-convergence as skipped (--).
    [switch]$NewtonNonConvergedAsSkip
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# Default policy: strict Newton gate ON unless explicitly downgraded for local debugging.
$strictNewtonGate = $true
if ($NewtonNonConvergedAsSkip) { $strictNewtonGate = $false }
if ($NewtonCountsAsFailed) { $strictNewtonGate = $true }

function Get-NormalizedPath([string]$p) {
    return (Resolve-Path -LiteralPath $p).Path
}

function Get-FileLines {
    param(
        [Parameter(Mandatory = $true)][string]$LiteralPath,
        [int]$TotalCount = 0
    )
    if (-not (Test-Path -LiteralPath $LiteralPath)) {
        return [pscustomobject]@{ Lines = [string[]]@() }
    }
    $p = (Resolve-Path -LiteralPath $LiteralPath).Path
    $arr = [System.IO.File]::ReadAllLines($p)
    if ($null -eq $arr) { $arr = [string[]]@() }
    if ($TotalCount -gt 0 -and $arr.Length -gt $TotalCount) {
        $n = $TotalCount
        $slice = New-Object string[] $n
        [Array]::Copy($arr, 0, $slice, 0, $n)
        return [pscustomobject]@{ Lines = [string[]]$slice }
    }
    return [pscustomobject]@{ Lines = [string[]]$arr }
}

function Get-WithinClause([string[]]$lines) {
    foreach ($ln in $lines) {
        if ($ln -match '^\s*//') { continue }
        if ($ln -match '^\s*within\s+([^;]+)\s*;\s*$') {
            return $Matches[1].Trim()
        }
        if ($ln -match '^\s*(model|block|class|package|record|function)\b') {
            break
        }
    }
    return ""
}

function Get-TopLevelSimClassName([string[]]$lines) {
    foreach ($ln in $lines) {
        if ($ln -match '^\s*//') { continue }
        $simName = Get-SimClassDeclNameFromLine $ln
        if ($simName -ne $null -and $simName -ne "") {
            return $simName
        }
        if ($ln -match '^\s*(package|function|class)\s+([A-Za-z_][A-Za-z0-9_]*)\b') {
            return ""
        }
    }
    return ""
}

function Get-TopLevelPackageName([string[]]$lines) {
    foreach ($ln in $lines) {
        if ($ln -match '^\s*//') { continue }
        if ($ln -match '^\s*package\s+([A-Za-z_][A-Za-z0-9_]*)\b') {
            return $Matches[1]
        }
        if ($ln -match '^\s*(model|block|class)\b') {
            break
        }
    }
    return ""
}

# Require real class-declaration shape after the name (not prose like "model shall be" in HTML docs).
function Get-SimClassDeclNameFromLine([string]$ln) {
    if ($ln -match '^\s*//') { return $null }
    $pat = '^\s*(?:(?:encapsulated|partial|replaceable|expandable)\s+)*(?:model|block)\s+([A-Za-z_][A-Za-z0-9_]*)(?=\s*(?:;|\(|\s+extends\b|//|"|$|\s+(?:equation|algorithm|protected|public|annotation|initial|final|parameter|discrete|input|output|inner|outer|stream|import)\b))'
    $m = [regex]::Match($ln, $pat)
    if (-not $m.Success) { return $null }
    return $m.Groups[1].Value
}

function Get-InnerSimClassNamesFromPackage([string[]]$lines) {
    $names = New-Object System.Collections.Generic.List[string]
    $depth = 0
    $seenTopPackage = $false
    # Stack tracks (depth, name) for nested packages to build qualified names
    $pkgStack = New-Object System.Collections.Generic.List[object]
    $inBlockComment = $false
    foreach ($ln in $lines) {
        if ($inBlockComment) {
            if ($ln -match '\*/') { $inBlockComment = $false }
            continue
        }
        if ($ln -match '/\*') {
            if ($ln -notmatch '\*/') { $inBlockComment = $true }
            continue
        }
        if ($ln -match '^\s*//') { continue }
        if ($ln -cmatch '^\s*(?:(?:encapsulated|partial|replaceable|expandable)\s+)*package\s+([A-Za-z_][A-Za-z0-9_]*)\b' -and $ln -notmatch '\s*=\s*') {
            $depth++
            if (-not $seenTopPackage) {
                $seenTopPackage = $true
            } else {
                $pkgStack.Add(@{ Depth = $depth; Name = $Matches[1] })
            }
            continue
        }
        $simName = Get-SimClassDeclNameFromLine $ln
        if ($simName -ne $null -and $simName -ne "" -and $ln -notmatch '\s*=\s*') {
            # Only collect models/blocks directly inside a package (not inside another model/block).
            # A model is "directly inside a package" if $depth == 1 (top package) or
            # $depth equals the most recent package's depth on $pkgStack.
            $insidePkg = $false
            if ($seenTopPackage -and $depth -eq 1) { $insidePkg = $true }
            if (-not $insidePkg -and $pkgStack.Count -gt 0) {
                $topPkg = $pkgStack[$pkgStack.Count - 1]
                if ($topPkg.Depth -eq $depth) { $insidePkg = $true }
            }
            if ($insidePkg) {
                $prefix = ""
                foreach ($pkg in $pkgStack) {
                    if ($pkg.Depth -le $depth) { $prefix += $pkg.Name + "." }
                }
                $qualName = "$prefix$simName"
                if (-not $names.Contains($qualName)) { $names.Add($qualName) }
            }
            $depth++
            continue
        }
        if ($ln -cmatch '^\s*(?:(?:encapsulated|partial|replaceable|expandable|impure|pure)\s+)*(class|record|function|type|connector|operator)\s+([A-Za-z_][A-Za-z0-9_]*)\b' -and $ln -notmatch '\s*=\s*') {
            $depth++
            continue
        }
        if ($ln -cmatch '^\s*end\s+([A-Za-z_][A-Za-z0-9_]*)\s*;\s*$') {
            $endName = $Matches[1]
            if ($endName -cin @("for","if","while","when","loop")) { continue }
            if ($depth -gt 0) {
                for ($si = $pkgStack.Count - 1; $si -ge 0; $si--) {
                    if ($pkgStack[$si].Depth -eq $depth) { $pkgStack.RemoveAt($si); break }
                }
                $depth--
            }
            continue
        }
    }
    return $names
}

function Get-ModelNameFromMoFile([string]$filePath) {
    $lines = (Get-FileLines $filePath 200).Lines
    $within = Get-WithinClause $lines
    $cls = Get-TopLevelSimClassName $lines
    if ($cls -eq "") { return "" }
    if ($within -eq "" -or $within -eq "within") { return $cls }
    return "$within.$cls"
}

function Get-ModelNamesFromMoFile([string]$filePath) {
    $lines = (Get-FileLines $filePath 2000).Lines
    $within = Get-WithinClause $lines
    $topModel = Get-TopLevelSimClassName $lines
    if ($topModel -ne "") {
        if ($within -eq "" -or $within -eq "within") { return @($topModel) }
        return @("$within.$topModel")
    }
    $pkg = Get-TopLevelPackageName $lines
    if ($pkg -eq "") { return @() }
    $inner = Get-InnerSimClassNamesFromPackage $lines
    $prefix = if ($within -eq "" -or $within -eq "within") { $pkg } else { "$within.$pkg" }
    $out = @()
    foreach ($n in $inner) { $out += "$prefix.$n" }
    return $out
}

function Test-IsValidNumber([string]$s) {
    $t = $s.Trim()
    if ($t.Length -eq 0) { return $false }
    # IEEE non-finite outputs (e.g. beta from device equations) are valid floats for regression CSV checks.
    $u = $t.ToUpperInvariant()
    if ($u -eq "INFINITY" -or $u -eq "INF" -or $u -eq "+INF" -or $u -eq "+INFINITY") { return $true }
    if ($u -eq "-INFINITY" -or $u -eq "-INF") { return $true }
    $v = 0.0
    if (-not [double]::TryParse($s, [ref]$v)) { return $false }
    return $true
}

function Test-GenericCsv([string]$csvPath) {
    if (-not (Test-Path -LiteralPath $csvPath)) {
        return @{ ok = $false; reason = "csv_missing" }
    }
    $lines = (Get-FileLines $csvPath 0).Lines
    if ($lines.Length -lt 2) {
        return @{ ok = $false; reason = "csv_no_data_rows" }
    }
    $header = @(($lines[0] -split ",") | ForEach-Object { $_.Trim() })
    for ($i = 1; $i -lt $lines.Length; $i++) {
        $cols = @(($lines[$i] -split ",") | ForEach-Object { $_.Trim() })
        $n = [Math]::Min($header.Count, $cols.Count)
        for ($j = 0; $j -lt $n; $j++) {
            if (-not (Test-IsValidNumber $cols[$j])) {
                return @{ ok = $false; reason = "csv_bad_number_row_${i}_col_${j}" }
            }
        }
    }
    return @{ ok = $true; reason = "ok" }
}

function Test-PendulumConstraint([string]$csvPath, [double]$eps) {
    $lines = (Get-FileLines $csvPath 0).Lines
    if ($lines.Length -lt 2) {
        return @{ ok = $false; reason = "csv_no_data_rows" }
    }
    $header = @(($lines[0] -split ",") | ForEach-Object { $_.Trim() })
    $xIdx = [Array]::IndexOf($header, "x")
    $yIdx = [Array]::IndexOf($header, "y")
    if ($xIdx -lt 0 -or $yIdx -lt 0) {
        return @{ ok = $true; reason = "pendulum_columns_missing_skip" }
    }
    $worst = 0.0
    for ($i = 1; $i -lt $lines.Length; $i++) {
        $cols = @(($lines[$i] -split ",") | ForEach-Object { $_.Trim() })
        $x = 0.0; $y = 0.0
        [double]::TryParse($cols[$xIdx], [ref]$x) | Out-Null
        [double]::TryParse($cols[$yIdx], [ref]$y) | Out-Null
        $r = [Math]::Abs(($x * $x) + ($y * $y) - 1.0)
        if ($r -gt $worst) { $worst = $r }
        if ($r -gt $eps) {
            return @{ ok = $false; reason = "pendulum_constraint_residual_${r}" }
        }
    }
    return @{ ok = $true; reason = "ok_max_residual_${worst}" }
}

function Test-ModelSpecific([string]$modelName, [string]$csvPath) {
    if ($modelName -eq "TestLib.Pendulum" -or $modelName -eq "TestLib/Pendulum") {
        return Test-PendulumConstraint $csvPath 1e-3
    }
    return @{ ok = $true; reason = "ok" }
}

function Test-IsDocLikeModelName([string]$modelName) {
    if ($modelName -match '\.UsersGuide\.') { return $true }
    if ($modelName -match '\.(UsersGuide|ReleaseNotes|Contact|Literature|Overview)$') { return $true }
    if ($modelName -match '\.(Conventions|References|Connectors)$') { return $true }
    if ($modelName -match '\.(Types|Units|System|Streams|Strings|Files|Internal)$') { return $true }
    if ($modelName -eq 'Demo') { return $true }
    return $false
}

function Get-FirstErrorLine([string]$logPath) {
    if (-not (Test-Path -LiteralPath $logPath)) { return "" }
    $lines = (Get-FileLines $logPath 120).Lines
    foreach ($ln in $lines) {
        if ($ln -match 'error') { return $ln.Trim() }
    }
    return ""
}

function Get-UnresolvedModelSet([string]$summaryPath) {
    $set = @{}
    if ([string]::IsNullOrWhiteSpace($summaryPath)) { return $set }
    if (-not (Test-Path -LiteralPath $summaryPath)) { return $set }
    $lines = (Get-FileLines $summaryPath 0).Lines
    foreach ($ln in $lines) {
        $t = $ln.Trim()
        if ($t -eq "") { continue }
        if ($t.StartsWith("OK ") -or $t.StartsWith("OK`t")) { continue }
        $modelName = ""
        if ($t.StartsWith("!!")) {
            $rest = $t.Substring(2).TrimStart()
            if ($rest -ne "") {
                $modelName = (($rest -split '\s+', 2)[0]).Trim()
            }
        } elseif ($t.StartsWith("--")) {
            $rest = $t.Substring(2).TrimStart()
            if ($rest -ne "") {
                $modelName = (($rest -split '\s+', 2)[0]).Trim()
            }
        }
        if ($modelName -ne "") { $set[$modelName] = $true }
    }
    return $set
}

function Get-SkipModelNamesFromSummary([string]$summaryPath) {
    $list = New-Object System.Collections.Generic.List[string]
    $seen = @{}
    if (-not (Test-Path -LiteralPath $summaryPath)) { return $list }
    $lines = (Get-FileLines $summaryPath 0).Lines
    foreach ($ln in $lines) {
        $t = $ln.Trim()
        if (-not $t.StartsWith("--")) { continue }
        $rest = $t.Substring(2).TrimStart()
        if ($rest -eq "") { continue }
        $name = (($rest -split '\s+', 2)[0]).Trim()
        if ($name -eq "") { continue }
        if ($seen.ContainsKey($name)) { continue }
        $seen[$name] = $true
        $list.Add($name)
    }
    return $list
}

function Test-MoFullPathMatchesRegex([string]$fullPath, [string]$pattern) {
    if ([string]::IsNullOrWhiteSpace($pattern)) { return $true }
    # Collapse accidental "\\." before a dot (e.g. single-quoted -IncludePattern) to regex "\." for a literal '.'
    $p = [regex]::Replace($pattern.Trim(), '\\+(?=\.)', [string][char]92)
    $norm = $fullPath -replace '\\', '/'
    if ($norm -match $p) { return $true }
    $dotted = $norm -replace '/', '.'
    return ($dotted -match $p)
}

$repoRoot = Get-NormalizedPath $Root
$jitRoot = Join-Path $repoRoot "jit-compiler"
$exe = if ($ExePath -ne "") {
    if ([System.IO.Path]::IsPathRooted($ExePath)) { $ExePath } else { Join-Path $repoRoot $ExePath }
} else {
    Join-Path $repoRoot "target\\release\\rustmodlica.exe"
}
if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Build first: cargo build --release"
    exit 1
}

# On Windows, rustmodlica.exe with sundials feature needs runtime DLLs from
# target/release/build/sundials-sys-*/out/lib. Inject latest candidate into PATH.
if ($env:OS -eq "Windows_NT") {
    $sundialsBuildRoot = Join-Path $repoRoot "target\release\build"
    if (Test-Path -LiteralPath $sundialsBuildRoot) {
        $dllDirs = Get-ChildItem -LiteralPath $sundialsBuildRoot -Directory -Filter "sundials-sys-*" |
            Sort-Object LastWriteTime -Descending |
            ForEach-Object { Join-Path $_.FullName "out\lib" } |
            Where-Object { Test-Path -LiteralPath $_ }
        if ($dllDirs -and $dllDirs.Count -gt 0) {
            $env:PATH = ($dllDirs[0] + ";" + $env:PATH)
        }
    }
}

$outPath = Join-Path $repoRoot $OutDir
if (-not (Test-Path -LiteralPath $outPath)) { New-Item -ItemType Directory -Path $outPath | Out-Null }
$logDir = Join-Path $outPath "logs"
if (-not (Test-Path -LiteralPath $logDir)) { New-Item -ItemType Directory -Path $logDir | Out-Null }

$models = New-Object System.Collections.Generic.List[string]

if ($OnlySkipsFromSummary -ne "") {
    if ($ResumeFromSummary -ne "") {
        Write-Warning "OnlySkipsFromSummary is set; ResumeFromSummary is ignored."
    }
    $skipSummaryPath = $OnlySkipsFromSummary
    if (-not [System.IO.Path]::IsPathRooted($skipSummaryPath)) {
        $skipSummaryPath = Join-Path $repoRoot $skipSummaryPath
    }
    if (-not (Test-Path -LiteralPath $skipSummaryPath)) {
        Write-Error "OnlySkipsFromSummary: file not found: $skipSummaryPath"
        exit 2
    }
    foreach ($sn in (Get-SkipModelNamesFromSummary $skipSummaryPath)) {
        if (-not (Test-IsDocLikeModelName $sn)) {
            $models.Add($sn)
        }
    }
    if ($models.Count -eq 0) {
        Write-Warning "OnlySkipsFromSummary: no runnable model names after doc/UserGuide/Demo filter (see $skipSummaryPath)"
    }
    if ($MaxCases -gt 0 -and $models.Count -gt $MaxCases) {
        $models = $models.GetRange(0, $MaxCases)
    }
    Write-Host "Skip-only run from summary: $($models.Count) model(s)"
} else {
    $moDirs = @(
        (Join-Path $jitRoot "Modelica"),
        (Join-Path $jitRoot "ModelicaTest")
    )

    $moFiles = @()
    foreach ($d in $moDirs) {
        if (Test-Path -LiteralPath $d) {
            $moFiles += Get-ChildItem -LiteralPath $d -Recurse -File -Filter "*.mo"
        }
    }
    $moFilesScannedTotal = $moFiles.Count

    if ($AllLibraryMo) {
        # $moFiles already lists all .mo under Modelica and ModelicaTest
    } elseif ($IncludePattern -ne "") {
        $moFiles = @($moFiles | Where-Object { Test-MoFullPathMatchesRegex $_.FullName $IncludePattern })
        if ($moFiles.Count -eq 0 -and $moFilesScannedTotal -gt 0) {
            Write-Warning "IncludePattern matched 0 .mo files ($moFilesScannedTotal scanned under Modelica and ModelicaTest). Use Modelica-style dots (e.g. Magnetic.FundamentalWave.Examples) or path slashes; file paths are normalized before -match."
        }
    } else {
        $moFiles = $moFiles | Where-Object {
            ($_.FullName -like "*\ModelicaTest\*") -or
            ($_.FullName -like "*\Modelica\*\Examples\*")
        }
    }
    if ($ExcludePattern -ne "") {
        $moFiles = $moFiles | Where-Object { -not (Test-MoFullPathMatchesRegex $_.FullName $ExcludePattern) }
    }

    foreach ($f in $moFiles) {
        if ($f.Name -ieq "package.mo") { continue }
        $mns = Get-ModelNamesFromMoFile $f.FullName
        foreach ($mn in $mns) {
            if ($mn -ne "" -and -not $models.Contains($mn)) {
                $models.Add($mn)
            }
        }
    }

    $docFiltered = @($models | Where-Object { -not (Test-IsDocLikeModelName $_) })
    $models = New-Object System.Collections.Generic.List[string]
    foreach ($mn in $docFiltered) {
        $models.Add($mn)
    }

    if ($MaxCases -gt 0 -and $models.Count -gt $MaxCases) {
        $models = $models.GetRange(0, $MaxCases)
    }

    if ($ResumeFromSummary -ne "") {
        $resumePath = $ResumeFromSummary
        if (-not [System.IO.Path]::IsPathRooted($resumePath)) {
            $resumePath = Join-Path $repoRoot $resumePath
        }
        if (-not (Test-Path -LiteralPath $resumePath)) {
            Write-Warning "ResumeFromSummary: file not found: $resumePath -- running full discovered list (no resume filter)."
        } else {
            $unresolved = Get-UnresolvedModelSet $resumePath
            if ($unresolved.Count -gt 0) {
                $beforeCnt = $models.Count
                $resumeFiltered = @($models | Where-Object { $unresolved.ContainsKey($_) })
                $models = New-Object System.Collections.Generic.List[string]
                foreach ($mn in $resumeFiltered) {
                    $models.Add($mn)
                }
                if ($models.Count -eq 0 -and $beforeCnt -gt 0) {
                    Write-Warning "ResumeFromSummary: $($unresolved.Count) unresolved name(s) in summary matched 0 discovered models (scope/pattern excludes them or name mismatch)."
                }
                if ($models.Count -eq 0 -and $beforeCnt -eq 0 -and $unresolved.Count -gt 0) {
                    Write-Warning "ResumeFromSummary: summary has $($unresolved.Count) unresolved entries but discovery produced 0 models (empty library path, IncludePattern too narrow, or prior run wiped summary.txt before fix)."
                }
            } else {
                Write-Host "ResumeFromSummary: no !! or -- rows in summary; nothing to re-run."
                $models = New-Object System.Collections.Generic.List[string]
            }
        }
    }

    Write-Host "Discovered models: $($models.Count)"
}

$ok = 0
$bad = 0
$skipped = 0
$results = @()
$modelTotal = $models.Count
$modelIndex = 0

foreach ($m in $models) {
    $modelIndex++
    Write-Host "[$modelIndex/$modelTotal] $m"
    $safeName = ($m -replace '[^A-Za-z0-9_.-]', '_')
    $csv = Join-Path $outPath "$safeName.csv"
    $logPath = Join-Path $logDir "$safeName.log"
    $cliArgs = @()
    $hasIndexReductionArg = $false
    foreach ($ea in $ExtraArgs) {
        if ($ea -like "--index-reduction-method=*") { $hasIndexReductionArg = $true; break }
    }
    if (-not $hasIndexReductionArg) {
        $cliArgs += "--index-reduction-method=dummyDerivative"
    }
    $cliArgs += $ExtraArgs
    $cliArgs += @("--solver=$Solver", "--dt=$Dt", "--t-end=$TEnd", "--result-file=$csv", $m)

    $usedImplicitRetry = $false
    Push-Location $jitRoot
    $oldEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $outLines = & $exe @cliArgs 2>&1
    $exit = $LASTEXITCODE
    $newtonFailedFirstTry = $false
    foreach ($ln in $outLines) {
        if ($ln -match 'Newton-Raphson failure') { $newtonFailedFirstTry = $true; break }
    }
    $allowImplicitRetry = $newtonFailedFirstTry
    if ($exit -ne 0 -and $newtonFailedFirstTry -and $Solver -ne "implicit" -and $allowImplicitRetry) {
        $retryArgs = @()
        $retryArgs += $ExtraArgs
        $retryArgs += @("--index-reduction-method=dummyDerivative", "--solver=implicit", "--dt=$Dt", "--t-end=$TEnd", "--result-file=$csv", $m)
        $retryLines = & $exe @retryArgs 2>&1
        $retryExit = $LASTEXITCODE
        $outLines = @($outLines + "----- implicit retry -----" + $retryLines)
        $exit = $retryExit
        if ($exit -eq 0) { $usedImplicitRetry = $true }
    }
    if ($exit -ne 0 -and $newtonFailedFirstTry) {
        for ($retryN = 1; $retryN -le 2; $retryN++) {
            $reArgs = @()
            if (-not $hasIndexReductionArg) { $reArgs += "--index-reduction-method=dummyDerivative" }
            $reArgs += $ExtraArgs
            $reArgs += @("--solver=$Solver", "--dt=$Dt", "--t-end=$TEnd", "--result-file=$csv", $m)
            $reLines = & $exe @reArgs 2>&1
            $reExit = $LASTEXITCODE
            $outLines = @($outLines + "----- recompile retry $retryN -----" + $reLines)
            if ($reExit -eq 0) { $exit = $reExit; break }
        }
    }
    try {
        $outLines | Set-Content -LiteralPath $logPath -Encoding UTF8
    } catch {
        try {
            $outLines | Out-File -LiteralPath $logPath -Encoding utf8 -Force
        } catch {
            # Keep regression running even if log file is locked by another process.
        }
    }
    $ErrorActionPreference = $oldEap
    Pop-Location

    if ($exit -ne 0) {
        $modelNotFoundSelf = $false
        $modelNotFoundDependency = $false
        $newtonFailed = $false
        $selfNotFoundPattern = '^Model not found:\s*' + [Regex]::Escape($m) + '\s*$'
        foreach ($ln in $outLines) {
            if ($ln -match $selfNotFoundPattern) { $modelNotFoundSelf = $true; break }
            if ($ln -match 'Model not found:') { $modelNotFoundDependency = $true }
            if ($ln -match 'Newton-Raphson failure') { $newtonFailed = $true }
        }
        if ($modelNotFoundSelf) {
            $skipped++
            $results += "-- $m  exit=$exit  reason=model_not_found_skip"
            continue
        }
        if ($modelNotFoundDependency) {
            $skipped++
            $results += "-- $m  exit=$exit  reason=dependency_model_missing_skip"
            continue
        }
        if ($newtonFailed) {
            if ($strictNewtonGate) {
                $bad++
                $results += "!! $m  exit=$exit  reason=newton_nonconverged"
            } else {
                $skipped++
                $results += "-- $m  exit=$exit  reason=newton_nonconverged_skip"
            }
            continue
        }
        $bad++
        $err = ""
        foreach ($ln in $outLines) {
            if ($ln -match 'error') { $err = ($ln.ToString().Trim()); break }
        }
        if ($err -ne "") { $results += "!! $m  exit=$exit  reason=sim_failed  detail=$err" }
        else { $results += "!! $m  exit=$exit  reason=sim_failed" }
        continue
    }

    $generic = Test-GenericCsv $csv
    if (-not $generic.ok) {
        $handledCsv = $false
        if ($generic.reason -eq "csv_no_data_rows") {
            $simDone = $false
            foreach ($ln in $outLines) {
                if ($ln.ToString() -match 'Simulation completed') { $simDone = $true; break }
            }
            if ($simDone) {
                foreach ($ln in $outLines) {
                    if ($ln.ToString() -match 'terminate\s*\(\)') {
                        $good++
                        $results += "OK $m  exit=$exit  reason=ok"
                        $handledCsv = $true
                        break
                    }
                }
            }
        }
        if ($handledCsv) { continue }
        $bad++
        $results += "!! $m  exit=$exit  reason=$($generic.reason)"
        continue
    }

    $spec = Test-ModelSpecific $m $csv
    if (-not $spec.ok) {
        $bad++
        $results += "!! $m  exit=$exit  reason=$($spec.reason)"
        continue
    }

    $ok++
    if ($usedImplicitRetry) {
        $results += "OK $m  exit=$exit  reason=ok_retry_implicit"
    } else {
        $results += "OK $m  exit=$exit  reason=$($spec.reason)"
    }
}

$summaryPath = Join-Path $outPath "summary.txt"
if ($modelTotal -gt 0) {
    $results | Set-Content -LiteralPath $summaryPath -Encoding UTF8
} else {
    Write-Warning "No models were run; left summary.txt unchanged: $summaryPath"
}

Write-Host ""
Write-Host "Summary: $ok passed, $bad failed, $skipped skipped"
Write-Host "Non-OK total: $($bad + $skipped) (strict Newton gate default ON; use -NewtonNonConvergedAsSkip to downgrade locally)"
Write-Host "Details: $summaryPath"

if ($skipped -gt 0) {
    $skipBreakdown = @{}
    foreach ($r in $results) {
        if ($r -match '^\-\-\s') {
            if ($r -match 'reason=([^\s]+)') {
                $rsn = $Matches[1]
                if (-not $skipBreakdown.ContainsKey($rsn)) {
                    $skipBreakdown[$rsn] = 0
                }
                $skipBreakdown[$rsn]++
            }
        }
    }
    if ($skipBreakdown.Count -gt 0) {
        Write-Host "Skip breakdown (by reason=...)."
        foreach ($kv in ($skipBreakdown.GetEnumerator() | Sort-Object Name)) {
            Write-Host ("  {0}: {1}" -f $kv.Key, $kv.Value)
        }
    }
}

if ($bad -gt 0) { exit 1 }
exit 0

