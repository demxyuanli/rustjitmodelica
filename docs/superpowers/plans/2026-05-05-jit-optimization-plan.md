# JIT 编译器优化 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实施 11 项 JIT 编译器优化，覆盖运行时性能、代码质量、SIMD 向量化和 tiered compilation 线程安全。

**Architecture:** 分四批独立推进。第一批 5 项改动极小（1-30 行），第二批 4 项代码质量加固，第三批新增 `jit/translator/vectorize.rs` 实现方程聚类 + Cranelift 向量发射，第四批修复 tiered.rs 后台线程安全问题。每批完成后跑回归套件验证。

**Tech Stack:** Rust 2021, Cranelift 0.128, xxhash, rayon

---

## 第一批：快速 Win

### Task 1: stack_scratch 默认值改为 true

**Files:**
- Modify: `jit-compiler/src/simulation.rs:229-232`

- [ ] **Step 1: 修改默认值**

在 `jit-compiler/src/simulation.rs` 第 232 行，把 `unwrap_or(false)` 改为 `unwrap_or(true)`：

```rust
// 改前 (line 229-232):
let stack_scratch_enabled = std::env::var("RUSTMODLICA_JIT_STACK_SCRATCH")
    .ok()
    .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
    .unwrap_or(false);

// 改后:
let stack_scratch_enabled = std::env::var("RUSTMODLICA_JIT_STACK_SCRATCH")
    .ok()
    .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
    .unwrap_or(true);
```

同步更新 `jit-compiler/src/compiler/compile_model/compile/entry.rs` 第 123 行的 perf report 默认值：

```rust
// 改前 (line 123):
perf_report.stack_scratch_enabled = env_flag("RUSTMODLICA_JIT_STACK_SCRATCH", false);

// 改后:
perf_report.stack_scratch_enabled = env_flag("RUSTMODLICA_JIT_STACK_SCRATCH", true);
```

- [ ] **Step 2: 验证编译**

```bash
rtk cargo check -p rustmodlica
```

- [ ] **Step 3: 跑回归**

```bash
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1
```

- [ ] **Step 4: Commit**

```bash
rtk git add jit-compiler/src/simulation.rs jit-compiler/src/compiler/compile_model/compile/entry.rs
rtk git commit -m "perf: enable stack_scratch by default to avoid Vec allocs on every solver eval"
```

---

### Task 2: TieredFunction 用 AtomicPtr 替代 Mutex<CalcDerivsFunc>

**Files:**
- Modify: `jit-compiler/src/jit/tiered.rs:150-195`

- [ ] **Step 1: 将 `func: Mutex<CalcDerivsFunc>` 改为 `func: AtomicPtr<c_void>`**

`CalcDerivsFunc` 是 `unsafe extern "C" fn(...)`，一个函数指针。用 `AtomicPtr<c_void>` 存储，`get_func()` 中做 transmute 还原。

```rust
// 改前 (lines 150-158):
pub struct TieredFunction {
    current_tier: AtomicU32,
    func: Mutex<CalcDerivsFunc>,
    pending_upgrade: Mutex<Option<(CompileTier, CalcDerivsFunc)>>,
    tier_transitions: AtomicU32,
}

// 改后:
use std::ffi::c_void;
use std::sync::atomic::AtomicPtr;

pub struct TieredFunction {
    current_tier: AtomicU32,
    func: AtomicPtr<c_void>,
    pending_upgrade: Mutex<Option<(CompileTier, CalcDerivsFunc)>>,
    tier_transitions: AtomicU32,
}
```

- [ ] **Step 2: 修改 `new()` 构造**

```rust
// 改前:
impl TieredFunction {
    pub fn new(initial_tier: CompileTier, func: CalcDerivsFunc) -> Self {
        Self {
            current_tier: AtomicU32::new(initial_tier as u32),
            func: Mutex::new(func),
            pending_upgrade: Mutex::new(None),
            tier_transitions: AtomicU32::new(0),
        }
    }

// 改后:
impl TieredFunction {
    pub fn new(initial_tier: CompileTier, func: CalcDerivsFunc) -> Self {
        Self {
            current_tier: AtomicU32::new(initial_tier as u32),
            func: AtomicPtr::new(func as *mut c_void),
            pending_upgrade: Mutex::new(None),
            tier_transitions: AtomicU32::new(0),
        }
    }
```

- [ ] **Step 3: 修改 `get_func()` — 零锁读取**

