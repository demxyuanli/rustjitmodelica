param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [ValidateSet("all", "non_triggered", "triggered")]
    [string]$BltGuardFilter = "non_triggered",
    [string]$ModelFilter = "",
    [string]$InputDir = "",
    [string]$OutputDir = "build_sparse_dense_summary"
)

$ErrorActionPreference = "Stop"

function Test-Truthy([string]$v) {
    if ($null -eq $v) { return $false }
    $t = $v.Trim().ToLowerInvariant()
    return -not ($t -eq "" -or $t -eq "0" -or $t -eq "false" -or $t -eq "off" -or $t -eq "no")
}

if (-not (Test-Truthy $env:RUSTMODLICA_SUMMARIZE_SPARSE_DENSE)) {
    Write-Host "[sparse-dense-summary] skipped (RUSTMODLICA_SUMMARIZE_SPARSE_DENSE not enabled)"
    exit 0
}

$summaryScript = Join-Path $RepoRoot "scripts/summarize_sparse_dense.ps1"
if (-not (Test-Path -LiteralPath $summaryScript)) {
    Write-Host ("[sparse-dense-summary] skipped: script not found (" + $summaryScript + ")")
    exit 0
}

if ([string]::IsNullOrWhiteSpace($InputDir)) {
    $InputDir = Join-Path $RepoRoot "jit-compiler/build_sparse_dense_bench"
} elseif (-not [System.IO.Path]::IsPathRooted($InputDir)) {
    $InputDir = Join-Path $RepoRoot $InputDir
}

if (-not (Test-Path -LiteralPath $InputDir)) {
    Write-Host ("[sparse-dense-summary] skipped: benchmark input dir not found (" + $InputDir + ")")
    exit 0
}

$modelArgs = @()
if (-not [string]::IsNullOrWhiteSpace($ModelFilter)) {
    $arr = @()
    foreach ($p in $ModelFilter.Split(",") ) {
        $s = $p.Trim()
        if ($s -ne "") { $arr += $s }
    }
    if ($arr.Count -gt 0) {
        $modelArgs = @("-ModelFilter") + $arr
    }
}

& powershell -NoProfile -ExecutionPolicy Bypass -File $summaryScript `
    -InputDir $InputDir `
    -OutputDir $OutputDir `
    -BltGuardFilter $BltGuardFilter `
    @modelArgs 2>&1 | Out-String | Out-Null
$exit = $LASTEXITCODE

$ok = ($exit -eq 0)
Write-Host ("[sparse-dense-summary] ok={0} exit={1} filter={2} input_dir={3} output_dir={4}" -f $ok, $exit, $BltGuardFilter, $InputDir, $OutputDir)
if ($ok) { exit 0 } else { exit 1 }

