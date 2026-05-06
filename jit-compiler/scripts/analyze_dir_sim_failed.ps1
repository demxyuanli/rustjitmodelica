# Classify DIR regression sim_failed models: package prefix + log error heuristics.
# Reads: build_modelica_dir_regress/summary.txt
# Log lookup: parallel_shard_*/logs/<safe>.log then build_modelica_dir_regress/logs/<safe>.log
param(
    [string]$RepoRoot = (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent),
    [string]$SummaryRel = "build_modelica_dir_regress\summary.txt",
    [string]$OutReport = ""
)
$summaryPath = Join-Path $RepoRoot $SummaryRel
if (-not (Test-Path -LiteralPath $summaryPath)) {
    Write-Error "Missing summary: $summaryPath"
    exit 2
}

function Get-SafeLogName([string]$model) {
    return ($model -replace '[^A-Za-z0-9_.-]', '_')
}

function Find-ModelLog([string]$repoRoot, [string]$safe) {
    $dirReg = Join-Path $repoRoot "build_modelica_dir_regress"
    $candidates = @()
    $candidates += Get-ChildItem -LiteralPath $dirReg -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like 'parallel_shard_*' } |
        ForEach-Object { Join-Path $_.FullName "logs\$safe.log" }
    $candidates += Join-Path $dirReg "logs\$safe.log"
    foreach ($p in $candidates) {
        if (Test-Path -LiteralPath $p) { return $p }
    }
    return $null
}

function Classify-LogText([string]$text) {
    $t = $text.ToLowerInvariant()
    if ($t -match 'jit compilation failed|array access \(nested\)|should have been flattened before jit') { return 'jit_codegen_or_flatten_gap' }
    if ($t -match 'flatten_incompatible_connector|\[flatten_incompatible') { return 'flatten_incompatible_connector' }
    if ($t -match 'panic|assertion failed|internal error') { return 'panic_or_assert' }
    if ($t -match 'newton-raphson|nonconverg') { return 'newton_mention' }
    if ($t -match 'event|zero crossing|state event') { return 'event_related' }
    if ($t -match 'singular|ill-condition|matrix is singular') { return 'linear_singular' }
    if ($t -match '\bcvode\b|\bida\b|\bkinsol\b|solver error') { return 'solver_api' }
    if ($t -match 'derivative|smooth|no\s+derivative') { return 'differentiability' }
    if ($t -match 'initial|initialization|consistent') { return 'initialization' }
    if ($t -match 'index|high\s+index|structurally') { return 'index_dae' }
    if ($t -match 'overflow|nan|inf\b') { return 'numeric_nan_inf' }
    if ($t -match 'discrete|boolean|integer') { return 'discrete_hint' }
    if ($t -match 'algebraic|nonlinear\s+system') { return 'nonlinear_algebraic' }
    if ($t -match 'status_access_violation|-1073741819|0xc0000005|access violation') { return 'win_access_violation' }
    if ($t -match 'spice3|mosfet|\bmos\b|bsim|uhdl|differential pair|inverter chain') { return 'spice_mosfet_circuit' }
    if ($t -match 'electrical\.digital|xor gate|nand gate|nor gate|halfadder|fulladder|adder4') { return 'digital_logic_circuit' }
    if ($t -match 'model compilation failed|compilation failed|failed to compile') { return 'compile_failed_banner' }
    if ($t -match 'warning: could not evaluate array size') { return 'flatten_array_size_warn' }
    if ($t -match 'error|failed') { return 'generic_error_word' }
    return 'no_classify_hit'
}

$rows = @()
Get-Content -LiteralPath $summaryPath | ForEach-Object {
    $ln = $_
    if ($ln -notmatch '^\s*!!\s+(\S+)\s+exit=(\S+)\s+reason=sim_failed(?:\s+detail=(.*))?$') { return }
    $model = $Matches[1]
    $exitC = $Matches[2]
    $detail = if ($Matches.Count -gt 3) { $Matches[3] } else { '' }
    $safe = Get-SafeLogName $model
    $logPath = Find-ModelLog $RepoRoot $safe
    $logTail = ""
    if ($null -ne $logPath) {
        try {
            $raw = [System.IO.File]::ReadAllText($logPath)
            if ($raw.Length -gt 400000) { $raw = $raw.Substring($raw.Length - 400000) }
            $logTail = $raw
        } catch { $logTail = "" }
    }
    $cls = if ($logTail.Length -gt 0) { Classify-LogText $logTail } else { 'log_missing_or_empty' }
    if ($cls -eq 'no_classify_hit' -and $logTail.Length -gt 0) {
        if ($model -match 'Modelica\.Electrical\.Spice3') { $cls = 'spice3_log_no_keyword_match' }
        elseif ($model -match 'Modelica\.Electrical\.Digital') { $cls = 'digital_log_no_keyword_match' }
    }
    $detailCls = if ($detail.Length -gt 0) { Classify-LogText $detail } else { '' }
    $prefix = if ($model -match '^(ModelicaTest\.[^.]+\.[^.]+)') { $Matches[1] }
        elseif ($model -match '^(Modelica\.[^.]+\.[^.]+\.[^.]+)') { $Matches[1] }
        elseif ($model -match '^(Modelica\.[^.]+\.[^.]+)') { $Matches[1] }
        elseif ($model -match '^([^.]+\.[^.]+\.[^.]+)') { $Matches[1] }
        else { 'other' }
    $rows += [pscustomobject]@{
        Model     = $model
        Exit      = $exitC
        Prefix3   = $prefix
        LogPath   = $logPath
        Classify  = $cls
        DetailCls = $detailCls
        Detail    = $detail
    }
}

Write-Output "sim_failed_count=$($rows.Count)"
Write-Output "--- by Prefix3 (top 25) ---"
$rows | Group-Object Prefix3 | Sort-Object Count -Descending | Select-Object -First 25 |
    ForEach-Object { Write-Output ("{0}`t{1}" -f $_.Count, $_.Name) }

Write-Output "--- by Classify (log tail heuristic) ---"
$rows | Group-Object Classify | Sort-Object Count -Descending |
    ForEach-Object { Write-Output ("{0}`t{1}" -f $_.Count, $_.Name) }

Write-Output "--- by Prefix3 x Classify (count>=5 only) ---"
$rows | Group-Object { "$($_.Prefix3)|$($_.Classify)" } |
    Where-Object { $_.Count -ge 5 } | Sort-Object Count -Descending | Select-Object -First 30 |
    ForEach-Object { Write-Output ("{0}`t{1}" -f $_.Count, $_.Name) }

Write-Output "--- detail= present (first 20 distinct details) ---"
$rows | Where-Object { $_.Detail.Length -gt 0 } | Group-Object Detail | Sort-Object Count -Descending | Select-Object -First 20 |
    ForEach-Object { Write-Output ("{0}`t{1}" -f $_.Count, $_.Name) }

if (-not [string]::IsNullOrWhiteSpace($OutReport)) {
    $dir = Split-Path -Parent $OutReport
    if (-not [string]::IsNullOrWhiteSpace($dir) -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    }
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.AppendLine("# sim_failed breakdown (model`tclassify`tprefix3`texit`tdetail)")
    foreach ($r in ($rows | Sort-Object Prefix3, Model)) {
        [void]$sb.AppendLine(("{0}`t{1}`t{2}`t{3}`t{4}" -f $r.Model, $r.Classify, $r.Prefix3, $r.Exit, ($r.Detail -replace "`t", " ")))
    }
    [System.IO.File]::WriteAllText($OutReport, $sb.ToString())
    Write-Output ""
    Write-Output "wrote_tsv=$OutReport"
}
