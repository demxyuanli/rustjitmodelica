# Modai Studio × Simulink 能力对照

日期：2026-07-21  
目的：以 Simulink（含 Library Browser / Model Editor / Simulation / Simscape 类比）为功能参照，补全 Modai Studio 的需求与分期，**不是**逐功能复刻 MATLAB/Simulink。  
语义底座：Modelica + `rustmodlica`（因果信号流 Blocks ≈ Simulink；非因果物理域 ≈ Simscape）。

## 1. 参照范围说明


| Simulink 产品族概念            | Studio 对应策略                                |
| ------------------------- | ------------------------------------------ |
| Simulink（信号/控制系统框图）       | 一等公民：Blocks 信号流建模                          |
| Simscape（物理网络）            | 一等公民（分期）：Electrical / Mechanical connector |
| Stateflow                 | 不做独立状态机产品；可用 when/if 子集远期探索                |
| Embedded Coder / HDL      | 非目标；远期或走 FMU/C emit（编译器已有能力另议）             |
| Simulink Test / Coverage  | 非目标（可用外部 regress-harness）                  |
| MATLAB 工作区 / App Designer | 非目标；Studio 自含项目与曲线                         |


原则：**交互形态学 Simulink，语义学 Modelica**。

### 1.1 相对 jit-compiler 的完整度结论（摘要）

详细评估与**“为何实现不了完整 Simulink”**论证见 [07-compiler-feasibility.md §1.1](./07-compiler-feasibility.md)。

| 问题 | 结论 |
|------|------|
| 能否完整实现 Simulink？ | **不能**——差在产品族（MATLAB/Stateflow/Coder/生态）、语义模型（因果采样时间体系 ≠ Modelica 方程）、运行时（无 MATLAB/MEX/在线调参）、以及编译器成熟度（全 MSL/fallback/仿真控制 API） |
| P0 MVP | **编译器侧可行**（catalog 需 FQN 门禁） |
| P1 Core | **UI 项可行**；取消/真进度 **Blocked 于引擎** |
| P2 Pro | **Partial**（嵌套有基础；查表/路由/SDI 产品化不足） |
| P3 Future | **Partial**（有 emit-fmu/c；无真暂停/完整 Coder） |

对照 Simulink 只为排**交互主路径**优先级，不是承诺兼容或替换 Simulink。

## 2. 能力域总览（分期）

优先级约定（与功能说明一致并扩展）：


| 阶段  | 代号          | 含义                  |
| --- | ----------- | ------------------- |
| P0  | MVP         | 最小闭环：搭模型 → 仿真 → 看曲线 |
| P1  | Studio-Core | 接近日常 Simulink 建模手感  |
| P2  | Studio-Pro  | 层级、诊断、数据与库管理        |
| P3  | Future      | 高级分析、代码/FMU、生态互联    |
| —   | Out         | 明确不做或极远期            |




## 3. 对照表



### 3.1 库与组件（Library Browser）


| Simulink 能力                        | Studio 能力                  | 阶段     | 备注        |
| ---------------------------------- | -------------------------- | ------ | --------- |
| Library Browser 树                  | 组件库面板（域/类别/搜索）             | P0     |           |
| 常用库（Sources/Sinks/Math/Continuous） | 内置 catalog 信号流子集           | P0     |           |
| Continuous / Discrete 分区           | Continuous 优先；Discrete 块分期 | P1     | 依赖编译器离散支持 |
| Lookup Tables                      | 一维/二维查表块                   | P2     |           |
| 用户自定义库                             | 用户 catalog 扩展包             | P2     |           |
| 库链接 / 锁定库模块                        | 库链接与更新提示                   | P3     |           |
| Simscape 基础库                       | Analog / Rotational 极小子集   | P0–P1  |           |
| 第三方 Blockset                       | 插件式 catalog                | Out/P3 |           |




### 3.2 模型编辑（Model Editor）


| Simulink 能力       | Studio 能力        | 阶段    | 备注                |
| ----------------- | ---------------- | ----- | ----------------- |
| 拖拽添加模块            | 拖拽添节点            | P0    |                   |
| 端口连线              | 端口连边 + 类型规则      | P0    |                   |
| 智能引导线 / 自动路由      | 折点编辑；自动正交路由      | P1    |                   |
| 对齐分布 / 网格吸附       | 网格吸附；对齐工具条       | P1    |                   |
| 复制粘贴 / 复制模块       | 剪贴板复制节点子图        | P1    |                   |
| 撤销重做              | 编辑栈              | P1    |                   |
| 注释 Annotation     | 画布文本注释节点         | P1    |                   |
| 模块旋转翻转            | rotation 90° 步进  | P1    |                   |
| 信号线命名 / 标签        | edge 信号名；显示标签    | P1    |                   |
| Goto / From       | 信号跳转对            | P2    | 生成器需特殊处理          |
| Mux / Demux / Bus | 向量/总线拼拆          | P2–P3 | Modelica 用数组/总线记录 |
| 区域选择框选            | marquee 多选       | P0    |                   |
| 查找模块              | 按名/类型搜索画布        | P1    |                   |
| 打印/导出框图           | 导出 PNG/SVG       | P2    |                   |
| 对比模型 Diff         | 项目 diff（diagram） | P3    |                   |




