# 展平连接与OOP回归 / Flatten Connect OOP Regression

## 范围 / Scope

覆盖展平、连接语义、继承与包加载等结构性能力。  
Covers flattening, connect semantics, inheritance, package loading, and OOP-style composition.

- `extends` 与嵌套组件展开 / `extends` and nested component expansion
- connector 兼容性与连接图处理 / connector compatibility and connection graph handling
- package/type alias 解析 / package/type alias resolution
- 类层级与模型组合 / class hierarchy and model composition

## 关键能力映射 / Key Capability Mapping

- 分层模型展平管线稳定性 / flatten pipeline stability for hierarchical models
- connect 类型检查（含预期失败路径） / connect type checking (including expected fail path)
- 跨文件与包命名空间解析 / cross-file and package namespace resolution
- 结构化模型组合正确性 / structural model composition correctness

## 代表用例 / Representative Cases

| Case | Expect | Risk Focus |
|---|---|---|
| `TestLib/HierarchicalMod` | pass | Hierarchy flatten |
| `TestLib/NestedConnect` | pass | Nested connector links |
| `TestLib/LoopConnect` | pass | Connection loop handling |
| `TestLib/ArrayConnect` | pass | Array connector expansion |
| `TestLib/Circuit` | pass | Circuit-level composition |
| `TestLib/Sub` | pass | Submodel load path |
| `TestLib/Parent` | pass | Parent-child resolve |
| `TestLib/Child` | pass | Child reference resolve |
| `TestLib/ChildWithMod` | pass | Modifier propagation |
| `TestLib/TypeAliasTest` | pass | Type alias chain |
| `TestLib/ReplaceableTest` | pass | Replaceable handling |
| `TestLib/PkgA.PkgB.Inner` | pass | Package namespace path |
| `TestLib/BadConnect` | fail | Connect type mismatch rejection |
| `TestLib/UnknownTypeError` | fail | Unknown type rejection |
| `TestLib/BadSyntax` | fail | Syntax error rejection |

## 执行命令 / Execution Command

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File d:/source/repos/rustmodlica/run_regression.ps1
```

单用例语法/connect 失败检查 / Single-case syntax/connect error checks:

```powershell
cargo run -p rustmodlica --bin rustmodlica --release -- TestLib/BadConnect
cargo run -p rustmodlica --bin rustmodlica --release -- TestLib/BadSyntax
```

## 判定标准 / Verdict Criteria

- 结构类 `pass` 用例退出码为 `0` / structural pass cases exit code `0`
- 预期 parse/type/connect 失败用例退出码为非 `0` / expected parse/type/connect fail cases exit non-zero
- `pass` 集合中不应出现意外类/包解析错误 / no unexpected class/package resolution error in pass set

## 常见失败模式 / Common Failure Modes

- 嵌套组合场景展平不完整 / incomplete flatten expansion in nested composition
- connector 类型检查误报或漏报 / connector type mismatch false positives/negatives
- package 解析回退路径失败 / package resolution fallback failure
- modifier 应用顺序不一致 / modifier application order inconsistencies
