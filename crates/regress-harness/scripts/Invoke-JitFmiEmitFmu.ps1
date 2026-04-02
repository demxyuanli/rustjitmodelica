param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [string]$CargoTargetDir = "target_regression",
    [string]$OutDir = "build_regress_fmu",
    [string]$Model = "TestLib/SimpleTest"
)

$ErrorActionPreference = "Stop"

$jitDir = Join-Path $RepoRoot "jit-compiler"
if (-not (Test-Path -LiteralPath $jitDir)) {
    Write-Host ("[fmi] missing jit dir: " + $jitDir)
    exit 2
}

Push-Location $jitDir
try {
    if (-not (Test-Path -LiteralPath $OutDir)) {
        New-Item -ItemType Directory -Path $OutDir | Out-Null
    }

    $fmiEnvKeys = @("RUSTMODLICA_FMI_MODEL_ID", "RUSTMODLICA_FMI_MODEL_ID_PREFIX", "RUSTMODLICA_FMI_GUID")
    $saved = @{}
    foreach ($k in $fmiEnvKeys) {
        $saved[$k] = [Environment]::GetEnvironmentVariable($k, "Process")
        Remove-Item ("Env:{0}" -f $k) -ErrorAction SilentlyContinue
    }

    try {
        & cargo --target-dir $CargoTargetDir run -- --emit-fmu=$OutDir $Model 2>&1 | Out-String | Out-Null
        $exit = $LASTEXITCODE
    } finally {
        foreach ($k in $fmiEnvKeys) {
            $v = $saved[$k]
            if ([string]::IsNullOrEmpty($v)) {
                Remove-Item ("Env:{0}" -f $k) -ErrorAction SilentlyContinue
            } else {
                Set-Item -Path ("Env:{0}" -f $k) -Value $v
            }
        }
    }

    $mdPath = Join-Path $OutDir "modelDescription.xml"
    $cPath = Join-Path $OutDir "fmi2_cs.c"
    $ok = ($exit -eq 0) -and (Test-Path -LiteralPath $mdPath) -and (Test-Path -LiteralPath $cPath)

    $flags = ""
    if ($ok) {
        $mdText = Get-Content -LiteralPath $mdPath -Raw
        $hasFmi2 = ($mdText -match 'fmiVersion="2\.0"')
        $hasGuid = ($mdText -match '<fmiModelDescription[^>]*\bguid="[^"]+"')
        $hasCS = ($mdText -match '<CoSimulation\b')
        $hasModelId = ($mdText -match 'modelIdentifier="SimpleTest"')
        $hasReal = ($mdText -match '<Real\s*/>')
        $ok = $ok -and $hasFmi2 -and $hasGuid -and $hasCS -and $hasModelId -and $hasReal
        $flags = ("md_fmi2={0};md_guid={1};md_cs={2};md_modelId={3};md_real={4}" -f $hasFmi2, $hasGuid, $hasCS, $hasModelId, $hasReal)
    }

    Write-Host ("[fmi] ok={0} exit={1} out_dir={2} md={3} c={4} {5}" -f $ok, $exit, $OutDir, $mdPath, $cPath, $flags)
    if ($ok) { exit 0 } else { exit 1 }
} finally {
    Pop-Location
}

