# 方程/组件级依赖追踪与最小重编译粒度设计

## 1. 当前系统依赖追踪架构

### 1.1 依赖追踪层级

```
┌─────────────────────────────────────────────────────────────────┐
│                        文件级 (当前实现)                          │
│  DepHashEntry { path, content_hash }                            │
│  ReverseDepStore: file → Set<model_name>                        │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                        模型级 (当前实现)                          │
│  affected_models(changed_files) → Vec<model_name>               │
│  （进程内：依赖本次进程内 query/展平记录的反向索引，重启后为空）      │
│  Salsa: source_text → … → flattened_model_q（见 §5）              │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    参数依赖图 (ParamPassOptimizer)               │
│  param_deps: param → Set<param>                                 │
│  dependents_index: param → Vec<dependent_param>                 │
│  用于: 参数传播优化、增量参数求值                                  │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    方程-变量图 (EquationGraph)                   │
│  nodes: [equation, variable]                                    │
│  edges: [equation --depends--> variable, equation --solves--> ] │
│  当前用途: 可视化/调试 (未用于增量编译)                           │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 现有细粒度分析能力

**已实现**：
```rust
// src/analysis/variable_collection.rs
pub fn extract_unknowns(eq: &Equation, knowns: &HashSet<String>) -> Vec<String>
pub(crate) fn collect_vars_eq(eq: &Equation, vars: &mut HashSet<String>)

// flatten/decl_expand.rs - ParamPassOptimizer
struct ParamPassOptimizer {
    param_deps: HashMap<String, HashSet<String>>,      // param → 依赖的 params
    dependents_index: HashMap<String, Vec<String>>,   // param → 被谁依赖
    last_change_pass: HashMap<String, usize>,         // param → 上次变更的 pass
}

// src/equation_graph.rs（对外公开类型）
pub struct EquationGraph {
    pub nodes: Vec<EquationGraphNode>,
    pub edges: Vec<EquationGraphEdge>,
    pub truncated: bool,
    pub total_equations: usize,
    pub included_equations: usize,
    pub omitted_equations: usize,
}
```

**缺失或仅部分具备**：
1. 方程 → 源位置（哪个 `.mo` 文件的哪一行）：**未**在 IR 中系统化挂接
2. 组件实例 → 类型/定义：**部分具备** — `FlattenedModel.instances`（`full_path → type_name`）及 `inst_records` / `path_to_inst`（`InstPathRecord`：qualified_class、component_path）；缺与**单条方程**、**源文件行号**的绑定
3. 变量 → 声明位置/修改方程的溯源：**未**系统化
4. 变更传播的增量计算（用于安全省略编译步骤）：**未**实现；`affected_models` 仅缩小「待验证模型名」集合，不保证闭包完备

**相关但本文未展开的实现**（与「文件级依赖」重叠）：`DepClosureFingerprint` / 库 epoch、按 `CacheStage` 的阶段缓存等，见 `DEPENDENCY_EPOCH_DESIGN.md` 与 `cache/lib_epoch.rs`、`cache/ir_epoch.rs`。

---

## 2. 最小重编译粒度实现方案

### 2.1 方程级依赖溯源

**目标**：追踪每个方程的来源，支撑**影响分析**与（仅在满足 [2.4](#24-增量-jit-成立条件) 时）**增量 codegen** 的决策；**不**将「仿真运行时修改参数向量 `p`」与「必须重生成机器码」混为一谈。

```rust
/// 方程来源溯源信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationProvenance {
    /// 方程在展平后模型中的索引
    pub flat_eq_index: usize,
    /// 源文件路径
    pub source_file: PathBuf,
    /// 源文件行号（如果可确定）
    pub source_line: Option<u32>,
    /// 产生此方程的组件实例路径 (e.g., "resistor1.R")
    pub instance_path: Option<String>,
    /// 此方程依赖的参数/变量
    pub depends_on_vars: Vec<String>,
    /// 此方程求解的变量
    pub solves_vars: Vec<String>,
}

