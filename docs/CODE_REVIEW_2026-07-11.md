# RustModlica 代码审查报告（2026-07-11）

审查对象：`jit-compiler`（crate `rustmodlica`）。
方法：按子系统并行派发多个审查 agent，用 codegraph 定位，只记录有代码证据的**正确性**问题；主线对高危项独立核实。

**图例**：✅ = 已读源码独立确认；⚠️ = agent 有证据、未逐行复核。
**门控**：许多问题仅在对应特性开关开启时才走，已在每条标注。

---

## 第一轮：整 crate 审查

### 🔴 Critical（默认路径、静默算错）

| # | 位置 | 缺陷 | 触发场景 |
|---|------|------|---------|
| C1 ✅ | `jit/translator/expr/compile.rs:81-85` | 除法用 `fmax(r,1e-12)` 钳位分母，所有负分母被替换成 `1e-12` | `10/-2` → `1e13`，符号量级全错；解释器 tier 正确 → tier 间不一致 |
| C2 ✅ | `analysis/blt/sort.rs:204-238` | `dfs_iter` 增广路径把变量绑到子帧方程而非父帧 → 起始方程仍未匹配 | `unassigned_count` 虚高 → 误报 `differential_index=2` → 错误指数约简 + 错误 BLT 块；含残差方程 `0=f(...)` 的正常模型即命中 |
| C3 ✅ | `solver/mod.rs:554-656` + `simulation.rs:659,868` | AdaptiveRK45 内部缩小 `dt` 算出 `y5` 写回，但不回传实际步长，driver 按原 `dt` 推进时间 | `--solver rk45` 触发一次拒绝 → 状态对应 `t+dt/2`、时间记为 `t+dt`，整条轨迹失配 |
| C4 ✅ | `flatten/expand/flattener_impl_early.rs:337-364` | 常量范围 `for` 迭代 >100 的分支丢弃 `temp_conn`(connect) 和 `temp_alg`(algorithm) | `for i in 1:200 loop connect(pin[i],bus); end for;` → 200 条 connect 全丢，无诊断；100 次正常、101 次坏 |

### 🟠 High

| # | 位置 | 缺陷 | 触发/门控 |
|---|------|------|---------|
| H1 ⚠️ | `analysis/blt/sort.rs:444-547` | SCC 块按 condensation 拓扑序发射为消费者先于生产者（反了）；仅全 `Simple` 方程时才被修正 | 代数环块 + 简单赋值混合时，块用陈旧值求解 |
| H2 ⚠️ | `solver/qss.rs:135-155` | 每微步对所有非穿越状态从陈旧 `q[i]` anchor 重算，只有穿越态刷新 anchor | `--solver qss` 多状态耦合 → 非穿越态反复被拉回旧值 |
| H3 ⚠️ | `simulation.rs:747-763,863` | 零穿越落在步首时 `dt_event≈0`，`<1e-10` guard 为空 | `when x>=0` 恰在采样点变活 → 仿真挂死 |
| H4 ⚠️ | `query_db/mod_queries_tail.rs:87-105` + `cache/cache_key.rs:112-124` | 缺失文件探测不记负依赖；`libs_closure_hash` 只哈希路径字符串非目录内容 | 先扁平化 `M`（`P.Foo` 不存在）→ 新增 `P/Foo.mo` → 冷进程缓存 key 命中 → 复用过期扁平化 |
| H5 ✅ | `modelica_random.rs:23-51` | xorshift64* 乘法后才存状态（应存乘法前）；128+ 把 `+s0` 折进持久状态（同文件 1024* 正确，佐证） | 首样本对、之后与 OMC/Dymola 发散；`Xorshift64star/128plus` 不可复现 |

### 🟡 Medium（feature-gated / opt-in）

