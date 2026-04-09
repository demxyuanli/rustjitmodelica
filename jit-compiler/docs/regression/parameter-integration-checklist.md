# Parameter Convergence Integration Checklist

## Specification consistency

- [ ] `parameter-convergence.md` and `profile-templates.json` profile names are identical.
- [ ] `parameter-metadata.json` includes every high-impact parameter referenced in README quick tables.
- [ ] Precedence text is consistent everywhere: `CLI > env > profile > default`.
- [ ] Run the fixed matrix/metadata consistency command from `parameter-convergence.md` section `4.1.5`.
- [ ] Verify pass criteria: `missing_in_matrix == 0`, `missing_in_metadata == 0`, `matrix_total == metadata_total`.
- [ ] If failed, remediate in order: matrix rows -> metadata env_catalog -> rerun command -> refresh summary counts.

## Selector behavior

- [ ] Each `(goal, symptom, context)` input set resolves to one profile.
- [ ] Hard rules are evaluated before soft scoring.
- [ ] Every conflict includes type (`C1..C4`) and remediation hint.

## Snapshot quality

- [ ] Every effective option contains `value` and `source`.
- [ ] Overrides are explicitly logged when lower-priority values are ignored.
- [ ] Snapshot schema version field is present.

## Documentation quality

- [ ] Commands are Windows PowerShell compatible.
- [ ] No Chinese comments or non-ASCII special symbols inside code snippets.
- [ ] Troubleshooting table includes profile and minimal command path.

## README linkage

- [ ] `jit-compiler/docs/regression/README.md` links to all new spec assets.
- [ ] `docs/regression/README.md` contains concise summary plus cross-reference.

## Review evidence

- [ ] Attach the fixed command output in review notes.
- [ ] Mark final consistency status as `aligned 1:1` or `mismatch unresolved`.

## JIT Next Level A+B rollout

- [ ] `CompilePerfReport` contains rollout fields for incremental cache, const-fold/DCE, builtin inline, hotspot, SIMD, type profile, and runtime boundary epoch.
- [ ] `parameter-convergence.md` includes the A+B switch catalog section.
- [ ] Changing `RUSTMODLICA_RUNTIME_BOUNDARY_EPOCH` changes query cache keys.
