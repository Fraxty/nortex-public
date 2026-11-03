# Chapter 12 – Compilation and Technical Architecture

← [Chapter 11](11-errors-and-debugging.md) · [Contents](README.md) · Next: [Chapter 13](13-examples.md)

## Pipeline

```text
.nx → Lexer → Parser/AST → semantische Prüfung → HIR → MIR
    → MIR-Validierung + Optimierung → Cranelift → Windows-x64-COFF (.obj)
    → MSVC-Linker (+ NORTEX-Runtime) → .exe
```

| Stage | Source | Purpose |
|---|---|---|
| Lexer | `compiler/src/lexer/mod.rs` | ASCII identifiers, keywords, numbers, strings/chars, symbols, comment spans (for the IDE) |
| Parser/AST | `parser/mod.rs`, `ast/mod.rs` | Pratt parser for expressions, declarations, statements; errors as `NX0100` |
| Semantics | `semantic/mod.rs`, `type_checker.rs` | Names, types, mutability, return paths, basic Move/Borrow checks, style warnings |
| HIR | `hir/mod.rs` | Resolved, typed intermediate form |
| MIR | `mir/lowering.rs`, `mir/mod.rs` | CFG with blocks; scalar subset; definite-assignment and loan checks; `NX2001`/`NX2002` diagnostics for unsupported constructs |
| Optimizer | `optimizer/mod.rs` | Optimizations on validated MIR |
| Codegen | `codegen/mod.rs` | Cranelift IR → COFF object for `x86_64-pc-windows-msvc`; overflow trap |
| Linker | `linker/mod.rs` | Invokes MSVC `link.exe`, links CRT and runtime; extra arguments from manifest |
| Driver | `driver/mod.rs` | Connects the stages |

## Commands

| Command | Effect |
|---|---|
| `nortexc check <file>` | Lexer + parser + semantics |
| `nortexc mir <file>` | prints validated, optimized MIR |
| `nortexc build <file\|directory\|nortex.toml> [-o …] [--emit-obj] [--overflow-checks[=bool]]` | produces `.exe` or `.obj` |

## Backend properties

- Target: Windows x64 (MSVC) only. No Linux/ARM64.
- Entry: CRT's C `main`; NORTEX `int main()` returns the process exit code.
- String values are represented as the address of a 16-byte UTF-8 slice descriptor; `Console.write(Line)` loads its data pointer and byte length for the runtime ABI call.
- Unsupported constructs produce diagnostics rather than being silently ignored (exception: in the tested case, an `async` declaration builds without effect; unused `enum`/`class` declarations are accepted).

## Note about older documents

[`docs/compiler/architecture.md`](../../compiler/architecture.md) describes an earlier state (“native backend not yet finished”). The native path for the scalar subset is now available and covered by end-to-end tests (`sum_to(10)` → exit code 55).

## ABI (design target)

`extern "C"` marks a function with the C ABI; only ABI-compatible parameters/returns are allowed. Structs are C-compatible only with `[repr(C)]`; `string`, class owners, generics, exceptions, `Task`/`Future`, and arbitrary `Result`/`Option` layouts are not automatically C-compatible. Strings are passed as a UTF-8 pointer plus byte length; memory is freed by the allocator that provided it. Symbol names, calling convention, and ABI major version are part of the interface contract. A published binary compatibility guarantee and an FFI test run are missing; authoritative details are in [Phase 4](../../design/phase-4-native-runtime-abi.md).

## `.ntx` package format (planned)

`.nx` is source code, `nortex.toml` is the TOML manifest, and `.ntx` is a custom deterministic binary container (not ZIP and not source code). **Reader and writer are not implemented.** Design (container format 1, little-endian): 48-byte header with magic `NORTEXPK`, major/minor version, flags (unknown required flags → rejection), TOML manifest offset/length, index offset, entry count, and reserved zero bytes; one index entry per file with path length, type, flags, offsets, sizes, and SHA-256, sorted lexicographically; 8-byte-aligned payloads, no compression. Validation: relative UTF-8 paths using `/` only (no `..`, no duplicates), every range within the file, size limits before allocation, integer overflow checks, SHA-256 verification (corruption protection, not authentication), optional signatures (method undecided), atomic installation without symlinks. Builds should be byte-for-byte reproducible.
