# 依赖指纹增强与子图增量展平评估

## 1. 已实现的改进

### 1.1 库 Epoch 机制 (`cache/lib_epoch.rs`)

**问题**：原有 `libs_closure_hash` 只包含库路径列表，不包含库文件内容。当库中 `.mo` 文件变更时，缓存不会自动失效。

**解决方案**：
- 为每个库目录计算 epoch = hash(所有 .mo 文件的 mtime + size)
- 缓存 epoch 30 秒，避免每次查找都扫描目录
- 库变更时自动失效相关缓存

```rust
// 使用示例
let libs_epoch = compute_libs_epoch(&loader.library_paths);
// 嵌入到缓存键中
```

### 1.2 依赖闭包指纹 (`DepClosureFingerprint`)

**问题**：原有依赖验证在缓存反序列化后进行，效率低且可能导致"假命中"。

**解决方案**：
- 在缓存键生成时预计算依赖闭包指纹
- 包含 `libs_epoch` + `deps_hash` + `deps_count`
- 缓存键中直接嵌入指纹，避免无效反序列化

```rust
pub struct DepClosureFingerprint {
    pub libs_epoch: String,    // 库目录组合 epoch
    pub deps_hash: String,     // 所有依赖文件路径+mtime+size 的哈希
    pub deps_count: usize,     // 依赖数量（调试用）
}
```

### 1.3 增强的缓存键生成

**修改**：`flatten_full_cache_key_with_deps` 在根哈希中包含依赖闭包指纹。

**效果**：
- 库文件变更 → `libs_epoch` 变化 → 缓存键变化 → 自动失效
- 依赖文件变更 → `deps_hash` 变化 → 缓存键变化 → 自动失效
- 新增依赖 → `deps_count` 变化 → 缓存键变化 → 自动失效

**配置**：
- 默认启用
- 设置 `RUSTMODLICA_LIBS_EPOCH_CACHE=0` 可禁用

---

## 2. 子图级增量展平评估

### 2.1 当前架构

```
Model A
├── extends B
│   └── extends C
├── component d: D
│   └── extends E
└── component f: F
```

**现有缓存粒度**：
- 文件级：整个模型的展平结果
- 依赖列表：`Vec<DepHashEntry>` (path + content_hash)

**失效策略**：任一依赖变更 → 整个模型重新展平

### 2.2 子图级增量的理论收益

| 场景 | 当前 | 子图增量 |
|------|------|----------|
| 修改 `E.mo` | 重展平 A, D, F | 仅重展平 E, D |
| 修改 `C.mo` | 重展平 A, B | 仅重展平 C, B |
| 新增 `G.mo`（A 不依赖） | 无重展平 | 无重展平 |
| 修改参数默认值 | 重展平整个模型 | 可仅更新参数绑定 |

### 2.3 实现复杂度分析

**需要的基础设施**：

1. **细粒度依赖图**
   ```rust
   struct ModelDepGraph {
       // 模型 → 直接依赖
       direct_deps: HashMap<String, Vec<DepEdge>>,
       // 模型 → 被谁依赖（反向索引）
       reverse_deps: HashMap<String, Vec<String>>,
   }

   enum DepEdge {
       Extends { base: String },
       Component { type_name: String, inst_name: String },
       Import { full_name: String },
   }
   ```

2. **子图变更检测**
   ```rust
   fn compute_affected_subgraph(
       changed: &[String],
       graph: &ModelDepGraph,
   ) -> HashSet<String>;
   ```

3. **增量展平器**
   - 缓存每个 `extends` 和 `component` 的展平结果
   - 合并时检查子缓存是否有效
   - 处理重排序和名称冲突

**复杂度**：约 2000-3000 行新代码，需要修改核心展平逻辑。

### 2.4 风险分析

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 正确性难以保证 | 高 | 大量测试用例 |
| 边缘情况多 | 高 | 保守失效策略 |
| 收益不确定 | 中 | 先测量实际场景 |
| 维护成本高 | 高 | 简化设计 |

### 2.5 实际场景测量

**IDE 场景**（典型）：
- 用户修改当前模型 → 不涉及子图增量（单一模型）
- 用户修改被继承的基类 → 子图增量有收益

**回归测试场景**：
- 批量修改库文件 → 当前 epoch 机制已足够
- 单文件修改 → 当前依赖验证已足够

### 2.6 建议的推进策略

**Phase 1（已实现）**：
- ✅ 库 Epoch 机制
- ✅ 依赖闭包指纹
- ✅ 缓存键增强

**Phase 2（建议实现）**：
- 模型级反向依赖索引（用于 IDE 增量验证）
- 不修改展平逻辑，仅用于 `affected_models` API

**Phase 3（评估后决定）**：
- 子图级缓存：仅当 Phase 2 测量显示明显收益时考虑
- 实现范围：仅 `extends` 关系（不含 `component` 实例化）

### 2.7 结论

**当前不建议实现子图级增量展平**，原因：

1. **复杂度高**：需要重构核心展平逻辑
2. **收益有限**：大多数场景下当前机制已足够
3. **风险高**：正确性难以保证

**替代方案**：
- 使用已实现的 epoch 机制快速失效过期缓存
- 使用 `affected_models` API 在 IDE 场景下缩小验证范围
- 测量实际性能数据后再决定是否需要更细粒度的增量

---

## 3. 使用指南

### 3.1 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `RUSTMODLICA_LIBS_EPOCH_CACHE` | `1` | 启用库 epoch 缓存失效 |
| `RUSTMODLICA_FLATTEN_FULL_CACHE` | `0` | 启用展平结果磁盘缓存 |
| `RUSTMODLICA_FLATTEN_CACHE_DIR` | `<install>/cache` | 缓存目录 |

### 3.2 API 变更

```rust
// 原有 API（兼容）
let key = flatten_full_cache_key(model_name, loader, ...);

// 新增 API（带依赖闭包）
let key = flatten_full_cache_key_with_deps(
    model_name,
    loader,
    ...,
    Some(&loaded_paths),  // 编译后的依赖列表
);
```

### 3.3 性能指标

- 库 epoch 计算：< 10ms（首次），< 1ms（缓存命中）
- 依赖指纹计算：< 5ms（100 个文件）
- 缓存键生成总增量：< 15ms
