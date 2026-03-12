# RustModlica

RustModlica 是一个**一体运行**的 Modelica 系统：ModAI IDE 与 JIT 编译器（rustmodlica）在运行时协同工作、**相互促进**。ModAI 负责编辑与仿真体验，JIT 负责编译与执行；JIT 的能力边界通过 ModAI 内的自迭代持续扩展，扩展后的 JIT 又反哺 ModAI 的建模与仿真能力。二者不是两个独立产品，而是同一系统中相互依赖、相互增强的两翼。支持 JIT/AOT 编译、FMI 2.0 导出以及 AI 辅助开发（含编译器自迭代）。

---

## 系统目标

系统目标是提供 Modelica 开发与仿真能力，且 **ModAI 与 JIT 在运行时协同、相互促进**：JIT 支撑 ModAI 的验证与仿真，ModAI 在 JIT 遇限时触发自迭代以增强 JIT，形成闭环。

### 编译器（jit-compiler）

实现 Modelica 核心子集：解析、展平、BLT（块下三角）排序、JIT（默认）与可选 AOT 编译及仿真。与 OpenModelica 核心编译及仿真特性对齐（详见 `OPENMODELICA_VS_RUSTMODLICA.md`）。支持 FMI 2.0 CS/ME 导出（详见 `FMI_README.md`）。该引擎被 ModAI 在运行时调用，其能力边界通过 ModAI 触发的自迭代扩展。

### IDE（modai-ide）

提供以 AI Coding 为核心的 Modelica 开发环境：编辑、JIT 验证、仿真与结果可视化。依赖 JIT 完成验证与仿真；当 JIT 遇限时，在 ModAI 内触发自迭代（沙箱构建/测试、采纳或提交补丁），使 JIT 升级后反哺本 IDE。

---

## 设计思想

### ModAI 与 JIT 自迭代的运行时关系

- 运行时 **ModAI 调用 JIT** 做验证与仿真；用户在 ModAI 中编辑与运行模型，能力由 JIT 提供。
- **JIT 遇限**（如语法不支持）时，由 **ModAI 触发自迭代**：目标描述 → AI 生成补丁 → 沙箱构建/测试 → 用户采纳；升级的是 jit-compiler 代码库。
- 升级后的 JIT 被同一 ModAI 环境使用（或重启后加载），ModAI 无需改版即可获得更强的验证/仿真能力。
- 因此：**ModAI 驱动 JIT 进化，JIT 进化反哺 ModAI**，二者相互促进、非独立存在。自迭代不是在 ModAI 之外单独运行的工具，而是 ModAI 内针对 JIT 的闭环能力扩展。

### 其他设计原则

- **技术选型**：纯 Rust 编译器；JIT 基于 Cranelift，无需外部 C 编译器。IDE 采用 Tauri 2 + React/TypeScript，实现轻量级跨平台桌面应用。
- **AI 优先**：复杂任务（代码生成、编译器补丁）通过 DeepSeek API 完成；自迭代在 ModAI 内触发，在沙箱中应用 diff、执行构建与回归测试，由用户决定采纳或提交。
- **JIT 编译器自迭代优势**：通过上述运行时关系，编译器能够随使用场景自主扩展（新语法、优化、求解器），无需开发者手动编写每行代码，形成“编译器自我进化”的闭环。
- **Rust 开发的编译器核心优势**（与传统 C/C++ 编译器对比）：
  - **内存安全与零崩溃**：Rust 的所有权系统在编译期消除空指针、数据竞争与内存泄漏风险。
  - **高性能与零开销抽象**：性能接近或超越传统 C++ 编译器，编译与仿真速度更快、资源占用更低。
  - **纯 Rust JIT（Cranelift）**：无需外部 C 编译器或运行时依赖，单二进制跨平台部署。
  - **与 AI 自迭代深度融合**：强类型与 Cargo 生态使 AI 生成的补丁可自动编译、测试与合并，并在 ModAI 内完成采纳与反哺。
  - **可维护性与安全性**：现代工具链支持热重载、并行编译，长期维护成本低于传统大型 C++ 代码库。
- **范围界定**：与 OpenModelica 核心子集严格对齐，不追求与 OMC 全量特性一致。使用固定 MSL 子集（详见 `MSL_SUBSET.md`）。
- **安全与可维护性**：AI 生成补丁在沙箱中执行；API Key 加密存储；大文件拆分规范（800 行阈值）详见 `OPTIMIZATION_PLAN_CN.md`。

### 整体数据流

1. 用户在 **ModAI IDE** 中编辑 Modelica 模型。
2. **ModAI 调用 JIT**（Rust 后端）进行验证与仿真。
3. **验证通过**：JIT 完成仿真，结果在 ModAI 中展示。
4. **若 JIT 遇限**：ModAI 提示并触发自迭代（DeepSeek 生成补丁 → 沙箱构建/测试 → 用户采纳）→ JIT 升级 → 回到步骤 2，ModAI 继续用升级后的 JIT 验证/仿真。

