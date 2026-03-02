## Scope

- **Goal**: Incrementally align `rustmodlica` with OpenModelica’s core compiler and simulation features.
- **Granularity**: Each subtask is designed so that an AI agent can complete it in **one code-editing session**（一次调用）并通过编译验证。

---

## 1. 语言与语法扩展（Language Frontend）

### 1.1 初始阶段（已有基础上扩展）

- **T1-1: 扩展 `noEvent` 支持更多位置**
  - **内容**：
    - 确认 `noEvent(expr)` 在 equation、algorithm 和 when 条件中均可编译。
    - 为 `noEvent` 增加 2–3 个覆盖不同上下文的 `TestLib` 模型。
  - **最小改动包**：
    - 更新/补充 `TestLib` 下的测试 `.mo`。
    - 若需要，对 `jit/translator/expr.rs` 做微调。

- **T1-2: `initial()` / `terminal()` 语义补强（终止事件 stub → 明确行为）**
  - **内容**：
    - 在仿真结束前后一小段时间内（例如 \([t_{\text{end}}-ε, t_{\text{end}}+ε]\)）让 `terminal()` 返回 1，其余时间为 0。
    - 增加 `TestLib/TerminalWhen.mo` 来测试 `when terminal() then ... end when;` 行为。
  - **最小改动包**：
    - 在 `simulation.rs` 中，将 `t_end` 传入 JIT（如通过 artifacts 或额外参数）或增加简单近似逻辑。
    - 在 `jit/translator/expr.rs` 中实现 `terminal()` 返回值逻辑。
    - 添加 `TestLib/TerminalWhen.mo` 并跑通。

### 1.2 下一阶段：函数与记录（Function & Record）

- **T1-3: 解析与 AST 支持 `function` 定义（仅语法 + AST，不展平/JIT）**
  - **内容**：
    - 在 `modelica.pest` 中增加 `function` 定义语法。
    - 在 `ast.rs` 中增加 `Function` AST 结构。
    - 在 `parser.rs` 中解析 `function ... end function` 到 AST。
  - **最小改动包**：
    - 更新 `modelica.pest`、`ast.rs`、`parser.rs`。
    - 添加 `TestLib/SimpleFunctionDef.mo` 测试仅解析成功（不调用）。

- **T1-4: 支持简单的 Modelica 函数调用（纯数值函数，无副作用）**
  - **内容**：
    - 允许在表达式中调用用户定义函数：`f(x)`，其中 `f` 是无递归、无 side-effect、仅基于参数和局部变量的函数。
    - 将函数当作“内联展开”：在前端/展平阶段做简单 inline。
  - **最小改动包**：
    - 在 flatten 或 analysis 阶段增加“函数调用展开”的简单逻辑（仅支持一层调用）。
    - 编写 `TestLib/FuncInline.mo` 验证。

---

## 2. 展平与结构分析（Flatten & Analysis）

### 2.1 展平优化与健壮性

- **T2-1: For 展平的性能与安全检查**
  - **内容**：
    - 为 `expand_equation_list` 中 `For` 逻辑增加单元模型，覆盖多种范围（小循环、大循环、边界=1）。
    - 对 `count > 100` 分支的行为增加注释和测试（例如 `TestLib/BigFor.mo`）。
  - **最小改动包**：
    - 仅添加/调整 `TestLib` 模型与少量日志，不改核心算法。

- **T2-2: `connect` 类型检查更严格的错误信息**
  - **内容**：
    - 在 `flatten/connections.rs` 中，对不兼容连接的 `Error: Incompatible connector types...` 补充源位置信息（模型名/变量名）。
  - **最小改动包**：
    - 修改 `resolve_connections` 打印内容。
    - 新增 `TestLib/BadConnect.mo`，用于触发该错误并检查消息。

### 2.2 结构分析与 Index Reduction 准备

- **T2-3: 将 `time_derivative` 在代码中“可见”（但先不启用）**
  - **内容**：
    - 在 `analysis.rs` 中为 `time_derivative` 写一个简单示例调用（例如在 debug 模式下对某约束方程打印其时间导数）。
    - 添加 `TestLib/ConstraintEq.mo`（约束方程）并用 `--backend-dae-info` 打印调试信息。
  - **最小改动包**：
    - 在 `sort_algebraic_equations` 中，在某个受控 flag（例如 `opts.index_reduction_method == "debugPrint"`）下调用 `time_derivative` 并打印结果。

---

## 3. 代数环求解与 Jacobian（Algebraic Loops & Jacobians）

### 3.1 SolvableBlock 强化（当前基础之上）

