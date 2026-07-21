# Ideal Switch Simulation Guide

本文说明 RustModlica 对 Modelica **理想开关 / 理想半导体**（IdealDiode、IdealTransistor、PWM 斩波等）的仿真行为、常见失败原因、求解器选择与已有恢复机制。

相关实现主要在：

- `jit-compiler/src/newton_policy.rs` — 开关互补变量名识别
- `jit-compiler/src/simulation/newton_recovery/` — warm-start / homotopy / t0 / mode-seed
- `jit-compiler/src/simulation/driver/` — 主仿真循环（`run` / `crossing` / `sched` / `sundials_dispatch`）
- `jit-compiler/src/simulation/switch_drive/` — solver-step / trial-eval 恢复（`step` / `trial`）
- `jit-compiler/src/simulation/events.rs` — 事件迭代 force-`s=0` / fail-open
- `jit-compiler/src/analysis/blt/` — 禁止对 `time`/`*_start`/PWM start 做 Newton tearing
- `jit-compiler/src/jit/translator/equation/solvable_tearing.rs` — Newton 初值与失败防污染

---

## 1. Modelica 里的“理想开关”是什么

MSL 中理想二极管 / 晶体管 / 理想开关用**互补变量** `s` 刻画通断边界，而不是光滑电阻：

| 区域 | 典型含义（IdealDiode） |
|------|------------------------|
| `s < 0` | 关断侧（电压由 `s` 相关关系给出） |
| `s > 0` | 导通侧（电流由 `s` 相关关系给出） |
| `s = 0` | 通断边界（数值上最稳妥的初值） |

展平后变量名常见形态：

- `..._diode_..._s` / `..._diode.s`
- `..._transistor_..._s`
- `..._thyristor_..._s` / GTO / IdealOpeningSwitch 等

PowerConverters 斩波、桥式电路还会叠加 **PWM**（锯齿/三角载波与 duty），在边沿处离散模式切换，连续代数环的 Newton 初值很容易失效。

---

## 2. 为什么仿真会挂在 Newton（status 2）

典型日志：

```text
[newton-diag] phase=solver-step|event-step|event-iteration
diag_residual=1.0  diag_x~1e8
Tearing variable(s): ... diode_*_s, transistor_*_s, pwm_* ...
Solver step failed with status 2
```

根因归纳：

1. **显式 RK4 阶段跨开关**  
   一步内多个中间时刻可能已越过 PWM/二极管通断，而离散状态仍按步前模式，代数环残差无解或 Jacobian 病态。

2. **事件检测滞后**  
   过零检测通常在“成功走出一步”之后才做。若积分步本身 Newton 失败，永远到不了过零定位。

3. **warm-start 中毒**  
   失败 Newton 若把 `|s|~1e8` 写回 `outputs`，下一步仍从爆炸点起步，形成连锁失败。

4. **trial-eval 污染**  
   试探步在 `t_trial` 写入后交叉点代数解，回退连续/离散状态时若未回退 `outputs`，事件微步初值不一致。

---

## 3. 求解器怎么选（`--solver=`）

| CLI | 积分器 | 对理想开关 |
|-----|--------|------------|
| `rk4` | 显式 RK4 | 多数 EM Examples 可用；斩波/桥式易卡 |
| `rk45` | 自适应 RK45 | 可作自洽门禁对照 |
| **`implicit`** | **Backward Euler（后向欧拉）** | **PWM/开关更稳；Chopper 模板推荐** |
| `radau` | Radau | 与隐式类似，偏刚性 |
| `qss` / `cvode` / `ida` | 其它 | 视模型与 feature |

### `--solver=implicit` 含义

不使用显式 RK4，而用 **隐式 Backward Euler**：在 \(t+dt\) 处解 \(y_{n+1} = y_n + dt\cdot f(t+dt, y_{n+1})\)。  
相对显式多级法，一步只求一次（或定点迭代）右端，更不容易在步内“跨过”开关不连续点。

示例：

```powershell
.\target\release\rustmodlica.exe `
  --lib-path=<OM_MSL_root> `
  --solver=implicit `
  --dt=1e-4 --t-end=0.05 `
  --result-file=chopper.csv `
  Modelica.Electrical.PowerConverters.Examples.DCDC.ExampleTemplates.ChopperBuckBoost
