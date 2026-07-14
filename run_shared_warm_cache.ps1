$ErrorActionPreference='Stop'
$out = 'build/jit_validate_perf_multibody_warm_shared'
if (-not (Test-Path $out)) { New-Item -ItemType Directory -Path $out | Out-Null }

$models = 'Modelica.Mechanics.MultiBody.Examples.Elementary.DoublePendulum,Modelica.Mechanics.MultiBody.Examples.Elementary.ForceAndTorque,Modelica.Mechanics.MultiBody.Examples.Elementary.FreeBody,Modelica.Mechanics.MultiBody.Examples.Loops.Fourbar1,Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.oneAxis,Modelica.Mechanics.MultiBody.Examples.Systems.RobotR3.fullRobot'

# Full mode
Write-Output "=== Running full mode ==="
& ./target/release/regress-harness.exe jit validate-perf --exe ./target/release/rustmodlica.exe --lib-path ./jit-compiler --out-dir $out --validate-tier analyze --validation-mode full --models $models --hot-runs 8 --stage-trace --perf-trace --scenarios devloop_multi_model 2>&1
if ($LASTEXITCODE -ne 0) { exit $last }

Copy-Item "$out/report.json" "$out/report_full.json" -Force

# Quick mode
Write-Output "=== Running quick mode ==="
& ./target/release/regress-harness.exe jit validate-perf --exe ./target/release/rustmodlica.exe --lib-path ./jit-compiler --out-dir $out --validate-tier analyze --validation-mode quick --models $models --hot-runs 8 --stage-trace --perf-trace --scenarios devloop_multi_model 2>&1
if ($LASTEXITCODE -ne 0) { exit $last }

Copy-Item "$out/report.json" "$out/report_quick.json" -Force

# Superfast mode
Write-Output "=== Running superfast mode ==="
& ./target/release/regress-harness.exe jit validate-perf --exe ./target/release/rustmodlica.exe --lib-path ./jit-compiler --out-dir $out --validate-tier analyze --validation-mode superfast --models $models --hot-runs 8 --stage-trace --perf-trace --scenarios devloop_multi_model 2>&1
if ($LASTEXITCODE -ne 0) { exit $last }

Copy-Item "$out/report.json" "$out/report_superfast.json" -Force

Write-Output "=== All done ==="
