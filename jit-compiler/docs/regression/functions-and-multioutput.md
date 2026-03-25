# 函数与多输出回归 / Functions And Multioutput Regression

## 范围 / Scope

覆盖函数链路与多输出赋值行为。  
Covers function pipeline and multi-output assignment behavior.

- 用户函数定义/调用/内联/递归路径 / user function definition/call/inline/recursive path
- 多输出函数赋值成功路径 / multi-output function assignment success path
- 多输出形状/类型不匹配失败路径 / multi-output shape/type mismatch fail path
- 嵌套与跨模块不匹配诊断 / nested and cross-module mismatch diagnostics

## 关键能力映射 / Key Capability Mapping

- 函数调用正确性 / function invocation correctness
- 多目标写入合法性检查 / multi-target write legality checks
- 形状/基数/类型保护规则 / shape/cardinality/type guardrails
- 跨模块与别名链不匹配识别 / cross-module and alias-chain mismatch detection

## 代表用例 / Representative Cases

### 成功基线 / Success Baseline

| Case | Expect | Risk Focus |
|---|---|---|
| `TestLib/SimpleFunctionDef` | pass | Function parse + call |
| `TestLib/FuncInline` | pass | Inline expansion |
| `TestLib/RecursiveFunc` | pass | Recursive function route |
| `TestLib/MultiOutputFunc` | pass | Multi-output baseline |
| `TestLib/MultiOutputNestedExpr` | pass | Nested expr output mapping |
| `TestLib/MultiOutputMixedArrayScalar` | pass | Mixed output target shape |
| `TestLib/MixedMultiTargetSafePass` | pass | Multi-target legal store |

### 预期失败基线 / Expected Failure Baseline

| Case | Expect | Risk Focus |
|---|---|---|
| `TestLib/MultiOutputShapeMismatch` | fail | Shape mismatch rejection |
| `TestLib/MultiOutputRecordShapeMismatch` | fail | Record output mismatch |
| `TestLib/MultiOutput2DArrayShapeMismatch` | fail | 2D shape mismatch |
| `TestLib/MultiOutputComprehensionShapeMismatch` | fail | Comprehension mismatch |
| `TestLib/MultiOutputRecordNestedArrayMismatch` | fail | Nested record/array mismatch |
| `TestLib/MultiOutputCrossLayerComprehensionMismatch` | fail | Cross-layer mismatch |
| `TestLib/MultiOutputComplexLhsFieldStore` | fail | Invalid complex LHS store |
| `TestLib/DeepRecordNestedMismatch` | fail | Deep nested type mismatch |
| `TestLib/MixedNestedLhsFieldStoreMismatch` | fail | Mixed nested invalid LHS |
| `TestLib/MixedMultiTargetFieldStoreFail` | fail | Multi-target invalid field store |
| `TestLib/CrossModuleComprehensionMismatch` | fail | Cross-module mismatch |
| `TestLib/CrossModuleRecordCompositeMismatch` | fail | Cross-module record mismatch |
| `TestLib/AliasChainTypeMismatch` | fail | Alias chain mismatch |

## 执行命令 / Execution Command

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File d:/source/repos/rustmodlica/run_regression.ps1
```

单个预期失败检查 / Single expected-fail check:

```powershell
cargo run -p rustmodlica --bin rustmodlica --release -- TestLib/MultiOutputShapeMismatch
```

## 判定标准 / Verdict Criteria

- 成功基线退出码为 `0` / success baseline exits with `0`
- 预期失败基线退出码为非 `0` / expected-fail baseline exits with non-zero
- 任意期望反转都记为回归不匹配 / any inversion of expected verdict is treated as regression mismatch

## 常见失败模式 / Common Failure Modes

- 多赋值参数个数不匹配 / arity mismatch in multi-assignment
- 嵌套目标非法字段写入 / illegal field store on nested targets
- 类型别名解链不完整 / incomplete type alias resolution
- 跨模块推导输出形状传播错误 / cross-module inferred output shape not propagated correctly