### 3.3 层级与封装（Subsystem / Mask）


| Simulink 能力                     | Studio 能力             | 阶段    | 备注                    |
| ------------------------------- | --------------------- | ----- | --------------------- |
| Subsystem 下钻                    | 子系统节点 + 子 diagram     | P2    | 生成 `model`/`block` 嵌套 |
| Model Reference                 | 引用外部 `.mo` 模型         | P2    |                       |
| Mask 参数对话框                      | 组件参数面板（已有简化版）         | P0    |                       |
| Mask 图标脚本                       | catalog icon / 简易 SVG | P1–P2 |                       |
| Variant Subsystem               | 变体组件                  | P3    | `redeclare`/条件组件      |
| Configurable Subsystem          | 类型可切换实例               | P2    | replaceable 子集        |
| Create Subsystem from selection | 选区封装                  | P2    |                       |




### 3.4 参数与模型资源（Model Explorer）


| Simulink 能力          | Studio 能力                   | 阶段  | 备注          |
| -------------------- | --------------------------- | --- | ----------- |
| Block Parameters 对话框 | 属性面板                        | P0  |             |
| Model Explorer 树     | 实例树（大纲）                     | P1  |             |
| 模型工作区变量              | 顶层 `parameter`/`constant` 表 | P1  |             |
| 数据字典                 | 项目级参数字典 JSON                | P2  |             |
| 回调 InitFcn/StartFcn  | 预仿真脚本钩子                     | P3  | 慎用；默认不做任意脚本 |




### 3.5 仿真配置（Configuration Parameters）


| Simulink 能力         | Studio 能力           | 阶段    | 备注             |
| ------------------- | ------------------- | ----- | -------------- |
| Stop time           | `stopTime`          | P0    |                |
| Solver 选择           | rk4/rk45/implicit 等 | P0    | 映射 rustmodlica |
| Fixed/Variable step | 固定步长优先；可变步长选项       | P1    |                |
| Rel/Abs tol         | tolerance           | P0–P1 |                |
| Max step / Min step | 高级求解选项面板            | P1    |                |
| Zero-crossing 控制    | 事件/过零选项             | P2    | 依赖编译器开关        |
| Algebraic loop 诊断   | 环路诊断消息              | P2    |                |
| 数据导入/导出配置           | 记录变量白名单             | P1    |                |




### 3.6 仿真控制（Simulation）


| Simulink 能力  | Studio 能力  | 阶段  | 备注          |
| ------------ | ---------- | --- | ----------- |
| Run          | 运行仿真       | P0  |             |
| Stop / 取消    | 取消仿真       | P1  |             |
| Pause        | 暂停（若引擎支持）  | P3  | 当前引擎多不支持真暂停 |
| Step Forward | 单步/断点式推进   | P3  |             |
| Fast Restart | 热重启同结构改参   | P2  | Studio P310 已落地 |
| 仿真进度条        | 进度事件       | P1  |             |
| 仿真出错定位到模块    | 错误映射到 node | P1  |             |




### 3.7 观测与可视化（Scopes / SDI）


| Simulink 能力               | Studio 能力    | 阶段  | 备注             |
| ------------------------- | ------------ | --- | -------------- |
| Scope                     | 结果曲线面板（多通道）  | P0  | 不强制画布内 Scope 块 |
| 画布内 Scope 块               | Sink 节点绑定曲线页 | P1  |                |
| Floating Scope / 点击看信号    | 选中边/端口快速绘图   | P1  |                |
| Simulation Data Inspector | 多 run 对比     | P2  |                |
| Display 块                 | 仿真结束显示终值     | P1  |                |
| XY Graph                  | 相平面图         | P2  |                |
| Dashboard（Knob/Gauge）     | 简易仪表盘绑定参数    | P3  |                |
| To Workspace              | 导出 CSV/JSON  | P1  |                |
| From Workspace            | 输入轨迹文件       | P2  |                |
| 信号记录勾选                    | 变量记录集        | P1  |                |




### 3.8 信号与采样（Sample Time / Logging）


| Simulink 能力     | Studio 能力   | 阶段  | 备注      |
| --------------- | ----------- | --- | ------- |
| 采样时间颜色          | 连续/离散着色图例   | P2  |         |
| Rate Transition | 显式速率转换块     | P3  | 已落地 RATE（ZOH/UD 映射） |
| 信号属性（单位、维度）     | port 元数据扩展  | P2  |         |
| 日志与 MAT 文件      | CSV/JSON 序列 | P1  | 不追求 MAT |




### 3.9 诊断与助手（Diagnostics / Advisor）


| Simulink 能力         | Studio 能力        | 阶段     | 备注       |
| ------------------- | ---------------- | ------ | -------- |
| 连接错误即时提示            | 连线规则 + reasonKey | P0–P1  |          |
| 更新图 Update Diagram  | validate / 编译检查  | P0     |          |
| Model Advisor 风格检查单 | 静态检查清单           | P2     |          |
| 性能分析 Profiler       | 仿真耗时分解           | P3     | 已落地 F89（阶段 wall-clock） |
| 线性化 / 频域工具          | Bode 等           | Out/P3 | 非 MVP 方向 |