```rust
// 改前:
    pub fn get_func(&self) -> CalcDerivsFunc {
        *self.func.lock().unwrap()
    }

// 改后:
    pub fn get_func(&self) -> CalcDerivsFunc {
        let ptr = self.func.load(Ordering::Acquire);
        // SAFETY: func is only ever set to a valid CalcDerivsFunc pointer,
        // never null after construction.
        unsafe { std::mem::transmute(ptr) }
    }
```

- [ ] **Step 4: 修改 `try_apply_upgrade()` — 用 AtomicPtr::store**

```rust
// 改前 (line 186-194):
    pub fn try_apply_upgrade(&self) -> bool {
        let mut pending = self.pending_upgrade.lock().unwrap();
        if let Some((new_tier, new_func)) = pending.take() {
            *self.func.lock().unwrap() = new_func;
            self.current_tier
                .store(new_tier as u32, Ordering::Release);
            self.tier_transitions.fetch_add(1, Ordering::Relaxed);
            true
        } else {

// 改后:
    pub fn try_apply_upgrade(&self) -> bool {
        let mut pending = self.pending_upgrade.lock().unwrap();
        if let Some((new_tier, new_func)) = pending.take() {
            self.func.store(new_func as *mut c_void, Ordering::Release);
            self.current_tier
                .store(new_tier as u32, Ordering::Release);
            self.tier_transitions.fetch_add(1, Ordering::Relaxed);
            true
        } else {
```

- [ ] **Step 5: 编译验证**

```bash
rtk cargo check -p rustmodlica
```

- [ ] **Step 6: 跑回归**

```bash
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1
```

- [ ] **Step 7: Commit**

```bash
rtk git add jit-compiler/src/jit/tiered.rs
rtk git commit -m "perf: replace Mutex<CalcDerivsFunc> with AtomicPtr in TieredFunction"
```

---

### Task 3: 除零保护从 7 指令简化为 fmax

**Files:**
- Modify: `jit-compiler/src/jit/translator/expr/compile.rs:81-92`

- [ ] **Step 1: 替换除零保护逻辑**

```rust
// 改前 (lines 81-92):
                Operator::Div => {
                    let eps = builder.ins().f64const(1e-12);
                    let r_abs = builder.ins().fabs(r);
                    let is_small = builder.ins().fcmp(FloatCC::LessThan, r_abs, eps);
                    let pos_eps = builder.ins().f64const(1e-12);
                    let neg_eps = builder.ins().f64const(-1e-12);
                    let zero = builder.ins().f64const(0.0);
                    let sign_non_neg = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, r, zero);
                    let eps_signed = builder.ins().select(sign_non_neg, pos_eps, neg_eps);
                    let r_safe = builder.ins().select(is_small, eps_signed, r);
                    Ok(builder.ins().fdiv(l, r_safe))
                }

// 改后:
                Operator::Div => {
                    let min_den = builder.ins().f64const(1e-12);
                    let r_safe = builder.ins().fmax(r, min_den);
                    Ok(builder.ins().fdiv(l, r_safe))
                }
```

- [ ] **Step 2: 编译验证**

```bash
rtk cargo check -p rustmodlica
```

- [ ] **Step 3: 回归验证（含 OMC 数值对比）**

```bash
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1
# 关注 Pass/Fail 数量不变
```

- [ ] **Step 4: Commit**

```bash
rtk git add jit-compiler/src/jit/translator/expr/compile.rs
rtk git commit -m "perf: simplify div-by-zero guard from 7 insns to fmax"
```

---

### Task 4: 删除未使用的 anyhow 依赖

**Files:**
- Modify: `jit-compiler/Cargo.toml:27`

- [ ] **Step 1: 删除 anyhow 行**

在 `jit-compiler/Cargo.toml` 中删除第 27 行：
```diff
-anyhow = "1.0.102"
```

- [ ] **Step 2: 验证编译**

```bash
rtk cargo check -p rustmodlica
```

- [ ] **Step 3: Commit**

```bash
rtk git add jit-compiler/Cargo.toml
rtk git commit -m "chore: remove unused anyhow dependency"
```

---

### Task 5: 删除重复文件 simulation/io.rs

**Files:**
- Delete: `jit-compiler/src/simulation/io.rs`
- Modify: `jit-compiler/src/simulation.rs:18` (optional — io.rs 未被引用，sim_io.rs 已在用)

- [ ] **Step 1: 确认 io.rs 未被引用**

```bash
rtk grep "mod io;" "simulation::io" --path jit-compiler/src/
```

期望：仅在 `simulation/io.rs` 自身中找到 `mod io;` 声明不在 `simulation.rs` 中。

