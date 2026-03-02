# Modelica Standard Library (MSL) – Version and Subset

**MSL-1 (version pin):** This project targets compatibility with **Modelica Standard Library 3.2.3** (structure and semantics). The `StandardLib/` directory holds a minimal subset; full MSL can be added alongside or loaded from an external path.

**MSL-2 (Blocks core):** The following are present under `StandardLib/Modelica/Blocks/` and are intended to work with the compiler/JIT:

- **Interfaces:** `RealInput`, `RealOutput`, `SO` (single output), `SISO` (single input single output).
- **Sources:** `Constant`, `Step`, `Sine`.
- **Continuous:** `Integrator`, `TransferFunction`.

Support for `extends`, connector `signal`, and parameter binding is required for full Blocks usage.

**MSL-3 (Math):** Built-in math used by the compiler and JIT:

- Standard functions: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `exp`, `log`, `log10`, `sqrt`, `abs`, `sign`, `min`, `max`, `mod`, `rem`, `div`, `integer`, `ceil`, `floor`.
- Namespace `Modelica.Math.*` is recognized for inlining and function entry evaluation (e.g. `Modelica.Math.sin`).

Constants such as `Modelica.Constants.pi` can be added via package/constant resolution in the loader or inlining.
