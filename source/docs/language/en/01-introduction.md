# Chapter 1 – Introduction

← [Contents](README.md) · Next: [Chapter 2](02-installation.md)

## What is NORTEX?

NORTEX is a statically and strongly typed, compiled programming language with C#-like declaration syntax. The `nortexc` compiler is written in Rust and uses Cranelift to produce native Windows x64 programs (`.exe`).

```nortex
public int main() {
    return 42;
}
```

## Goals

According to the project documentation: memory safety without a garbage collector (Ownership/Borrowing), explicit mutability, stable diagnostic codes (`NXnnnn`), and a compiled execution path. **Mutability rules and diagnostics are implemented**; ownership checking is partially implemented (see [Chapter 9](09-memory-management.md)).

## Verified properties

- Identifiers and keywords are case-sensitive; identifiers are ASCII-only.
- Bindings are **immutable** by default; `mut` makes them mutable.
- No `null` in safe code.
- Integer overflow checking is enabled by default.
- German compiler messages with stable codes.

## Distinction

NORTEX looks like C#, but is an independent language: top-level functions without a class, `mut` bindings, ownership concepts, and a considerably smaller feature set. Do not assume syntax from other languages (for example, no casts, no `do-while`, and no `let`).

## Realistic use cases today

Small console programs with integer logic, compiler/language experiments, and learning projects. General application development will require strings, structs, a heap, and a standard library (see [README](README.md#feature-overview-implementation-status)).

## Technical requirements

Windows 10/11 x64, the Rust toolchain (Edition 2024, Cargo ≥ 1.88), and the MSVC Build Tools (`link.exe` linker). See [Chapter 2](02-installation.md).