- M1 ⚠️ `simulation/sundials/linsol.rs:199-207`：KLU 稀疏 Jacobian 从全零 dense 矩阵取稀疏模式 → NNZ=0 恒空。仅 `sundials-klu`。
- M2 ⚠️ `flatten/mod.rs`：legacy `flatten_with_mode` 与 salsa `eq_expand` 对 algorithm/initial-equation 处理不同 → 同模型同 mode 可翻转 validate 结论。门控 `RUSTMODLICA_SALSA`。
- M3 ⚠️ `compiler/jacobian.rs:41-80`：符号 ODE Jacobian 只对状态符号求导，忽略代数变量依赖（`der_x=a;a=x*x` 行全零）。opt-in symbolic。
- M4 ✅ `expr_eval.rs:47`：`integer()` 用 `trunc` 而非 `floor`，负数错。作用域窄（function-entry 标量回退）。
- M5 ⚠️ `loader.rs:317-322 等`：同名短名跨包首次插入者胜出，非限定查找可能绑错包。
- M6 ⚠️ `jit/deopt.rs:110-125`：`check_and_apply` 无条件清全局 `DEOPT_PENDING`；多 manager/并发被吞。

### ⚪ Low / 加固

- L1 ⚠️ `math_fft.rs:49-60`：偶长实 FFT 的 Nyquist bin 用 `2/nu`（应 `1/nu`），幅值 2×。未对齐 MSL 确认。
- L2 ⚠️ `cache/cache_key.rs:61-77`：`stable_hash` 变长字段无分隔符拼接，结构非单射（当前不可利用）。
- L3 ⚠️ `jit/translator/expr/compile.rs:97-98`：JIT 用精确 `==`，解释器用 `1e-15` 容差 → tier 分歧。

### 死代码（非 bug，编辑陷阱）
`flatten/flattener_impl.rs` 是 `mod.rs` 中 `impl Flattener` 块的字节级重复副本，**未挂载**（`mod.rs` 无 `mod flattener_impl;`）。改错副本会静默无效。

---

## 第二轮：JIT 子系统（`jit/`，~16k 行 / 58 文件）深挖

### 🔴 Critical（默认路径、静默算错）

| # | 位置 | 缺陷 | 触发场景 |
|---|------|------|---------|
| J1 ✅ | `translator/expr/compile.rs:81-85` | 同 C1（除法钳位） | 默认 JIT 路径 |
| J2 ✅ | `translator/vectorize.rs:328-354` | SIMD 全块循环用 `base=chunk*vs*8` 作偏移，**丢弃 per-array `start_index`**（`dst_off/src1_off` 被 `let _=` 弃）；remainder 循环正确用 `(start+k)*8` 反证 | SIMD 默认开、≥4 方程触发；多数组共享存储类时读/写错地址、覆写源数组 |
| J3 ✅ | `translator/algorithm/control_flow.rs:133-136` | `for` 恒用 `fcmp(LessThanOrEqual)`，不看步长符号 | `for i in 3:-1:1` 不执行（数组推导路径检查了方向 → 自相矛盾） |

### 🟠 High（真实，多为开关/opt-in/跨机）

| # | 位置 | 缺陷 | 触发/门控 |
|---|------|------|---------|
| J4 ⚠️ | `object_emit.rs:14` + `aot_archive.rs:28-34,144-150` | AOT/codegen 缓存烘入宿主 CPU 指令，key/指纹只含 arch-os+exe 哈希、无 CPU 特性 | 跨机共享缓存目录 → 非法指令 SIGILL；JIT 免疫 |
| J5 ⚠️ | `interpreter.rs:222-291` | 解释器只算 diff 方程，**从不算代数方程**，代数变量取 0.0 | `y=2*x;der(x)=-y` → `der(x)=0`。门控 `RUSTMODLICA_JIT_TIER0_BYPASS` |
| J6 ⚠️ | `interpreter.rs:256/271` | 解释器丢弃 `time` 参数 → 解析为 0.0 | `der(x)=time` → 常数。同上门控 |
| J7 ⚠️ | `tiered.rs:337-338` | 后台 tier-up 存裸 fn 指针后析构 `artifacts`，释放代码内存 | 悬垂指针 → UAF。门控 `RUSTMODLICA_TIERED_COMPILATION` |
| J8 ⚠️ | `tiered.rs:328-338` | 后台重编译独立重导变量顺序（且关 const-fold/DCE），不校验布局就装载 | 新函数按错索引读写 states/discrete → 错值或 OOB。同上门控 |
| J9 ⚠️ | `interpreter.rs` / `deopt.rs:16` | `INTERPRETER_CTX`、`DEOPT_PENDING` 进程级全局、不按模型区分 | IDE 并发多仿真串味；跨仿真误/漏 deopt |
| J10 ⚠️ | `codegen_cache/cache_key.rs:266-277` + `cache_store.rs:159-171` | 磁盘缓存 key/校验遗漏 `type_profile_hash`、`param_signature`（内存缓存含） | opt-in `RUSTMODLICA_JIT_TYPE_SPECIALIZATION`：整数特化代码服务给小数场景 |
| J11 ⚠️ | `object_emit.rs:29-39` + `compile.rs` | block-compile 对象缓存只导出 `calc_derivs`，丢 `__block_N` → 裸 blob 回退含悬垂 PC 相对调用 | 门控 `RUSTMODLICA_BLOCK_COMPILE` |