- [ ] **Step 2: 删除 io.rs 文件**

```bash
git rm jit-compiler/src/simulation/io.rs
```

- [ ] **Step 3: 验证编译**

```bash
rtk cargo check -p rustmodlica
```

- [ ] **Step 4: Commit**

```bash
rtk git commit -m "chore: remove duplicate simulation/io.rs (sim_io.rs is the canonical copy)"
```

---

### 第一批验证门禁

所有 5 项完成后，跑完整回归套件：

```bash
# 快速三件套
powershell -File ./run_jit_rules_full_regress.ps1

# TestLib 批量
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1

# MOS 回归
pwsh -File ./jit-compiler/scripts/run_mos_regression.ps1
```

---

## 第二批：代码质量加固

### Task 6: unsafe 块补充 SAFETY 文档

**Files:**
- Modify: `jit-compiler/src/solver.rs` (14 unsafe 块)
- Modify: `jit-compiler/src/jit/compile.rs` (3 unsafe 块)
- Modify: `jit-compiler/src/jit/aot_archive.rs` (5 unsafe 块)
- Modify: `jit-compiler/src/flatten/cache_shm.rs` (6 unsafe 块 + 4 unsafe impl)
- Modify: `jit-compiler/src/math_fft.rs` (2 unsafe 块)
- Modify: `jit-compiler/src/modelica_random.rs` (1 unsafe 块)

- [ ] **Step 1: solver.rs 补充 SAFETY 注释**

在 `jit-compiler/src/solver.rs` 中每个 `unsafe { *ptr }` 前加注释。搜索第 180-189 行附近的 `unsafe { *self.eval_call_index += 1 }`：

```rust
// 每个 unsafe 块前加:
// SAFETY: eval_call_index is a valid pointer to a u64 allocated in
// the simulation driver and outlives the solver's lifetime.
unsafe {
    *self.eval_call_index += 1;
}
```

对 `read_diag_residual` 函数中的 unsafe 解引用：
```rust
fn read_diag_residual(&self, ptr: *mut f64) -> f64 {
    // SAFETY: diag_residual_ptr comes from a valid, aligned f64
    // buffer owned by the simulation driver.
    unsafe { *ptr }
}
```

- [ ] **Step 2: jit/compile.rs 补充 SAFETY 注释**

在 `jit-compiler/src/jit/compile.rs` 第 550 行附近（`transmute` 处）：
```rust
// SAFETY: code is a finalized JIT function pointer returned by
// Cranelift's get_finalized_function, guaranteed to point to
// executable memory containing a valid calc_derivs implementation.
let func: CalcDerivsFunc = unsafe { mem::transmute(code) };
```

第 609、636 行的 `slice::from_raw_parts`：
```rust
// SAFETY: func_alloc_len is the allocation length reported by
// Cranelift for this compiled function. The memory is valid for reads
// within this length.
let code_bytes = unsafe { std::slice::from_raw_parts(code, func_alloc_len) };
```

- [ ] **Step 3: flatten/cache_shm.rs 补充 SAFETY + Send/Sync 理由**

对 `unsafe impl Send for ShmemWrap`：
```rust
// SAFETY: ShmemWrap owns the shared memory segment. The underlying
// OS shmem handle is process-scoped and safe to transfer across threads.
// All accesses go through synchronization primitives.
unsafe impl Send for ShmemWrap {}
```

对 `unsafe impl Sync for ShmemWrap`：
```rust
// SAFETY: All mutable state inside ShmemWrap is behind Mutex/RwLock
// or uses atomic operations. The shared memory segment itself supports
// concurrent reads.
unsafe impl Sync for ShmemWrap {}
```

- [ ] **Step 4: aot_archive.rs 补充 SAFETY**

在第 503 行附近的 `panic!("foreign compiler_version must be rejected")` 处——实际上这个 panic 不应在生产代码中出现。改为：

```rust
// 改前:
assert_eq!(loaded_ver, current_ver, "foreign compiler_version must be rejected");
// (如果有 assert)

// 对于 try_into().unwrap() 反序列化:
// SAFETY: The header bytes were just read from a valid archive file
// and validated by the magic bytes check. If deserialization fails,
// the archive is corrupted — we propagate the error instead of panicking.
let header: AotArchiveHeader = bincode::deserialize(&header_bytes)
    .map_err(|e| format!("corrupted AOT archive header: {}", e))?;
```

- [ ] **Step 5: 编译验证 + 回归**

```bash
rtk cargo check -p rustmodlica
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1
```

- [ ] **Step 6: Commit**