/// 组件实例到定义的映射
#[derive(Debug, Clone)]
pub struct ComponentInstanceMap {
    /// 实例路径 → 组件类型名
    pub instance_to_type: HashMap<String, String>,
    /// 实例路径 → 源文件
    pub instance_to_source: HashMap<String, PathBuf>,
    /// 类型名 → 定义文件
    pub type_to_source: HashMap<String, PathBuf>,
}
```

### 2.2 增量重编译流程

下列流程中，**第 4 步「仅重新生成部分方程的代码」仅当 [2.4](#24-增量-jit-成立条件) 中 **Phase 3（增量 codegen）** 条件成立时才适用。若仅为**运行时**更新 `p` 且残差/雅可比从内存读取参数，则**无需**第 4 步，仿真继续即可。

```
用户修改参数 R.resistance（须先区分：源码/编译期 vs 运行时 p）
        │
        ▼
┌───────────────────────────┐
│ 1. 查找参数定义位置          │
│    R.resistance → resistor1 │
│    → Resistor.resistance    │
└───────────────────────────┘
        │
        ▼
┌───────────────────────────┐
│ 2. 计算受影响变量闭包        │
│    resistance → current    │
│    → power (传递闭包)       │
└───────────────────────────┘
        │
        ▼
┌───────────────────────────┐
│ 3. 找出依赖这些变量的方程    │
│    eq_5 depends on current │
│    eq_7 depends on power   │
└───────────────────────────┘
        │
        ▼
┌───────────────────────────┐
│ 4. 增量重编译               │
│    - 不重新展平整个模型      │
│    - 仅重新生成 eq_5, eq_7  │
│    - 合并到现有 JIT 模块    │
└───────────────────────────┘
```

### 2.3 关键数据结构

```rust
/// 增量编译上下文
pub struct IncrementalCompileContext {
    /// 上次编译的完整展平模型
    previous_flat: Arc<FlattenedModel>,
    /// 方程来源索引
    equation_provenance: Vec<EquationProvenance>,
    /// 变量 → 依赖它的方程 (反向索引)
    var_to_equations: HashMap<String, Vec<usize>>,
    /// 参数 → 依赖它的变量 (传递闭包)
    param_to_dependent_vars: HashMap<String, HashSet<String>>,
    /// 组件实例图
    instance_graph: ComponentInstanceMap,
}

impl IncrementalCompileContext {
    /// 计算参数变更的影响范围
    pub fn compute_param_change_impact(
        &self,
        param_name: &str,
    ) -> ImpactResult {
        // 1. 找出依赖此参数的所有变量
        let affected_vars = self.param_to_dependent_vars
            .get(param_name)
            .cloned()
            .unwrap_or_default();

        // 2. 找出依赖这些变量的方程
        let mut affected_eqs: HashSet<usize> = HashSet::new();
        for var in &affected_vars {
            if let Some(eqs) = self.var_to_equations.get(var) {
                affected_eqs.extend(eqs);
            }
        }

        ImpactResult {
            affected_vars: affected_vars.into_iter().collect(),
            affected_equations: affected_eqs.into_iter().collect(),
            requires_full_recompile: false,
        }
    }