### 🟡 Medium（策略路由静默退化 / 门控 / 定位错）

- J12 ⚠️ `jit_policy.rs:158-182`：`merge_policy` 漏拷 overlay 布尔 → JSON 无法关掉 `pre(带_名)→0` 宽泛回退。
- J13 ⚠️ `jit_policy.rs:163,380-411`：overlay 规则追加末尾 + 首匹配胜出 → 无法覆盖/纠正默认规则。
- J14 ⚠️ `jit_policy.rs:402-404,313-334`：strict 对 `function_builtin`/`dot`/`algorithm` fail-open，`homotopy`/`regStep`/`semiLinear` 未链接 → 退化成 `args[0]`，反而更错。
- J15 ⚠️ `default_jit_policy.json`：`_f`/`_u`/`Trigger` 等常见后缀未解析变量静默返 0/1，掩盖丢变量 bug。
- J16 ⚠️ `default_function_builtin_rules.json`：`contains ".Internal."`/`BaseClasses`→const0 遮蔽专用 blend handler。
- J17 ⚠️ `native.rs:675` + `clock_lowering.rs`：`sample()` 在 `k*T-ε` 提前一 tick 触发，跨步时可能重复触发。
- J18 ⚠️ `context.rs:361-377`：`array_storage` 名字碰撞按 State→…→Output 首命中 → Output 变量若存在同名 State `{name}_1` 则寻错段。
- J19 ✅ `cache_store.rs:149,198`：多层缓存读取 `.ok()?` 首层缺文件即整体返回 → user/std 层永不检查 → 多余重编译（安全，应改 `continue`）。
- J20 ⚠️ `context.rs:310-314`：block 子上下文丢 string/f64-array/external-call 状态（门控）。
- J21 ⚠️ 解释器 `mod` 截断余数 vs JIT Euclidean（`mod(-3,2)` 1 vs -1）、`pre(x)` 返回当前值、div/array/未知调用静默返 0（门控）。

### ⚪ Low / 潜伏 / 本机不编译

- J22 ⚠️ `macho_reloc.rs:328-331`：macOS exec buffer 从不 `make_rx` → 每次缓存命中崩溃；J23 ⚠️ 整个 Mach-O 重定位器类型不匹配、疑似不编译且逻辑错。**macOS `#[cfg]`，本机不构建**。
- J24 ⚠️ `coff_reloc.rs:260-271`/`elf_reloc.rs:186-197`：`ImageOffset` 当 `Absolute` 处理，当前无触发，潜伏。
- J25 ⚠️ SIMD 用裸 `fdiv`、标量钳位 → 聚簇与否结果不同；FMA 单次舍入 vs 标量两次 → tier 末位分歧。
- J26 ⚠️ `expr/compile.rs:244-297`：`ArrayComprehension`/`ArrayLiteral` 只算首元素。
- J27 ⚠️ `speculation.rs`：guard 实为 no-op，`NewtonDense/Sparse/ZeroCrossingNeverTriggers` 是假设但从不校验的不变量。
- J28 ⚠️ `jit_policy.rs:281-299`：`value_to_f64` 遇坏值 `?` 中止整个查找（潜伏）。
- J29 ⚠️ `builtin_policy_dispatch.rs:410-433`：`max(A)`/`min(A)` 数组归约未处理，数组参数直通。

