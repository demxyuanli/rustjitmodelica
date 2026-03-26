# 回归案例总览 / Regression Case Guide

## 目的 / Purpose

本目录按功能对 `rustmodlica` 回归案例进行分类整理。  
This folder organizes `rustmodlica` regression cases by feature area.

该文档体系仅做说明与追溯，不新增测试程序。  
This documentation is for guidance and traceability only, without adding new test programs.

## 入口脚本 / Entry Scripts

- 主回归脚本 / Main regression script: `d:/source/repos/rustmodlica/run_regression.ps1`
- 目录回归脚本 / Directory regression script: `d:/source/repos/rustmodlica/run_modelica_dir_regression.ps1`
- OMC 对比脚本 / OMC compare script: `d:/source/repos/rustmodlica/compare_omc.ps1`

## 分类文档 / Category Documents

- [核心仿真 / Core Simulation](./core-simulation.md)
- [事件与时钟 / Events And Clock](./events-and-clock.md)
- [函数与多输出 / Functions And Multioutput](./functions-and-multioutput.md)
- [展平连接与OOP / Flatten Connect OOP](./flatten-connect-oop.md)
- [工具链FMI EmitC Script / Toolchain FMI EmitC Script](./toolchain-fmi-emitc-script.md)（含 `modelDescription` 字段、CLI/`RUSTMODLICA_FMI_*` 与 `run_regression.ps1` FMI 断言） / includes `modelDescription` fields, CLI and `RUSTMODLICA_FMI_*` env, and `run_regression.ps1` FMI checks
- [MSL与ModelicaTest目录回归 / MSL And ModelicaTest Directory Regression](./msl-modelicatest-dir-regression.md)

## 统一判定规则 / Unified Verdict Rules

- `pass` 用例 / case: 命令退出码为 `0`
- `fail` 用例 / case: 命令退出码为非 `0`
- 产物检查 / artifact check: 需要输出文件的功能必须产生对应文件
- 稳定性检查 / stability check: 确定性场景重复执行结果文件应一致

## 追溯基线 / Traceability Baseline

分类映射与以下来源保持一致。  
Category mapping is aligned with the following sources.

- `run_regression.ps1` 的用例清单与专项检查 / case list and special checks
- `JIT_DEVELOPMENT_ANALYSIS.md` 的回归覆盖统计 / regression coverage sections
- `README.md` 的求解器与工具链入口说明 / solver and toolchain usage sections

## 更新规范 / Update Rules

- 新增案例时，写入对应分类文档 / Add new cases to the corresponding category document
- 命令示例保持 Windows PowerShell 兼容 / Keep command examples Windows PowerShell compatible
- 期望结果与失败模式需和脚本行为同步 / Keep expected verdict and failure mode synchronized with script behavior
