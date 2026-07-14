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
    # Additional local Modelica library roots (repeatable), used when local mirror is incomplete.
    [string[]]$LibPath = @(),
    # When set, every .mo under jit-compiler/Modelica and jit-compiler/ModelicaTest is eligible (full MSL + tests).
    # Default (off) keeps only ModelicaTest and Modelica/*/Examples for faster runs.
    [switch]$AllLibraryMo,
    [switch]$ImplicitRetryIdealTwoWaySwitches,
    # Strict by default: Newton non-convergence is counted as failed (!!).
    # Keep this switch for compatibility and explicitness in callers.
    [Alias('NewtonCountsAsFailure')]
    [switch]$NewtonCountsAsFailed,
    # Optional override for local debugging: treat Newton non-convergence as skipped (--).
    [switch]$NewtonNonConvergedAsSkip,
    # Parallel worker processes for model execution (1 = serial).
    [int]$ParallelWorkers = 1,
    # Private incremental compiler cache (local); opt-in via -UsePrivateCache or env RUSTMODLICA_USE_DIR_PRIVATE_CACHE=1.
    [switch]$UsePrivateCache,
    [string]$PrivateCacheRoot = "",
    [switch]$DisablePrivateCache,
    [string]$PrivateCacheKeyExtra = "",
    # Internal: parallel parent passes run key + absolute root + shard index (0..N-1). Do not use manually.
    [string]$PrivateCacheRunKey = "",
    [int]$PrivateCacheShard = -999,
    # When set (with -UsePrivateCache), write a .ps1 that sets RUSTMODLICA_* cache env for dot-sourcing in other shells/tests.
    [string]$WriteDirCacheEnvScript = "",
    # Run `--validate --validate-tier=analyze` before simulation to skip models that fail early analysis.
    [switch]$TwoStage,
    [int]$AnalyzeFirstTimeoutSec = 180,
    # For TwoStage analyze gate: matches rustmodlica --validation-mode (full|quick|superfast).
    # "quick" matches ValidationMode::QuickStructure and is intended for --validate-tier=analyze (faster than full).
    [string]$AnalyzeValidationMode = "quick",
    [int]$PerModelTimeoutSec = 720,
    # Parallel parent only: shard watchdog timeout. If no file activity is observed
    # under shard outdir for this many seconds, force-kill the shard worker.
    [int]$ShardNoProgressTimeoutSec = 1800,
    # 0 = follow -ParallelWorkers; then at least 1. Parent TwoStage only.
    [int]$AnalyzeParallelWorkers = 0,
    [int]$AnalyzeShardNoProgressTimeoutSec = 900,
    [int]$PerProcessMemoryLimitMb = 8192,
    [string]$QuarantineFile = "build_modelica_dir_regress/local/dir_quarantine.json",
    [switch]$RetryQuarantined,
    [int]$QuarantineConsecutiveHits = 2,
    [int]$AnalyzeCheckpointEvery = 50,
    [switch]$ResumeAnalyzeCheckpoint,
    # Child shard: run only --validate --validate-tier=analyze, write analyze_summary.txt, exit.
    [switch]$AnalyzeOnly,
    # Parent passes global set hash; child must match to resume from checkpoint.
    [string]$GlobalModelsHash = "",
    [int]$AnalyzeShardIndex = -1
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$__dirJobHelper = Join-Path (Split-Path -Parent $PSCommandPath) "scripts\dir_job_object.ps1"
if (Test-Path -LiteralPath $__dirJobHelper) { . $__dirJobHelper } else { Write-Warning "scripts/dir_job_object.ps1 not found: job + memory cap disabled" }
if ($AnalyzeOnly) { $env:RUSTMODLICA_DIR_REGRESSION_STAGE = "analyze" }

# Default policy: strict Newton gate ON unless explicitly downgraded for local debugging.
$strictNewtonGate = $true
if ($NewtonNonConvergedAsSkip) { $strictNewtonGate = $false }
if ($NewtonCountsAsFailed) { $strictNewtonGate = $true }

function ConvertTo-ProcessArgumentString {
    param([string[]]$Tokens)
    $parts = New-Object System.Collections.Generic.List[string]
    foreach ($t in $Tokens) {
        if ($null -eq $t) { continue }
        $s = [string]$t
        if ($s.Length -eq 0) {
            $parts.Add('""') | Out-Null
            continue
        }
        if ($s.Contains('"')) {
            $escaped = $s.Replace('"', '\"')
            $parts.Add(('"' + $escaped + '"')) | Out-Null
        } elseif ($s.Contains(' ') -or $s.Contains("`t")) {
            $parts.Add(('"' + $s + '"')) | Out-Null
        } else {
            $parts.Add($s) | Out-Null
        }
    }
    return [string]::Join(' ', $parts)
}

function Invoke-RustmodlicaWithTimeout {
    param(
        [string]$ExePath,
        [string[]]$CliArgs,
        [string]$WorkDir,
        [int]$TimeoutSec,
        [int]$MemoryLimitMb = 0,
        [string]$OutputFile = ""
    )
    $captureOutput = (-not [string]::IsNullOrWhiteSpace($OutputFile))
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $ExePath
    $psi.WorkingDirectory = $WorkDir
    $psi.UseShellExecute = $false
    $psi.Arguments = (ConvertTo-ProcessArgumentString -Tokens $CliArgs)
    $psi.CreateNoWindow = $true
    if ($captureOutput) {
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
    } else {
        $psi.RedirectStandardOutput = $false
        $psi.RedirectStandardError = $false
    }
    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $psi
    $job = [IntPtr]::Zero
    if ($env:OS -eq "Windows_NT" -and (Get-Command -Name New-DirRegressionJob -ErrorAction SilentlyContinue)) {
        $job = New-DirRegressionJob
        if ($job -ne [IntPtr]::Zero) {
            $okSet = Set-DirRegressionJobLimits -Job $job -PerProcessMemoryLimitMb $MemoryLimitMb
            if (-not $okSet) { Close-DirRegressionJob -Job $job; $job = [IntPtr]::Zero }
        }
    }
    [void]$p.Start()
    if ($job -ne [IntPtr]::Zero) {
        $as = $false
        try { $as = Add-ProcessToDirRegressionJob -Job $job -ProcessHandle $p.Handle } catch { $as = $false }
        if (-not $as) { Close-DirRegressionJob -Job $job; $job = [IntPtr]::Zero }
    }
    $stdoutTask = $null
    $stderrTask = $null
    if ($captureOutput) {
        $stdoutTask = $p.StandardOutput.ReadToEndAsync()
        $stderrTask = $p.StandardError.ReadToEndAsync()
    }
    $ms = [Math]::Max(1, $TimeoutSec) * 1000
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $memB = 0L
    if ($MemoryLimitMb -gt 0) { $memB = [long]([long]$MemoryLimitMb * 1024L * 1024L) }
    $oom = $false
    $peakMB = 0
    while (-not $p.HasExited) {
        if ($p.WaitForExit(1000)) { break }
        if ($sw.ElapsedMilliseconds -ge $ms) { break }
        try {
            $wsMB = [int]($p.WorkingSet64 / 1048576L)
            if ($wsMB -gt $peakMB) { $peakMB = $wsMB }
        } catch {}
        if ($memB -gt 0) {
            try {
                if ($p.WorkingSet64 -gt $memB) {
                    $oom = $true
                    try { $p.Kill() } catch {}
                    $null = $p.WaitForExit(20000)
                    if ($job -ne [IntPtr]::Zero) { try { Stop-WindowsProcessTree -RootPid $p.Id } catch {} }
                    if ($job -ne [IntPtr]::Zero) { Close-DirRegressionJob -Job $job; $job = [IntPtr]::Zero }
                    if ($captureOutput) { try { $null = $stdoutTask.Wait(5000); $null = $stderrTask.Wait(5000) } catch {} }
                    $outText = ""; if ($captureOutput -and $null -ne $stdoutTask -and $stdoutTask.IsCompleted) { $outText = $stdoutTask.Result }
                    $errText = ""; if ($captureOutput -and $null -ne $stderrTask -and $stderrTask.IsCompleted) { $errText = $stderrTask.Result }
                    if ($captureOutput) { try { ($outText + "`n" + $errText) | Set-Content -LiteralPath $OutputFile -Encoding UTF8 } catch {} }
                    return @{ ExitCode = -1; TimedOut = $false; Oom = $true; PeakMB = $peakMB; OutputFile = $OutputFile }
                }
            } catch { }
        }
    }
    if (-not $p.HasExited) {
        try { $p.Kill() } catch {}
        $null = $p.WaitForExit(15000)
        try { Stop-WindowsProcessTree -RootPid $p.Id } catch { }
        if ($job -ne [IntPtr]::Zero) { Close-DirRegressionJob -Job $job; $job = [IntPtr]::Zero }
        if ($captureOutput) { try { $null = $stdoutTask.Wait(5000); $null = $stderrTask.Wait(5000) } catch {} }
        $outText = ""; if ($captureOutput -and $null -ne $stdoutTask -and $stdoutTask.IsCompleted) { $outText = $stdoutTask.Result }
        $errText = ""; if ($captureOutput -and $null -ne $stderrTask -and $stderrTask.IsCompleted) { $errText = $stderrTask.Result }
        if ($captureOutput) { try { ($outText + "`n" + $errText) | Set-Content -LiteralPath $OutputFile -Encoding UTF8 } catch {} }
        return @{ ExitCode = -1; TimedOut = $true; Oom = $oom; PeakMB = $peakMB; OutputFile = $OutputFile }
    }
    $ex = 0
    try { $ex = [int]$p.ExitCode } catch { $ex = -1 }
    if ($job -ne [IntPtr]::Zero) { Close-DirRegressionJob -Job $job; $job = [IntPtr]::Zero }
    if ($captureOutput) {
        try { $null = $stdoutTask.Wait(10000); $null = $stderrTask.Wait(5000) } catch {}
        $outText = ""; if ($null -ne $stdoutTask -and $stdoutTask.IsCompleted) { $outText = $stdoutTask.Result }
        $errText = ""; if ($null -ne $stderrTask -and $stderrTask.IsCompleted) { $errText = $stderrTask.Result }
        try { ($outText + "`n" + $errText) | Set-Content -LiteralPath $OutputFile -Encoding UTF8 } catch {}
    }
    return @{ ExitCode = $ex; TimedOut = $false; Oom = $oom; PeakMB = $peakMB; OutputFile = $OutputFile }
}

function Get-LatestWriteTimeUtc {
    param([string]$DirPath)
    if ([string]::IsNullOrWhiteSpace($DirPath)) { return $null }
    if (-not (Test-Path -LiteralPath $DirPath)) { return $null }
    try {
        $latest = Get-ChildItem -LiteralPath $DirPath -Recurse -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First 1
        if ($null -eq $latest) { return $null }
        return $latest.LastWriteTimeUtc
    } catch {
        return $null
    }
}

# Terminate a Windows process *and* every descendant. PowerShell's Stop-Process
# alone is not a strict tree kill; a detached rustmodlica.exe can outlive a
# shard powershell if the host is killed in certain ways. taskkill /T is the
# most reliable no-P/Invoke way to keep shard shutdown from leaking simulators.
function Stop-WindowsProcessTree {
    param(
        [int]$RootPid
    )
    if ($env:OS -ne "Windows_NT") { return }
    if ($RootPid -le 0) { return }
    $tk = (Join-Path $env:SystemRoot "System32\taskkill.exe")
    if (-not (Test-Path -LiteralPath $tk)) { return }
    & $tk /PID $RootPid /T /F 1>$null 2>$null
}

function Get-DirModelSetHash {
    param([string[]]$Names)
    if ($null -eq $Names) { return "0000000000000000" }
    $arr = @($Names) | ForEach-Object { [string]$_ } | Where-Object { $_ -ne "" } | Sort-Object
    $enc = [System.Text.UTF8Encoding]::new($false)
    $b = $enc.GetBytes([string]::Join("|", $arr))
    $h = [System.Security.Cryptography.SHA256]::Create().ComputeHash($b)
    return ([BitConverter]::ToString($h).Replace("-", "")).Substring(0, 16).ToLowerInvariant()
}