---

## 结论与优先级

**默认路径必修 3 条**（静默算错、影响面大、修复局部）：
1. **C1/J1** 除法钳位 —— 改为符号保持的 guard 或去掉钳位。
2. **J2** SIMD 偏移 —— 全块循环 load/store 偏移从 `base` 换成 `(src1_start+chunk*vs)*8` 等、补 `src2_off/src3_off`，对齐 remainder 循环约定。
3. **C3** RK45 时间/状态失配 —— `step` 回传实际步长，driver 按其推进 `time`。
4. **C2** BLT 增广路径 —— 变量应绑父帧方程；**C4** for>100 分支补 `extend(temp_conn)`/`extend(temp_alg)`。

**架构性根因**：tiered/deopt/interpreter/缓存大量依赖进程级全局单例（`INTERPRETER_CTX`/`DEOPT_PENDING`）与不含模型标识/CPU 特性/变量布局的缓存 key —— 任何 tier/deopt/跨机复用都可能把代码绑到错的模型/布局/指令集（J4/J8/J9/J10）。

**门控项**在对应开关关闭时不触发，但一旦启用即为真实缺陷。

---

## 第三轮：对抗性复核判决（grill）

对 9 条关键结论派对抗性 refuter agent 主动反驳（默认倾向驳回/收窄，除非铁证），验证**可达性、上游 guard、调用方补偿、算法语义**。结果：**0 驳回，8 CONFIRMED，1 收窄为条件性**——无一被推翻，但触发面被厘清。

| 结论 | 判决 | 反驳后要点 |
|------|------|-----------|
| C1/J1 除法钳位 | ✅ CONFIRMED | 无条件、默认路径；`Div` 是原生 BinaryOp 不经 policy 拦截；物理模型除以带符号量极常见。**最硬** |
| J2 SIMD 偏移 | ✅ CONFIRMED | 驳回"指针已含偏移"辩护——`resolve_array_ptr` 返回共享基址 + 独立 `start_index`。触发：数组不在其 ArrayType 存储偏移 0（如 `y[1..4]` 排在 `x[1..4]` 后）——常见 |
| C3 RK45 失配 | ✅ CONFIRMED | 无条件（只要一步被拒）；live 路径 `rk45 && when_count==0`；普通步 + 事件步两处都中 |
| H5 xorshift | ✅ CONFIRMED | 对照 Vigna 权威实现坐实 64*/128+ 错、1024* 对（乘数常量正确、位置错） |
| H4 缓存失效 | ✅ CONFIRMED | 无兜底；遮蔽场景（`import P.*` 后加 `P/N.mo`）每阶段陈旧命中。`decl_expanded` 纯报错→可解析能自愈，`inheritance_flattened` 不能 |
| C2 BLT 增广 | ✅ CONFIRMED（收窄） | 干净 3→2 追踪证明双绑；但需长度≥2 增广路径 → 只咬**耦合/隐式代数**结构，贪心预匹配覆盖常见解形式 |
| H1 SCC 顺序 | ✅ CONFIRMED（收窄） | 只在 **≥2 个 SolvableBlock** 成生产者→消费者链时咬；全 `Simple` 被 `reorder_simple_variable_equations` 补偿 |
| C4 for>100 | ✅ CONFIRMED（明显收窄） | 需 count>100 + 体内 connect/when；且 connect 情形两分支都残缺（未替换 loop 变量），**只有 when/reinit 的 algorithm 丢失是干净真损失** |
| J4 AOT CPU 缓存 | ⚠️ NARROWED→条件性 | 机制真实但**非默认**：默认缓存根本机私有（`%LOCALAPPDATA%` 非漫游）。需显式共享/漫游缓存根 + 异构 CPU + 同 exe 三者齐备才 SIGILL |

