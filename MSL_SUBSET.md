# Modelica Standard Library (MSL) Subset

rustmodlica uses a minimal MSL subset for compatibility with common Modelica models.
This document pins the supported scope and version reference.

## Version Reference (MSL-1)

- **Pinned version:** Modelica Standard Library 3.2.3
- **Target alignment:** MSL 3.2.3 (or compatible subset)
- **Subset location:** `StandardLib/Modelica/`
- **Loader:** Models are loaded via `Modelica.Class.Name` -> `StandardLib/Modelica/Class/Name.mo`

## MSL-1: Included Packages

### Modelica.Constants
- `pi`, `e`, `g_n` (constants)

### Modelica.SIunits (MSL-4)
- Type aliases (Time, Temperature, Pressure, etc.) resolved as Real in flatten (`Modelica.SIunits.*` treated as primitive); units not enforced.

### Modelica.Blocks.Interfaces
- `RealInput` (input Real signal)
- `RealOutput` (output Real signal)
- `SO` (Single Output: RealOutput y)
- `SISO` (Single Input Single Output: RealInput u, RealOutput y)

### Modelica.Blocks.Sources
- `Constant` (y = k)
- `Step` (y = offset + (time >= startTime ? height : 0))
- `Sine` (y = offset + amplitude * sin(2*pi*freqHz*(time-startTime) + phase))

### Modelica.Blocks.Continuous
- `Integrator` (der(x) = k*u; y = x)
- `TransferFunction` (der(x) = u - a*x; y = b*x)

## MSL-3: Modelica.Math

Built-in functions are provided via JIT symbols (no .mo files):

| Function       | Implementation |
|----------------|----------------|
| sin, cos, tan  | f64::sin, cos, tan |
| asin, acos, atan, atan2 | f64::asin, acos, atan, atan2 |
| sinh, cosh, tanh | f64::sinh, cosh, tanh |
| exp, log, log10 | f64::exp, ln, log10 |
| sqrt, abs      | f64::sqrt, abs |
| ceil, floor    | f64::ceil, floor |
| mod, rem, div  | Modelica semantics (rem_euclid, %, trunc) |
| sign, min, max | Modelica semantics |
| integer       | trunc |

Both short names (`sin`, `sqrt`) and `Modelica.Math.*` aliases are registered.

## Test Models

- `TestLib/LibraryTest`: Sine + Integrator + connect + Modelica.Math.min/max/mod/sign
- `TestLib/MSLBlocksTest`: Constant + Step
- `TestLib/MSLTransferFunctionTest`: Constant + TransferFunction (MSL-2)
- `TestLib/SIunitsTest`: Modelica.SIunits.Time (MSL-4)

## Limitations

- No Modelica.Math package .mo files; all math is native/JIT.
- SIunits: types parsed, units not validated.
- Replaceable, redeclare, conditional components: not yet supported.