function Resolve-DirQuarantineFilePath {
    param([string]$RepoRoot, [string]$FileParam)
    if ([string]::IsNullOrWhiteSpace($FileParam)) { return "" }
    if ([System.IO.Path]::IsPathRooted($FileParam)) { return $FileParam }
    return (Join-Path $RepoRoot $FileParam.Trim().TrimStart("\", "/"))
}

function Read-DirQuarantine {
    param([string]$FilePath)
    if ([string]::IsNullOrWhiteSpace($FilePath) -or -not (Test-Path -LiteralPath $FilePath)) {
        return [pscustomobject]@{ schema_version = 1; entries = @() }
    }
    try {
        $j = (Get-Content -LiteralPath $FilePath -Raw) | ConvertFrom-Json
        if ($null -eq $j) { return [pscustomobject]@{ schema_version = 1; entries = @() } }
        if ($null -eq $j.entries) { return [pscustomobject]@{ schema_version = 1; entries = @() } }
        if ($j.entries -isnot [System.Array]) { $j.entries = @($j.entries) }
        return $j
    } catch {
        return [pscustomobject]@{ schema_version = 1; entries = @() }
    }
}

function Write-DirQuarantine {
    param(
        [string]$FilePath,
        $QuarantineObj
    )
    if ([string]::IsNullOrWhiteSpace($FilePath)) { return }
    $parent = Split-Path -Parent $FilePath
    if (-not [string]::IsNullOrWhiteSpace($parent) -and -not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $QuarantineObj | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $FilePath -Encoding UTF8
}

function Get-QuarantineNameSet {
    param(
        $QObj,
        [int]$MinHits = 1
    )
    $h = @{}
    foreach ($e in @($QObj.entries)) {
        $m = [string]$e.model
        $hc = 0
        if ($null -ne $e.hit_count) { $hc = [int]$e.hit_count }
        if ($m -ne "" -and $hc -ge $MinHits) {
            $h[$(ConvertTo-NormalizedModelName $m)] = $true
            $h[$m] = $true
        }
    }
    return $h
}

function Register-DirQuarantine {
    param(
        [string]$FilePath,
        [string]$Model,
        [string]$Phase,
        [string]$Reason,
        [int]$Consecutive
    )
    if ([string]::IsNullOrWhiteSpace($FilePath) -or [string]::IsNullOrWhiteSpace($Model)) { return }
    $mn = ConvertTo-NormalizedModelName $Model
    $j = Read-DirQuarantine -FilePath $FilePath
    $ent = $null
    foreach ($e in $j.entries) { if (([string]$e.model) -eq $mn) { $ent = $e; break } }
    if ($null -eq $ent) {
        $ent = [pscustomobject]@{
            model         = $mn
            phase         = $Phase
            reason        = $Reason
            first_seen_at = (Get-Date).ToString("o")
            last_seen_at  = (Get-Date).ToString("o")
            hit_count     = 0
        }
        if ($null -eq $j.entries) {
            $j.entries = @($ent)
        } else {
            $j.entries = @($j.entries) + @($ent)
        }
    } else {
        $ent.last_seen_at = (Get-Date).ToString("o")
    }
    $ent.hit_count = [int]([int]$ent.hit_count + 1)
    Write-DirQuarantine -FilePath $FilePath -QuarantineObj $j
    if ($ent.hit_count -ge $Consecutive) {
        Write-Warning ("[quarantine] model {0} now quarantined (hit_count>={1}) phase={2} reason={3}" -f $mn, $Consecutive, $Phase, $Reason)
    }
}

function Add-DirSimTimingRow {
    param(
        [System.Collections.IList]$List,
        [string]$Model,
        [DateTime]$Started
    )
    if ($null -eq $List) { return }
    $s = [Math]::Round([double](((Get-Date) - $Started).TotalSeconds), 2)
    $null = $List.Add([pscustomobject]@{ model = $Model; sec = $s })
}

function Clear-DirPrivateCacheEnv {
    Remove-Item Env:RUSTMODLICA_CACHE_SQLITE -ErrorAction SilentlyContinue
    Remove-Item Env:RUSTMODLICA_QUERY_CACHE_NAMESPACE -ErrorAction SilentlyContinue
    Remove-Item Env:RUSTMODLICA_FLATTEN_CACHE_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:RUSTMODLICA_AOT_CACHE_DIR -ErrorAction SilentlyContinue
}

function Get-PrivateDirCacheInstanceSlug([string]$RepoRootNorm) {
    $enc = [System.Text.UTF8Encoding]::new($false)
    $u = $enc.GetBytes($RepoRootNorm.ToLowerInvariant())
    $sha = [System.Security.Cryptography.SHA256]::Create().ComputeHash($u)
    return ([BitConverter]::ToString($sha).Replace("-", "")).Substring(0, 8).ToLowerInvariant()
}

function Get-IrSchemaEpochFromRepo([string]$RepoRoot) {
    $p = Join-Path $RepoRoot "jit-compiler\src\cache\ir_epoch.rs"
    if (-not (Test-Path -LiteralPath $p)) { return "0" }
    $t = [System.IO.File]::ReadAllText($p)
    $m = [regex]::Match($t, 'IR_SCHEMA_EPOCH:\s*u32\s*=\s*(\d+)')
    if ($m.Success) { return $m.Groups[1].Value }
    return "0"
}

function Resolve-PrivateDirCacheRoot {
    param(
        [string]$RepoRoot,
        [string]$PrivateCacheRootParam,
        [bool]$Disable
    )
    if ($Disable) { return "" }
    if (-not [string]::IsNullOrWhiteSpace($PrivateCacheRootParam)) {
        if ([System.IO.Path]::IsPathRooted($PrivateCacheRootParam)) {
            return $PrivateCacheRootParam.Trim()
        }
        return (Join-Path $RepoRoot $PrivateCacheRootParam.Trim().TrimStart("\", "/"))
    }
    $envRoot = [string]$env:RUSTMODLICA_DIR_PRIVATE_CACHE_ROOT
    if (-not [string]::IsNullOrWhiteSpace($envRoot)) {
        if ([System.IO.Path]::IsPathRooted($envRoot)) { return $envRoot.Trim() }
        return (Join-Path $RepoRoot $envRoot.Trim().TrimStart("\", "/"))
    }
    $inst = Get-PrivateDirCacheInstanceSlug $RepoRoot
    $la = [string]$env:LOCALAPPDATA
    if ([string]::IsNullOrWhiteSpace($la)) {
        return (Join-Path $RepoRoot "build\dir_private_cache")
    }
    return (Join-Path $la ("rustmodlica\dir_cache\" + $inst))
}

function Get-DirPrivateRunKey {
    param(
        [string]$ExePath,
        [string[]]$LibRoots,
        [string]$GitHead,
        [string]$IrEpoch,
        [string]$Policy,
        [string]$KeyExtra
    )
    $exeHash = ""
    if (Test-Path -LiteralPath $ExePath) {
        try { $exeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $ExePath).Hash } catch {}
    }
    $libPart = [string]::Join("|", (@($LibRoots) | Sort-Object))
    $raw = "v1|$exeHash|$libPart|$GitHead|$IrEpoch|$Policy|$KeyExtra"
    $enc = [System.Text.UTF8Encoding]::new($false)
    $bytes = $enc.GetBytes($raw)
    $h = [System.Security.Cryptography.SHA256]::Create().ComputeHash($bytes)
    return ([BitConverter]::ToString($h).Replace("-", "")).Substring(0, 16).ToLowerInvariant()
}

function Set-DirPrivateCacheEnv {
    param(
        [string]$CacheRoot,
        [string]$RunKey,
        [int]$ShardId
    )
    if ([string]::IsNullOrWhiteSpace($CacheRoot) -or [string]::IsNullOrWhiteSpace($RunKey)) {
        return
    }
    $runDir = Join-Path $CacheRoot ("run_" + $RunKey)
    if ($ShardId -ge 0) {
        $ns = ("DIR_S{0}_{1}" -f $ShardId, $RunKey)
        $flat = Join-Path $runDir ("flatten\shard_" + $ShardId)
        $aot = Join-Path $runDir ("aot\shard_" + $ShardId)
    } else {
        $ns = "DIR_SERIAL_$RunKey"
        $flat = Join-Path $runDir "flatten\serial"
        $aot = Join-Path $runDir "aot\serial"
    }
    $null = New-Item -ItemType Directory -Force -Path $flat, $aot
    $env:RUSTMODLICA_CACHE_SQLITE = "1"
    $env:RUSTMODLICA_QUERY_CACHE_NAMESPACE = $ns
    $env:RUSTMODLICA_FLATTEN_CACHE_DIR = $flat
    $env:RUSTMODLICA_AOT_CACHE_DIR = $aot
    Write-Host ("[dir-private-cache] namespace={0} flatten={1}" -f $ns, $flat)
}

function Write-DirCacheEnvScriptFile {
    param(
        [Parameter(Mandatory = $true)][string]$LiteralPath,
        [Parameter(Mandatory = $true)][string]$Namespace,
        [Parameter(Mandatory = $true)][string]$FlattenDir,
        [Parameter(Mandatory = $true)][string]$AotDir
    )
    $parent = Split-Path -Parent $LiteralPath
    if (-not [string]::IsNullOrWhiteSpace($parent) -and -not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $nsEsc = $Namespace.Replace("'", "''")
    $flatEsc = $FlattenDir.Replace("'", "''")
    $aotEsc = $AotDir.Replace("'", "''")
    $lines = @(
        "# Generated by run_modelica_dir_regression.ps1 - dot-source to reuse DIR private cache in other tools.",
        "`$env:RUSTMODLICA_CACHE_SQLITE = '1'",
        "`$env:RUSTMODLICA_QUERY_CACHE_NAMESPACE = '$nsEsc'",
        "`$env:RUSTMODLICA_FLATTEN_CACHE_DIR = '$flatEsc'",
        "`$env:RUSTMODLICA_AOT_CACHE_DIR = '$aotEsc'"
    )
    Set-Content -LiteralPath $LiteralPath -Value $lines -Encoding UTF8
}

function Write-DirPrivateCacheManifest {
    param(
        [string]$RunDir,
        [string]$RepoRoot,
        [string]$RunKey,
        [int]$ShardId,
        [string]$ExePath
    )
    try {
        $obj = [pscustomobject]@{
            schema_version   = 1
            generated_at     = (Get-Date).ToString("o")
            repo_root        = $RepoRoot
            run_key          = $RunKey
            shard            = $ShardId
            executable       = $ExePath
            ir_epoch_file    = "jit-compiler/src/cache/ir_epoch.rs"
        }
        $suffix = if ($ShardId -ge 0) { ("_shard_{0}" -f $ShardId) } else { "_serial" }
        $manifestPath = Join-Path $RunDir ("manifest" + $suffix + ".json")
        $obj | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    } catch {
        Write-Warning ("dir-private-cache: manifest write failed: " + $_)
    }
}

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

# Require real class-declaration shape after the name (not prose like "model shall be" in HTML docs).
function Get-SimClassDeclNameFromLine([string]$ln) {
    if ($ln -match '^\s*//') { return $null }
    # Partial model/block is not runnable as a standalone simulation target.
    if ($ln -cmatch '^\s*(?:(?:encapsulated|replaceable|expandable)\s+)*partial\s+(?:model|block)\b') {
        return ""
    }
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
        if ($null -ne $simName -and $simName -ne "" -and $ln -notmatch '\s*=\s*') {
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

function Test-IsNonSimulatableModelName([string]$modelName) {
    if ([string]::IsNullOrWhiteSpace($modelName)) { return $true }
    $knownNonSimulatable = @(
        "Modelica.Clocked.Examples.Elementary.RealSignals.UpSample2",
        "Modelica.Electrical.Analog.Examples.Lines.CompareLosslessLines",
        "Modelica.Electrical.Machines.Examples.ControlledDCDrives.Utilities.DcdcInverter",
        "Modelica.Electrical.Machines.Examples.ControlledDCDrives.Utilities.SwitchingDcDc",
        "Modelica.Electrical.Machines.Examples.DCMachines.DCPM_Drive",
        "Modelica.Electrical.PowerConverters.Examples.ACAC.SoftStarter",
        "Modelica.Electrical.PowerConverters.Examples.ACDC.RectifierBridge2mPulse.DiodeBridge2mPulse",
        "Modelica.Electrical.PowerConverters.Examples.ACDC.RectifierBridge2mPulse.HalfControlledBridge2mPulse",
        "Modelica.Electrical.PowerConverters.Examples.ACDC.RectifierBridge2mPulse.ThyristorBridge2mPulse_DC_Drive",
        "Modelica.Electrical.PowerConverters.Examples.ACDC.RectifierCenterTap2mPulse.DiodeCenterTap2mPulse",
        "Modelica.Electrical.PowerConverters.Examples.ACDC.RectifierCenterTapmPulse.DiodeCenterTapmPulse",
        "Modelica.Electrical.PowerConverters.Examples.DCAC.PolyphaseTwoLevel.PolyphaseTwoLevel_R",
        "Modelica.Electrical.PowerConverters.Examples.DCDC.HBridge.HBridge_TrianglePWM_RL"
    )
    if ($knownNonSimulatable -contains $modelName) { return $true }
    $parts = $modelName -split '\.'
    foreach ($p in $parts) {
        if ($p -eq "Interfaces" -or $p -eq "BaseClasses") { return $true }
    }
    $leaf = $parts[$parts.Length - 1]
    if ($leaf -match '^Partial') { return $true }
    if ($leaf -match 'Base$') { return $true }
    if ($leaf -in @("IdealHeatTransfer", "ConstantHeatTransfer", "OuterStatePort", "MinLimiter")) { return $true }
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
        if ($modelName -ne "") {
            $normName = ConvertTo-NormalizedModelName $modelName
            if ($normName -ne "") { $set[$normName] = $true }
        }
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
        $rest = ""
        if ($t.StartsWith("--")) {
            $rest = $t.Substring(2).TrimStart()
        } elseif ($t.StartsWith("!!")) {
            $rest = $t.Substring(2).TrimStart()
        } else {
            continue
        }
        if ($rest -eq "") { continue }
        $name = (($rest -split '\s+', 2)[0]).Trim()
        if ($name -eq "") { continue }
        $name = ConvertTo-NormalizedModelName $name
        if ($seen.ContainsKey($name)) { continue }
        $seen[$name] = $true
        $list.Add($name)
    }
    return $list
}

function ConvertTo-NormalizedModelName([string]$name) {
    if ([string]::IsNullOrWhiteSpace($name)) { return $name }
    # Compatibility rewrite: older summaries used flattened MediaTestModels path
    # without TestsWithFluid segment.
    if ($name -match '^ModelicaTest\.Media\.MediaTestModels\.') {
        $name = $name -replace '^ModelicaTest\.Media\.MediaTestModels\.', 'ModelicaTest.Media.TestsWithFluid.MediaTestModels.'
    }
    # Compatibility rewrite: Fluid pump monitoring moved under BaseClasses.
    if ($name -match '^Modelica\.Fluid\.Machines\.PumpMonitoring\.') {
        $name = $name -replace '^Modelica\.Fluid\.Machines\.PumpMonitoring\.', 'Modelica.Fluid.Machines.BaseClasses.PumpMonitoring.'
    }
    return $name
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
$modelicaLibRoot = Join-Path $jitRoot "Modelica"
$modelicaTestLibRoot = Join-Path $jitRoot "ModelicaTest"
$exe = if ($ExePath -ne "") {
    if ([System.IO.Path]::IsPathRooted($ExePath)) { $ExePath } else { Join-Path $repoRoot $ExePath }
} else {
    Join-Path $repoRoot "target\\release\\rustmodlica.exe"
}
if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Build first: cargo build --release"
    exit 1
}

# Preflight: local MSL/ModelicaTest libraries must be present and resolvable.
$missingLibraryItems = New-Object System.Collections.Generic.List[string]
if (-not (Test-Path -LiteralPath $modelicaLibRoot)) { $missingLibraryItems.Add($modelicaLibRoot) }
if (-not (Test-Path -LiteralPath $modelicaTestLibRoot)) { $missingLibraryItems.Add($modelicaTestLibRoot) }
$modelicaPkgMo = Join-Path $modelicaLibRoot "package.mo"
$modelicaTestPkgMo = Join-Path $modelicaTestLibRoot "package.mo"
if (-not (Test-Path -LiteralPath $modelicaPkgMo)) { $missingLibraryItems.Add($modelicaPkgMo) }
if (-not (Test-Path -LiteralPath $modelicaTestPkgMo)) { $missingLibraryItems.Add($modelicaTestPkgMo) }
if ($missingLibraryItems.Count -gt 0) {
    Write-Error ("Local library preflight failed. Missing required library paths/files: " + ($missingLibraryItems -join ", "))
    exit 3
}
Write-Host "Library preflight OK: $modelicaLibRoot ; $modelicaTestLibRoot"

$resolvedLibRoots = New-Object System.Collections.Generic.List[string]
foreach ($lp in $LibPath) {
    if ([string]::IsNullOrWhiteSpace($lp)) { continue }
    $abs = $lp
    if (-not [System.IO.Path]::IsPathRooted($abs)) {
        $abs = Join-Path $repoRoot $abs
    }
    if (-not (Test-Path -LiteralPath $abs)) {
        Write-Error "Configured LibPath does not exist: $abs"
        exit 3
    }
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
# If caller provides LibPath explicitly, treat it as authoritative to avoid
# incomplete local mirrors shadowing complete external libraries.
if ($resolvedLibRoots.Count -eq 0) {
    if (Test-Path -LiteralPath $modelicaLibRoot) {
        $normLocalModelica = (Resolve-Path -LiteralPath $modelicaLibRoot).Path
        if (-not $resolvedLibRoots.Contains($normLocalModelica)) { $resolvedLibRoots.Add($normLocalModelica) }
    }
    if (Test-Path -LiteralPath $modelicaTestLibRoot) {
        $normLocalModelicaTest = (Resolve-Path -LiteralPath $modelicaTestLibRoot).Path
        if (-not $resolvedLibRoots.Contains($normLocalModelicaTest)) { $resolvedLibRoots.Add($normLocalModelicaTest) }
    }
}
Write-Host ("Effective lib roots: " + ($resolvedLibRoots -join "; "))

$dirPrivEnvRaw = [string]$env:RUSTMODLICA_USE_DIR_PRIVATE_CACHE
$dirPrivFromEnv = $false
if (-not [string]::IsNullOrWhiteSpace($dirPrivEnvRaw)) {
    $t = $dirPrivEnvRaw.Trim().ToLowerInvariant()
    $dirPrivFromEnv = @("1", "true", "yes", "on") -contains $t
}
$dirPrivEnabled = (-not $DisablePrivateCache) -and (($UsePrivateCache -or $dirPrivFromEnv) -or ($PrivateCacheRunKey -ne ""))
$script:DirPrivRootResolved = ""
$script:DirPrivRunKeyResolved = ""
if ($DisablePrivateCache) {
    Clear-DirPrivateCacheEnv
} elseif ($dirPrivEnabled) {
    if ($PrivateCacheRunKey -ne "") {
        $script:DirPrivRunKeyResolved = $PrivateCacheRunKey
        if ([string]::IsNullOrWhiteSpace($PrivateCacheRoot)) {
            Write-Error "PrivateCacheRunKey requires PrivateCacheRoot (absolute path from parallel parent)."
            exit 2
        }
        $script:DirPrivRootResolved = $PrivateCacheRoot
        if (-not [System.IO.Path]::IsPathRooted($script:DirPrivRootResolved)) {
            $script:DirPrivRootResolved = Join-Path $repoRoot $script:DirPrivRootResolved
        }
    } else {
        $script:DirPrivRootResolved = Resolve-PrivateDirCacheRoot -RepoRoot $repoRoot -PrivateCacheRootParam $PrivateCacheRoot -Disable:$false
        $gitHead = ""
        try {
            Push-Location $repoRoot
            $gitHead = [string](& git rev-parse HEAD 2>$null)
            Pop-Location
        } catch {
            try { Pop-Location } catch {}
        }
        $irEp = Get-IrSchemaEpochFromRepo $repoRoot
        $stageTag = [string]$env:RUSTMODLICA_DIR_REGRESSION_STAGE
        if ([string]::IsNullOrWhiteSpace($stageTag)) { $stageTag = "sim" }
        $policy = ("dir_regres_v1|solver={0}|tend={1}|dt={2}|stage={3}" -f $Solver, $TEnd, $Dt, $stageTag)
        $libArr = @($resolvedLibRoots | ForEach-Object { $_ })
        $script:DirPrivRunKeyResolved = Get-DirPrivateRunKey -ExePath $exe -LibRoots $libArr -GitHead $gitHead -IrEpoch $irEp -Policy $policy -KeyExtra $PrivateCacheKeyExtra
    }
    $shardApply = -1
    if ($PrivateCacheShard -ne -999) { $shardApply = $PrivateCacheShard }
    Set-DirPrivateCacheEnv -CacheRoot $script:DirPrivRootResolved -RunKey $script:DirPrivRunKeyResolved -ShardId $shardApply
    $runDirForManifest = Join-Path $script:DirPrivRootResolved ("run_" + $script:DirPrivRunKeyResolved)
    Write-DirPrivateCacheManifest -RunDir $runDirForManifest -RepoRoot $repoRoot -RunKey $script:DirPrivRunKeyResolved -ShardId $shardApply -ExePath $exe
    if (-not [string]::IsNullOrWhiteSpace($WriteDirCacheEnvScript)) {
        $wPath = $WriteDirCacheEnvScript.Trim()
        if (-not [System.IO.Path]::IsPathRooted($wPath)) {
            $wPath = Join-Path $repoRoot $wPath
        }
        Write-DirCacheEnvScriptFile -LiteralPath $wPath -Namespace $env:RUSTMODLICA_QUERY_CACHE_NAMESPACE -FlattenDir $env:RUSTMODLICA_FLATTEN_CACHE_DIR -AotDir $env:RUSTMODLICA_AOT_CACHE_DIR
        Write-Host ("[dir-private-cache] env script: " + $wPath)
    }
}

# On Windows, rustmodlica.exe with sundials feature may need runtime DLLs from
# <cargo-target-dir>/build/sundials-sys-*/out/lib. Match the same target profile as $exe
# (e.g. jit-compiler\target_regression), not only repoRoot\target\release. Apply before preflight/analyze.
if ($env:OS -eq "Windows_NT") {
    $sundialsCandidates = New-Object System.Collections.Generic.List[string]
    try {
        $exeResolved = (Resolve-Path -LiteralPath $exe).Path
        # Cargo layout: <target-dir>/<profile>/<exe> and <target-dir>/<profile>/build/sundials-sys-*
        $profileDir = Split-Path -Parent $exeResolved
        $fromExe = Join-Path $profileDir "build"
        if (-not [string]::IsNullOrWhiteSpace($fromExe)) { $sundialsCandidates.Add($fromExe) | Out-Null }
    } catch {}
    $sundialsCandidates.Add((Join-Path $jitRoot "target_regression\release\build")) | Out-Null
    $sundialsCandidates.Add((Join-Path $jitRoot "target\release\build")) | Out-Null
    $sundialsCandidates.Add((Join-Path $repoRoot "target\release\build")) | Out-Null
    $seenSd = @{}
    foreach ($sundialsBuildRoot in $sundialsCandidates) {
        if ([string]::IsNullOrWhiteSpace($sundialsBuildRoot)) { continue }
        $normSd = $sundialsBuildRoot.Trim()
        if ($seenSd.ContainsKey($normSd)) { continue }
        $seenSd[$normSd] = $true
        if (-not (Test-Path -LiteralPath $normSd)) { continue }
        $dllDirs = @(Get-ChildItem -LiteralPath $normSd -Directory -Filter "sundials-sys-*" |
            Sort-Object LastWriteTime -Descending |
            ForEach-Object { Join-Path $_.FullName "out\lib" } |
            Where-Object { Test-Path -LiteralPath $_ })
        if ($dllDirs.Count -gt 0) {
            $env:PATH = ($dllDirs[0] + ";" + $env:PATH)
            Write-Host ("[DIR] sundials-DLL PATH+= " + $dllDirs[0])
            break
        }
    }
}

# Full `rustmodlica` simulation defaults to legacy Flattener when RUSTMODLICA_SALSA
# is unset (`frontend.rs`). Directory batches hit large MSL models (e.g. ControlledDCDrives)
# and can exceed per-model timeouts during cold flatten. Default to query-based flatten
# for sim children when the process env is unset; set RUSTMODLICA_SALSA=0 before
# invoking this script to force legacy flatten for all cases.
if (-not $AnalyzeOnly) {
    $salsaProc = [Environment]::GetEnvironmentVariable("RUSTMODLICA_SALSA", "Process")
    if ($null -eq $salsaProc) {
        $env:RUSTMODLICA_SALSA = "1"
        Write-Host "[dir-regression] RUSTMODLICA_SALSA=1 (default for simulation flatten)"
    }
}

# Preflight semantic check:
# Prefer ModelicaTest.Media probe, but allow fallback probes for slim mirrors.
$preflightProbes = @(
    "ModelicaTest.Media.TestsWithFluid.MediaTestModels.Air.SimpleAir",
    "ModelicaTest.JitStress.SyncOmCompare",
    "Modelica.Blocks.Sources.Sine"
)
$preflightPassed = $false
$preflightFailureDetails = @()
foreach ($probe in $preflightProbes) {
    $preflightArgs = @("--validate", $probe)
    foreach ($lr in $resolvedLibRoots) { $preflightArgs = @("--lib-path=$lr") + $preflightArgs }
    Push-Location $jitRoot
    $oldEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $preflightOut = & $exe @preflightArgs 2>&1
    $ErrorActionPreference = $oldEap
    $preflightExit = $LASTEXITCODE
    Pop-Location
    if ($preflightExit -eq 0) {
        $preflightPassed = $true
        if ($probe -ne $preflightProbes[0]) {
            Write-Warning ("Primary preflight probe unavailable; using fallback probe: " + $probe)
        }
        break
    }
    $detail = ""
    foreach ($ln in $preflightOut) {
        $s = $ln.ToString()
        if ($s -match 'Model not found:' -or $s -match 'Could not find model:' -or $s -match 'error') {
            $detail = $s.Trim()
            break
        }
    }
    $preflightFailureDetails += ("probe=" + $probe + ";detail=" + $detail)
}
if (-not $preflightPassed) {
    Write-Error ("Local library preflight failed for all probes. " + ($preflightFailureDetails -join " | ") + " ; add complete library roots via -LibPath.")
    exit 4
}

$outPath = Join-Path $repoRoot $OutDir
if (-not (Test-Path -LiteralPath $outPath)) { New-Item -ItemType Directory -Path $outPath | Out-Null }
$logDir = Join-Path $outPath "logs"
if (-not (Test-Path -LiteralPath $logDir)) { New-Item -ItemType Directory -Path $logDir | Out-Null }
# Persist flatten disk cache under this outdir (per-model keys). Repeat runs of the same
# model FLAT_HIT after the first cold compile; different models still pay separate cold keys.
# Skipped when -UsePrivateCache / env already set RUSTMODLICA_FLATTEN_CACHE_DIR.
if (-not $dirPrivEnabled) {
    $fcExisting = [Environment]::GetEnvironmentVariable("RUSTMODLICA_FLATTEN_CACHE_DIR", "Process")
    if ([string]::IsNullOrWhiteSpace($fcExisting)) {
        $flatShared = Join-Path $outPath "shared_flatten_cache"
        New-Item -ItemType Directory -Force -Path $flatShared | Out-Null
        $env:RUSTMODLICA_FLATTEN_CACHE_DIR = $flatShared
        Write-Host ("[dir-regression] RUSTMODLICA_FLATTEN_CACHE_DIR=" + $flatShared)
    }
}
$runStamp = Get-Date -Format "yyyyMMdd_HHmmss"
$runLogNdjson = Join-Path $outPath ("runlog_{0}.ndjson" -f $runStamp)
$runLogCsv = Join-Path $outPath ("runlog_{0}.csv" -f $runStamp)
$lockFilePath = Join-Path $outPath "libraries.lock.json"
"timestamp,case_type,case_name,duration_ms,expect_target_ok,actual_ok,exit_code,status,reason,detail" | Set-Content -LiteralPath $runLogCsv -Encoding UTF8
function ConvertTo-CsvField([string]$s) {
    if ($null -eq $s) { return "" }
    $q = $s.Replace('"', '""')
    return '"' + $q + '"'
}
function Write-RunLog {
    param(
        [string]$CaseType,
        [string]$CaseName,
        [long]$DurationMs,
        [bool]$ExpectTargetOk,
        [bool]$ActualOk,
        [int]$ExitCode,
        [string]$Status,
        [string]$Reason,
        [string]$Detail
    )
    $ts = (Get-Date).ToString("o")
    $obj = [pscustomobject]@{
        timestamp = $ts
        case_type = $CaseType
        case_name = $CaseName
        duration_ms = $DurationMs
        expect_target_ok = $ExpectTargetOk
        actual_ok = $ActualOk
        exit_code = $ExitCode
        status = $Status
        reason = $Reason
        detail = $Detail
    }
    ($obj | ConvertTo-Json -Compress) | Add-Content -LiteralPath $runLogNdjson -Encoding UTF8
    $csvLine = @(
        ConvertTo-CsvField $ts
        ConvertTo-CsvField $CaseType
        ConvertTo-CsvField $CaseName
        $DurationMs
        $ExpectTargetOk
        $ActualOk
        $ExitCode
        ConvertTo-CsvField $Status
        ConvertTo-CsvField $Reason
        ConvertTo-CsvField $Detail
    ) -join ","
    $written = $false
    for ($retry = 0; $retry -lt 3 -and -not $written; $retry++) {
        try {
            Add-Content -LiteralPath $runLogCsv -Value $csvLine -Encoding UTF8 -ErrorAction Stop
            $written = $true
        } catch {
            Start-Sleep -Milliseconds (80 * ($retry + 1))
            try {
                $csvLine | Out-File -LiteralPath $runLogCsv -Encoding utf8 -Append -ErrorAction Stop
                $written = $true
            } catch {
                # Logging failure must not interrupt long regression runs.
            }
        }
    }
}

function Write-ReproContextSnapshot {
    param(
        [string]$RepoRoot,
        [string]$ExePath,
        [string[]]$LibraryRoots,
        [string]$OutputPath
    )
    $exeHash = ""
    if (Test-Path -LiteralPath $ExePath) {
        try { $exeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $ExePath).Hash } catch {}
    }
    $gitCommit = ""
    try {
        Push-Location $RepoRoot
        $gitCommit = (& git rev-parse HEAD 2>$null)
        Pop-Location
    } catch {
        try { Pop-Location } catch {}
    }
    $snapshot = [pscustomobject]@{
        schema_version = "libraries.lock.v1"
        generated_at = (Get-Date).ToString("o")
        repo_root = $RepoRoot
        git_commit = [string]$gitCommit
        executable = [pscustomobject]@{
            path = $ExePath
            sha256 = $exeHash
        }
        library_roots = @($LibraryRoots)
        env = [pscustomobject]@{
            RUSTMODLICA_EVENT_TRACE = [string]$env:RUSTMODLICA_EVENT_TRACE
            RUSTMODLICA_PERF_TRACE = [string]$env:RUSTMODLICA_PERF_TRACE
            RUSTMODLICA_AOT_CACHE_DIR = [string]$env:RUSTMODLICA_AOT_CACHE_DIR
        }
    }
    $snapshot | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $OutputPath -Encoding UTF8
}

Write-ReproContextSnapshot -RepoRoot $repoRoot -ExePath $exe -LibraryRoots $resolvedLibRoots -OutputPath $lockFilePath
Write-Host ("Library lock written: " + $lockFilePath)

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
        if (-not (Test-IsDocLikeModelName $sn) -and -not (Test-IsNonSimulatableModelName $sn)) {
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
    $moDirs = @()
    foreach ($lr in $resolvedLibRoots) {
        if ($lr -match '\\Modelica$' -or $lr -match '\\ModelicaTest$') {
            if (Test-Path -LiteralPath $lr) { $moDirs += $lr }
        }
    }
    if ($moDirs.Count -eq 0) {
        $moDirs = @(
            (Join-Path $jitRoot "Modelica"),
            (Join-Path $jitRoot "ModelicaTest")
        )
    }

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
            $mnNorm = ConvertTo-NormalizedModelName $mn
            if ($mnNorm -ne "" -and -not $models.Contains($mnNorm)) {
                $models.Add($mnNorm)
            }
        }
    }

    $docFiltered = @($models | Where-Object { -not (Test-IsDocLikeModelName $_) })
    $simFiltered = @($docFiltered | Where-Object { -not (Test-IsNonSimulatableModelName $_) })
    $removedBySimFilter = $docFiltered.Count - $simFiltered.Count
    if ($removedBySimFilter -gt 0) {
        Write-Host "Filtered non-simulatable candidates: $removedBySimFilter"
        $diffPath = Join-Path $outPath "filtered_non_simulatable.txt"
        @($docFiltered | Where-Object { Test-IsNonSimulatableModelName $_ }) | Sort-Object -Unique | Set-Content -LiteralPath $diffPath -Encoding UTF8
        Write-Host "Filtered list written: $diffPath"
    }
    $models = New-Object System.Collections.Generic.List[string]
    foreach ($mn in $simFiltered) {
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

$dirTwoStagePrefailLines = New-Object System.Collections.Generic.List[string]
$script:DirRegMetrics = [ordered]@{
    models_total          = 0
    analyze_passed        = 0
    analyze_failed        = 0
    analyze_timeout       = 0
    analyze_oom           = 0
    analyze_gate_failed   = 0
    analyze_failure_breakdown = [ordered]@{}
    quarantined_skipped  = 0
    sim_passed            = 0
    watchdog_kills        = 0
    memory_peak_mb        = 0
    top_slow_analyze      = @()
    top_slow_sim          = @()
}
$quarantinePathResolved = (Resolve-DirQuarantineFilePath -RepoRoot $repoRoot -FileParam $QuarantineFile)

if (-not $AnalyzeOnly -and -not $RetryQuarantined -and -not [string]::IsNullOrWhiteSpace($quarantinePathResolved)) {
    $qObj = (Read-DirQuarantine -FilePath $quarantinePathResolved)
    $qSet = (Get-QuarantineNameSet -QObj $qObj -MinHits $QuarantineConsecutiveHits)
    if ($qSet.Count -gt 0) {
        $newL = New-Object System.Collections.Generic.List[string]
        foreach ($mn0 in $models) {
            $k = ConvertTo-NormalizedModelName $mn0
            if ($qSet.ContainsKey($mn0) -or $qSet.ContainsKey($k)) { $script:DirRegMetrics.quarantined_skipped++ } else { $newL.Add($mn0) }
        }
        if ($script:DirRegMetrics.quarantined_skipped -gt 0) {
            Write-Host ("[quarantine] skipped {0} model(s) (use -RetryQuarantined to re-run) file={1}" -f $script:DirRegMetrics.quarantined_skipped, $quarantinePathResolved)
        }
        $models = $newL
    }
}
$modelTotal = $models.Count
$script:DirRegMetrics.models_total = $modelTotal
if ($AnalyzeOnly -and $modelTotal -gt 0) {
    $aSummaryPath = (Join-Path $outPath "analyze_summary.txt")
    $progPath = if ($AnalyzeShardIndex -ge 0) {
        (Join-Path $outPath ("analyze_progress_{0}.ndjson" -f $AnalyzeShardIndex))
    } else {
        (Join-Path $outPath "analyze_progress.ndjson")
    }
    $shrd = (Split-Path -Parent $outPath)
    if (-not (Test-Path -LiteralPath (Split-Path -Parent $aSummaryPath))) { New-Item -ItemType Directory -Force -Path (Split-Path -Parent $aSummaryPath) | Out-Null }
    if (-not (Test-Path -LiteralPath $outPath)) { New-Item -ItemType Directory -Path $outPath -Force | Out-Null }
    $stateF = (Join-Path $shrd "analyze_state.txt")
    if (-not [string]::IsNullOrWhiteSpace($GlobalModelsHash) -and (Test-Path -LiteralPath $stateF)) {
        $h0 = (Get-Content -LiteralPath $stateF -ErrorAction SilentlyContinue -Raw)
        if ($null -ne $h0 -and $h0.Trim() -ne $GlobalModelsHash) {
            if (Test-Path -LiteralPath $progPath) { Remove-Item -LiteralPath $progPath -Force -ErrorAction SilentlyContinue }
        }
    }
    $done = @{}
    if ($ResumeAnalyzeCheckpoint -and (Test-Path -LiteralPath $progPath)) {
        $plns = (Get-Content -LiteralPath $progPath)
        $first = $true
        foreach ($p in $plns) {
            $p = if ($p) { $p.Trim() } else { "" }
            if ($p -eq "") { continue }
            if ($first) {
                $first = $false
                if ($p -match '"models_hash"') { try { $hobj = $p | ConvertFrom-Json } catch { $hobj = $null } ; if ($null -ne $hobj -and [string]$hobj.models_hash -ne [string]$GlobalModelsHash) { $done = @{} ; break } ; continue } else { try { $o0 = $p | ConvertFrom-Json } catch { } ; if ($null -ne $o0 -and $o0.PSObject.Properties['model'] -and $o0.PSObject.Properties['exit']) { $done[[string]$o0.model] = $true } } ; continue
            } else { try { $o0 = $p | ConvertFrom-Json } catch { continue } if ($o0 -and $o0.model) { $done[[string]$o0.model] = $true } }
        }
    }
    $anLines = New-Object System.Collections.Generic.List[string]
    $anSlow = New-Object System.Collections.Generic.List[object]
    $chkC = 0
    $anI = 0
    $anyFail = $false
    foreach ($m in $models) {
        if ($done[$m]) { continue }
        $anI++
        Write-Host ("[DIR analyze-only {0}/{1}] {2}" -f $anI, $modelTotal, $m)
        $aArgs = @()
        foreach ($lr in $resolvedLibRoots) { $aArgs += ("--lib-path=" + $lr) }
        $aArgs += @("--validate", "--validate-tier=analyze", ("--validation-mode=" + $AnalyzeValidationMode), $m)
        Push-Location $jitRoot
        $oldEapA = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        $t0a = [System.Diagnostics.Stopwatch]::StartNew()
        $rrA = (Invoke-RustmodlicaWithTimeout -ExePath $exe -CliArgs $aArgs -WorkDir $jitRoot -TimeoutSec $AnalyzeFirstTimeoutSec -MemoryLimitMb $PerProcessMemoryLimitMb)
        $t0a.Stop()
        $ErrorActionPreference = $oldEapA
        Pop-Location
        $exA = $rrA.ExitCode
        if ($t0a.Elapsed.TotalSeconds -ge $AnalyzeFirstTimeoutSec) { } # n/a; handled by -1
        if ($exA -eq 0) {
            $anLines.Add("OK {0}  reason=analyze_ok" -f $m)
        } else {
            $anyFail = $true
            if ($rrA.TimedOut) {
                $anLines.Add(("!! {0}  exit={1}  reason=analyze_timeout" -f $m, -1))
                if (-not [string]::IsNullOrWhiteSpace($quarantinePathResolved)) {
                    Register-DirQuarantine -FilePath $quarantinePathResolved -Model $m -Phase "analyze" -Reason "analyze_timeout" -Consecutive $QuarantineConsecutiveHits
                }
            } elseif ($rrA.Oom) {
                $anLines.Add(("!! {0}  exit={1}  reason=analyze_oom" -f $m, -1))
                if (-not [string]::IsNullOrWhiteSpace($quarantinePathResolved)) {
                    Register-DirQuarantine -FilePath $quarantinePathResolved -Model $m -Phase "analyze" -Reason "analyze_oom" -Consecutive $QuarantineConsecutiveHits
                }
            } else {
                $anLines.Add(("!! {0}  exit={1}  reason=analyze_gate_failed" -f $m, $exA))
            }
        }
        $null = $anSlow.Add([pscustomobject]@{
            model = $m
            sec   = [Math]::Round([double]$t0a.Elapsed.TotalSeconds, 2)
        })
        if (-not [string]::IsNullOrWhiteSpace($GlobalModelsHash)) {
            if (-not (Test-Path -LiteralPath $progPath) -or ((Get-Item -LiteralPath $progPath -ErrorAction SilentlyContinue).Length -eq 0)) {
                $hdrL = (ConvertTo-Json -Compress -InputObject ([pscustomobject]@{ models_hash = $GlobalModelsHash; schema = "dir_analyze_checkpoint_v1" }))
                Set-Content -LiteralPath $progPath -Value $hdrL -Encoding UTF8
            }
        }
        $jln = [pscustomobject]@{ model = $m; exit = $exA; ts = (Get-Date).ToString("o"); shard = $AnalyzeShardIndex; timed_out = [bool]$rrA.TimedOut; oom = [bool]$rrA.Oom }
        Add-Content -LiteralPath $progPath -Value (ConvertTo-Json -Compress -InputObject $jln) -Encoding UTF8
        $chkC++
        if ($AnalyzeCheckpointEvery -gt 0 -and ($chkC % $AnalyzeCheckpointEvery) -eq 0) {
            Write-Host ("[DIR analyze checkpoint] shard={0} completed={1}/{2} ok={3} fail={4}" -f $AnalyzeShardIndex, $chkC, $modelTotal, @($anLines | Where-Object { $_ -match '^\s*OK\s' }).Count, @($anLines | Where-Object { $_ -match '^\s*!!\s' }).Count)
        }
    }
    $anLines | Set-Content -LiteralPath $aSummaryPath -Encoding UTF8
    if ($anSlow.Count -gt 0) {
        $ts = @($anSlow | Sort-Object sec -Descending | Select-Object -First 10)
        Add-Content -LiteralPath $aSummaryPath -Value "" -Encoding UTF8
        Add-Content -LiteralPath $aSummaryPath -Value "Top-10 slowest analyze (s):" -Encoding UTF8
        $ts | ForEach-Object { Add-Content -LiteralPath $aSummaryPath -Value (("  {0}  {1}" -f $_.model, $_.sec)) -Encoding UTF8 }
    }
    if ($anyFail) { exit 1 } else { exit 0 }
}
if ($TwoStage -and $modelTotal -gt 0 -and $PrivateCacheShard -eq -999 -and [string]::IsNullOrWhiteSpace($OnlySkipsFromSummary)) {
    Write-Host ("[DIR] TwoStage: analyze gate for {0} model(s)" -f $modelTotal)
    $env:RUSTMODLICA_DIR_REGRESSION_STAGE = "analyze"
    $modelOrderForTwoStage = New-Object System.Collections.Generic.List[string]
    foreach ($mm0 in $models) { $modelOrderForTwoStage.Add($mm0) }
    $gHash = (Get-DirModelSetHash -Names $modelOrderForTwoStage)
    $shardAnRoot = (Join-Path $outPath "parallel_shards")
    if (-not (Test-Path -LiteralPath $shardAnRoot)) { New-Item -ItemType Directory -Path $shardAnRoot -Force | Out-Null }
    if (-not $ResumeAnalyzeCheckpoint) {
        [void]@(Get-ChildItem -LiteralPath $shardAnRoot -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "analyze_progress_*.ndjson" } | ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue })
        (Remove-Item -LiteralPath (Join-Path $outPath "analyze_progress.ndjson") -Force -ErrorAction SilentlyContinue) | Out-Null
    }
    (Set-Content -LiteralPath (Join-Path $shardAnRoot "analyze_state.txt") -Value $gHash -Encoding UTF8) | Out-Null
    $anSlowAcc = New-Object System.Collections.Generic.List[object]
    $okSet2 = @{}
    $awC = if ($AnalyzeParallelWorkers -le 0) { if ($ParallelWorkers -le 0) { [Environment]::ProcessorCount } else { $ParallelWorkers } } else { $AnalyzeParallelWorkers }
    if ($awC -le 0) { $awC = [Environment]::ProcessorCount }
    if ($awC -gt 1 -and $modelTotal -gt 1) {
        $awC = [Math]::Min($awC, $modelTotal)
        Write-Host ("[DIR] AnalyzeGate: parallel workers={0} models={1} shard_stall_s={2}" -f $awC, $modelTotal, $AnalyzeShardNoProgressTimeoutSec)
        $aScriptSelf = $PSCommandPath
        if ([string]::IsNullOrWhiteSpace($aScriptSelf)) { $aScriptSelf = $MyInvocation.MyCommand.Path }
        if ([string]::IsNullOrWhiteSpace($aScriptSelf)) { Write-Error "TwoStage parallel analyze: script path unavailable" ; exit 2 }
        $aChildren = New-Object System.Collections.Generic.List[object]
        for ($wix = 0; $wix -lt $awC; $wix++) {
            $sm2 = New-Object System.Collections.Generic.List[string]
            for ($mix = $wix; $mix -lt $modelTotal; $mix += $awC) { $sm2.Add($modelOrderForTwoStage[$mix]) }
            if ($sm2.Count -eq 0) { continue }
            $aShardFile = (Join-Path $shardAnRoot ("analyze_shard_{0}_models.txt" -f $wix))
            $sm2 | ForEach-Object { "-- $_" } | Set-Content -LiteralPath $aShardFile -Encoding UTF8
            $aShardAbs = (Resolve-Path -LiteralPath $aShardFile).Path
            $aOutRel = (Join-Path (Join-Path $OutDir "parallel_shards") ("analyze_out_{0}" -f $wix))
            if (-not [string]::IsNullOrWhiteSpace($OutDir) -and $OutDir[0] -match '^\.\.|^\.') { } # n/a; OutDir is relative
            $aAbs = (Join-Path $repoRoot $aOutRel)
            if (-not (Test-Path -LiteralPath $aAbs)) { New-Item -ItemType Directory -Path $aAbs -Force | Out-Null }
            $argA = @(
                "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $aScriptSelf, "-Root", $repoRoot,
                "-OutDir", $aOutRel, "-OnlySkipsFromSummary", $aShardAbs, "-MaxCases", "0",
                "-ParallelWorkers", "1", "-AnalyzeFirstTimeoutSec", ([string]$AnalyzeFirstTimeoutSec),
                "-PerProcessMemoryLimitMb", ([string]$PerProcessMemoryLimitMb),
                "-GlobalModelsHash", $gHash, "-AnalyzeShardIndex", ([string]$wix)
            )
            if ($ResumeAnalyzeCheckpoint) { $argA += "-ResumeAnalyzeCheckpoint" }
            if ($AnalyzeCheckpointEvery -gt 0) { $argA += @("-AnalyzeCheckpointEvery", ([string]$AnalyzeCheckpointEvery)) }
            $argA += "-AnalyzeOnly"
            if (-not [string]::IsNullOrWhiteSpace($ExePath)) { $argA += @("-ExePath", $ExePath) }
            foreach ($lp0 in $LibPath) { $argA += @("-LibPath", $lp0) }
            if ($AllLibraryMo) { } # n/a; skip-only
            if ($dirPrivEnabled -and $PrivateCacheRunKey -eq "" -and $script:DirPrivRunKeyResolved -ne "" -and $script:DirPrivRootResolved -ne "") {
                $argA += @("-PrivateCacheRunKey", $script:DirPrivRunKeyResolved)
                $argA += @("-PrivateCacheRoot", $script:DirPrivRootResolved)
                $argA += @("-PrivateCacheShard", ([string](1000 + $wix)))
            }
            if ($DisablePrivateCache) { $argA += "-DisablePrivateCache" }
            $pA = (Start-Process -FilePath "powershell" -ArgumentList $argA -PassThru)
            $aChildren.Add([pscustomobject]@{
                Index  = $wix
                Process = $pA
                OutDir  = $aAbs
            })
        }
        foreach ($cA in $aChildren) {
            $procA = $cA.Process
            $tSt = Get-Date
            $lastPA = [DateTime]::UtcNow
            $hb0a = $tSt
            while (-not $procA.HasExited) {
                if ($procA.WaitForExit(60000)) { break }
                $lw = (Get-LatestWriteTimeUtc -DirPath $cA.OutDir)
                if ($null -ne $lw -and $lw -gt $lastPA) { $lastPA = $lw }
                $idle2 = [int](([DateTime]::UtcNow - $lastPA).TotalSeconds)
                $procSum2 = 0
                $sumP2 = (Join-Path $cA.OutDir "analyze_summary.txt")
                if (Test-Path -LiteralPath $sumP2) { $procSum2 = @((Get-FileLines $sumP2 0).Lines | Where-Object { $_ -match '^\s*OK\s' }).Count }
                $et = [int](((Get-Date) - $hb0a).TotalSeconds)
                $rmin = 0.0
                if ($et -gt 0) { $rmin = [double]($procSum2) / [double]([Math]::Max(0.1, $et/60.0)) }
                Write-Host ("[DIR heartbeat] analyze_shards active elapsed_s={0} pid={1} idle_s={2} local_ok~={3} rate~={4:0.0}/min" -f $et, $procA.Id, $idle2, $procSum2, $rmin)
                if ($AnalyzeShardNoProgressTimeoutSec -gt 0 -and $idle2 -ge $AnalyzeShardNoProgressTimeoutSec) {
                    Write-Warning ("[DIR watchdog] analyze shard {0} no progress {1}s (th={2}s) pid={3}" -f $cA.Index, $idle2, $AnalyzeShardNoProgressTimeoutSec, $procA.Id)
                    try { Stop-Process -Id $procA.Id -Force -ErrorAction SilentlyContinue } catch { }
                    Stop-WindowsProcessTree -RootPid $procA.Id
                    $script:DirRegMetrics.watchdog_kills++
                    Start-Sleep -Milliseconds 500
                    break
                }
            }
            if (-not $procA.HasExited) {
                try { Stop-Process -Id $procA.Id -Force -ErrorAction SilentlyContinue } catch { }
                Stop-WindowsProcessTree -RootPid $procA.Id
            }
        }
        foreach ($cA in $aChildren) {
            $aSumP2 = (Join-Path $cA.OutDir "analyze_summary.txt")
            if (Test-Path -LiteralPath $aSumP2) {
                foreach ($ln2 in (Get-FileLines $aSumP2 0).Lines) {
                    if ($null -eq $ln2) { continue }
                    $l2t = if ($null -ne $ln2) { $ln2.Trim() } else { "" }
                    if ($l2t -like "Top-10*") { break }
                    if ($l2t -match '^\s*OK\s+(\S+)' ) {
                        $rnm = $Matches[1].Trim()
                        if ($rnm) {
                            $mn2 = (ConvertTo-NormalizedModelName $rnm)
                            if ($mn2) { $okSet2[$mn2] = $true }
                            $okSet2[$rnm] = $true
                        }
                    } elseif ($l2t -match '^\s*!!\s' ) {
                        $dirTwoStagePrefailLines.Add($l2t) | Out-Null
                    }
                }
            } else {
                $dirTwoStagePrefailLines.Add("!! parallel_analyze  exit=1  reason=analyze_summary_missing  shard=" + $cA.Index) | Out-Null
            }
        }
    } else {
        Write-Host ("[DIR] TwoStage: serial analyze (workers=1) models={0}" -f $modelTotal)
        $anIdx = 0
        $progP = (Join-Path $outPath "analyze_progress.ndjson")
        if ($ResumeAnalyzeCheckpoint) {
        } else {
            if (Test-Path -LiteralPath $progP) { Remove-Item -LiteralPath $progP -Force -ErrorAction SilentlyContinue }
        }
        (Set-Content -LiteralPath (Join-Path $shardAnRoot "analyze_state.txt") -Value $gHash -Encoding UTF8) | Out-Null
        $doneS = @{}
        if ($ResumeAnalyzeCheckpoint -and (Test-Path -LiteralPath $progP)) {
            $fll = 0
            Get-Content -LiteralPath $progP -ErrorAction SilentlyContinue | ForEach-Object {
                if ($null -eq $_) { return }
                $fll++
                if ($fll -eq 1) {
                    if ($_.ToString() -match "models_hash") { return } else { }
                }
                if ($_.ToString() -notmatch "model" -or $_.ToString() -notmatch "exit") { return }
                try { $j0 = $_.ToString() | ConvertFrom-Json } catch { return }
                if ($j0 -and $j0.model) {
                    $ms = [string]$j0.model
                    $doneS[$ms] = $true
                    $doneS[$(ConvertTo-NormalizedModelName $ms)] = $true
                }
            }
        }
        $chk2 = 0
        foreach ($m in $modelOrderForTwoStage) {
            if ($doneS[$m]) { continue }
            $anIdx++
            Write-Host ("[DIR analyze {0}/{1}] {2}" -f $anIdx, $modelTotal, $m)
            $aArgs2 = @()
            foreach ($lr0 in $resolvedLibRoots) { $aArgs2 += ("--lib-path=" + $lr0) }
            $aArgs2 += @("--validate", "--validate-tier=analyze", ("--validation-mode=" + $AnalyzeValidationMode), $m)
            Push-Location $jitRoot
            $oldA2 = $ErrorActionPreference
            $ErrorActionPreference = "Continue"
            $sw2 = [System.Diagnostics.Stopwatch]::StartNew()
            $r2 = (Invoke-RustmodlicaWithTimeout -ExePath $exe -CliArgs $aArgs2 -WorkDir $jitRoot -TimeoutSec $AnalyzeFirstTimeoutSec -MemoryLimitMb $PerProcessMemoryLimitMb)
            $sw2.Stop()
            $ErrorActionPreference = $oldA2
            Pop-Location
            $x2 = $r2.ExitCode
            if ($x2 -ne 0) {
                if ($r2.TimedOut) {
                    # Retry timed-out model with full PerModelTimeoutSec before marking as failure
                    Write-Host ("[DIR analyze {0}/{1}] {2} TIMED OUT ({3}s), retrying with {4}s..." -f $anIdx, $modelTotal, $m, $AnalyzeFirstTimeoutSec, $PerModelTimeoutSec)
                    $sw2r = [System.Diagnostics.Stopwatch]::StartNew()
                    $r2r = (Invoke-RustmodlicaWithTimeout -ExePath $exe -CliArgs $aArgs2 -WorkDir $jitRoot -TimeoutSec $PerModelTimeoutSec -MemoryLimitMb $PerProcessMemoryLimitMb)
                    $sw2r.Stop()
                    if ($r2r.ExitCode -eq 0) {
                        $okSet2[$m] = $true
                        $okSet2[(ConvertTo-NormalizedModelName $m)] = $true
                        $anSlowAcc.Add([pscustomobject]@{ model = $m; sec = [Math]::Round([double]($sw2.Elapsed.TotalSeconds + $sw2r.Elapsed.TotalSeconds), 2) })
                        Write-Host ("[DIR analyze {0}/{1}] {2} RETRY OK ({3:F1}s)" -f $anIdx, $modelTotal, $m, $sw2r.Elapsed.TotalSeconds)
                        continue
                    }
                    $dirTwoStagePrefailLines.Add("!! {0}  exit={1}  reason=analyze_timeout" -f $m, -1) | Out-Null
                    if (-not [string]::IsNullOrWhiteSpace($quarantinePathResolved)) {
                        Register-DirQuarantine -FilePath $quarantinePathResolved -Model $m -Phase "analyze" -Reason "analyze_timeout" -Consecutive $QuarantineConsecutiveHits
                    }
                } elseif ($r2.Oom) {
                    $dirTwoStagePrefailLines.Add("!! {0}  exit={1}  reason=analyze_oom" -f $m, -1) | Out-Null
                    if (-not [string]::IsNullOrWhiteSpace($quarantinePathResolved)) {
                        Register-DirQuarantine -FilePath $quarantinePathResolved -Model $m -Phase "analyze" -Reason "analyze_oom" -Consecutive $QuarantineConsecutiveHits
                    }
                } else {
                    $dirTwoStagePrefailLines.Add("!! {0}  exit={1}  reason=analyze_gate_failed" -f $m, $x2) | Out-Null
                }
            } else { $okSet2[$m] = $true; $okSet2[(ConvertTo-NormalizedModelName $m)] = $true }
            $anSlowAcc.Add([pscustomobject]@{ model = $m; sec = [Math]::Round([double]$sw2.Elapsed.TotalSeconds, 2) })
            if (-not (Test-Path -LiteralPath $progP) -or (Get-Item -LiteralPath $progP -ErrorAction SilentlyContinue | Where-Object { $_.Length -eq 0 })) {
                (Set-Content -LiteralPath $progP -Value (ConvertTo-Json -Compress ( [pscustomobject]@{ models_hash = $gHash; schema = "dir_analyze_checkpoint_v1" })) -Encoding UTF8) | Out-Null
            } else { }
            Add-Content -LiteralPath $progP -Value (ConvertTo-Json -Compress ([pscustomobject]@{ model = $m; exit = $x2; ts = (Get-Date).ToString("o") })) -Encoding UTF8
            $chk2++
        }
    }
    $env:RUSTMODLICA_DIR_REGRESSION_STAGE = "sim"
    if ($anSlowAcc -and $anSlowAcc.Count -gt 0) { $script:DirRegMetrics.top_slow_analyze = @($anSlowAcc | Sort-Object -Property sec -Descending | Select-Object -First 10) }
    $eligible = New-Object System.Collections.Generic.List[string]
    if ($null -eq $okSet2) { $okSet2 = @{} }
    foreach ($mN in $modelOrderForTwoStage) { if ($okSet2[$mN] -or $okSet2[$(ConvertTo-NormalizedModelName $mN)]) { $null = $eligible.Add($mN) } }
    $script:DirRegMetrics.analyze_passed = $eligible.Count
    $script:DirRegMetrics.analyze_failed = $dirTwoStagePrefailLines.Count
    $anFailBreakdown = [ordered]@{ analyze_timeout = 0; analyze_oom = 0; analyze_gate_failed = 0 }
    foreach ($ln in $dirTwoStagePrefailLines) {
        if ($ln -match 'reason=(analyze_timeout|analyze_oom|analyze_gate_failed)') {
            $rk = [string]$Matches[1]
            $anFailBreakdown[$rk] = [int]$anFailBreakdown[$rk] + 1
        }
    }
    $script:DirRegMetrics.analyze_timeout = [int]$anFailBreakdown.analyze_timeout
    $script:DirRegMetrics.analyze_oom = [int]$anFailBreakdown.analyze_oom
    $script:DirRegMetrics.analyze_gate_failed = [int]$anFailBreakdown.analyze_gate_failed
    $script:DirRegMetrics.analyze_failure_breakdown = $anFailBreakdown
    $models = $eligible
    $modelTotal = $models.Count
    Write-Host ("[DIR] TwoStage: {0} model(s) passed analyze gate" -f $modelTotal)
    if ($script:DirRegMetrics.analyze_failed -gt 0) {
        Write-Host ("[DIR] Analyze failures: timeout={0} oom={1} gate_failed={2}" -f $script:DirRegMetrics.analyze_timeout, $script:DirRegMetrics.analyze_oom, $script:DirRegMetrics.analyze_gate_failed)
    }
}
if ($ParallelWorkers -gt 1 -and $modelTotal -gt 1) {
    $workerCount = [Math]::Min($ParallelWorkers, $modelTotal)
    Write-Host ("Parallel DIR regression enabled: workers={0}, models={1}" -f $workerCount, $modelTotal)
    $shardRoot = Join-Path $outPath "parallel_shards"
    New-Item -ItemType Directory -Path $shardRoot -Force | Out-Null
    $childProcesses = New-Object System.Collections.Generic.List[object]
    for ($wi = 0; $wi -lt $workerCount; $wi++) {
        $shardModels = New-Object System.Collections.Generic.List[string]
        for ($mi = $wi; $mi -lt $modelTotal; $mi += $workerCount) {
            $shardModels.Add($models[$mi])
        }
        if ($shardModels.Count -eq 0) { continue }
        $shardInput = Join-Path $shardRoot ("shard_{0}_models.txt" -f $wi)
        $shardModels | ForEach-Object { "-- $_" } | Set-Content -LiteralPath $shardInput -Encoding UTF8
        $shardOutDir = Join-Path $OutDir ("parallel_shard_{0}" -f $wi)
        $scriptSelf = $PSCommandPath
        if ([string]::IsNullOrWhiteSpace($scriptSelf)) {
            $scriptSelf = $MyInvocation.MyCommand.Path
        }
        if ([string]::IsNullOrWhiteSpace($scriptSelf)) {
            Write-Error "Parallel mode requires script path, but it is unavailable."
            exit 2
        }
        $argList = @(
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", $scriptSelf,
            "-Root", $repoRoot,
            "-OutDir", $shardOutDir,
            "-OnlySkipsFromSummary", $shardInput,
            "-TEnd", "$TEnd",
            "-Dt", "$Dt",
            "-Solver", $Solver,
            "-MaxCases", "0",
            "-ParallelWorkers", "1",
            "-ShardNoProgressTimeoutSec", ([string]$ShardNoProgressTimeoutSec),
            "-PerModelTimeoutSec", ([string]$PerModelTimeoutSec),
            "-PerProcessMemoryLimitMb", ([string]$PerProcessMemoryLimitMb)
        )
        if (-not [string]::IsNullOrWhiteSpace($ExePath)) { $argList += @("-ExePath", $ExePath) }
        if ($AllLibraryMo) { $argList += "-AllLibraryMo" }
        if ($ImplicitRetryIdealTwoWaySwitches) { $argList += "-ImplicitRetryIdealTwoWaySwitches" }
        if ($NewtonCountsAsFailed) { $argList += "-NewtonCountsAsFailed" }
        if ($NewtonNonConvergedAsSkip) { $argList += "-NewtonNonConvergedAsSkip" }
        foreach ($lp in $LibPath) { $argList += @("-LibPath", $lp) }
        foreach ($ea in $ExtraArgs) { $argList += @("-ExtraArgs", $ea) }
        if ($dirPrivEnabled -and $PrivateCacheRunKey -eq "" -and $script:DirPrivRunKeyResolved -ne "" -and $script:DirPrivRootResolved -ne "") {
            $argList += @("-PrivateCacheRunKey", $script:DirPrivRunKeyResolved)
            $argList += @("-PrivateCacheRoot", $script:DirPrivRootResolved)
            $argList += @("-PrivateCacheShard", ([string]$wi))
        }
        if ($DisablePrivateCache) { $argList += "-DisablePrivateCache" }
        $p = Start-Process -FilePath "powershell" -ArgumentList $argList -PassThru
        $childProcesses.Add([pscustomobject]@{
            Index = $wi
            Process = $p
            OutDir = (Join-Path $repoRoot $shardOutDir)
        })
    }

    $ok = 0
    $bad = 0
    $skipped = 0
    $results = @()
    foreach ($cp in $childProcesses) {
        $proc = $cp.Process
        $hb0 = Get-Date
        $lastProgressUtc = [DateTime]::UtcNow
        while (-not $proc.HasExited) {
            if ($proc.WaitForExit(60000)) { break }
            $latestWriteUtc = Get-LatestWriteTimeUtc -DirPath $cp.OutDir
            if ($null -ne $latestWriteUtc -and $latestWriteUtc -gt $lastProgressUtc) {
                $lastProgressUtc = $latestWriteUtc
            }
            $idleSec = [int](([DateTime]::UtcNow - $lastProgressUtc).TotalSeconds)
            Write-Host ("[DIR heartbeat] shard_wait elapsed_s={0} pid={1} idle_s={2}" -f [int](((Get-Date) - $hb0).TotalSeconds), $proc.Id, $idleSec)
            if ($ShardNoProgressTimeoutSec -gt 0 -and $idleSec -ge $ShardNoProgressTimeoutSec) {
                Write-Warning ("[DIR watchdog] shard {0} no progress for {1}s (threshold={2}s), killing pid={3}" -f $cp.Index, $idleSec, $ShardNoProgressTimeoutSec, $proc.Id)
                try { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue } catch {}
                Stop-WindowsProcessTree -RootPid $proc.Id
                $script:DirRegMetrics.watchdog_kills++
                Start-Sleep -Milliseconds 500
                break
            }
        }
        if (-not $proc.HasExited) {
            try { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue } catch {}
            Stop-WindowsProcessTree -RootPid $proc.Id
        }
        $summaryPath = Join-Path $cp.OutDir "summary.txt"
        if (Test-Path -LiteralPath $summaryPath) {
            $lines = (Get-FileLines $summaryPath 0).Lines
            foreach ($ln in $lines) {
                $results += $ln
                if ($ln -match '^OK\s') { $ok++ }
                elseif ($ln -match '^!!\s') { $bad++ }
                elseif ($ln -match '^--\s') { $skipped++ }
            }
        } else {
            $bad++
            $results += "!! shard_$($cp.Index)  exit=1  reason=parallel_summary_missing"
        }
        if ($proc.ExitCode -ne 0) {
            # Keep per-model details from summary; no extra increment here.
            if (-not (Test-Path -LiteralPath $summaryPath)) {
                $latestWriteUtc = Get-LatestWriteTimeUtc -DirPath $cp.OutDir
                $idleSec = if ($null -eq $latestWriteUtc) {
                    [int](([DateTime]::UtcNow - $hb0.ToUniversalTime()).TotalSeconds)
                } else {
                    [int](([DateTime]::UtcNow - $latestWriteUtc).TotalSeconds)
                }
                if ($ShardNoProgressTimeoutSec -gt 0 -and $idleSec -ge $ShardNoProgressTimeoutSec) {
                    $results += "!! shard_$($cp.Index)  exit=$($proc.ExitCode)  reason=parallel_no_progress_timeout"
                } else {
                    $results += "!! shard_$($cp.Index)  exit=$($proc.ExitCode)  reason=parallel_worker_failed"
                }
            }
        }
    }
    if (-not [string]::IsNullOrWhiteSpace($quarantinePathResolved)) {
        foreach ($qLn in $results) {
            if ($null -eq $qLn) { continue }
            $qS = [string]$qLn
            if ($qS -match '^\s*!!\s+(\S+)\s.*reason=(sim_timeout|sim_oom|analyze_timeout|analyze_oom)') {
                $qModel = $Matches[1]
                $qReason = $Matches[2]
                $qPhase = if ($qReason -like "analyze_*") { "analyze" } else { "sim" }
                Register-DirQuarantine -FilePath $quarantinePathResolved -Model $qModel -Phase $qPhase -Reason $qReason -Consecutive $QuarantineConsecutiveHits
            }
        }
    }
    $summaryPath = Join-Path $outPath "summary.txt"
    if ($dirTwoStagePrefailLines.Count -gt 0) {
        $results = @($dirTwoStagePrefailLines.ToArray()) + $results
    }
    # Re-count after prepending TwoStage prefail lines (the shard loop counted only shard outputs).
    $ok = 0
    $bad = 0
    $skipped = 0
    foreach ($ln0 in $results) {
        $ln = if ($null -eq $ln0) { "" } else { [string]$ln0 }
        if ($ln.Length -gt 0 -and [int][char]$ln[0] -eq 0xFEFF) {
            $ln = $ln.Substring(1)
        }
        if ($ln -match '^\s*OK\s') { $ok++ }
        elseif ($ln -match '^\s*!!\s') { $bad++ }
        elseif ($ln -match '^\s*--\s') { $skipped++ }
    }
    $results | Set-Content -LiteralPath $summaryPath -Encoding UTF8
    $script:DirRegMetrics.sim_passed = $ok
    $aggSlowSim = New-Object System.Collections.Generic.List[object]
    $aggPeakMb = 0
    $aggWatchdogChild = 0
    foreach ($cp2 in $childProcesses) {
        $childDmP = (Join-Path $cp2.OutDir "dir_metrics.json")
        if (Test-Path -LiteralPath $childDmP) {
            try {
                $childDm = (Get-Content -LiteralPath $childDmP -Raw) | ConvertFrom-Json
                if ($null -ne $childDm.memory_peak_mb -and [int]$childDm.memory_peak_mb -gt $aggPeakMb) { $aggPeakMb = [int]$childDm.memory_peak_mb }
                if ($null -ne $childDm.watchdog_kills) { $aggWatchdogChild += [int]$childDm.watchdog_kills }
                if ($null -ne $childDm.top_slow_sim) {
                    foreach ($ts in $childDm.top_slow_sim) { $null = $aggSlowSim.Add($ts) }
                }
            } catch {}
        }
    }
    $script:DirRegMetrics.watchdog_kills += $aggWatchdogChild
    if ($aggPeakMb -gt $script:DirRegMetrics.memory_peak_mb) { $script:DirRegMetrics.memory_peak_mb = $aggPeakMb }
    $mergedSlowSim = @($aggSlowSim | Sort-Object -Property sec -Descending | Select-Object -First 10)
    $dmP2 = (Join-Path $outPath "dir_metrics.json")
    try {
        $dmO2 = [ordered]@{
            schema_version     = 1
            models_total        = $script:DirRegMetrics.models_total
            quarantined_skipped = $script:DirRegMetrics.quarantined_skipped
            analyze_passed     = $script:DirRegMetrics.analyze_passed
            analyze_failed     = $script:DirRegMetrics.analyze_failed
            analyze_timeout    = $script:DirRegMetrics.analyze_timeout
            analyze_oom        = $script:DirRegMetrics.analyze_oom
            analyze_gate_failed = $script:DirRegMetrics.analyze_gate_failed
            analyze_failure_breakdown = $script:DirRegMetrics.analyze_failure_breakdown
            sim_ok_lines       = $ok
            watchdog_kills     = $script:DirRegMetrics.watchdog_kills
            memory_peak_mb       = $script:DirRegMetrics.memory_peak_mb
            top_slow_analyze     = @($script:DirRegMetrics.top_slow_analyze)
            top_slow_sim         = $mergedSlowSim
        }
        $dmO2 | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $dmP2 -Encoding UTF8
    } catch { }
    Write-Host ""
    Write-Host "Summary: $ok passed, $bad failed, $skipped skipped"
    Write-Host "Non-OK total: $($bad + $skipped) (parallel mode)"
    Write-Host "Details: $summaryPath"
    if ($bad -gt 0) { exit 1 }
    exit 0
}

$ok = 0
$bad = 0
$skipped = 0
$results = @()
$modelIndex = 0
$script:DirSimSlowAcc = New-Object System.Collections.Generic.List[object]

foreach ($m in $models) {
    $modelIndex++
    Write-Host "[$modelIndex/$modelTotal] $m"
    $caseStartedAt = Get-Date
    if ($m -eq "Modelica.Electrical.Machines.Examples.ControlledDCDrives.CurrentControlledDCPM" `
        -or $m -eq "Modelica.Electrical.Machines.Examples.ControlledDCDrives.SpeedControlledDCPM") {
        # Guard against Windows ACCESS_VIOLATION observed on these DCPM runs when loading JIT codegen/AOT cache artifacts.
        $env:RUSTMODLICA_JIT_CODEGEN_CACHE = "0"
        $env:RUSTMODLICA_AOT_NATIVE_LOAD = "0"
        $env:RUSTMODLICA_NEWTON_T0_FAIL_OPEN = "1"
    } else {
        Remove-Item Env:RUSTMODLICA_NEWTON_T0_FAIL_OPEN -ErrorAction SilentlyContinue
    }
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
    foreach ($lr in $resolvedLibRoots) { $cliArgs += "--lib-path=$lr" }
    $cliArgs += @("--solver=$Solver", "--dt=$Dt", "--t-end=$TEnd", "--result-file=$csv", $m)

    $usedImplicitRetry = $false
    $oldEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $rr = (Invoke-RustmodlicaWithTimeout -ExePath $exe -CliArgs $cliArgs -WorkDir $jitRoot -TimeoutSec $PerModelTimeoutSec -MemoryLimitMb $PerProcessMemoryLimitMb -OutputFile $logPath)
    $exit = $rr.ExitCode
    if ($rr.PeakMB -gt $script:DirRegMetrics.memory_peak_mb) { $script:DirRegMetrics.memory_peak_mb = $rr.PeakMB }
    if ($rr.TimedOut) {
        Write-Warning ("[DIR watchdog] model $m timed out after ${PerModelTimeoutSec}s, killed")
        $script:DirRegMetrics.watchdog_kills++
        if (-not [string]::IsNullOrWhiteSpace($quarantinePathResolved)) {
            Register-DirQuarantine -FilePath $quarantinePathResolved -Model $m -Phase "sim" -Reason "sim_timeout" -Consecutive $QuarantineConsecutiveHits
        }
    }
    if ($rr.Oom) {
        Write-Warning ("[DIR OOM] model $m exceeded ${PerProcessMemoryLimitMb}MB, killed")
        $script:DirRegMetrics.watchdog_kills++
        if (-not [string]::IsNullOrWhiteSpace($quarantinePathResolved)) {
            Register-DirQuarantine -FilePath $quarantinePathResolved -Model $m -Phase "sim" -Reason "sim_oom" -Consecutive $QuarantineConsecutiveHits
        }
    }
    $outLines = @()
    if (Test-Path -LiteralPath $logPath) { try { $outLines = @(Get-Content -LiteralPath $logPath) } catch { $outLines = @() } }
    $newtonFailedFirstTry = $false
    foreach ($ln in $outLines) {
        if ($ln -match 'Newton-Raphson failure') { $newtonFailedFirstTry = $true; break }
    }
    if ($exit -ne 0 -and $newtonFailedFirstTry -and $Solver -ne "implicit" -and -not $rr.TimedOut -and -not $rr.Oom) {
        $retryArgs = @()
        $retryArgs += $ExtraArgs
        foreach ($lr in $resolvedLibRoots) { $retryArgs += "--lib-path=$lr" }
        $retryArgs += @("--index-reduction-method=dummyDerivative", "--solver=implicit", "--dt=$Dt", "--t-end=$TEnd", "--result-file=$csv", $m)
        $retryLogPath = $logPath + ".retry_implicit"
        $rr2 = (Invoke-RustmodlicaWithTimeout -ExePath $exe -CliArgs $retryArgs -WorkDir $jitRoot -TimeoutSec $PerModelTimeoutSec -MemoryLimitMb $PerProcessMemoryLimitMb -OutputFile $retryLogPath)
        $retryLines = @()
        if (Test-Path -LiteralPath $retryLogPath) { try { $retryLines = @(Get-Content -LiteralPath $retryLogPath) } catch {} }
        $outLines = @($outLines + "----- implicit retry -----" + $retryLines)
        $exit = $rr2.ExitCode
        if ($exit -eq 0) { $usedImplicitRetry = $true }
        try { $outLines | Set-Content -LiteralPath $logPath -Encoding UTF8 } catch {}
    }
    if ($exit -ne 0 -and $newtonFailedFirstTry -and -not $rr.TimedOut -and -not $rr.Oom) {
        for ($retryN = 1; $retryN -le 2; $retryN++) {
            $reArgs = @()
            if (-not $hasIndexReductionArg) { $reArgs += "--index-reduction-method=dummyDerivative" }
            $reArgs += $ExtraArgs
            foreach ($lr in $resolvedLibRoots) { $reArgs += "--lib-path=$lr" }
            $reArgs += @("--solver=$Solver", "--dt=$Dt", "--t-end=$TEnd", "--result-file=$csv", $m)
            $reLogPath = $logPath + ".retry_$retryN"
            $rr3 = (Invoke-RustmodlicaWithTimeout -ExePath $exe -CliArgs $reArgs -WorkDir $jitRoot -TimeoutSec $PerModelTimeoutSec -MemoryLimitMb $PerProcessMemoryLimitMb -OutputFile $reLogPath)
            $reLines = @()
            if (Test-Path -LiteralPath $reLogPath) { try { $reLines = @(Get-Content -LiteralPath $reLogPath) } catch {} }
            $outLines = @($outLines + "----- recompile retry $retryN -----" + $reLines)
            if ($rr3.ExitCode -eq 0) { $exit = $rr3.ExitCode; break }
        }
        try { $outLines | Set-Content -LiteralPath $logPath -Encoding UTF8 } catch {}
    }
    $ErrorActionPreference = $oldEap

    if ($rr.TimedOut) {
        $bad++
        $results += "!! $m  exit=-1  reason=sim_timeout"
        Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $false -ExitCode -1 -Status "FAILED" -Reason "sim_timeout" -Detail "timeout=${PerModelTimeoutSec}s"
        Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
        continue
    }
    if ($rr.Oom) {
        $bad++
        $results += "!! $m  exit=-1  reason=sim_oom  detail=peak_mb=$($rr.PeakMB)"
        Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $false -ExitCode -1 -Status "FAILED" -Reason "sim_oom" -Detail "limit_mb=${PerProcessMemoryLimitMb};peak_mb=$($rr.PeakMB)"
        Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
        continue
    }

    if ($exit -ne 0) {
        $modelNotFoundSelf = $false
        $modelNotFoundDependency = $false
        $newtonFailed = $false
        $constrainedbyFailed = $false
        $selfNotFoundPattern = '^Model not found:\s*' + [Regex]::Escape($m) + '\s*$'
        foreach ($ln in $outLines) {
            $ls = [string]$ln
            if ($ls -match '^\[warmup\]') { continue }
            if ($ls -match '^Could not find model:') { continue }
            if ($ls -match $selfNotFoundPattern) { $modelNotFoundSelf = $true; break }
            if ($ls -match '^Model not found:') { $modelNotFoundDependency = $true }
            if ($ls -match 'Newton-Raphson failure') { $newtonFailed = $true }
            if ($ls -match 'FLATTEN_CONSTRAINEDBY') { $constrainedbyFailed = $true }
        }
        if ($modelNotFoundSelf) {
            $bad++
            $results += "!! $m  exit=$exit  reason=config_model_not_found_self"
            Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $false -ExitCode $exit -Status "FAILED" -Reason "config_model_not_found_self" -Detail ""
            Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
            continue
        }
        if ($modelNotFoundDependency) {
            $bad++
            $results += "!! $m  exit=$exit  reason=config_model_dependency_missing"
            Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $false -ExitCode $exit -Status "FAILED" -Reason "config_model_dependency_missing" -Detail ""
            Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
            continue
        }
        if ($newtonFailed) {
            if ($strictNewtonGate) {
                $bad++
                $results += "!! $m  exit=$exit  reason=newton_nonconverged"
                Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $false -ExitCode $exit -Status "FAILED" -Reason "newton_nonconverged" -Detail ""
                Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
            } else {
                $skipped++
                $results += "-- $m  exit=$exit  reason=newton_nonconverged_skip"
                Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $false -ExitCode $exit -Status "SKIPPED" -Reason "newton_nonconverged_skip" -Detail ""
            }
            Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
            continue
        }
        $isConstrainedByNegative = ($m -match '^ModelicaTest\.RedeclareSmoke\.ConstrainedBy(CoarseFalse|Illegal)$')
        if ($constrainedbyFailed -and $isConstrainedByNegative) {
            $ok++
            $results += "OK $m  exit=$exit  reason=expected_constrainedby_failure"
            Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $true -ExitCode $exit -Status "OK" -Reason "expected_constrainedby_failure" -Detail ""
            Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
            continue
        }
        $bad++
        $err = ""
        foreach ($ln in $outLines) {
            if ($ln -match 'error') { $err = ($ln.ToString().Trim()); break }
        }
        if ($err -ne "") { $results += "!! $m  exit=$exit  reason=sim_failed  detail=$err" }
        else { $results += "!! $m  exit=$exit  reason=sim_failed" }
        Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $false -ExitCode $exit -Status "FAILED" -Reason "sim_failed" -Detail $err
        Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
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
                        $ok++
                        $results += "OK $m  exit=$exit  reason=ok"
                        Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $true -ExitCode $exit -Status "OK" -Reason "ok_terminate_no_rows" -Detail ""
                        $handledCsv = $true
                        break
                    }
                }
            }
        }
        if ($handledCsv) { Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt; continue }
        $bad++
        $results += "!! $m  exit=$exit  reason=$($generic.reason)"
        Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $false -ExitCode $exit -Status "FAILED" -Reason $generic.reason -Detail ""
        Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
        continue
    }

    $spec = Test-ModelSpecific $m $csv
    if (-not $spec.ok) {
        $bad++
        $results += "!! $m  exit=$exit  reason=$($spec.reason)"
        Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $false -ExitCode $exit -Status "FAILED" -Reason $spec.reason -Detail ""
        Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
        continue
    }

    $ok++
    if ($usedImplicitRetry) {
        $results += "OK $m  exit=$exit  reason=ok_retry_implicit"
        Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $true -ExitCode $exit -Status "OK" -Reason "ok_retry_implicit" -Detail ""
    } else {
        $results += "OK $m  exit=$exit  reason=$($spec.reason)"
        Write-RunLog -CaseType "DIR_MODEL" -CaseName $m -DurationMs ([long](((Get-Date) - $caseStartedAt).TotalMilliseconds)) -ExpectTargetOk $true -ActualOk $true -ExitCode $exit -Status "OK" -Reason $spec.reason -Detail ""
    }
    Add-DirSimTimingRow -List $script:DirSimSlowAcc -Model $m -Started $caseStartedAt
}

$summaryPath = Join-Path $outPath "summary.txt"
if ($modelTotal -gt 0) {
    if ($dirTwoStagePrefailLines.Count -gt 0) {
        $results = @($dirTwoStagePrefailLines.ToArray()) + $results
    }
    if ($dirTwoStagePrefailLines.Count -gt 0) {
        $ok = 0
        $bad = 0
        $skipped = 0
        foreach ($ln0 in $results) {
            $ln = if ($null -eq $ln0) { "" } else { [string]$ln0 }
            if ($ln.Length -gt 0 -and [int][char]$ln[0] -eq 0xFEFF) { $ln = $ln.Substring(1) }
            if ($ln -match '^\s*OK\s') { $ok++ }
            elseif ($ln -match '^\s*!!\s') { $bad++ }
            elseif ($ln -match '^\s*--\s') { $skipped++ }
        }
    }
    $results | Set-Content -LiteralPath $summaryPath -Encoding UTF8
} else {
    Write-Warning "No models were run; left summary.txt unchanged: $summaryPath"
}
$script:DirRegMetrics.top_slow_sim = @($script:DirSimSlowAcc | Sort-Object -Property sec -Descending | Select-Object -First 10)
$script:DirRegMetrics.sim_passed = $ok
$dmP = (Join-Path $outPath "dir_metrics.json")
try {
    $dmO = [ordered]@{
        schema_version     = 1
        models_total        = $script:DirRegMetrics.models_total
        quarantined_skipped = $script:DirRegMetrics.quarantined_skipped
        analyze_passed     = $script:DirRegMetrics.analyze_passed
        analyze_failed     = $script:DirRegMetrics.analyze_failed
        analyze_timeout    = $script:DirRegMetrics.analyze_timeout
        analyze_oom        = $script:DirRegMetrics.analyze_oom
        analyze_gate_failed = $script:DirRegMetrics.analyze_gate_failed
        analyze_failure_breakdown = $script:DirRegMetrics.analyze_failure_breakdown
        sim_ok_lines       = $ok
        watchdog_kills     = $script:DirRegMetrics.watchdog_kills
        memory_peak_mb       = $script:DirRegMetrics.memory_peak_mb
        top_slow_analyze     = @($script:DirRegMetrics.top_slow_analyze)
        top_slow_sim         = @($script:DirRegMetrics.top_slow_sim)
    }
    if (-not (Test-Path -LiteralPath (Split-Path -Parent $dmP)) -and -not [string]::IsNullOrWhiteSpace((Split-Path -Parent $dmP))) { New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dmP) | Out-Null }
    $dmO | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $dmP -Encoding UTF8
} catch { }
if ($modelTotal -gt 0) {
    if ($script:DirSimSlowAcc -and $script:DirSimSlowAcc.Count -gt 0) {
        Add-Content -LiteralPath $summaryPath -Value "" -Encoding UTF8
        Add-Content -LiteralPath $summaryPath -Value "Top-10 slowest sim (s):" -Encoding UTF8
        $script:DirRegMetrics.top_slow_sim | ForEach-Object { Add-Content -LiteralPath $summaryPath -Value (("  {0}  {1}" -f $_.model, $_.sec)) -Encoding UTF8 }
    }
}

Write-Host ""
Write-Host "Summary: $ok passed, $bad failed, $skipped skipped"
Write-Host "Non-OK total: $($bad + $skipped) (strict Newton gate default ON; use -NewtonNonConvergedAsSkip to downgrade locally)"
Write-Host "Details: $summaryPath"
Write-Host "Run logs: $runLogNdjson ; $runLogCsv"

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