### 经 grill 修订的优先级

- **真正无条件、默认触发的仅 3 条**：C1/J1（除法）、J2（SIMD，偏移≠0 即中）、C3（RK45 拒绝步）——必修头牌，grill 后毫发无损。
- **C2/H1** 是真 bug，但只咬耦合/隐式-块模型 → 优先级应**低于**头牌（前文表格摆得偏高）。
- **C4** 严重度**下调**：干净损失仅限 when/reinit 场景。
- **J4** 从 High **降级**为条件性部署问题（漫游 profile + 混合 CPU 机群），非默认缺陷。

---

## 第四轮：已应用的修复（提交记录）

本会话已修复并提交以下项，全部编译通过、142 单元测试无回归。

| commit | 修复项 | 说明 & 验证 |
|--------|--------|------------|
| `0ab7382` | **C1/J1** 除法钳位 | `fmax` → 保号钳位（`fabs`+`select`），仅钳制近零分母量级、保留符号。验证：`10/-2` 正确 |
| `0ab7382` | **J2** SIMD 偏移 | 全块循环 load/store 改用 per-array 字节偏移 `(start+elem)*8`。验证：数组非零偏移不再串址 |
| `0ab7382` | **C3** RK45 失配 | `step` 内部子步进精确覆盖整个 `dt`，状态/时间同步。验证：rk45 大/小步长末态与 rk4 一致 |
| `304a602` | **C4** for>100 丢 connect/algorithm | 符号化快捷路径仅用于纯方程 body；含 connect/when 回退到完整展开。验证：150 迭代 when 保留 |
| `0cf1c2f` | solvable-block 缺 time/t_end | 块内 `sub_var_map` 补 seed time/t_end（latent，修块编译路径） |
| `407d8c8` | **`time` 读成 params[0]**（reinit 根因①） | var_map["time"] 被 param 循环覆盖 → `time` 读 0 → `when time>0.5` 永假 → reinit 从不触发。修复：time/t_end 的 var_map 插入移到 state/param 循环之后。验证：`der(x)=time`→x(1)=0.5 |
| `0f73557` | **when-edge pre 被清零**（reinit 根因②） | `evaluate_scratch` 用 `buf_when.fill(0.0)` 清零边沿 pre → 永真 `when` 每 RK 阶段重触发、状态冻结。修复：拷贝真实 when_states 保留 pre。验证：`when time>0.5 reinit(x,5)`→x(1)≈3.05 |
| `8386907` | **C2** BLT 增广路径 | dfs_iter 回溯把变量绑到子帧而非父帧方程 → 长度≥2 增广路径下起始方程未匹配、误报 index-2。修复：帧携带 `(变量, 父方程)`。手工追踪确凿正确；端到端复现被 `eliminate_aliases` 掩盖 |

### reinit "失效" 的完整根因链（重点记录）

表象是 `when time>0.5 then reinit(...)` 不生效，实为**两个独立根因叠加**：
1. **`time` = 0**（`407d8c8`）—— analysis 把 `time` 误分类为参数，param 循环用 `load(params_ptr)` 覆盖了 var_map 里正确的 block-param 值。IR dump（`y=time` → `load(params_ptr)`）坐实。→ `when` 条件永假。
2. **when-edge pre 清零**（`0f73557`）—— 修好 time 后暴露：`evaluate_scratch` 清零 when 缓冲，使永真条件的 reinit 每步重触发、状态冻结。

reinit/when 机制**本身正确**（状态条件 `when x<0.5` 一直正常）。

### 仍开放（建议单独、带回归护栏处理）

- **H1 / 别名回代顺序** —— 残差 + 别名链模型（如 `a=b+1; b=c+1; 0=a-10`）求解顺序错（消费者块先于生产者块，`a=10` 未回代到 c）。修复需动 SCC 块发射顺序，风险高，须先建回归护栏。
- **H4 缓存失效** —— 新增 `.mo` 不失效依赖模型的持久 flatten 缓存（缺负依赖 + `libs_closure_hash` 只哈希路径）。架构性。