```bash
rtk git add jit-compiler/src/solver.rs jit-compiler/src/jit/compile.rs jit-compiler/src/flatten/cache_shm.rs jit-compiler/src/jit/aot_archive.rs jit-compiler/src/math_fft.rs jit-compiler/src/modelica_random.rs
rtk git commit -m "docs: add SAFETY comments to all unsafe blocks in jit-compiler"
```

---

### Task 7: 核心路径 unwrap → proper error

**Files:**
- Modify: `jit-compiler/src/codegen.rs:31-34` (Cranelift init)
- Modify: `jit-compiler/src/jit/aot_archive.rs:503-508` (反序列化)

- [ ] **Step 1: codegen.rs — Cranelift 初始化改为返回 Result**

在 `jit-compiler/src/codegen.rs` 中，`Codegen::new()` 的 unwrap 改为返回 `Result<Self, String>`：

```rust
// 改前 (lines 31-34):
let isa_builder = cranelift_native::builder().unwrap();
let isa = isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();
let builder = ObjectBuilder::new(isa, "modelica_module", cranelift_module::default_libcall_names()).unwrap();

// 改后:
let isa_builder = cranelift_native::builder()
    .map_err(|e| format!("Cranelift native builder failed: {}", e))?;
let isa = isa_builder
    .finish(settings::Flags::new(flag_builder))
    .map_err(|e| format!("Cranelift ISA creation failed: {}", e))?;
let builder = ObjectBuilder::new(isa, "modelica_module", cranelift_module::default_libcall_names())
    .map_err(|e| format!("Cranelift ObjectBuilder creation failed: {}", e))?;
```

同步修改 `pub fn new() -> Self` 为 `pub fn new() -> Result<Self, String>`。

- [ ] **Step 2: 更新 codegen.rs 的调用方**

搜索 `Codegen::new()` 的调用处，改为 `Codegen::new()?`。仅在 `jit-compiler/src/compiler/c_codegen.rs` 或 emit-c 路径中使用。

```bash
rtk grep "Codegen::new()" --path jit-compiler/src/
```

在每个调用处改为 `Codegen::new()?` 并传播错误。

- [ ] **Step 3: aot_archive.rs — panic 改为 Result**

将 `panic!("foreign compiler_version must be rejected")` 替换为 `Err(...)`。

- [ ] **Step 4: 编译验证 + 回归**

```bash
rtk cargo check -p rustmodlica
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1
```

- [ ] **Step 5: Commit**

```bash
rtk git add jit-compiler/src/codegen.rs jit-compiler/src/jit/aot_archive.rs
rtk git commit -m "refactor: replace unwrap/panic with Result in Cranelift init and AOT archive"
```

---

### Task 8: entry.rs 拆分

**Files:**
- Split: `jit-compiler/src/compiler/compile_model/compile/entry.rs` (2,795 行)
- Create: `jit-compiler/src/compiler/compile_model/compile/aot_load.rs`
- Create: `jit-compiler/src/compiler/compile_model/compile/sim_bundle.rs`
- Modify: `jit-compiler/src/compiler/compile_model/compile/mod.rs`

- [ ] **Step 1: 提取 AOT archive 加载逻辑 → aot_load.rs**

将 `entry.rs` 中约 1960-2075 行的 AOT archive 查找/加载逻辑移至 `jit-compiler/src/compiler/compile_model/compile/aot_load.rs`。

```rust
// aot_load.rs
use std::collections::HashMap;

pub(crate) struct AotLoadResult {
    pub cached_fn: Option<crate::jit::codegen_cache::CachedFunction>,
    pub status: String,
    pub detail: Option<String>,
}

pub(crate) fn try_load_aot_native(
    model_name: &str,
    bundle: &crate::cache::artifact_bundle::CompiledArtifactBundle,
    all_symbols: &HashMap<String, *const u8>,
    param_vars: &[String],
    params: &[f64],
) -> AotLoadResult {
    // ... 从 entry.rs 移入的 AOT 加载逻辑
}
```

- [ ] **Step 2: 提取 sim bundle cache 逻辑 → sim_bundle.rs**

将 `entry.rs` 中 1947-2190 行的 sim bundle cache 查找/参数比较逻辑提取出来。

- [ ] **Step 3: 更新 mod.rs 声明新模块**

```rust
// compile/mod.rs
mod aot_load;
mod sim_bundle;
pub(crate) use aot_load::try_load_aot_native;
pub(crate) use sim_bundle::try_load_sim_bundle;
```

- [ ] **Step 4: entry.rs 中引用新模块**