```

---

## 4. 运行时已启用的开关恢复机制

当 tearing/output 名匹配理想开关 `s`，且 Newton 返回 status 2 时，驱动会尝试：

| 机制 | 作用 |
|------|------|
| `prepare_event_step_algebraic_guess` | 事件微步前回退 `save_outputs`，并把过大的 `s` clamp 到 0 |
| JIT Newton 初值 | 开关 `s` **固定从 0 起步**；失败路径不再把爆炸迭代写回 outputs |
| homotopy 重试 | 外层 `homotopy_lambda` 短路径再求一次 |
| 隐式微步优先 | 恢复时先试 Backward Euler，再试 RK4 |
| dt 二分定位 | 在 \((0, dt]\) 找最大可走步长 |
| sticky `dt_active` | 开关区保持缩小步长，避免每步又回到名义 dt |
| `prefer_implicit_near_switch` | 恢复成功后短窗内主步进也走隐式 |
| discontinuity nudge | 仍走不动时有界前推时间（有预算上限） |
| event force-`s=0` | 事件迭代再强制边界初值；**仅当 `|r|<=tol` 才默认接受** |
| fail-open（可选） | 仅 `RUSTMODLICA_SWITCH_FAIL_OPEN=1` 时，允许 `|r|>tol` 继续，并打 `[degraded:switch-fail-open]` |

### Fail-open 策略（默认关闭）

| 环境变量 | 默认 | 含义 |
|----------|------|------|
| `RUSTMODLICA_SWITCH_FAIL_OPEN` | **关** | `1` 时允许 force-s0 在残差超限后降级继续 |
| `RUSTMODLICA_SWITCH_ACCEPT_TOL` | `1e-5` | 生产路径接受 force-s0 的 `|r|` 上限 |
| `RUSTMODLICA_STRICT_NEWTON` | 关 | 开启时同时禁用 fail-open 与宽松 residual 接受 |

生产默认：force-s0 后必须 `|r| < RUSTMODLICA_SWITCH_ACCEPT_TOL`，否则 abort，并提示可设 fail-open。  
显式降级：设置 `RUSTMODLICA_SWITCH_FAIL_OPEN=1` 后继续仿真，stderr 必有 `[degraded:switch-fail-open]`（同时计入 fallback `switch_fail_open`）。

日志关键字：

```text
[newton-switch-recover] implicit micro-step ok ...
[newton-switch-recover] bisect localize ok ...
[newton-switch-recover] discontinuity nudge ...
[newton-switch-recover] event-iteration force-s0 accepted|rejected ...
[degraded:switch-fail-open] stage=... residual=...
```

---

## 5. 已知模型与标签

`crates/regress-harness/examples/regress_msl_broad.json` 相关 tag：

| 模型 | 建议 | Tag / 备注 |
|------|------|------------|
| `...DCMachines.DCPM_Drive` | rk4 short-sim OK | 曾标 `sim_newton_ideal_switch`，已清除 |
| `...DCAC.ExampleTemplates.SinglePhaseTwoLevel` | rk4 OK | 已清除 |
| `...DCDC.ExampleTemplates.HBridge` | rk4 OK | 已清除 |
| `...DCDC.ExampleTemplates.ChopperBuckBoost` | **优先 `--solver=implicit`** | 严格 short-sim OK（`sim_ok_implicit`） |
| `...ChopperBuckBoost.ChopperBuckBoost_DutyCycle` | **优先 `--solver=implicit`** | 严格 short-sim OK（不再依赖 fail-open） |

Validate（`--validate`）可通过、仿真才失败的情况，属于**数值/事件路径**问题，不是 flatten/analyze 缺失。

---

## 6. 数值对标（OMC CSV / 自洽门禁）

脚本：`compare_omc.ps1`

```powershell
# 仅跑 rustmodlica
.\compare_omc.ps1 -Model <name> -LibPath <MSL> -TEnd 0.05 -Dt 1e-4 -OutDir build\em_omc

# 本机无 omc：rk4 vs rk45 末行 maxDiff
.\compare_omc.ps1 -Model <name> -LibPath <MSL> -TEnd 0.05 -Dt 1e-4 `
  -OutDir build\em_omc -CrossSolverSelfGate

# 有 OMC 导出 CSV
.\compare_omc.ps1 -Model <name> -LibPath <MSL> -OmcOut path\to\ref.csv -OutDir build\em_omc
# 或把参考文件放在 OutDir 下命名为 <safe_model>.omc.csv
```

开关模型建议对标时也加上 `--solver=implicit`（经 `-RustArgs` 或改脚本默认 Solver）。

---

## 7. 排障清单

1. 确认失败 phase：`solver-step` / `event-step` / `event-iteration` / `trial-eval`。
2. tearing 名是否含 `diode_*_s` / `transistor_*_s` / `pwm_*`。
3. 先试 `--solver=implicit`（或 `radau`），缩小 `--dt`。
4. 看是否出现 `[newton-switch-recover]`；若直接 assert storm，考虑环境变量 `RUSTMODLICA_ASSERT_STORM_LIMIT`。
5. 需要索引约简时加：`--index-reduction-method=dummyDerivative`。
6. 与 OMC 比时对齐 `t_end`/`dt`/solver，比较 CSV **末行** maxDiff。

---

## 8. 未闭环项

- Fail-open 仍保留为可选逃逸阀（`RUSTMODLICA_SWITCH_FAIL_OPEN=1`）；Chopper DutyCycle 在 BLT 禁止对 `time`/`pwm_*_start` tearing 后，严格路径 short-sim 已绿。
- 生产用法仍建议 **`--solver=implicit`**（rk4 在 PWM 边沿更依赖恢复路径）。
- 本仓库开发机若无 `omc`，真实 OMC CSV 对标需外部提供参考文件。
- `switch_drive/` 已按 step/trial 拆分；仿真驱动侧模块均低于 1000 行。
