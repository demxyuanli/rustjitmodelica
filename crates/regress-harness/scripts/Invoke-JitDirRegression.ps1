param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [string]$CargoTargetDir = "target_regression",
    [int]$ParallelWorkers = 0,
    [string]$OutDir = "build_modelica_dir_regress"
)

$ErrorActionPreference = "Stop"

$scriptPath = Join-Path $RepoRoot "run_modelica_dir_regression.ps1"
if (-not (Test-Path -LiteralPath $scriptPath)) {
    Write-Host ("[dir-regress] missing script: " + $scriptPath)
    exit 2
}

$dirExeRel = Join-Path "jit-compiler" (Join-Path $CargoTargetDir "release/rustmodlica.exe")

$workers = $ParallelWorkers
if ($workers -le 0) {
    $workers = [Math]::Max(1, [Environment]::ProcessorCount)
}

& powershell -NoProfile -ExecutionPolicy Bypass -File $scriptPath `
    -Root $RepoRoot `
    -OutDir $OutDir `
    -MaxCases 0 `
    -AllLibraryMo `
    -NewtonCountsAsFailed `
    -ExePath $dirExeRel `
    -ParallelWorkers $workers 2>&1 | Out-String | Out-Null
$exit = $LASTEXITCODE

$ok = ($exit -eq 0)
Write-Host ("[dir-regress] ok={0} exit={1} workers={2} exe={3} out_dir={4}" -f $ok, $exit, $workers, $dirExeRel, $OutDir)
if ($ok) { exit 0 } else { exit 1 }