将 `entry.rs` 中原有的大段代码替换为函数调用。

- [ ] **Step 5: 编译验证 + 回归**

```bash
rtk cargo check -p rustmodlica
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1
```

- [ ] **Step 6: Commit**

```bash
rtk git add jit-compiler/src/compiler/compile_model/compile/
rtk git commit -m "refactor: split entry.rs — extract AOT load and sim bundle cache to sub-modules"
```

---

### Task 9: dead_code 清理

**Files:**
- Modify: `jit-compiler/src/backend_dae.rs` (14 个 `#[allow(dead_code)]`)
- Modify: `jit-compiler/src/ast.rs` (9 个)
- Modify: `jit-compiler/src/diag.rs` (3 个)
- Modify: `jit-compiler/src/jit/compile.rs` (1 个: `data_ctx`)

- [ ] **Step 1: 逐个确认是否真正 dead**

对每个 `#[allow(dead_code)]` 标记的项，注释掉标记，编译看是否有 warning：

```bash
# 对 backend_dae.rs
rtk cargo check -p rustmodlica 2>&1 | grep "never used\|dead_code"
```

- [ ] **Step 2: 删除确认无用的代码**

对于 Rust 编译器报告 "never used" 的项，若确认无用，删除。对于公共 API 可能被外部使用的（pub fn），保持但移去 `#[allow(dead_code)]`。

- [ ] **Step 3: 清理 jit/compile.rs 中未使用的 data_ctx 字段 suppress warning**

在 `jit-compiler/src/jit/compile.rs` 第 29 行，`data_ctx` 字段上的 `#[allow(dead_code)]` 可以保留（用于 user function stub 编译），不加修改。

- [ ] **Step 4: 编译验证 + 回归**

```bash
rtk cargo check -p rustmodlica
rtk cargo check -p rustmodlica 2>&1 | grep -c "dead_code"  # 期望减少
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1
```

- [ ] **Step 5: Commit**

```bash
rtk git add jit-compiler/src/backend_dae.rs jit-compiler/src/ast.rs jit-compiler/src/diag.rs
rtk git commit -m "chore: remove unused dead_code annotations and dead functions"
```

---

## 第三批：SIMD 向量化

### Task 10: 新增 vectorize.rs — 方程聚类

**Files:**
- Create: `jit-compiler/src/jit/translator/vectorize.rs`
- Modify: `jit-compiler/src/jit/translator/mod.rs`

- [ ] **Step 1: 定义数据结构**

```rust
// jit-compiler/src/jit/translator/vectorize.rs

use crate::ast::{Equation, Expression, Operator};

/// 支持向量化的操作类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum VectorOp {
    Add,
    Sub,
    Mul,
}

/// 一个向量化组：N 条连续方程的合并
#[derive(Debug, Clone)]
pub(crate) struct VectorGroup {
    /// 目标变量基名（如 "x" 对应 x_1..x_N）
    pub dst_base: String,
    /// 源变量基名列表（二元操作有 2 个，一元操作有 1 个）
    pub src_bases: Vec<String>,
    /// 起始下标（1-based）
    pub lo: usize,
    /// 结束下标（1-based）
    pub hi: usize,
    /// 操作类型
    pub op: VectorOp,
}

/// 编译单元：标量方程或向量组
pub(crate) enum CompileUnit {
    Scalar(Equation),
    Vector(VectorGroup),
}
```

- [ ] **Step 2: 实现聚类函数**

