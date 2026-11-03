# Native sum-to example

This example sums the integers from 1 through 10 and writes a message to the
native console through the NORTEX Runtime. It exercises typed locals, a `while`
loop, an internal function call, MIR optimization, Cranelift COFF code
generation, Runtime static linking, and MSVC linking.

From the repository root in PowerShell:

```powershell
cargo build -p nortex-runtime
cargo run -p nortex-compiler --bin nortexc -- check .\examples\native-sum\main.nx
cargo run -p nortex-compiler --bin nortexc -- build .\examples\native-sum\main.nx --output .\examples\native-sum\native-sum.exe
& .\examples\native-sum\native-sum.exe
$LASTEXITCODE
```

The program prints `The sum from 1 through 10 is 55.` and returns `55` as its
process exit code. `Console.write` and `Console.writeLine` accept string values,
including variables and expressions. Convert scalar values with
`Convert.toString` before writing them.

The compiler links the matching `nortex_runtime.lib` from the Cargo target
directory. When it is stored elsewhere, set `NORTEX_RUNTIME_LIB` to its path.
