# REG-2: Comparing rustmodlica with OpenModelica (OMC)

For a small set of models, you can compare simulation output or final state with OMC using the same solver and tolerances.

## Prerequisites

- **OpenModelica** (OMC): install from [openmodelica.org](https://openmodelica.org) or use `omc` in PATH.
- **rustmodlica**: build with `cargo build --release`.

## Same solver / tolerance

- rustmodlica: `--solver=rk4` or `--solver=rk45`, `--dt=0.01`, `--atol=1e-6`, `--rtol=1e-3`, `--t-end=T`.
- OMC: default is often CVODE; for comparable results use Euler or rkfix2 with fixed step, or set similar tolerances.

Example OMC script (run in OMEdit or `omc`):

```modelica
// In OMEdit: Simulation Setup -> Integration -> Method: Euler, step size 0.01, end time 10
// Or via API: setParameterValue(SimulationOptions, "stepSize", "0.01")
```

## Compare outputs

1. **Run both** (same model, same t_end, similar dt/tolerances):

   ```powershell
   # rustmodlica (Windows): write CSV to file (no stdout dump)
   .\target\release\rustmodlica.exe --solver=rk4 --dt=0.01 --t-end=10 --result-file=rust_out.csv TestLib/InitDummy

   # Or redirect stdout (legacy): .\target\release\rustmodlica.exe ... TestLib/InitDummy > rust_out.csv

   # OMC: simulate and export CSV, then compare last row or full trajectory
   ```

2. **Compare final state**  
   Use the REG-2 comparison script to run rustmodlica and optionally compare with an OMC-exported CSV:
   ```powershell
   .\compare_omc.ps1 -Model TestLib/InitDummy -TEnd 10 -Dt 0.01
   # After exporting OMC result to omc_out.csv:
   .\compare_omc.ps1 -Model TestLib/InitDummy -TEnd 10 -Dt 0.01 -OmcOut omc_out.csv
   ```
   The script reports max absolute difference on the last row (final state). Small differences are expected due to different solvers or order of operations.

3. **Compare trajectory**  
   For strict comparison, use same method (e.g. fixed-step rk4, same dt) and compare column-by-column; normalize paths if needed (OMC may use different variable names after flatten).

## Suggested comparison set

- `TestLib/InitDummy` (simple ODE) — example above
- `TestLib/InitWithParam` (parameter in initial/ODE):  
  `.\compare_omc.ps1 -Model TestLib/InitWithParam -TEnd 10 -Dt 0.01` then `-OmcOut <omc_csv>` to compare
- `TestLib/AdaptiveRKTest` (adaptive RK45; use same t_end for both tools)
- `TestLib/MSLBlocksTest` (if OMC uses same Blocks subset)

## Notes

- OMC uses different backend (DAE index reduction, tearing); algebraic and state order may differ.
- For regression, rustmodlica uses `REGRESSION_CASES.txt` and `run_regression.ps1`; OMC comparison is optional and documented here for manual or future automated checks.

## Next steps (optional)

- Run OMC for one of the models above, export CSV, then run `compare_omc.ps1 ... -OmcOut <path>` and record max diff (e.g. in this file or a short `OMC_COMPARISON_RESULTS.txt`).
- For alignment task list: see `OPENMODELICA_FULL_ALIGNMENT_TASKS.md`; remaining P3 gap is CG1-4 (array preservation in generated code).