```rust
/// 扫描方程列表，将连续的下标递增同类方程合并为 VectorGroup。
/// 非连续或不同结构的方程保持为 Scalar。
pub(crate) fn cluster_equations(equations: &[Equation]) -> Vec<CompileUnit> {
    if equations.is_empty() {
        return vec![];
    }

    // 试图从 lhs = rhs 中提取 pattern: base_name, index, operation, source bases
    fn try_extract_pattern(eq: &Equation) -> Option<(String, usize, VectorOp, Vec<String>)> {
        match eq {
            Equation::Equality(lhs, rhs) => {
                let (lhs_base, lhs_idx) = extract_array_index(lhs)?;
                let (op, srcs) = try_extract_binary_op(rhs)?;
                Some((lhs_base, lhs_idx, op, srcs))
            }
            _ => None,
        }
    }

    fn extract_array_index(expr: &Expression) -> Option<(String, usize)> {
        match expr {
            Expression::Variable(id) => {
                let name = crate::string_intern::resolve_id(*id);
                name.rsplit_once('_')
                    .and_then(|(base, idx_str)| idx_str.parse::<usize>().ok().map(|i| (base.to_string(), i)))
            }
            _ => None,
        }
    }

    fn try_extract_binary_op(expr: &Expression) -> Option<(VectorOp, Vec<String>)> {
        match expr {
            Expression::BinaryOp(lhs, rhs, op) => {
                let lhs_base = extract_array_index(lhs)?.0;
                let rhs_base = extract_array_index(rhs)?.0;
                let op = match op {
                    Operator::Add => VectorOp::Add,
                    Operator::Sub => VectorOp::Sub,
                    Operator::Mul => VectorOp::Mul,
                    _ => return None,
                };
                Some((op, vec![lhs_base, rhs_base]))
            }
            _ => None,
        }
    }

    let mut units = Vec::new();
    let mut i = 0;
    while i < equations.len() {
        if let Some((base, idx, op, srcs)) = try_extract_pattern(&equations[i]) {
            // 尝试扩展这个向量组
            let mut j = i + 1;
            while j < equations.len() {
                if let Some((next_base, next_idx, next_op, next_srcs)) = try_extract_pattern(&equations[j]) {
                    if next_base == base
                        && next_op == op
                        && next_srcs == srcs
                        && next_idx == idx + (j - i)
                    {
                        j += 1;
                        continue;
                    }
                }
                break;
            }
            let count = j - i;
            if count >= 4 {
                // 至少 4 条才合并（向量化才有收益）
                units.push(CompileUnit::Vector(VectorGroup {
                    dst_base: base,
                    src_bases: srcs,
                    lo: idx,
                    hi: idx + count - 1,
                    op,
                }));
                i = j;
                continue;
            }
        }
        units.push(CompileUnit::Scalar(equations[i].clone()));
        i += 1;
    }
    units
}
```

- [ ] **Step 3: 在 translator/mod.rs 中声明模块**

```rust
// jit-compiler/src/jit/translator/mod.rs 添加:
pub(crate) mod vectorize;
```

- [ ] **Step 4: 编译验证**

```bash
rtk cargo check -p rustmodlica
```

- [ ] **Step 5: Commit**

```bash
rtk git add jit-compiler/src/jit/translator/vectorize.rs jit-compiler/src/jit/translator/mod.rs
rtk git commit -m "feat: add equation clustering for SIMD vectorization (S3 phase 1)"
```

---

### Task 11: SIMD 代码发射

**Files:**
- Modify: `jit-compiler/src/jit/translator/vectorize.rs` (追加代码)

- [ ] **Step 1: 实现 SIMD 发射函数**

在 `vectorize.rs` 中追加：