ModAI 依赖 JIT 提供能力，JIT 的不足通过 ModAI 触发的自迭代补全，升级后的 JIT 再服务 ModAI，形成**相互促进的闭环**。

---

## 与其他 Modelica 编译器的比较

RustModlica 定位为轻量级、AI 增强型、纯 Rust 实现的 Modelica 工具链，以 **ModAI + JIT 一体、自迭代相互促进** 为差异化特点，与主流开源及商业工具在以下关键维度存在显著差异：

| 维度                  | RustModlica                          | OpenModelica (开源)                  | Dymola (商业)                        | SimulationX / Wolfram SystemModeler 等 |
|-----------------------|--------------------------------------|--------------------------------------|--------------------------------------|----------------------------------------|
| **实现语言**          | 纯 Rust + Cranelift JIT              | C/C++                                | C/C++                                | C/C++ 或混合                           |
| **内存与运行时安全**  | 原生内存安全，零崩溃风险             | 存在内存泄漏/崩溃风险                | 存在内存泄漏/崩溃风险                | 存在内存泄漏/崩溃风险                  |
| **部署方式**          | 单二进制，无外部依赖，极简跨平台     | 需要完整安装环境，依赖较多           | 需要完整安装环境                     | 需要完整安装环境                       |
| **JIT 支持**          | 原生进程内 Cranelift JIT（默认）     | 解释执行 + C 代码生成                | 优化 C 代码生成                      | 优化代码生成                           |
| **AOT 支持**          | 可选系统链接器 AOT                   | 支持 C 代码生成与外部编译            | 强优化 C 代码生成                    | 强优化代码生成                         |
| **AI 自迭代能力**     | 原生支持（DeepSeek + 沙箱闭环）      | 无                                   | 无                                   | 无                                     |
| **IDE 集成**          | 内置 ModAI IDE（Tauri + AI 面板）    | OMEdit（功能完备但传统）             | 完整图形化 IDE                       | 完整图形化 IDE                         |
| **FMI 2.0 支持**      | CS/ME 完整导出                       | 完整支持                             | 完整支持（含 FMI 3.0）               | 完整支持                               |
| **语言覆盖**          | 核心子集（逐步对齐 OpenModelica）    | 接近完整 Modelica 规范               | 完整 Modelica 规范                   | 完整或接近完整                         |
| **主要适用场景**      | AI 辅助快速原型、研究、教育、轻量部署 | 学术研究、开源项目、工业验证         | 汽车/航空高端工业应用                | 多领域工业仿真、特定行业优化           |

RustModlica 在**安全性**、**部署简易性**与 **ModAI 与 JIT 相互促进的持续进化** 上具备明显差异化优势；在全语言覆盖与工业级优化深度上，仍处于追赶主流工具的阶段。

---

## 主要功能

### 运行时的一体协同

在用户视角下，ModAI 与 JIT 作为**一个系统、两翼协同**运作：编辑在 ModAI、验证与仿真由 JIT 执行；遇限时在 ModAI 内发起自迭代并采纳以升级 JIT，升级后继续在 ModAI 中使用 JIT。二者相互促进：ModAI 为 JIT 提供真实使用场景与触发条件（遇限即补全），JIT 能力增强后 ModAI 能验证与仿真的模型更多、体验更好。

### JIT 编译器（rustmodlica / jit-compiler）

作为**引擎**提供的能力。该引擎被 ModAI 在运行时调用，其能力边界通过 ModAI 触发的自迭代扩展。

- **前端**：基于 Pest 的语法解析器、AST、加载器及展平模块（继承、实例化、connect、数组、for/when）。流水线详见 `OPTIMIZATION_PLAN_CN.md`。
- **后端与求解器**：变量分类、导数规范化、BLT 排序、别名消除、tearing（1–32 个残差）、可选 index reduction（dummyDerivative）。默认 Cranelift JIT；可选 AOT（.o + 系统链接器）。求解器：RK4（含事件）、RK45、隐式（如 BackwardEuler）。
- **运行与导出**：`--result-file` CSV、`--repl`、`--script`、`--emit-c`、`--emit-fmu`、`--emit-fmu-me`（FMI 2.0 CS/ME）。详见 `FMI_README.md`。
- **MSL 支持**：固定子集（Constants、SIunits、Blocks 子集、内置 Math）。详见 `MSL_SUBSET.md`。

**CLI 功能概览**：解析器（Modelica 子集）、解释器（Rust 内求值）、JIT 编译器（默认，纯 Rust）、AOT 编译器（可选，.o + cl.exe/gcc）、内置数学函数（sin、cos、tan、sqrt、exp、log 等）。

