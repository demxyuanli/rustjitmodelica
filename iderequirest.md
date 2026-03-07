# ModAI IDE 产品需求文档（详细设计版）

## 1. 产品概述

**产品名称**：ModAI IDE  
**产品描述**：ModAI IDE 是一个以 AI Coding 为核心导向的 Modelica 开发环境，集成自研的 Rust 开发的 Modelica JIT 编译器（rustmodlica）。它支持 Modelica 模型的编程、仿真运行和结果可视化，同时允许通过 AI Coding 实现编译器的功能补全和自迭代优化。  
**目标用户**：Modelica 工程师、仿真开发者、AI 辅助编程爱好者、编译器优化研究者。  
**核心卖点**：  
- AI Coding 驱动 Modelica 代码生成与优化。  
- 实时 JIT 编译验证，无需外部工具。  
- 当 JIT 覆盖不足时，AI 自动补全/迭代编译器本身。  
**版本规划**：MVP v1.0 聚焦两大功能方向，后续迭代添加高级特性如 FMI 导出、多用户协作。  

## 2. 需求背景与设计原则

- **重新设计动机**：聚焦 AI Coding 作为主线，简化用户工作流；集成 Tauri 框架，确保跨平台轻量级。  
- **设计原则**：  
  - **AI 导向**：所有复杂操作（如代码生成、优化）优先用 DeepSeek API 实现，减少手动干预。  
  - **轻量高效**：Tauri 框架下，安装包 <50MB，启动 <5s。  
  - **两大方向平衡**：Modelica 开发为主，自迭代为辅（触发于 JIT 失败场景）。  
  - **界面风格**：参考 Windsurf（干净、低密度、AI 面板主导）+ 圆润现代感（border-radius 8–12px）。  
  - **安全性**：沙箱执行 AI 生成代码，加密 API Key。  
- **技术约束**：无本地 AI 模型，统一用 DeepSeek API；Rust 后端处理 JIT/仿真，前端 React 处理 UI/可视化。  

## 3. 核心目标与KPI

| 优先级 | 目标描述                                      | KPI（MVP 验收标准）                                      |
|--------|-----------------------------------------------|-----------------------------------------------------------|
| P0     | 支持 Modelica 编程 + AI Coding 生成代码       | AI 生成代码通过率 ≥60%，手动插入顺畅                     |
| P0     | 实时 JIT 编译验证 + 仿真运行                  | 验证/仿真响应 <2s，成功率 ≥95%（基准模型）                |
| P0     | 结果可视化：曲线/表格交互显示                 | 支持 zoom/pan/导出，渲染延迟 <1s                          |
| P1     | 编译器功能补全：AI 生成补丁修复 JIT 不足      | 补全至少 2 个场景（e.g., 新语法支持），测试通过率 ≥50%    |
| P1     | 自迭代优化：AI 驱动编译器性能/功能迭代        | 完成 2–3 次迭代循环，性能提升 ≥5%（e.g., 仿真时间减少）  |
| P2     | 跨平台稳定 + 费用控制                         | 支持 Win/macOS/Linux，无崩溃；单次迭代 token <30k         |

## 4. 系统架构详细设计

### 4.1 整体架构图（文本表示）

```
[用户界面 (React + Monaco + Plotly)]
  │
  ├── 编辑区：Modelica 代码编辑 + AI inline 补全
  ├── AI 面板：自然语言输入 → 代码/补丁生成
  ├── 仿真面板：参数设置 + 运行按钮 + 可视化图表
  │
  ▼ (IPC 调用)
[Rust 后端 (Tauri Commands + rustmodlica)]
  │
  ├── JIT 模块：编译/仿真/沙箱测试
  ├── AI Wrapper：reqwest 调用 DeepSeek API
  │
  ▼ (HTTP)
[DeepSeek API]: 生成 Modelica 代码 / rustmodlica 补丁
```

- **前端组件**：React Hooks 管理状态，Monaco for 编辑，Plotly for 图表。  
- **后端模块**：Tauri Commands 如 `ai_code_gen(prompt: String) -> String`，集成 rustmodlica::Compiler。  
- **数据流**：用户输入 → 前端 → Rust Command → DeepSeek → 返回 JSON（代码/补丁） → 前端渲染。  

### 4.2 用户界面布局（详细）