    /// 增量重编译受影响的方程
    pub fn incremental_recompile(
        &mut self,
        changed_params: &[String],
        jit: &mut Jit,
    ) -> Result<IncrementalCompileResult, String> {
        // 1. 计算影响范围
        let mut all_affected_eqs: HashSet<usize> = HashSet::new();
        for param in changed_params {
            let impact = self.compute_param_change_impact(param);
            all_affected_eqs.extend(impact.affected_equations);
        }

        // 2. 如果影响范围过大，回退到全量重编译
        let total_eqs = self.previous_flat.equations.len();
        if all_affected_eqs.len() * 3 > total_eqs {
            // 影响超过 1/3 的方程，不值得增量
            return Ok(IncrementalCompileResult::FallbackToFull);
        }

        // 3. 仅重新编译受影响的方程
        let mut new_equations = Vec::new();
        for &idx in &all_affected_eqs {
            // 重新求值/生成代码
        }

        Ok(IncrementalCompileResult::Partial {
            recompiled_count: all_affected_eqs.len(),
            total_count: total_eqs,
        })
    }
}
```

### 2.4 增量 JIT 成立条件

增量优化分两条**独立**轨道，不可混在同一套「20x」叙事里。

**轨道 A：减少展平 / 前端重跑（与是否重 codegen 无关）**

- 依赖：文件与库闭包指纹、阶段缓存（`CacheStage`）、Salsa query、`affected_models` 等既有机制。
- **成立条件**：源码或依赖闭包变化可定位到「需重算的流水线阶段」或「需重验证的根模型集合」。
- **收益度量**：单独统计 `flatten_wall_us`、`inline_wall_us`、解析/继承阶段耗时；报表中称为 **frontend_skip_ratio** 或 **stage_cache_hit**。

**轨道 B：Phase 3 增量 codegen（仅在有明确理由时）**

- **运行时参数 `p` 可变、且已编译进同一份残差/雅可比例程并从内存读 `p`**：**不**因此触发重编译；更新 `p` 即可。此时 **codegen 增量收益为 N/A（应为 0 次重编译）**。
- **考虑增量 codegen 的典型成立条件**（需同时满足「展平形状不变」或已接受全量展平后的新 `FlattenedModel`，且下列至少一条）：
  1. **编译期常量折叠 / 特化**：部分方程或分支因 `final`/静态求值被**固化进机器码**，源码或编译期常量变化导致**已生成 IR 语义变化**。
  2. **结构级变化**：方程数量、变量布局、索引降阶结果、`when`/`if equation` 结构等变化 → 通常**应回退全量 codegen**（或整模块替换），不宜用「几条方程热插拔」冒充安全增量。
  3. **明确划分的残差分块**：已实现稳定的方程区域边界与调用约定，且可证明区域外接口（共享数组、别名、连接器展开）不变。

**立项约束**

- 任何「按方程子集重生成 Cranelift / JIT」必须在设计评审中**勾选**轨道 B 条件，并默认 **保守回退全量 codegen**。
- 文档与基准中的倍数（如「相对全量」）必须标明是 **(A) 前端** 还是 **(B) codegen**，禁止合并为单一「增量重编译收益」。

---

## 3. 实现策略评估

### 3.1 收益分析（拆分度量，禁止混表）

以下比例均为**假设性示例**，用于说明**两类指标须分开填报**；正式决策前应用实测替换。

**表 3.1a — 轨道 A：仅减少展平 / 前端重跑**（阶段缓存命中、缩小验证范围、依赖闭包命中等）

| 场景 | 基准（全量前端） | 目标（增量前端） | 备注 |
|------|-----------------|-----------------|------|
| 单文件小改、依赖闭包未变 | 100% | 视阶段而定（可 ~0% 展平） | 度量：`flatten_wall_us`、各 `CacheStage` |
| 修改库中被继承基类 | 100% | 中间值 | 可与 `affected_models` 联动 |
| 修改继承/连接结构 | 100% | 常需全量展平 | 与 codegen 无关 |

**表 3.1b — 轨道 B：Phase 3 增量 codegen**（仅当 [2.4](#24-增量-jit-成立条件) 成立）

| 场景 | 基准（全量 codegen） | 目标（部分重生成） | 备注 |
|------|---------------------|-------------------|------|
| 运行时仅改 `p` | 不应作为基准 | **0**（不重编译） | 与表 3.1a 无倍数关系 |
| 编译期常量/特化导致 IR 变化 | 100% | 待测 | 需分块边界与正确性证明 |
| 结构变化 | 100% | 回退全量 | 一般不定义「部分 codegen」收益 |

**已废弃的合并表述**：旧版将「少展平」与「少 codegen」压在一张表内并给出 20x/10x 等倍数，易误导；以 3.1a / 3.1b 与实测为准。

**报表字段（实现侧）**：`CompilePerfReport` 中轨道 A 使用 `flatten_wall_us`、`inline_wall_us`（及毫秒派生字段）；轨道 B 使用 `codegen_wall_us` / `codegen_wall_ms`（与 `jit_ms` 同源测量窗口）。设置 `RUSTMODLICA_PERF_TRACE=1` 时额外打印 `tracks trackA_flatten_wall_us=... trackA_inline_wall_us=... trackB_codegen_wall_us=...`。

**消费端**：`regress-harness` 的 `jit-validate-perf` 在汇总 `report.json` 的 `stats.by_scenario.*.*` 中写入 `flatten_wall_us_*`、`inline_wall_us_*`、`codegen_wall_ms_*`、`codegen_wall_us_*`，并在控制台打印一行 `jit-validate-perf trackA: ... | trackB: ...`。`sparse_dense` bench 的 CSV/JSON 增加 `compile_codegen_wall_ms` 列。`modai-ide` 校验结果 `compile_trace` 含 trackA/trackB 行；可选 `options.paramChangeImpactProbe` + 响应 `provenance`（统计与 `paramChangeImpact`）。

### 3.2 实现复杂度

| 组件 | 代码量 | 风险 | 优先级 |
|------|--------|------|--------|
| EquationProvenance 追踪 | ~500 行 | 中 | P1 |
| 变量→方程反向索引 | ~200 行 | 低 | P1 |
| 参数依赖闭包计算 | ~300 行 | 中 | P1 |
| 增量 JIT 代码生成 | ~800 行 | 高 | P2 |
| 变更传播优化 | ~400 行 | 中 | P2 |

### 3.3 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 溯源信息不完整 | 误判影响范围 | 保守策略：不确定时回退全量 |
| 方程索引变化 | 缓存失效 | 使用稳定索引或内容哈希 |
| JIT 热更新 | 正确性问题 | 验证测试 + A/B 对比 |
| 复杂依赖环 | 影响范围爆炸 | 设置阈值，超过则全量 |

---

## 4. 推荐实现路径

### Phase 1: 基础溯源 (P1)
1. 在展平/内联**之后**由 `provenance_index_from_flat_model` 从 `FlattenedModel` 构建 `ProvenanceIndex`（含每条方程的溯源、反向索引、参数闭包）；主路径见 `compiler/pipeline/frontend.rs` 的 `frontend_stage_from_flat`。
2. `Compiler::last_provenance_index` 保存最近一次成功 `flatten_and_inline` 的索引（与当前 `flat_model` 一致）。

### Phase 2: 影响分析 API (P1)
```rust
// api.rs — 需已有 `ProvenanceIndex`（例如编译后 `compiler.last_provenance_index` 或 `provenance_index_for_flat_model`）
pub fn analyze_change_impact(
    provenance: &ProvenanceIndex,
    changed_params: &[String],
) -> ImpactAnalysisResult;
pub fn provenance_index_for_flat_model(flat: &FlattenedModel, root_source_file: Option<&str>) -> ProvenanceIndex;
```

### Phase 3: 增量 JIT (P2)
**前置**：满足 [2.4](#24-增量-jit-成立条件) 轨道 B；**独立度量**表 3.1b，不与表 3.1a 混报。

1. 支持部分函数替换（仅在残差分块边界稳定且接口不变的前提下）
2. 增量代码生成
3. 缓存增量结果

### Phase 4: IDE 集成 (P3)
1. 实时变更影响预览
2. 增量验证提示
3. 性能仪表盘

---

## 5. 与现有 Salsa 集成

当前 Salsa query DAG:
```
source_text → parsed_items → model_ast
    → inheritance_flattened → decl_expanded
    → eq_expanded → flattened_model_q