### ModAI IDE（modai-ide）

作为**工作台**提供的能力。依赖 JIT 完成验证与仿真；JIT 遇限时通过本 IDE 完成自迭代，使 JIT 升级后反哺本 IDE。

- **编辑与导航**：Monaco 编辑器、文件树、大纲、代码索引/符号、可选 Git 集成。
- **验证与仿真**：JIT 验证（CompilerOptions）、仿真运行、结果可视化（Plotly 曲线与表格、时间线）。
- **AI 功能**：自然语言生成 Modelica 代码、AI 面板；JIT 遇限时提示“使用 AI 补全”并触发自迭代：描述目标 → 生成/粘贴 diff → 沙箱（check、build/test、mo 回归）→ 采纳或提交。详见 `modai-ide/JIT_SELF_ITERATE_FULL_TEST_CASE.md`。
- **双工作区**：Modelica 项目开发与 JIT 编译器自迭代（JitIdeWorkspace）可切换。

---

## 预期目标与路线图

- **当前状态**：核心编译/仿真、JIT/AOT、FMI 2.0 CS/ME 导出、IDE 基础功能及自迭代流程已实现，与 `OPENMODELICA_FULL_ALIGNMENT_TASKS` 核心子集基本对齐。
- **MVP / 近期**（详见 `iderequirest.md`）：P0 编辑 + AI 生成、JIT 验证 + 仿真、可视化；P1 编译器功能补全、2–3 轮自迭代与性能提升（在 ModAI 内完成，与 JIT 能力扩展形成闭环）；P2 跨平台稳定与 token 预算控制。
- **后续方向**：扩展 MSL/语法覆盖、FMI 导入、更多求解器与运行时选项（见 `FULL_MODELICA_SPEC_TASKS`、`ALIGNMENT_TASK_GAP`）。

---

## 项目结构

- **Workspace**（根目录 `Cargo.toml`）：成员 `jit-compiler`（rustmodlica）、`modai-ide/src-tauri`（Tauri 后端，依赖 jit-compiler）。运行时 ModAI 调用 JIT，自迭代则修改 jit-compiler 源码并重新构建，二者在运行时相互促进。
- **jit-compiler/**：编译器库与 CLI（parser、ast、flatten、analysis、jit、solver、simulation、fmi、script、api 等）。可单独构建与测试，亦供 ModAI IDE 在运行时调用。
- **modai-ide/**：Tauri + React 应用。前端 `modai-ide/src`，Tauri 后端 `modai-ide/src-tauri`。

---

## 构建与运行

**环境要求**：Rust 2021 edition。modai-ide 需 Node.js 与 npm。仅 AOT 或 FMU 打包时需要系统 C 编译器。

### 仅编译器（jit-compiler）

从仓库根目录：

```bash
cargo build --release -p rustmodlica
cargo run -p rustmodlica -- complex.mo
```

从 `jit-compiler/` 目录：

```bash
cargo build --release
cargo test --release
cargo run -- complex.mo
```

- **JIT（默认）**：进程内运行，无需外部工具。
- **AOT**：`cargo run -- complex.mo obj`，生成 .o 并链接（需 cl.exe 或 gcc）。
- **FMI 导出**：`--emit-fmu=<dir>`（CS）、`--emit-fmu-me=<dir>`（ME）。详见 `FMI_README.md`。

### 示例模型（complex.mo）

```modelica
model ComplexMath
  Real x;
  Real y;
  Real z;
  Real w;
equation
  x = 3.1415926;
  y = sin(x / 2.0);
  z = y * cos(x) + sqrt(4.0);
  w = exp(1.0);
end ComplexMath;
```

### ModAI IDE

```bash
cd modai-ide
npm install
npm run tauri dev
```

发布构建：`npm run tauri build`。

---

## 文档索引

| 分类           | 文档 |
|----------------|------|
| 编译器/规范     | `FMI_README.md`、`MSL_SUBSET.md`、`OPENMODELICA_VS_RUSTMODLICA.md`、`OPENMODELICA_FULL_ALIGNMENT_TASKS.md`、`FULL_MODELICA_SPEC_TASKS.md` |
| IDE/产品       | `iderequirest.md`、`modai-ide/JIT_SELF_ITERATE_FULL_TEST_CASE.md` |
| 优化/实现      | `OPTIMIZATION_PLAN_CN.md`、`OPTIMIZATION_PLAN.md`、`EXTERNAL_FUNCTION_ABI.md` |

更多见根目录：`OPENMODELICA_*.md`、`MSL_VERSION.md`、`ALIGNMENT_TASK_GAP.md`、`TestLib_COMPILER_ISSUES.md`。
