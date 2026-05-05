# JIT 编译器优化设计

日期：2026-05-05，分支：nicho-centipede

## 背景

对 jit-compiler 的全面代码审查发现了 12 项可优化点，覆盖运行时性能、编译时开销、代码质量和测试。本文档聚焦于有明确高收益路径的优化项，按依赖关系和风险分四批推进。经过对 SIMD 和并行化的深入技术讨论，确定了架构约束和可行边界。

关键发现：Cranelift 的 `FunctionBuilder` 不是 `Send + Sync`，因此单个 JIT 函数内的 IR 构建无法并行化；Cranelift 目前没有自动向量化能力。模型内编译并行化在当前架构下不可行。

## 分批计划

### 第一批：快速 Win（5 项，无架构依赖）

1. **stack_scratch 默认开启** — `solver.rs`：将 `RUSTMODLICA_JIT_STACK_SCRATCH` 默认值从 `false` 改为 `true`，消除每次求解器求值时的 Vec 分配
2. **TieredFunction Mutex → AtomicPtr** — `jit/tiered.rs`：热路径上的 `get_func()` 每次调用都做 `Mutex::lock().clone()`，改为 `AtomicPtr` 零开销读取
3. **除零保护简化** — `jit/translator/expr/compile.rs`：将 7 条指令的除零保护改为 `fmax(分母, 1e-12)`，减少代码膨胀
4. **删除 dead dependency anyhow** — `Cargo.toml`：从未被使用的依赖
5. **删除重复文件** — `simulation/io.rs` 和 `simulation/sim_io.rs`：完全重复的 12 行文件，保留一个

### 第二批：代码质量加固（4 项，独立于功能）

6. **unsafe 块补充 SAFETY 文档** — 约 60+ 处 unsafe 块添加 `// SAFETY:` 注释
7. **核心路径 unwrap → proper error** — 解析器、Cranelift 初始化、AOT archive 的 unwrap 改为 Result 传播
8. **entry.rs 拆分** — 2,795 行拆分为多个子模块
9. **`#[allow(dead_code)]` 清理** — 移除约 30 处 dead_code 标记或删除对应代码

### 第三批：SIMD 向量化（S3 方案）

10. **方程级聚类 + SIMD 代码生成** — 在代码生成边界层，扫描连续同类标量方程，合并为向量操作，用 Cranelift 向量类型（F64X2/F64X4）发射 SIMD IR

**方案选择**：经过三种方案对比（S1 展平保留数组、S2 Cranelift 自动向量化、S3 边界层聚类），选择 S3：
- S1 改动范围过大（展平器 + IR + 代码生成器），作为长期演进方向
- S2 不可行——Cranelift 没有自动向量化 pass
- S3 在方程到 IR 的边界层做轻量聚类，不改 IR 结构，不改展平器

### 第四批：Tiered Compilation 增强

11. **TieredFunction 后台编译线程安全修复** — `jit/tiered.rs` 中后台线程使用 `set_var`（非线程安全），改为显式传参。增强后台切换逻辑。

## S3 详细设计：SIMD 向量化

### 架构位置

```
jit/compile.rs (修改)
  └── 调用 compile_equation 之前，插入聚类逻辑

新增文件:
  jit/translator/vectorize.rs  — 聚类 + SIMD 发射
```

### 聚类规则

扫描连续的 `Equation::Equality(lhs, rhs)`，从变量名后缀提取下标：

```
检测到连续 N 条：
  x_1 = a_1 + b_1    (lhs="x", idx=1; rhs=BinaryOp(Add, Var("a_1"), Var("b_1")))
  x_2 = a_2 + b_2    (同结构，idx=2)
  ...
  x_N = a_N + b_N    (同结构，idx=N)

→ VectorGroup { dst: "x", src1: "a", src2: "b", lo: 1, hi: N, op: Add }
```

支持操作：Add、Sub、Mul、常数赋值。不支持的情况自动 fallback 到原标量路径。

### 代码生成

对每个 `VectorGroup`，按平台能力选择向量宽度（AVX2 → F64X4，SSE2 → F64X2），生成向量 load/fadd/fmul/store 循环，余数走标量。Cranelift 后端负责将 F64X2/F64X4 映射到 SSE/AVX 指令。

### 环境变量

- `RUSTMODLICA_JIT_SIMD=1` — 启用 SIMD 向量化（默认开启）
- `RUSTMODLICA_JIT_SIMD_WIDTH=auto|avx2|sse2` — 向量宽度（默认 auto，自动检测）

## 风险矩阵

| 优化项 | 可行性风险 | 正确性风险 | 回滚难度 |
|--------|-----------|-----------|---------|
| stack_scratch 默认开启 | 低 | 低（已有实现，只是改默认值） | 一行 |
| Mutex → AtomicPtr | 低 | 中（并发正确性） | 局部回滚 |
| 除零保护简化 | 低 | 低（数值行为微调） | 一行 |
| dead dep 删除 | 零 | 零 | 一行 |
| 重复文件删除 | 零 | 低 | 还原文件 |
| SAFETY 文档 | 零 | 零 | 无需回滚 |
| unwrap → Result | 低 | 低 | 局部回滚 |
| entry.rs 拆分 | 中 | 中 | git revert |
| SIMD 向量化 | 中 | 中（需回归验证） | feature gate |
| Tiered 线程安全 | 低 | 中 | 局部回滚 |

## 验证策略

- 第一批：现有回归套件（`run_regression.ps1`、`run_testlib_validate.ps1`、`run_mos_regression.ps1`）全部通过
- 第二批：编译通过 + 回归套件通过
- 第三批：SIMD 开关对比测试，验证数值一致性；`compare_omc.ps1` 对比 OMC 结果
- 第四批：`RUSTMODLICA_JIT_TIERED=1` 下的回归套件通过
