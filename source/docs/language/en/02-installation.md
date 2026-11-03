# Chapter 2 – Installation and Setup

← [Chapter 1](01-introduction.md) · [Contents](README.md) · Next: [Chapter 3](03-language-basics.md)

Requirements (Windows x64, Rust ≥ 1.88, MSVC Build Tools), build commands, a first program, and the `nortex.toml` manifest are covered in the central [installation guide](../../getting-started/installation.md).

Quick reference:

```powershell
cargo build --release -p nortex-compiler
nortexc build hello.nx -o hello.exe
```

Note: `using util;` resolves user-defined `.nx` files (flat namespace); see the [implemented specification](SPECIFICATION_IMPLEMENTED.md).