### 3.10 物理建模（Simscape 类比）


| Simulink/Simscape 能力 | Studio 能力                | 阶段     | 备注    |
| -------------------- | ------------------------ | ------ | ----- |
| 物理连接线（无向）            | connector 边              | P0     |       |
| 域专用库                 | Electrical/Mechanical 子集 | P0–P2  |       |
| 求解器配置（局部）            | 全局求解器为主                  | P1     |       |
| 物理探头 / 传感器块          | 传感器组件入 catalog           | P1     |       |
| Multibody 3D         | 远期                       | P3     | Studio P311 视口已落地 |




### 3.11 代码生成与互联


| Simulink 能力         | Studio 能力    | 阶段  | 备注                       |
| ------------------- | ------------ | --- | ------------------------ |
| 生成 C/C++            | 可选调用编译器 emit | P3  | F90 Coder package via Export Suite |
| FMU 导出              | FMU 导出向导     | P3  | F90 CS/ME wizard + history      |
| FMI 协仿              | 导入 FMU 节点    | P3  |                          |
| External Mode / PIL | Out          | —   |                          |
| MATLAB Function 块   | 自定义算法块       | Out | 用 Modelica function/算法代替 |




### 3.12 工程与协作


| Simulink 能力 | Studio 能力    | 阶段  | 备注                                 |
| ----------- | ------------ | --- | ---------------------------------- |
| .slx 工程     | 目录项目 + 双文件   | P0  |                                    |
| 项目依赖管理      | libraryPaths | P1  |                                    |
| 示例 / Demo   | examples/    | P0  |                                    |
| 需求追溯        | Out          | —   | IDE 侧已有 traceability，不进 Studio MVP |
| 版本控制友好      | 文本 JSON + mo | P0  | 刻意优于二进制 slx                        |




## 4. 用户故事映射（Simulink 典型任务）


| #   | Simulink 用户怎么做                        | Studio 目标体验        | 阶段    |
| --- | ------------------------------------- | ------------------ | ----- |
| U1  | 打开 Library，拖 Source/Gain/Scope，连线，Run | 库 → 画布 → Run → 曲线页 | P0    |
| U2  | 双击模块改参数，再 Run                         | 属性面板改参，保存并仿真       | P0    |
| U3  | 配置 Solver / Stop time                 | 仿真配置条              | P0    |
| U4  | 用 Subsystem 整理大图                      | 子系统下钻              | P2    |
| U5  | Scope 里对比多次 run                       | Run 历史 + 叠加曲线      | P2    |
| U6  | Simscape 搭 RC，看电压                     | 电气子集示例             | P0–P1 |
| U7  | 导出数据到工作区画图                            | 导出 CSV，面板已绘        | P1    |
| U8  | 查找并替换一批 Gain                          | 查找 + 多选改参          | P1–P2 |
| U9  | 封装可复用库模块                              | 用户 catalog / 模型引用  | P2    |
| U10 | 生成代码部署                                | FMU/C（远期）          | P3    |




## 5. 对需求文档的增量约束

1. **对标声明**：产品对标的是 Simulink **建模-仿真-观测**主路径，不是 MATLAB 生态全家桶。
2. **语义边界**：信号流保持因果连接；物理域保持非因果 `connect`；禁止用信号线伪连接电气 pin（规则引擎强制）。
3. **Scope 策略**：P0 采用“全局结果面板”（类似一开即用的 Data 可视化），降低必须放置 Scope 块的门槛；P1 再补画布 Sink。
4. **层级策略**：无子系统也可完成 MVP；P2 前数据结构需预留 `parentId` / 子文档引用（见数据结构补丁）。
5. **不做清单固化**：Stateflow 产品化、Embedded Coder 级生成、HIL、MATLAB 工作区互操作 = Out。



## 6. 里程碑重排（相对原 M0–M4）


| 里程碑 | 对标 Simulink 能力包                  | 交付焦点                    |
| --- | -------------------------------- | ----------------------- |
| M0  | 规格                               | 本文档集评审                  |
| M1  | 工程骨架                             | 项目 I/O、空画布              |
| M2  | Library + Editor + Run + Scope面板 | 信号流闭环                   |
| M3  | Simscape-lite + 诊断               | 物理子集与连线规则               |
| M4  | Core 手感                          | 撤销、对齐、信号标签、取消仿真、CSV、实例树 |
| M5  | Pro 结构                           | 子系统、多 run、数据字典、模型引用     |
| M6  | Future                           | FMU/C、变体、仪表盘            |




## 7. 评审检查清单

- [ ] 同意“交互学 Simulink / 语义学 Modelica”
- [ ] 同意 P0 Scope = 全局曲线面板（非强制 Scope 块）
- [ ] 同意 Stateflow / Embedded Coder / HIL 为 Out
- [ ] 同意 M4=Core、M5=Pro 的分期
- [ ] Catalog 名单可按编译器支持度替换（保持数量级）