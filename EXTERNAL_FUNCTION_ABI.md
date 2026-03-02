# External Function ABI (F3-4)

Parsing and AST support for Modelica `external` functions are implemented. Linking is not yet automated; this document describes the expected ABI for future linking.

## Parsed form

- Grammar: `external_section = "external" (string_comment)? (identifier "(" ... ")")? annotation? ";"`
- AST: `Function.external_info: Option<ExternalDecl>` with `language: Option<String>`, `c_name: Option<String>`.
- When `external_info` is present, the function is not inlined; a call remains as `Call(name, args)` and is compiled to a JIT call. The JIT symbol must be provided (e.g. by loading a library or registering the symbol).

## Expected C ABI (for future linking)

- Calling convention: platform C (cdecl on x86; default on x64).
- Scalar types: `Real` -> `double`, `Integer` -> `int`, `Boolean` -> `int` (0/1).
- Arguments: inputs in declaration order, then output pointers in declaration order (e.g. `void f(double x, int n, double* y)` for one input Real, one input Integer, one output Real).
- Return: void; outputs are written through pointer arguments.
- Library: user supplies a shared library (e.g. `.dll` / `.so`) and the C name (from `external "C" c_name(...)` or the Modelica function name). The tool will load the library and resolve the symbol before JIT link.

## Current behavior

- External functions parse correctly. Calls to them are not inlined; the JIT compiles a call to the function name. If the symbol is not in the JIT symbol table (e.g. not registered or loaded), link will fail with an undefined-symbol error. A future option (e.g. `--external-lib=path`) could load a library and register external function symbols before compile.
