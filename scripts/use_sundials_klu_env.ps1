param(
    [string]$VcpkgRoot = "D:/repos/vcpkg",
    [string]$Command = "cargo build -p rustmodlica --features sundials,sundials-klu"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$installed = Join-Path $VcpkgRoot "installed/x64-windows"
$includeRoot = Join-Path $installed "include"
$suitesparseInclude = Join-Path $includeRoot "suitesparse"
$libRoot = Join-Path $installed "lib"
function ConvertTo-PosixPath([string]$p) {
    return $p.Replace("\", "/")
}

if (-not (Test-Path -LiteralPath $suitesparseInclude)) {
    throw "SuiteSparse include path not found: $suitesparseInclude"
}
if (-not (Test-Path -LiteralPath $libRoot)) {
    throw "SuiteSparse lib path not found: $libRoot"
}

$installedPosix = ConvertTo-PosixPath $installed
$includeRootPosix = ConvertTo-PosixPath $includeRoot
$suitesparseIncludePosix = ConvertTo-PosixPath $suitesparseInclude
$libRootPosix = ConvertTo-PosixPath $libRoot

$env:BINDGEN_EXTRA_CLANG_ARGS = "-I$suitesparseIncludePosix"
$env:CMAKE_PREFIX_PATH = $installedPosix
$env:CMAKE_INCLUDE_PATH = "$includeRootPosix;$suitesparseIncludePosix"
$env:CMAKE_LIBRARY_PATH = $libRootPosix
$env:CMAKE_ARGS = @(
    "-DKLU_INCLUDE_DIR=$suitesparseIncludePosix",
    "-DKLU_LIBRARY=$libRootPosix/klu.lib",
    "-DAMD_LIBRARY=$libRootPosix/amd.lib",
    "-DCOLAMD_LIBRARY=$libRootPosix/colamd.lib",
    "-DBTF_LIBRARY=$libRootPosix/btf.lib",
    "-DSUITESPARSE_CONFIG_LIBRARY=$libRootPosix/suitesparseconfig.lib"
) -join " "

Write-Host "Configured SuiteSparse/KLU environment from: $installedPosix"
Write-Host "Running command: $Command"

Invoke-Expression $Command
exit $LASTEXITCODE
