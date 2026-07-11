# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

RustModlica is a Modelica toolchain with two tightly coupled parts: a **JIT compiler** (`jit-compiler`, crate name `rustmodlica`) and an **IDE** (`modai-ide`, Tauri 2 + React). The IDE calls the compiler at runtime for validation/simulation; when the compiler hits limitations, the IDE triggers AI self-iteration to patch the compiler, which then feeds back into the IDE.

## Build & Run Commands

### Compiler (jit-compiler / rustmodlica)

```bash
cargo build --release -p rustmodlica                    # Build JIT compiler
cargo build -p rustmodlica --features sundials --release # With SUNDIALS (CVODE/IDA)
cargo test -p rustmodlica -- --nocapture                 # Run all compiler unit tests
cargo test -p rustmodlica -q                             # Run tests (quiet, failures only)
cargo test -p rustmodlica <test_name> -- --nocapture     # Run a single test by name
cargo run -p rustmodlica -- <model.mo>                   # Simulate a Modelica file
cargo run -p rustmodlica -- --validate <model.mo>        # Validate only (JSON output)
```

All compiler tests are **inline** `#[cfg(test)] mod tests` blocks — there is no `jit-compiler/tests/` directory.

### IDE (modai-ide)

```bash
cd modai-ide && npm install && npm run tauri dev   # Dev server
npm run tauri build                                 # Production build
npm test                                           # Run frontend tests (vitest)
```

Frontend stack: React 19 + Vite 7 + TypeScript 5.8 + Tailwind + Monaco Editor.

### Regression & Testing

```bash
# Quick three-step: unit tests + TestLib batch validate + 14 MOS scripts
pwsh -File ./run_jit_rules_full_regress.ps1

# TestLib batch validation gate (positive + negative cases)
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1

# 14 OMC-style .mos script regression
pwsh -File ./jit-compiler/scripts/run_mos_regression.ps1

# Full regression matrix
pwsh -File ./run_regression.ps1

# Directory-level MSL/ModelicaTest regression
pwsh -File ./run_modelica_dir_regression.ps1

# Compare simulation CSV against OpenModelica reference
pwsh -File ./compare_omc.ps1
```

### Regress Harness CLI

```bash
cargo run -p regress-harness -- run                    # Run JSON test plan
cargo run -p regress-harness -- jit validate-perf      # JIT validate perf baseline
cargo run -p regress-harness -- jit compare-baseline   # Compare perf baseline
```

## Workspace Structure

```
Cargo.toml (workspace, resolver 2)
├── jit-compiler/          # Package: rustmodlica v0.9.0 — JIT compiler & CLI
├── modai-ide/src-tauri/   # Package: modai-ide v0.9.0 — Tauri backend
├── modai-worker/          # v0.1.0 — regression workspace orchestration
├── crates/regress-harness/  # JSON-driven regression test runner
├── crates/modai-paths/      # Shared path utilities
├── crates/modai-protocol/   # Serde types (plans, workspace state, run records)
└── regress-harness-ink/     # Node.js Ink-based terminal UI for regress-harness
```

Dependency graph: `modai-ide` → `rustmodlica` + `modai-worker` → `modai-paths` + `modai-protocol`. The `regress-harness` invokes `rustmodlica` as a subprocess.

## Compiler Pipeline Architecture

```
.mo source
  → Parser (Pest grammar: modelica.pest, mos.pest)
  → AST (ast.rs)
  → Loader (loader.rs) — import resolution, package scanning
  → Flatten (flatten/) — inheritance, instantiation, connect, for-expansion
  → Analysis (analysis/) — variable collection, BLT ordering, derivative classification
  → Compiler (compiler/) — equation conversion, Jacobian, initial conditions
  → JIT Backend (jit/) — Cranelift codegen, tiered compilation, deoptimization
  → Simulation (simulation/ + solver/) — solvers (RK4, RK45, BackwardEuler, Radau, QSS, CVODE, IDA), events
  → Output: CSV / REPL / FMU / C code
```