```

**增强方案（已实现 `provenance_index_q`）**：
```
source_text → parsed_items → model_ast
    → inheritance_flattened → decl_expanded
    → eq_expanded → flattened_model_q
                  ↘ provenance_index_q   // 已实现：依赖 flattened_model_q
```

**关键点**：
- `provenance_index_q` 的 flat 与 **Salsa 流水线** `flattened_model_q` 一致；**未经**与完整编译相同的 `inline_function_calls`，与 `Compiler` 路径上 `last_provenance_index`（后内联）可能不一致——IDE 若需与 JIT 输入一致，应使用编译器产物而非仅 Salsa query。
- 失效粒度仍为模型级；细粒度信息用于分析与提示，不单独保证可省略 codegen。

---

## 6. 结论

**当前系统**：
- 文件级和模型级依赖追踪已完善
- 方程-变量图已存在但未用于增量 codegen
- `ProvenanceIndex` 在编译主路径与 `provenance_index_q`（Salsa）中可用；公开 API：`analyze_change_impact`、`provenance_index_for_flat_model`

**实现最小重编译粒度的关键**：
1. ✅ 已有：变量收集、方程-变量图、`ProvenanceIndex`、影响分析 API、分轨 perf 字段
2. 📋 待评估：Phase 3 增量 JIT 代码生成的收益/复杂度比（须过 [§8](#8-phase-3-增量-codegen-门禁清单)）

**建议优先级**：
1. 继续 **分开测量** 表 3.1a / 3.1b；对「仅改 `p`」确认不触发 codegen
2. 根据表 3.1a 数据优化缓存与验证范围
3. 仅当 §8 门禁通过且表 3.1b 有实测收益时再实现增量 JIT

---

## 7. 文档与代码依存性（审查清单）

| 文档断言 | 代码位置 | 一致性 |
|---------|---------|--------|
| `DepHashEntry { path, content_hash }` | `flatten/flat_cache_v1.rs`：`path` 为 `String` | 一致（类型为 `String` 非 `PathBuf`） |
| `ReverseDepStore: file → models` | `query_db/mod.rs`：`ReverseDepStore`、`dep_record_file` | 一致 |
| `affected_models` | `query_db::affected_models`、`api::affected_models_for_changed_files` | 一致；须强调**仅本进程内**已填充 |
| Salsa 链到展平结果 | `query_db/mod.rs` `QueryDb`：`flattened_model_q` | §1.1 已改为与 §5 一致 |
| `ParamPassOptimizer` | `flatten/decl_expand.rs`，**非 `pub`** | 文档示例为内部结构，外部 crate 不可直接依赖 |
| `EquationGraph` 用于调试 | `equation_graph.rs`、`Compiler::get_equation_graph_from_source` | 一致；大图会 **truncated** |
| `QueryDb::provenance_index_q` | `query_db/mod.rs`、`query_db/provenance_q.rs` | 已实现；依赖 `flattened_model_q`（非后内联 flat） |
| `analyze_change_impact` | `api::analyze_change_impact` | 已实现 |
| `provenance_index_for_flat_model` | `api::provenance_index_for_flat_model` | 已实现 |
| `provenance_index_from_flat_model` | `analysis::provenance_index_from_flat_model` | 已实现 |
| `Compiler::last_provenance_index` | `compiler/mod.rs` | 已实现 |
| `ImpactAnalysisResult` | `analysis::provenance.rs` | 已实现（可 serde） |
| Phase 3 增量 JIT 分方程替换 | JIT/codegen 路径 | **未**实现；`IncrementalCompileContext` 仍为设计草稿 |

**结论**：§2 中数据结构已在 `analysis/provenance.rs` 落地；对外入口以 §7 表为准。新增 `pub` 符号时请同步更新本表。

---

## 8. Phase 3 增量 codegen 门禁清单

在启动 Phase 3（按方程子集重生成 Cranelift / 热替换）前，须在评审中**逐项勾选**：

1. [ ] 文档 [§2.4](#24-增量-jit-成立条件) 轨道 B 至少满足一条，且书面记录架构假设（共享残差向量、别名、连接器）。
2. [ ] 表 3.1b 已用 **`codegen_wall_us`** / `jit_ms` 填充实测；表 3.1a 与 3.1b **分开**汇报。
3. [ ] 已验证：仿真运行时仅更新参数向量 `p` 时 **codegen 调用次数为 0**（或等价：无 `jit.compile`）。
4. [ ] 残差分块边界与调用约定通过评审；默认策略仍为**保守全量 codegen**。

未全部勾选前：**不**实现方程级热插拔 codegen；仅推进轨道 A 与影响分析工具链。