- **主窗口**：无边框 + 圆润角（Tauri 配置），尺寸默认 1280x800。  
- **布局分区**（经典三栏 + 底部）：
  - **左侧（文件树，宽度 240px）**：项目浏览器、.mo 文件列表、依赖树（圆角卡片）。  
  - **中间（代码编辑区，占 60%）**：Monaco Editor，支持 inline AI 建议（浅蓝高亮框）。  
  - **右侧（AI Coding 面板，宽度 360px，可拖拽）**：聊天式输入/输出，生成代码片段 + “插入”按钮。  
  - **底部（仿真/日志面板，高度 200px，可展开）**：控制台日志 + 可视化图表（Plotly 嵌入）。  
- **主题**：Dark 默认（深灰 + 蓝高亮），Light 备选；字体：Monaco / Consolas。  
- **交互细节**：  
  - AI 建议：hover 时显示“采纳”/“修改” popover。  
  - JIT 按钮：工具栏醒目图标，点击后进度条实时反馈。  
  - 自迭代触发：JIT 失败时弹窗“使用 AI 补全？”。  

## 5. 功能详细需求

### 5.1 方向1: Modelica 开发

- **编程支持**：
  - 高亮：Modelica 关键字/运算符/变量（基于 Pest 规则）。  
  - 补全：静态（内置函数如 der/sin）+ AI（DeepSeek 生成）。  
  - 错误检查：集成 rustmodlica diag.rs，显示波浪线 + tooltip。  
- **JIT 验证**：
  - 输入：代码 + 参数（t_end/dt/atol 等）。  
  - 输出：编译 artifacts + 警告列表（WarningInfo）。  
  - 场景：保存后自动验证，或手动按钮。  
- **仿真运行**：
  - 参数面板：表单设置 CompilerOptions（solver/rkol等）。  
  - 执行：调用 run_simulation，返回时间序列数据。  
  - 错误处理：显示日志 + AI 建议修复（“可能初值问题，试试修改 start_value”）。  
- **结果可视化**：
  - 曲线：多变量 vs. 时间（zoom/pan/legend）。  
  - 表格：数据网格，支持排序/过滤/导出 CSV/JSON。  
  - 高级：热力图（多维数据）+ 动画回放（动态系统）。  

### 5.2 方向2: 编译器功能补全与自迭代

- **触发机制**：
  - JIT 失败（e.g., “语法不支持”）→ 自动建议“用 AI 补全？”。  
  - 手动：AI 面板输入目标（如“添加稀疏 Jacobian 支持”）。  
- **自迭代流程（详细步骤）**：
  1. 收集上下文：当前 Modelica 项目代码 + rustmodlica 相关文件。  
  2. DeepSeek 请求：系统提示 + 目标 → 输出 JSON（补丁/diff、测试模型、说明）。  
  3. 沙箱执行：Tauri 创建 temp dir → 应用 diff → cargo build/test。  
  4. 基准测试：运行预设 8–12 个模型，采集指标（时间/误差/内存）。  
  5. 迭代循环：结果回传 DeepSeek → 最多 5 轮修正。  
  6. 用户交互：inline diff 预览 + “采纳”按钮（git commit）。  
- **功能补全示例**：
  - 缺失 tearing：AI 生成 tearing_method 代码 + 测试 .mo。  
  - 优化：目标“加速 RK45” → 生成自适应步长补丁。  
- **历史管理**：SQLite 记录每次迭代（目标/diff/指标），前端列表查看/回滚。  

### 6. 非功能性需求（详细）

- **性能**：仿真 <2s 响应，AI 调用 <10s（超时重试）。  
- **安全性**：沙箱隔离（temp dir + no_std::env::set_var），API Key 加密（keyring crate）。  
- **费用**：token 估算（prompt 长度 * 1.2）+ 日上限（默认 50k）。  
- **可访问性**：支持键盘导航、屏幕阅读器（ARIA 标签）。  
- **国际化**：默认中文 + 英文切换（i18n.rs 集成）。  
- **日志**：Rust 端用 tracing，前端 console + Sentry 监控。  

## 7. 风险与缓解

| 风险                     | 缓解措施                                   |
|--------------------------|--------------------------------------------|
| AI 补丁 bug 高发         | 强制单元测试 + 人工预览前自动运行          |
| DeepSeek 延迟/费用超支   | 缓存常见提示 + 用户预算设置                |
| Tauri 跨平台兼容问题     | CI 测试三大系统 + 参考 Tauri 社区 issue    |
| Modelica 语法覆盖不足    | 优先基准模型，迭代中逐步补全               |