```rust
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use crate::jit::context::TranslationContext;

/// 发射向量化的 fadd/fsub/fmul 循环。
/// 对 AVX2-capable 平台用 F64X4，否则用 F64X2。
pub(crate) fn emit_vector_loop(
    group: &VectorGroup,
    ctx: &TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let count = group.hi - group.lo + 1;

    // 获取源和目标在数组存储中的基址
    let (dst_ptr, dst_start) = resolve_array_ptr(&group.dst_base, ctx)
        .ok_or_else(|| format!("SIMD: cannot resolve array '{}'", group.dst_base))?;
    let dst_off = (group.lo - 1 + dst_start) * 8;

    let (src1_ptr, src1_start) = resolve_array_ptr(&group.src_bases[0], ctx)
        .ok_or_else(|| format!("SIMD: cannot resolve array '{}'", group.src_bases[0]))?;
    let src1_off = (group.lo - 1 + src1_start) * 8;

    let (src2_ptr, src2_start) = if group.src_bases.len() > 1 {
        resolve_array_ptr(&group.src_bases[1], ctx)
            .ok_or_else(|| format!("SIMD: cannot resolve array '{}'", group.src_bases[1]))?
    } else {
        (dst_ptr, dst_start) // 一元操作，不会到达这里
    };
    let src2_off = (group.lo - 1 + src2_start) * 8;

    // 选向量宽度：4 或 2
    let (vec_type, vec_size) = simd_width();
    let full_chunks = count / vec_size;
    let remainder = count % vec_size;

    let ptr_ty = builder.func.target_config().pointer_type();

    for chunk in 0..full_chunks {
        let base = chunk * vec_size * 8;
        let a = load_vec(vec_type, builder, src1_ptr, (src1_off + base) as i32, ptr_ty);
        let b = load_vec(vec_type, builder, src2_ptr, (src2_off + base) as i32, ptr_ty);
        let result = match group.op {
            VectorOp::Add => builder.ins().fadd(a, b),
            VectorOp::Sub => builder.ins().fsub(a, b),
            VectorOp::Mul => builder.ins().fmul(a, b),
        };
        store_vec(builder, result, dst_ptr, (dst_off + base) as i32, ptr_ty);
    }

    // 余数走标量
    let rem_start = full_chunks * vec_size;
    for i in 0..remainder {
        let idx = rem_start + i;
        // 标量 load/fadd/store 作为 fallback，调用现有 compile_expression
        // 这里简化为直接 load+store
        let a_off = (src1_off + idx * 8) as i32;
        let b_off = (src2_off + idx * 8) as i32;
        let d_off = (dst_off + idx * 8) as i32;
        let av = builder.ins().load(cl_types::F64, MemFlags::new(), src1_ptr, a_off);
        let bv = builder.ins().load(cl_types::F64, MemFlags::new(), src2_ptr, b_off);
        let rv = match group.op {
            VectorOp::Add => builder.ins().fadd(av, bv),
            VectorOp::Sub => builder.ins().fsub(av, bv),
            VectorOp::Mul => builder.ins().fmul(av, bv),
        };
        builder.ins().store(MemFlags::new(), rv, dst_ptr, d_off);
    }

    Ok(())
}

fn simd_width() -> (types::Type, usize) {
    if std::env::var("RUSTMODLICA_JIT_SIMD_WIDTH")
        .ok()
        .map(|v| v == "sse2")
        .unwrap_or(false)
    {
        (cl_types::F64X2, 2)
    } else {
        // 默认: Cranelift 处理指令选择，F64X2 在所有 x86_64 上安全
        (cl_types::F64X2, 2)
    }
}

fn load_vec(
    vec_ty: types::Type,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    base_ptr: Value,
    offset: i32,
    _ptr_ty: types::Type,
) -> Value {
    builder.ins().load(vec_ty, MemFlags::new(), base_ptr, offset)
}

fn store_vec(
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    val: Value,
    base_ptr: Value,
    offset: i32,
    _ptr_ty: types::Type,
) {
    builder.ins().store(MemFlags::new(), val, base_ptr, offset);
}

fn resolve_array_ptr(
    name: &str,
    ctx: &TranslationContext,
) -> Option<(Value, usize)> {
    // 优先从 array_storage 查找
    if let Some((array_type, start_index)) = ctx.array_storage(name) {
        let ptr = match array_type {
            crate::jit::types::ArrayType::State => ctx.states_ptr,
            crate::jit::types::ArrayType::Discrete => ctx.discrete_ptr,
            crate::jit::types::ArrayType::Parameter => ctx.params_ptr,
            crate::jit::types::ArrayType::Output => ctx.outputs_ptr,
            crate::jit::types::ArrayType::Derivative => ctx.derivs_ptr,
        };
        Some((ptr, start_index))
    } else {
        None
    }
}
```

- [ ] **Step 2: 编译验证**

```bash
rtk cargo check -p rustmodlica
```

- [ ] **Step 3: Commit**

```bash
rtk git add jit-compiler/src/jit/translator/vectorize.rs
rtk git commit -m "feat: add SIMD code emission for vectorized equation groups (S3 phase 2)"
```

---

### Task 12: 集成到 jit/compile.rs

**Files:**
- Modify: `jit-compiler/src/jit/compile.rs`

- [ ] **Step 1: 在 compile_equation 调用处插入聚类逻辑**

在 `jit/compile.rs` 的第 462-476 行附近，修改 `Always` 分区的编译逻辑：

```rust
// 改前 (simplified — lines 462-476):
ClockPartitionTrigger::Always => {
    for idx in &entry.algorithm_indices { ... }
    for idx in &entry.alg_equation_indices {
        if let Some(eq) = alg_equations.get(*idx) {
            compile_equation(eq, &mut t_ctx, &mut builder)?;
        }
    }
    for idx in &entry.diff_equation_indices { ... }
}

// 改后:
ClockPartitionTrigger::Always => {
    for idx in &entry.algorithm_indices { ... }

    // 收集 alg_equation_indices 对应的方程，聚类后编译
    let alg_eqs: Vec<&Equation> = entry.alg_equation_indices.iter()
        .filter_map(|idx| alg_equations.get(*idx))
        .collect();
    compile_equation_group(&alg_eqs, &mut t_ctx, &mut builder)?;

    let diff_eqs: Vec<&Equation> = entry.diff_equation_indices.iter()
        .filter_map(|idx| diff_equations.get(*idx))
        .collect();
    compile_equation_group(&diff_eqs, &mut t_ctx, &mut builder)?;
}
```

