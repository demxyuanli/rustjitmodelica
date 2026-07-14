param(
    [string]$Root = ".",
    [string]$OutDir = "build_modelica_dir_validate",
    [string]$ExePath = "",
    [int]$MaxCases = 0,
    [string]$IncludePattern = "",
    [string]$ExcludePattern = "",
    [ValidateSet("full","quick","superfast")]
    [string]$ValidationMode = "full",
    [ValidateSet("full","parse","flatten","analyze")]
    [string]$ValidateTier = "full",
    # Additional local Modelica library roots (repeatable).
    [string[]]$LibPath = @(),
    # When set, every .mo under jit-compiler/Modelica and jit-compiler/ModelicaTest is eligible.
    # Default (off) keeps only ModelicaTest and Modelica/*/Examples for faster validate gates.
    [switch]$AllLibraryMo,
    [int]$ParallelWorkers = 1,
    [string]$CargoTargetSubdir = "target_regression"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

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

function Get-SimClassDeclNameFromLine([string]$ln) {
    if ($ln -match '^\s*//') { return $null }
    if ($ln -cmatch '^\s*(?:(?:encapsulated|replaceable|expandable)\s+)*partial\s+(?:model|block)\b') {
        return ""
    }
    $pat = '^\s*(?:(?:encapsulated|partial|replaceable|expandable)\s+)*(?:model|block)\s+([A-Za-z_][A-Za-z0-9_]*)(?=\s*(?:;|\(|\s+extends\b|//|"|$|\s+(?:equation|algorithm|protected|public|annotation|initial|final|parameter|discrete|input|output|inner|outer|stream|import)\b))'
    $m = [regex]::Match($ln, $pat)
    if (-not $m.Success) { return $null }
    return $m.Groups[1].Value
}

function Get-TopLevelSimClassName([string[]]$lines) {
    foreach ($ln in $lines) {
        if ($ln -match '^\s*//') { continue }
        $simName = Get-SimClassDeclNameFromLine $ln
        if ($null -ne $simName -and $simName -ne "") {
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

function Get-InnerSimClassNamesFromPackage([string[]]$lines) {
    $names = New-Object System.Collections.Generic.List[string]
    $depth = 0
    $seenTopPackage = $false
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
        if ($null -ne $simName -and $simName -ne "" -and $ln -notmatch '\s*=\s*') {
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
    $innerArr = @($inner)
    if ($innerArr.Count -eq 0) { return @() }
    if ($within -eq "" -or $within -eq "within") {
        return @($innerArr | ForEach-Object { "$pkg.$_" })
    }
    return @($innerArr | ForEach-Object { "$within.$pkg.$_" })
}

function Resolve-RustmodlicaExe([string]$repoRoot, [string]$jitRoot, [string]$exePath, [string]$cargoTargetSubdir) {
    if (-not [string]::IsNullOrWhiteSpace($exePath)) {
        $p = $exePath
        if (-not [System.IO.Path]::IsPathRooted($p)) { $p = Join-Path $repoRoot $p }
        if (Test-Path -LiteralPath $p) { return (Resolve-Path -LiteralPath $p).Path }
        throw "ExePath not found: $p"
    }
    $candidates = New-Object System.Collections.Generic.List[string]
    if (-not [string]::IsNullOrWhiteSpace($cargoTargetSubdir)) {
        $sub = $cargoTargetSubdir.Trim().TrimStart('\','/')
        foreach ($rel in @(
                (Join-Path $sub "release/rustmodlica.exe"),
                (Join-Path $sub "release/rustmodlica"),
                (Join-Path $sub "debug/rustmodlica.exe"),
                (Join-Path $sub "debug/rustmodlica")
            )) {
            [void]$candidates.Add((Join-Path $jitRoot $rel))
        }
    }
    foreach ($c in @(
            (Join-Path $repoRoot "target/release/rustmodlica.exe"),
            (Join-Path $repoRoot "target/release/rustmodlica"),
            (Join-Path $repoRoot "target/debug/rustmodlica.exe"),
            (Join-Path $repoRoot "target/debug/rustmodlica")
        )) {
        [void]$candidates.Add($c)
    }
    foreach ($c in $candidates) {
        if (Test-Path -LiteralPath $c) { return (Resolve-Path -LiteralPath $c).Path }
    }
    throw "rustmodlica binary not found. Tried CargoTargetSubdir='$cargoTargetSubdir' under jit-compiler and workspace target/release|debug."
}

function ConvertTo-CsvField([string]$s) {
    if ($null -eq $s) { return "" }
    $q = $s.Replace('"', '""')
    return '"' + $q + '"'
}

$repoRoot = Get-NormalizedPath $Root
$jitRoot = Join-Path $repoRoot "jit-compiler"
$modelicaLibRoot = Join-Path $jitRoot "Modelica"
$modelicaTestLibRoot = Join-Path $jitRoot "ModelicaTest"
$exe = Resolve-RustmodlicaExe $repoRoot $jitRoot $ExePath $CargoTargetSubdir

$resolvedLibRoots = New-Object System.Collections.Generic.List[string]
foreach ($lp in $LibPath) {
    if ([string]::IsNullOrWhiteSpace($lp)) { continue }
    $abs = $lp
    if (-not [System.IO.Path]::IsPathRooted($abs)) { $abs = Join-Path $repoRoot $abs }
    if (-not (Test-Path -LiteralPath $abs)) { throw "Configured LibPath does not exist: $abs" }
    $norm = (Resolve-Path -LiteralPath $abs).Path
    if (-not $resolvedLibRoots.Contains($norm)) {
        # If caller passes a bundle root containing Modelica/ and ModelicaTest/,
        # expand to package roots directly for loader compatibility.
        $bundleModelica = Join-Path $norm "Modelica"
        $bundleModelicaTest = Join-Path $norm "ModelicaTest"
        $addedExpanded = $false
        if (Test-Path -LiteralPath (Join-Path $bundleModelica "package.mo")) {
            if (-not $resolvedLibRoots.Contains($bundleModelica)) { $resolvedLibRoots.Add($bundleModelica) }
            $addedExpanded = $true
        }
        if (Test-Path -LiteralPath (Join-Path $bundleModelicaTest "package.mo")) {
            if (-not $resolvedLibRoots.Contains($bundleModelicaTest)) { $resolvedLibRoots.Add($bundleModelicaTest) }
            $addedExpanded = $true
        }
        if (-not $addedExpanded) {
            $resolvedLibRoots.Add($norm)
        }
    }
}
if ($resolvedLibRoots.Count -eq 0) {
    # Default to bundle root (jit-compiler) so fully-qualified names like
    # "Modelica.Blocks.Sources.Sine" resolve correctly.
    if (Test-Path -LiteralPath $jitRoot) { $resolvedLibRoots.Add((Resolve-Path -LiteralPath $jitRoot).Path) }
}

Write-Host ("rustmodlica: " + $exe)
Write-Host ("Effective lib roots: " + ($resolvedLibRoots -join "; "))

$outPath = Join-Path $repoRoot $OutDir
if (-not (Test-Path -LiteralPath $outPath)) { New-Item -ItemType Directory -Path $outPath | Out-Null }
$logDir = Join-Path $outPath "logs"
if (-not (Test-Path -LiteralPath $logDir)) { New-Item -ItemType Directory -Path $logDir | Out-Null }
$runStamp = Get-Date -Format "yyyyMMdd_HHmmss"
$runLogNdjson = Join-Path $outPath ("validate_{0}.ndjson" -f $runStamp)
$runLogCsv = Join-Path $outPath ("validate_{0}.csv" -f $runStamp)
"timestamp,model_name,duration_ms,exit_code,success,json_mode,detail" | Set-Content -LiteralPath $runLogCsv -Encoding UTF8

function Write-RunLog {
    param(
        [string]$ModelName,
        [long]$DurationMs,
        [int]$ExitCode,
        [bool]$Success,
        [bool]$JsonMode,
        [string]$Detail
    )
    $ts = (Get-Date).ToString("o")
    $obj = [pscustomobject]@{
        timestamp = $ts
        model_name = $ModelName
        duration_ms = $DurationMs
        exit_code = $ExitCode
        success = $Success
        json_mode = $JsonMode
        detail = $Detail
    }
    ($obj | ConvertTo-Json -Compress) | Add-Content -LiteralPath $runLogNdjson -Encoding UTF8
    $csvLine = ($ts + "," + (ConvertTo-CsvField $ModelName) + "," + $DurationMs + "," + $ExitCode + "," + $Success.ToString().ToLowerInvariant() + "," + $JsonMode.ToString().ToLowerInvariant() + "," + (ConvertTo-CsvField $Detail))
    $csvLine | Add-Content -LiteralPath $runLogCsv -Encoding UTF8
}

function Invoke-Validate([string]$modelName) {
    $cmdArgs = @("--validate", "--validate-tier=$ValidateTier", "--validation-mode=$ValidationMode", $modelName)
    foreach ($lr in $resolvedLibRoots) { $cmdArgs = @("--lib-path=$lr") + $cmdArgs }
    Push-Location $jitRoot
    $oldEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $raw = & $exe @cmdArgs 2>&1 | Out-String
    $exit = $LASTEXITCODE
    $sw.Stop()
    $ErrorActionPreference = $oldEap
    Pop-Location
    $jsonOk = $false
    if ($raw -match '"success"\s*:\s*true') { $jsonOk = $true }
    $detail = ""
    if (-not $jsonOk) {
        $detail = ($raw -split "`r?`n" | Select-Object -First 20) -join "`n"
    }
    Write-RunLog -ModelName $modelName -DurationMs $sw.ElapsedMilliseconds -ExitCode $exit -Success $jsonOk -JsonMode $true -Detail $detail
    return [pscustomobject]@{ Model = $modelName; ExitCode = $exit; Success = $jsonOk; Output = $raw; DurationMs = $sw.ElapsedMilliseconds }
}

function Get-MoFileList([string]$rootDir, [bool]$allLibraryMo) {
    $list = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path -LiteralPath $rootDir)) { return $list }
    if ($allLibraryMo) {
        Get-ChildItem -LiteralPath $rootDir -Recurse -Filter "*.mo" -File |
            ForEach-Object { [void]$list.Add($_.FullName) }
        return $list
    }
    $isModelica = (Split-Path -Leaf $rootDir) -eq "Modelica"
    if ($isModelica) {
        Get-ChildItem -LiteralPath $rootDir -Recurse -Filter "*.mo" -File |
            Where-Object { $_.FullName -match '\\Examples\\' } |
            ForEach-Object { [void]$list.Add($_.FullName) }
        return $list
    }
    Get-ChildItem -LiteralPath $rootDir -Recurse -Filter "*.mo" -File |
        ForEach-Object { [void]$list.Add($_.FullName) }
    return $list
}

$moFiles = New-Object System.Collections.Generic.List[string]
foreach ($p in @($modelicaLibRoot, $modelicaTestLibRoot)) {
    $files = Get-MoFileList $p $AllLibraryMo.IsPresent
    foreach ($f in $files) { [void]$moFiles.Add($f) }
}

if (-not [string]::IsNullOrWhiteSpace($IncludePattern)) {
    $tmp = $moFiles | Where-Object { $_ -match $IncludePattern }
    $moFiles = New-Object System.Collections.Generic.List[string]
    foreach ($f in $tmp) { [void]$moFiles.Add($f) }
}
if (-not [string]::IsNullOrWhiteSpace($ExcludePattern)) {
    $tmp = $moFiles | Where-Object { $_ -notmatch $ExcludePattern }
    $moFiles = New-Object System.Collections.Generic.List[string]
    foreach ($f in $tmp) { [void]$moFiles.Add($f) }
}

Write-Host ("Discovered .mo files: " + $moFiles.Count)

$models = New-Object System.Collections.Generic.List[string]
foreach ($f in $moFiles) {
    $mns = Get-ModelNamesFromMoFile $f
    foreach ($mn in $mns) {
        if ([string]::IsNullOrWhiteSpace($mn)) { continue }
        if (-not $models.Contains($mn)) { [void]$models.Add($mn) }
    }
}

$models = @($models | Sort-Object)
if ($MaxCases -gt 0 -and $models.Count -gt $MaxCases) {
    $models = @($models | Select-Object -First $MaxCases)
}

Write-Host ("Discovered validate targets: " + $models.Count)
Write-Host ("Logs: " + $runLogCsv)

$fails = New-Object System.Collections.Generic.List[string]
$ok = 0

if ($ParallelWorkers -le 1) {
    foreach ($m in $models) {
        $r = Invoke-Validate $m
        if ($r.Success) { $ok++ } else { [void]$fails.Add($m) }
    }
} else {
    $throttle = $ParallelWorkers
    $jobScript = {
        param($exe, $jitRoot, $resolvedLibRoots, $modelName, $validationMode, $validateTier)
        $cmdArgs = @("--validate", "--validate-tier=$validateTier", "--validation-mode=$validationMode", $modelName)
        foreach ($lr in $resolvedLibRoots) { $cmdArgs = @("--lib-path=$lr") + $cmdArgs }
        Push-Location $jitRoot
        $raw = & $exe @cmdArgs 2>&1 | Out-String
        $exit = $LASTEXITCODE
        Pop-Location
        $jsonOk = $false
        if ($raw -match '"success"\s*:\s*true') { $jsonOk = $true }
        return [pscustomobject]@{ Model = $modelName; ExitCode = $exit; Success = $jsonOk; Output = $raw }
    }
    $jobs = @()
    foreach ($m in $models) {
        $jobs += Start-Job -ScriptBlock $jobScript -ArgumentList @($exe, $jitRoot, @($resolvedLibRoots), $m, $ValidationMode, $ValidateTier)
        if ($jobs.Count -ge $throttle) {
            $done = Wait-Job -Job $jobs -Any
            foreach ($j in @($done)) {
                $r = Receive-Job -Job $j
                Remove-Job -Job $j | Out-Null
                $jobs = $jobs | Where-Object { $_.Id -ne $j.Id }
                if ($r.Success) { $ok++ } else { [void]$fails.Add($r.Model) }
                Write-RunLog -ModelName $r.Model -DurationMs 0 -ExitCode $r.ExitCode -Success $r.Success -JsonMode $true -Detail ""
            }
        }
    }
    if ($jobs.Count -gt 0) {
        Wait-Job -Job $jobs | Out-Null
        foreach ($j in $jobs) {
            $r = Receive-Job -Job $j
            Remove-Job -Job $j | Out-Null
            if ($r.Success) { $ok++ } else { [void]$fails.Add($r.Model) }
            Write-RunLog -ModelName $r.Model -DurationMs 0 -ExitCode $r.ExitCode -Success $r.Success -JsonMode $true -Detail ""
        }
    }
}

Write-Host ("PASS: " + $ok)
if ($fails.Count -gt 0) {
    Write-Host ("FAIL: " + $fails.Count)
    $fails | Select-Object -First 200 | ForEach-Object { Write-Host ("  " + $_) }
}

exit $(if ($fails.Count -gt 0) { 1 } else { 0 })