# ModAI IDE 实施方案（详细步骤）

## 1. 技术栈与环境准备

- **Tauri**：v2.0+，cargo install tauri-cli。  
- **前端**：React v18、Tailwind v3、Monaco Editor v0.45、Plotly.js v2。  
- **后端**：Rust 1.75+、rustmodlica crate（本地依赖）、reqwest（API 调用）、git2（diff 应用）、tempfile（沙箱）、sqlite（历史）。  
- **开发环境**：VS Code + Tauri 插件，目标平台 Win/macOS/Linux。  

## 2. 实施阶段分解

### 阶段1: Tauri 基础 + Modelica 开发（2–4 周）
1. 初始化：`tauri init modai-ide` → 配置窗口（标题“ModAI IDE”，无边框 + 圆角）。  
2. 前端布局：React App → 三栏 + 底部（useLayoutEffect 管理拖拽）。  
3. 编辑器集成：Monaco 配置 Modelica 语言（tokenizer from pest.rs）。  
4. JIT Command：Rust fn `jit_validate(code: String, options: CompilerOptions) -> Result<Artifacts, Error>`。  
5. 仿真 Command：fn `run_simulation(artifacts: Artifacts) -> Result<DataSeries, Error>`。  
6. 可视化：Plotly 组件接收 DataSeries JSON → 渲染曲线/表格。  

### 阶段2: AI Coding 集成（4–6 周）
1. API Wrapper：Rust fn `deepseek_call(prompt: String, api_key: String) -> Result<String, Error>`（reqwest post，JSON 输出）。  
2. 通用助手：前端聊天组件 → 发送 prompt（如“生成 Modelica 模型：{描述}”）→ 解析 JSON → inline 插入代码。  
3. 自迭代 Command：fn `self_iterate(target: String) -> Result<IterationResult, Error>`（上下文收集 + 多轮 API 调用 + 沙箱）。  
4. 沙箱实现：tempfile::TempDir → copy rustmodlica src → git apply diff → cargo build → run benchmarks。  
5. 基准模型：硬编码 8–12 个 .mo 文件（e.g., BouncingBall.mo），运行后比较指标。  

### 阶段3: UI 优化 + 测试（6–8 周）
1. 风格应用：Tailwind config（--radius: 10px, 主色 #3b82f6）。  
2. AI 面板：React Drawer 组件（右侧滑入），集成聊天 + diff viewer。  
3. 测试：单元（rustmodlica test）+ E2E（Playwright on Tauri）。  
4. 打包：`tauri build` → 生成 .exe/.dmg/.deb，测试跨平台。  

### 3. 关键代码骨架示例（Rust 后端）

```rust
// src-tauri/src/main.rs
use tauri::{command, Manager};
use rustmodlica::{Compiler, run_simulation};

#[command]
fn jit_validate(code: String, options: CompilerOptions) -> Result<Artifacts, String> {
    let mut compiler = Compiler::new();
    compiler.compile_model(&code, options).map_err(|e| e.to_string())
}

#[command]
async fn ai_code_gen(prompt: String, api_key: String) -> Result<String, String> {
    // reqwest 调用 DeepSeek，prompt = 系统模板 + 用户输入
    let response = reqwest::Client::new().post("https://api.deepseek.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({ "model": "deepseek-coder-v2", "messages": [{"role": "user", "content": prompt}] }))
        .send().await.map_err(|e| e.to_string())?;
    // 解析 JSON 输出
    Ok(response.text().await.map_err(|e| e.to_string())?)
}

#[command]
fn self_iterate(target: String, api_key: String) -> Result<IterationResult, String> {
    // 步骤：收集上下文 → DeepSeek 生成 → 沙箱测试 → 返回结果
    // ...
    Ok(IterationResult { diff: "...", metrics: Metrics { time_saved: 10.0 } })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![jit_validate, ai_code_gen, self_iterate])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 4. 资源估算

- **团队**：1–2 名 Rust 开发者 + 1 名前端（React）。  
- **预算**：DeepSeek API 测试费用 <100 USD/月。  
- **时间**：MVP 8–12 周，视基准模型复杂度调整。  

此方案详细、可落地，聚焦 AI Coding + Tauri。