The compiler pipeline entry points are in `compiler/pipeline/`. The public API surface is re-exported from `lib.rs`.

### Key sub-modules within `jit-compiler/src/`

- **`compiler/pipeline/`** — Pipeline orchestration: `classify.rs` classifies models, `classify_body.rs` handles body-level classification
- **`flatten/`** — Multi-layer caching (SQLite, shared memory, disk) integrated into the flattening pipeline; `Flattener` struct drives with three `ValidationMode` variants (`Full`, `QuickStructure`, `SuperFast`)
- **`jit/translator/`** — Cranelift IR translation (`algorithm/`, `equation/`, `expr/`)
- **`jit/codegen_cache/`** — Persistent codegen cache (COFF/ELF relocation, exec buffer)
- **`jit/tiered.rs`** — Tiered compilation (interpreter → JIT → AOT)
- **`query_db/`** — Salsa-based incremental query database
- **`cache/`** — Multi-tier caching (warmup, artifact, MSL pack, codegen index)
- **`equation_graph.rs` + `equation_graph_inc/`** — Equation dependency graph (incremental variant)

### IDE backend (`modai-ide/src-tauri/src/`)

- **`commands/`** — All Tauri IPC commands. JIT integration split across `jit.rs`, `jit_part_a.rs`, `jit_part_b.rs` due to size
- **`commands/iterate_commands.rs`** — Self-iteration loop: AI generates compiler patches → sandbox build/test → user adopts
- **`ai.rs` / `ai_tools.rs`** — DeepSeek API integration for code generation
- **`diagram/`** — Model diagram visualization (equation graph, types)

## Feature Gates

- `sundials` — Enables CVODE/IDA/KINSOL solvers
- `sundials-vendor` — Vendored SUNDIALS build
- `sundials-klu` — KLU sparse linear solver

No `rust-toolchain.toml`, `rustfmt.toml`, or clippy config — the project uses Rust 2021 edition defaults.

## Key Configuration & Environment Variables

| Variable | Purpose |
|----------|---------|
| `RUSTMODLICA_JIT_POLICY_JSON` | Override JIT policy file |
| `RUSTMODLICA_JIT_POLICY_STRICT` | Comma-separated domains to disable fallbacks |
| `RUSTMODLICA_FLATTEN_CACHE_DIR` | Flatten cache directory |
| `RUSTMODLICA_SUNDIALS_LINSOL` | Linear solver: auto/dense/spgmr/klu |
| `RUSTMODLICA_COVERAGE_STRICT` | Strict coverage gate |
| `RUSTMODLICA_NEWTON_SPARSE_POLICY` | Newton sparse solver: auto/dense/sparse |
| `RUSTMODLICA_QSS_MAX_STEPS` | Max integration steps for the QSS solver |
| `RUSTMODLICA_QSS_MIN_QUANTUM` | Minimum quantum size for the QSS solver |
| `LIBCLANG_PATH` | Required for SUNDIALS build |

JIT policy defaults: `jit-compiler/src/jit/default_jit_policy.json`
Built-in function routing: `jit-compiler/src/jit/default_function_builtin_rules.json`

## Test Libraries

- `jit-compiler/TestLib/*.mo` — Positive test cases (must pass `--validate`)
- `jit-compiler/TestLib/negative/*.mo` — Negative cases (must fail `--validate`)
- `jit-compiler/scripts/*.mos` — 14 OMC-style regression scripts

## Shell Command Convention

Prefix all terminal/shell commands with `rtk` to compress noisy output (e.g., `rtk cargo test`, `rtk git status`). This is configured in `.cursor/rules/rtk-shell.mdc`.

## Language

The codebase documentation and README are primarily in Chinese. The source code, comments, and CLI are in English.