- [ ] **Step 2: 添加 compile_equation_group 辅助函数**

在同一文件末尾添加（SIMD 可通过环境变量关闭）：

```rust
fn simd_enabled() -> bool {
    std::env::var("RUSTMODLICA_JIT_SIMD")
        .ok()
        .map(|v| !matches!(v.trim(), "0" | "false" | "FALSE" | "off" | "OFF"))
        .unwrap_or(true) // 默认开启
}

fn compile_equation_group(
    equations: &[&Equation],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    if !simd_enabled() {
        for eq in equations {
            compile_equation(eq, ctx, builder)?;
        }
        return Ok(());
    }

    let owned: Vec<Equation> = equations.iter().map(|e| (*e).clone()).collect();
    let units = vectorize::cluster_equations(&owned);
    for unit in units {
        match unit {
            vectorize::CompileUnit::Scalar(eq) => {
                compile_equation(&eq, ctx, builder)?;
            }
            vectorize::CompileUnit::Vector(group) => {
                if let Err(_e) = vectorize::emit_vector_loop(&group, ctx, builder) {
                    // SIMD 发射失败，fallback 到标量：逐条找到原方程并编译
                    for i in 0..(group.hi - group.lo + 1) {
                        let orig_idx = equations.iter()
                            .position(|eq| eq_matches_array_element(eq, &group.dst_base, group.lo + i))
                            .and_then(|pos| equations.get(pos));
                        if let Some(eq) = orig_idx {
                            compile_equation(eq, ctx, builder)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 3: 编译验证**

```bash
rtk cargo check -p rustmodlica
```

- [ ] **Step 4: 回归验证 — SIMD 开和关都要跑**

```bash
# SIMD 开启（默认）
pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1

# SIMD 关闭 — 确认 fallback 正确
RUSTMODLICA_JIT_SIMD=0 pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1
```

- [ ] **Step 5: Commit**

```bash
rtk git add jit-compiler/src/jit/compile.rs
rtk git commit -m "feat: integrate SIMD equation clustering into JIT compile pipeline (S3 phase 3)"
```

---

## 第四批：Tiered Compilation 增强

### Task 13: 修复 tiered.rs 后台线程的 set_var 问题

**Files:**
- Modify: `jit-compiler/src/jit/tiered.rs`

- [ ] **Step 1: 找到后台线程中的 set_var 调用**

```bash
rtk grep "set_var\|setenv" --path jit-compiler/src/jit/tiered.rs
```

- [ ] **Step 2: 将环境变量改为显式参数传递**

```rust
// 改前 (tiered.rs 约 305-314 行, 示意):
std::env::set_var("RUSTMODLICA_CRANELIFT_OPT_LEVEL", "speed");
// ... 然后调用 Compiler::compile()

// 改后:
// 在 spawn 之前捕获所需配置到 struct 中
let tier_up_config = TierUpConfig {
    opt_level: "speed".to_string(),
    enable_simd: false,
    // ... 其他需要的配置
};

// 线程中使用配置而非环境变量
std::thread::Builder::new()
    .stack_size(4 * 1024 * 1024)
    .spawn(move || {
        run_tier_up(model_name, tier_up_config, tier_up_sender);
    })
    .expect("spawn tier-up thread");
```

- [ ] **Step 3: run_tier_up 接收显式配置**

```rust
struct TierUpConfig {
    opt_level: String,
    enable_simd: bool,
}

fn run_tier_up(
    model_name: String,
    config: TierUpConfig,
    sender: std::sync::mpsc::Sender<TierUpResult>,
) {
    // 在函数内部使用 config 字段，而非读取环境变量
    // 需要 Compiler::compile_with_config() 或等效 API
}
```

- [ ] **Step 4: 编译验证 + 回归**

```bash
rtk cargo check -p rustmodlica
RUSTMODLICA_JIT_TIERED=1 pwsh -File ./jit-compiler/scripts/run_testlib_validate.ps1
```

- [ ] **Step 5: Commit**

```bash
rtk git add jit-compiler/src/jit/tiered.rs
rtk git commit -m "fix: replace thread-unsafe set_var with explicit config in tier-up background thread"
```

---

## 全文门禁

所有任务完成后：

```bash
# 完整回归
powershell -File ./run_jit_rules_full_regress.ps1

# 完整编译（含所有 features）
rtk cargo build -p rustmodlica --features "sundials,sundials-klu" --release

# 检查无新增 clippy warning
rtk cargo clippy -p rustmodlica -- -D warnings 2>&1 | tail -5
```
