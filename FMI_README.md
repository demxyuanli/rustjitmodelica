# FMI (Functional Mock-up Interface)

## Status

- **FMI 1.0**: Not implemented.
- **FMI 2.0 CS**: Minimal implementation. Use `--emit-fmu=<dir>` (implies `--emit-c=<dir>`). Writes `modelDescription.xml`, `fmi2_cs.c`, plus `model.c`/`model.h`. Compile with a C compiler and zip as FMU (binary or source FMU).
- **FMI 2.0 ME**: Not implemented (stub only).

## FMI 2.0 CS (Co-Simulation)

- **Export**: `rustmodlica --emit-fmu=out ModelName` produces in `out/`: `model.c`, `model.h`, `modelDescription.xml`, `fmi2_cs.c`. The C wrapper implements fmi2Instantiate, fmi2SetContinuousStates, fmi2GetDerivatives, fmi2DoStep, fmi2SetReal, fmi2GetReal, etc., using the generated `residual()`.
- **Pack FMU**: Zip the directory (or build a shared library and zip with binaries) per FMI 2.0 packaging. For source FMU, include the C sources and modelDescription.xml in the archive.

## References

- [FMI 2.0 Standard](https://fmi-standard.org/)
- [FMI 1.0](https://fmi-standard.org/docs/1.0/) (legacy)