- **T3-1: 为 SolvableBlock 多 residual 情况添加专门测试与错误信息验证**
  - **内容**：
    - 调整或新增一个 `TestLib/SolvableBlockMultiRes.mo`，确保生成 `residuals.len() > 1` 的块。
    - 支持 1 或 2 个 residual；当 N 不为 1 或 2 时，JIT 报错信息为：  
      `SolvableBlock with N residuals is not supported (1 or 2 allowed)`
  - **最小改动包**：
    - 仅增加或调整 `.mo` 文件，不改代码逻辑。

- **T3-2: 为 Newton 迭代失败的情况增加更多诊断信息**
  - **内容**：
    - 在 `jit/translator/equation.rs` 中，对：
      - 迭代次数超限
      - Jacobian 过小（`|J| < 1e-12`）
    - 在返回 `status=2` 的基础上，通过 `eprintln!` 打印出 tearing 变量名、当前残差值等。
  - **最小改动包**：
    - 修改 `Equation::SolvableBlock` 的 JIT 生成逻辑，仅增加日志与更精确的错误码说明（比如统一仍为 2，但 log 中写清楚原因）。

### 3.2 Jacobian 使用与验证

- **T3-3: 增加符号 Jacobian 与数值 Jacobian 的一致性测试**
  - **内容**：
    - 对 `TestLib/JacobianTest.mo`：
      - 用符号 Jacobian 的表达式对某个状态点代入数值，计算矩阵 `J_sym(t0)`;
      - 和 `compute_ode_jacobian_numeric` 得到的 `J_num(t0)` 做差，打印最大元素差。
  - **最小改动包**：
    - 在 `simulation.rs` 或一个专门的调试入口里：
      - 若同时启用 `symbolic` 与 `numeric`，就执行一次上述对比并打印结果。

---

## 4. 求解器与仿真（Solvers & Simulation）

### 4.1 变步长 ODE Solver 雏形

- **T4-1: 实现一个单文件的简易 RK45 变步长求解器（仅 ODE，无事件）**
  - **内容**：
    - 在 `solver.rs` 中新增 `AdaptiveRK45Solver`：
      - 使用简单的 Runge–Kutta–Fehlberg 或 Dormand–Prince 公式。
      - 接口与现有 `Solver` trait 一致。
    - 仅在 `when_count == 0 && crossings_count == 0` 时启用。
  - **最小改动包**：
    - 修改 `solver.rs`（新增 struct + `impl Solver`）。
    - 修改 `simulation.rs` 中 solver 选择逻辑。

- **T4-2: 为 AdaptiveRK45 增加测试模型**
  - **内容**：
    - 增加 `TestLib/AdaptiveRKTest.mo`（如 `der(x)=-x`），运行时打印步数或简单统计信息（通过日志）。
  - **最小改动包**：
    - 仅添加 `.mo` 文件和必要的打印，不改核心算法。

---

## 5. 标准库与回归测试（Stdlib & Regression）

### 5.1 最小回归集建设

- **T5-1: 建立一个小型“通过/失败”回归列表**
  - **内容**：
    - 在仓库增加一个纯文本列表（如 `REGRESSION_CASES.txt`，需你手动创建）列出：
      - 一批 `TestLib`、`StandardLib`、`IBPSA` 模型名称；
      - 当前状态（pass/fail）、备注（失败原因简要）。
  - **最小改动包**：
    - 不改代码，只新增/更新列表文件。

- **T5-2: 为每个新功能（初始化、noEvent、Jacobians 等）都挂一个对应回归模型**
  - **内容**：
    - 确保：
      - 初始化：`InitDummy`, `InitWithParam`, `InitAlg`, `InitWhen`；
      - Jacobian：`JacobianTest`；
      - 代数环：`AlgebraicLoop2Eq` / 其它；
      - noEvent：`NoEventTest`；
    - 这些名字都写入回归列表。
  - **最小改动包**：
    - 更新 `REGRESSION_CASES.txt`，不动现有 `.mo` 与代码。

---

## 6. 约定与实施建议

- **每个任务包的实施顺序**：
  - 先选定一个任务 ID（例如 `T3-2`）。
  - 修改对应少数几个 `.rs` / `.mo` 文件。
  - 必须在本地执行 `cargo build --release` 确认无编译错误。
  - 使用新或已有 `.mo` 模型跑一遍，确认行为符合需求。

- **优先级建议（短期）**：
  - **高优先级**：`T3-1`, `T3-2`, `T3-3`, `T4-1`, `T4-2`
  - **中优先级**：`T2-1`, `T2-2`
  - **后续阶段**：`T1-3`, `T1-4`, `T2-3`, `T5-1`, `T5-2`