<!-- rtk-instructions v2 -->
# RTK (Rust Token Killer) - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `rtk`**. If RTK has a dedicated filter, it uses it. If not, it passes through unchanged. This means RTK is always safe to use.

**Important**: Even in command chains with `&&`, use `rtk`:
```bash
# ❌ Wrong
git add . && git commit -m "msg" && git push

# ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## RTK Commands by Workflow

### Build & Compile (80-90% savings)
```bash
rtk cargo build         # Cargo build output
rtk cargo check         # Cargo check output
rtk cargo clippy        # Clippy warnings grouped by file (80%)
rtk tsc                 # TypeScript errors grouped by file/code (83%)
rtk lint                # ESLint/Biome violations grouped (84%)
rtk prettier --check    # Files needing format only (70%)
rtk next build          # Next.js build with route metrics (87%)
```

### Test (60-99% savings)
```bash
rtk cargo test          # Cargo test failures only (90%)
rtk go test             # Go test failures only (90%)
rtk jest                # Jest failures only (99.5%)
rtk vitest              # Vitest failures only (99.5%)
rtk playwright test     # Playwright failures only (94%)
rtk pytest              # Python test failures only (90%)
rtk rake test           # Ruby test failures only (90%)
rtk rspec               # RSpec test failures only (60%)
rtk test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
rtk git status          # Compact status
rtk git log             # Compact log (works with all git flags)
rtk git diff            # Compact diff (80%)
rtk git show            # Compact show (80%)
rtk git add             # Ultra-compact confirmations (59%)
rtk git commit          # Ultra-compact confirmations (59%)
rtk git push            # Ultra-compact confirmations
rtk git pull            # Ultra-compact confirmations
rtk git branch          # Compact branch list
rtk git fetch           # Compact fetch
rtk git stash           # Compact stash
rtk git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
rtk gh pr view <num>    # Compact PR view (87%)
rtk gh pr checks        # Compact PR checks (79%)
rtk gh run list         # Compact workflow runs (82%)
rtk gh issue list       # Compact issue list (80%)
rtk gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
rtk pnpm list           # Compact dependency tree (70%)
rtk pnpm outdated       # Compact outdated packages (80%)
rtk pnpm install        # Compact install output (90%)
rtk npm run <script>    # Compact npm script output
rtk npx <cmd>           # Compact npx command output
rtk prisma              # Prisma without ASCII art (88%)
```

### Files & Search (60-75% savings)
```bash
rtk ls <path>           # Tree format, compact (65%)
rtk read <file>         # Code reading with filtering (60%)
rtk grep <pattern>      # Search grouped by file (75%)
rtk find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
rtk err <cmd>           # Filter errors only from any command
rtk log <file>          # Deduplicated logs with counts
rtk json <file>         # JSON structure without values
rtk deps                # Dependency overview
rtk env                 # Environment variables compact
rtk summary <cmd>       # Smart summary of command output
rtk diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
rtk docker ps           # Compact container list
rtk docker images       # Compact image list
rtk docker logs <c>     # Deduplicated logs
rtk kubectl get         # Compact resource list
rtk kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
rtk curl <url>          # Compact HTTP responses (70%)
rtk wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
rtk gain                # View token savings statistics
rtk gain --history      # View command history with savings
rtk discover            # Analyze Claude Code sessions for missed RTK usage
rtk proxy <cmd>         # Run command without filtering (for debugging)
rtk init                # Add RTK instructions to CLAUDE.md
rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 90-99% |
| Build | next, tsc, lint, prettier | 70-87% |
| Git | status, log, diff, add, commit | 59-80% |
| GitHub | gh pr, gh run, gh issue | 26-87% |
| Package Managers | pnpm, npm, npx | 70-90% |
| Files | ls, read, grep, find | 60-75% |
| Infrastructure | docker, kubectl | 85% |
| Network | curl, wget | 65-70% |

Overall average: **60-90% token reduction** on common development operations.
<!-- /rtk-instructions -->