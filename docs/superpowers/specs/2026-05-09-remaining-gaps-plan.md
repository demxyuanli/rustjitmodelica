# Remaining Audit Gaps — Independent Project Plans

> Date: 2026-05-09
> Based on: `docs/JIT_COMPILER_AUDIT_VS_OMC_DYMOLA.md`
> Status: Design + implementation plan for 4 remaining P2/P3 items

---

## 1. FMI Export Completion (P2)

### Current state

FMI module (`jit-compiler/src/fmi/`) already generates:
- `modelDescription.xml` (CS and ME variants)
- `fmi2_cs.c` / `fmi2_me.c` (FMI 2.0 C API wrappers)
- Variable mapping (states, params, outputs) with value references
- GUID generation, model identifier sanitization
- CLI: `--emit-fmu=<dir>` and `--emit-fmu-me=<dir>`

Missing:
- No automatic C compilation (user must invoke CC manually)
- No .fmu ZIP packaging
- Requires two-step workflow: `--emit-c=<dir>` first, then `--emit-fmu=<dir>`

### Design

**Phase 1: ZIP packaging** (1 day)

Add automatic .fmu creation after emitting source files. An FMU is a ZIP with:
```
model.fmu
├── modelDescription.xml
├── binaries/win64/model.dll    (or linux64/model.so, darwin64/model.dylib)
├── sources/model.c
├── sources/model.h
├── sources/fmi2_cs.c
└── sources/fmi2Functions.h (minimal stub)
```

- Add `zip` crate dependency (or use `std::process::Command` to invoke system `zip`)
- New function `package_fmu(dir: &Path, output: &Path, platform: &str) -> Result<()>`
- Platform detection: `std::env::consts::OS` → win64/linux64/darwin64

**Phase 2: Automatic C compilation** (1 day)

- Detect system C compiler: `cc`/`gcc`/`clang` on PATH
- Compile `model.c + fmi2_cs.c` → shared library
- Windows: `cl /LD` or `gcc -shared`
- Linux: `gcc -shared -fPIC`
- macOS: `clang -dynamiclib`
- Place compiled .dll/.so/.dylib in `binaries/<platform>/`

**Phase 3: Single-step export** (0.5 day)

- New CLI: `--emit-fmu=<file.fmu>` does everything in one step
- Deprecate separate `--emit-c` + `--emit-fmu=<dir>` workflow

### Files

| File | Change |
|------|--------|
| `fmi/mod.rs` | Add `package_fmu()`, `compile_fmu_c()` |
| `fmi/fmi_part2.rs` | Integrate packaging into emit functions |
| `cli/run.rs` | Add `--emit-fmu=<file.fmu>` single-step flag |
| `Cargo.toml` | Add `zip` dependency |

### Effort: 2-3 days

---

## 2. macOS Mach-O Codegen Cache (P3)

### Current state

Codegen cache (`jit/codegen_cache/`) supports:
- **COFF** (Windows): `coff_reloc.rs` — full relocation, exec buffer mapping
- **ELF64** (Linux): `elf_reloc.rs` — full relocation, exec buffer mapping
- **macOS**: raw blob fallback in `exec_buffer.rs` — no relocation support

### Design

Add `macho_reloc.rs` following the same pattern as `coff_reloc.rs` and `elf_reloc.rs`:

1. **Parse Mach-O headers**: read mach_header_64, segment commands, section headers
2. **Find relocation entries**: `__TEXT,__text` section with relocation info
3. **Apply relocations**: ARM64 (AArch64) and x86_64 relocation types
   - AArch64: `ARM64_RELOC_PAGEOFF12`, `ARM64_RELOC_BRANCH26`, etc.
   - x86_64: `X86_64_RELOC_SIGNED`, `X86_64_RELOC_BRANCH`, `X86_64_RELOC_GOT_LOAD`
4. **Map executable memory**: `mmap` with `PROT_READ | PROT_WRITE` → apply relocs → `mprotect(PROT_READ | PROT_EXEC)`

### Files

| File | Change |
|------|--------|
| `jit/codegen_cache/macho_reloc.rs` | New file — Mach-O relocation logic |
| `jit/codegen_cache/mod.rs` | Add `#[cfg(target_os = "macos")]` module |
| `jit/codegen_cache/exec_buffer.rs` | Integrate Mach-O path |

### Effort: 2-3 days

### Risk

Requires macOS testing environment. Can develop structure on any platform but needs macOS for verification.

---

## 3. Checkpoint/Restart (P3)

### Current state

No implementation. Simulation state (states, time, discrete vars) lives in memory during simulation and is lost on exit.

### Design

**Phase 1: State serialization** (1 day)

- Serialize simulation state at user-defined intervals or on signal:
  - Time `t`
  - State vector `x[0..n_states]`
  - Discrete variable values
  - Event counter / iteration state
  - Solver-specific state (step size, order for CVODE)
- Format: JSON or binary (bincode) checkpoint file
- CLI: `--checkpoint-interval=<seconds>` or `--checkpoint-file=<path>`

**Phase 2: State restoration** (1 day)

- Load checkpoint file
- Restore all state vectors
- Resume solver from checkpoint time
- Validate model hash matches (prevent cross-model checkpoints)
- CLI: `--restore=<checkpoint_file>`

**Phase 3: SUNDIALS integration** (1 day)

- CVODE/IDA have native checkpoint API
- Use `CVodeGetStateVector` / `IDAGetStateVector` to capture full solver state
- Handle adaptive step size save/restore

### Files

| File | Change |
|------|--------|
| `simulation/checkpoint.rs` | New file — serialize/deserialize simulation state |
| `simulation.rs` | Add checkpoint interval logic, restore entry point |
| `simulation/sundials/run_common.rs` | SUNDIALS-specific checkpoint capture |
| `cli/run.rs` | Add `--checkpoint-interval`, `--restore` flags |

### Effort: 2-3 days

---

## 4. Runtime PGO Feedback (P3)

### Current state

Profile-guided optimization exists for training runs (`simulation.rs` has `arch`/`profile` collection during dedicated training), but no online feedback loop during actual simulation.

### Design

**Phase 1: Lightweight hot-loop detection** (1 day)

- During simulation, track per-equation evaluation count
- After N steps (configurable, default 1000), identify "hot" equations (>threshold evaluations)
- Hot equations trigger JIT recompilation at higher tier

**Phase 2: Recompilation triggering** (1 day)

- For equations identified as hot, recompile at `tier3` (optimized JIT)
- Use Cranelift optimization level `speed` instead of `speed_and_size`
- Hot-swap the new code in the solver loop (existing deopt infrastructure supports this)

**Phase 3: Feedback persistence** (0.5 day)

- Save hot-equation profile to disk cache
- On next run of same model, pre-compile hot equations at high tier
- Cache keyed by model artifact hash

### Files

| File | Change |
|------|--------|
| `simulation/pgo.rs` | New file — hot-loop detection + recompilation trigger |
| `simulation.rs` | Integrate PGO into step loop |
| `jit/tiered.rs` | Add PGO-driven tier promotion |
| `cache/` | Persist PGO profiles |

### Effort: 2-3 days

---

## Priority Order

| # | Project | Effort | Impact | Risk |
|---|---------|--------|--------|------|
| 1 | FMI Export | 2-3d | High (interop) | Low |
| 2 | Mach-O Cache | 2-3d | Low (macOS only) | Med (needs macOS) |
| 3 | Checkpoint | 2-3d | Med (fault tolerance) | Low |
| 4 | Runtime PGO | 2-3d | Med (adaptive perf) | Med (complex integration) |

Each project is independently implementable and testable. Total effort: ~10 days.
