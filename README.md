# RustModlica

A simple Modelica interpreter and compiler written in Rust.

## Features

- **Parser**: Parses a subset of Modelica (models, declarations, equations).
- **Interpreter**: Evaluates equations directly in Rust.
- **JIT Compiler** (Default): Compiles Modelica to machine code and executes it immediately in memory. **Pure Rust, no external C compiler required.**
- **AOT Compiler** (Optional): Compiles to object files (`.o`) and attempts to link using system compilers (`cl.exe` or `gcc`).
- **Math Functions**: Supports `sin`, `cos`, `tan`, `sqrt`, `exp`, `log`.

## Usage

1. Create a Modelica file (e.g., `complex.mo`):
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

2. Run with JIT (Default):
   ```bash
   cargo run complex.mo
   ```
   This compiles the model to machine code in memory and runs it. No external tools needed.

3. Run with Object File Output:
   ```bash
   cargo run complex.mo obj
   ```
   This generates `ComplexMath.o` and attempts to link it into an executable. Requires `cl.exe` or `gcc`.
