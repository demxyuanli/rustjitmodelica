# CLI/TUI Implementation Guide for JIT Parameter Convergence

## 1. Purpose

This guide maps the specification in `parameter-convergence.md` to practical CLI/TUI integration steps without changing execution semantics.

## 2. Input mapping

## 2.1 CLI flags

Recommended selector entry flags:

- `--goal=<speed|stability|precision|export|diagnose>`
- `--symptom=<flatten_fail|cache_miss_spike|perf_regress|numerical_drift|timeout|none>`
- `--context=<local|ci|performance_lab|production>`
- `--profile=<name>` (optional explicit override)

If `--profile` is present, selector still runs hard-rule validation before accepting it.

### 2.2 TUI controls

Map current interactive workflow fields to selector dimensions:

- `goal`: single-select radio
- `symptom`: single-select dropdown
- `context`: auto-detected default + editable dropdown
- `history`: pre-filled from latest run snapshot

## 3. Data contracts

### 3.1 Profile source

Read from:

- `profile-templates.json`

Expected fields:

- `must`, `optional`, `forbidden`
- `exit_conditions`
- `next_profile_on_failure`

### 3.2 Parameter metadata source

Read from:

- `parameter-metadata.json`

Expected fields:

- `layer`, `type`, `default`, `conflicts`, `trigger_when`, `risk`

## 4. Resolution algorithm (recommended)

1. Build input context (`goal`, `symptom`, `context`, `history`).
2. Evaluate hard rules and force profile if needed.
3. Apply soft scoring when hard rules do not force.
4. Build candidate option map from selected profile.
5. Merge explicit values by precedence:
   - CLI > env > profile > default
6. Run conflict detector (`C1..C4`).
7. Emit machine-readable snapshot.

## 5. Snapshot mapping to run options

For compatibility with existing run option snapshots, include:

```json
{
  "selector": {
    "profile": "CIGate",
    "inputs": {
      "goal": "stability",
      "symptom": "timeout",
      "context": "ci"
    }
  },
  "effective": {
    "validation_mode": { "value": "full", "source": "profile" },
    "query_cache_namespace": { "value": "ci-1234", "source": "env" }
  },
  "conflicts": [],
  "warnings": []
}
```

## 6. Error message templates

- Mutual exclusion:
  - `E_SELECTOR_CONFLICT_C1: option '<A>' conflicts with '<B>', choose one.`
- Range/domain:
  - `E_SELECTOR_CONFLICT_C3: option '<A>' is out of range: <value>.`
- Dependency:
  - `E_SELECTOR_CONFLICT_C4: option '<A>' requires '<B>'.`

## 7. Rollout strategy

1. Read-only mode first: print recommended profile and effective options, do not apply.
2. Soft-apply mode: apply profile defaults, preserve existing explicit CLI/env behavior.
3. Strict mode: enforce hard-rule blockers.

## 8. Review checklist

- Selector output is stable for identical input tuple.
- Source annotation is present for every effective option.
- Hard-rule violations are always surfaced before execution.
- Existing command paths continue to run when selector is bypassed.
