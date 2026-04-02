# Generates regress-harness JSON from run_regression.ps1 ($cases, $caseExtraArgs) and run_mos_regression.ps1 ($mosCases).
# Usage (from repo root): powershell -NoProfile -ExecutionPolicy Bypass -File crates/regress-harness/scripts/Export-RegressConfigFromPs1.ps1
param(
    [string]$RepoRoot = "",
    [string]$RustmodlicaExe = "jit-compiler/target_regression/release/rustmodlica.exe"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function ConvertTo-SnakeCaseSegment([string]$s) {
    if ([string]::IsNullOrEmpty($s)) { return "" }
    $x = [regex]::Replace($s, '([a-z0-9])([A-Z])', '$1_$2')
    return $x.ToLowerInvariant()
}

function Get-CaseIdFromTarget([string]$target) {
    $parts = $target -split '[/.]'
    $chunks = foreach ($p in $parts) {
        if ($p -eq "TestLib") { "testlib" }
        elseif ($p -eq "ModelicaTest") { "modelicatest" }
        else { ConvertTo-SnakeCaseSegment $p }
    }
    return ($chunks -join '_')
}

function Get-ExpectObject([string]$expect) {
    if ($expect -eq "pass") {
        return @{ kind = "exit_zero" }
    }
    return @{ kind = "non_zero" }
}

$repo = if ($RepoRoot -ne "") { $RepoRoot } else { (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path }
$runReg = Join-Path $repo "run_regression.ps1"
$mosReg = Join-Path $repo "jit-compiler\scripts\run_mos_regression.ps1"
$outTestlib = Join-Path $repo "crates\regress-harness\examples\testlib_from_run_regression.json"
$outMos = Join-Path $repo "crates\regress-harness\examples\mos_from_run_mos_regression.json"
if (-not (Test-Path -LiteralPath $runReg)) { throw "Missing $runReg" }
if (-not (Test-Path -LiteralPath $mosReg)) { throw "Missing $mosReg" }

$text = [System.IO.File]::ReadAllText($runReg)
$rx = [regex]::new('@\("([^"]+)",\s*"(pass|fail)"\)')
$matches = $rx.Matches($text)
if ($matches.Count -eq 0) { throw "No case rows matched in run_regression.ps1" }

$extraArgsByTarget = @{}
$extraBlock = [regex]::Match($text, '(?ms)\$caseExtraArgs\s*=\s*@\{(.*?)^\}')
if ($extraBlock.Success) {
    $inner = $extraBlock.Groups[1].Value
    $argRx = [regex]::new('"(?<path>[^"]+)"\s*=\s*@\((?<args>[^)]*)\)')
    foreach ($am in $argRx.Matches($inner)) {
        $path = $am.Groups["path"].Value
        $argsStr = $am.Groups["args"].Value
        $quoted = [regex]::Matches($argsStr, '"([^"]*)"')
        $list = foreach ($qm in $quoted) { $qm.Groups[1].Value }
        $extraArgsByTarget[$path] = @($list)
    }
}

$caseObjects = [System.Collections.Generic.List[object]]::new()
$seenIds = @{}
foreach ($m in $matches) {
    $target = $m.Groups[1].Value
    $expect = $m.Groups[2].Value
    $id = Get-CaseIdFromTarget $target
    if ($seenIds.ContainsKey($id)) { throw "Duplicate case id: $id (target=$target)" }
    $seenIds[$id] = $true

    $tags = [System.Collections.Generic.List[string]]::new()
    $tags.Add("testlib")
    $tags.Add("core")
    if ($expect -eq "fail") { $tags.Add("negative") }

    $extra = @()
    if ($extraArgsByTarget.ContainsKey($target)) {
        $extra = @($extraArgsByTarget[$target])
    }

    $caseObj = [ordered]@{
        id       = $id
        kind     = "model"
        target   = $target
        tags     = @($tags)
        expect   = (Get-ExpectObject $expect)
    }
    if ($extra.Count -gt 0) {
        $caseObj["extra_rust_args"] = $extra
    }
    $caseObjects.Add([pscustomobject]$caseObj)
}

$testlibDoc = [ordered]@{
    version     = 1
    defaults    = [ordered]@{
        repo_root           = "."
        rustmodlica_exe     = $RustmodlicaExe
        working_dir         = "jit-compiler"
        cargo_exe           = "cargo"
        solver              = "rk4"
        t_end               = [double]10.0
        dt                  = [double]0.01
        regression_data_root = "build/regression_data_testlib"
    }
    execution   = [ordered]@{ workers = 4; fail_fast = $false }
    incremental = [ordered]@{
        baseline_path = $null
        strategy      = "none"
    }
    tiers       = [ordered]@{
        core     = [ordered]@{ include_tags = @("core") }
        negative = [ordered]@{ include_tags = @("negative") }
    }
    cases       = @($caseObjects.ToArray())
}

$testlibJson = $testlibDoc | ConvertTo-Json -Depth 20
[System.IO.File]::WriteAllText($outTestlib, $testlibJson, [System.Text.UTF8Encoding]::new($false))
Write-Host "Wrote $($caseObjects.Count) cases -> $outTestlib"

$mosText = [System.IO.File]::ReadAllText($mosReg)
$mosRx = [regex]::new('"scripts/[^"]+\.mos"')
$mosMatches = $mosRx.Matches($mosText)
$mosPaths = foreach ($mm in $mosMatches) {
    $mm.Value.Trim('"')
}
if ($mosPaths.Count -eq 0) { throw "No MOS scripts matched in run_mos_regression.ps1" }

$mosCaseObjects = [System.Collections.Generic.List[object]]::new()
$seenMos = @{}
foreach ($rel in $mosPaths) {
    $baseName = [System.IO.Path]::GetFileNameWithoutExtension($rel)
    $mid = "mos_" + (ConvertTo-SnakeCaseSegment $baseName)
    if ($seenMos.ContainsKey($mid)) { continue }
    $seenMos[$mid] = $true
    $mosCaseObjects.Add([pscustomobject]([ordered]@{
                id     = $mid
                kind   = "mos"
                target = $rel
                tags   = @("mos", "core")
                expect = (Get-ExpectObject "pass")
            }))
}

$mosDoc = [ordered]@{
    version     = 1
    defaults    = [ordered]@{
        repo_root            = "."
        rustmodlica_exe      = $RustmodlicaExe
        working_dir          = "jit-compiler"
        cargo_exe            = "cargo"
        cargo_run_prefix     = @()
        solver               = "rk4"
        t_end                = [double]10.0
        dt                   = [double]0.01
        regression_data_root = "build/regression_data_mos"
    }
    execution   = [ordered]@{ workers = 1; fail_fast = $false }
    incremental = [ordered]@{
        baseline_path = $null
        strategy      = "none"
    }
    tiers       = [ordered]@{
        mos = [ordered]@{ include_tags = @("mos") }
    }
    cases       = @($mosCaseObjects.ToArray())
}

$mosJson = $mosDoc | ConvertTo-Json -Depth 20
[System.IO.File]::WriteAllText($outMos, $mosJson, [System.Text.UTF8Encoding]::new($false))
Write-Host "Wrote $($mosCaseObjects.Count) mos cases -> $outMos"
