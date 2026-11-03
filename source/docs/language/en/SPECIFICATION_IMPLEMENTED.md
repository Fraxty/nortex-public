# NORTEX Language Specification – Current Status 0.3.7-dev

Describes only what the compiler and `nortexc build` can do today. Design content (target vision) is marked “Planned” in Chapters 03–15 of the [language reference](./README.md); see the [language reference](./README.md) for details. Anything not implemented is marked as **Future**.

## 1. Toolchain

- Target: `x86_64-pc-windows-msvc` (the only supported target).
- Pipeline: source → AST → HIR → MIR → Cranelift → object file → MSVC linker → EXE.
- Commands: `nortexc check <file>`, `nortexc build <file|directory|nortex.toml> [-o <exe>] [--overflow-checks=<true|false>]`.

## 2. Lexical rules

- `.nx` files, UTF-8. Comments: `//` and `/* … */`.
- ASCII identifiers only (`[A-Za-z_][A-Za-z0-9_]*`); Unicode only in comments and string literals.
- PascalCase for types, camelCase for functions and values, and UPPER_SNAKE_CASE for `const` (warning NX0004).
- There is no `fn`, `let`, `i32`, or `str`. Instead: `public int name(...)`, `int x = …;`, `mut int x = …;`.

## 3. Types

| Type | Build status |
|---|---|
| `int` (32-bit, signed), `bool` | native |
| Global `int`/`bool` values | native; compile-time initializer, immutable unless `mut` |
| `string` | native literals, local variables, internal function parameters/returns; `Console.write`/`writeLine` accept one `string` |
| `double` | native in the MIR/COFF subset |
| `class` | native: heap references retained in an arena until normal return from `main`, scalar/string/class fields, constructors, methods, passing/returning, and assignment |
| single inheritance, `virtual`/`override` | native; flattened field layouts, parameterless base constructor, dispatch by dynamic class ID |
| closed generic classes and generic instance methods | native monomorphization for concrete `new` instantiations and explicit concrete method type arguments |
| `float`, `struct`, generic free functions | no native backend path yet |

Native class objects are aliasable pointers; the runtime arena releases them together on normal return from generated `main`. There is no individual reclamation or tracing GC, so memory can grow until then. Traps/aborts bypass explicit shutdown; when embedding, the host must call `nortex_object_arena_shutdown` only after no object pointer is in use. Programs must stop all threads using object pointers before `main` returns. Generic methods require explicit type arguments; inference and bounds are not supported. See [Memory management](09-memory-management.md) and [Classes](08-structs-and-data-structures.md).

Native string values are immutable UTF-8 slices: a 16-byte descriptor contains a data pointer and byte length; literal data and descriptors reside read-only in the COFF data section. Internally, the descriptor address is passed as a 64-bit value. Heap strings are created by `+` (`nortex_string_concat`, `HeapAlloc`, NUL-terminated) and freed at scope exit/`return` via `nortex_string_free`, unless moved (NX1202 on subsequent use). `==`/`!=` use `nortex_string_eq`. Top-level `const`/`mut string` values are pointers to descriptors in `.rdata`; `extern "C"` receives `string` arguments as `char*`. The linker rebuilds `nortex_runtime.lib` when runtime sources are newer.

## 4. Statements

```nortex
using System;

public int add(int a, int b)
{
    return a + b;
}

public int main()
{
    mut int sum = 0;
    for (mut int i = 1; i <= 10; i = i + 1)
    {
        sum = sum + i;
    }
    if (sum == 55) { Console.writeLine("ok"); }
    return add(sum, 0) - 55;
}
```

- Entry point: exactly `public int main()`; its return value is the exit code.
- Bindings are immutable unless declared with `mut`.
- Control flow: `if`/`else`, `while`, `for`, `return`; calls to free functions.
- Global scalars may precede functions, e.g. `public const int MAX_PLAYERS = 64;` or `mut int playerCount = 0;`. Only integer and `bool` globals are emitted natively.

## 5. Overflow and traps

- `overflow_checks = true` (manifest) or `--overflow-checks=true`: `int` overflow traps, exit code `0xC000001D` (`-1073741795`).
- `false`: wrap-around.
- Example: [02_math_and_overflows](../../../examples/language/02_math_and_overflows/main.nx).

## 6. Runtime

`Console.write`/`writeLine` and object allocation link against `nortex_runtime.lib` (ABI design: [Chapter 12](12-compiler-architecture.md#abi-design-target)). Search order: EXE directory → parent → `target\release` → `target\debug`; `NORTEX_RUNTIME_LIB` takes precedence. Object allocations are released together on normal return from `main`; there is no individual reclamation or GC.

## 7. Modules and FFI

- **Modules**: `using util;` loads `util.nx` (or `util\mod.nx`) relative to the importing file or the entry file's directory; `using a.b;` → `a\b.nx`. Each file is parsed separately, then declarations are merged into one program in dependency order (flat namespace, no `util::` qualification, no visibility checks between modules). Cycles are reported as errors; diamond imports load a module once. Semantic diagnostics are attributed to the entry file (parser errors carry the correct file). Example: [03_multi_file_project](../../../examples/language/03_multi_file_project/src/main.nx).
- **`extern "C"`**: `public extern "C" int GetTickCount();` declares a Win32/C function without a body. The symbol name is used unchanged (no decoration); calling convention is Windows x64. Libraries come from `[dependencies]` (`lib = "user32.lib"`). Only `int`/`bool` parameters and returns; pointers/strings (e.g. `MessageBoxA`) are **not** supported. Example: [04_win32_demo](../../../examples/language/04_win32_demo/src/main.nx).

## 8. Manifest `nortex.toml`

```toml
[package]
name = "demo"             # required
version = "0.1.0"         # default
authors = ["Example Author"]
description = "optional"

[build]
main = "src/main.nx"      # default
output_name = "demo"      # default: package.name
target = "x86_64-pc-windows-msvc"
overflow_checks = true    # optional
opt_level = 0             # 0..3
linker_flags = []

[dependencies]
kernel32 = { lib = "kernel32.lib" }   # exactly one of path | lib
```

- Output: `<manifest-directory>\build\<output_name>.exe`.
- `path` → `/LIBPATH:`, `lib` → linker argument.
- `docs/build-system/manifest.md` describes an older schema (`.nortex`, `[project]`); this specification is authoritative.

## 9. Known gaps

Structs/floats, freeing heap strings (drop), generic free functions, implicit method type-argument inference/bounds, class use without concrete instantiation, multiple inheritance and interfaces/traits, qualified module names and visibility across modules, pointer parameters in `extern "C"`, package manager, `nortexc fmt`, LSP.

## 10. Naming and layout conventions

These rules are partially checked with `NX0004`. They are warnings, not syntax errors.

| Element | Style | Examples |
|---|---|---|
| Classes, structs, enums, interfaces, traits | PascalCase | `Player`, `XmlParser`, `IOStream` |
| Functions, methods, variables, fields, parameters | camelCase | `takeDamage`, `isActive`, `targetPosition` |
| `const` values | UPPER_SNAKE_CASE | `MAX_PLAYERS`, `DEFAULT_TIMEOUT_MS` |
| Generic parameters | PascalCase | `T`, `TKey`, `TValue` |
| Module and file names | snake_case (recommended) | `math_utils`, `collision_detector.nx` |

- Types, functions, and values must not contain underscores, except `const` names.
- Two-letter acronyms remain uppercase (`IOStream`, `getID`); acronyms of three or more letters are written like ordinary words (`XmlParser`, `parseHttpHeader`).
- Interfaces and traits do not require an `I` prefix. Visibility is specified with `public`, `private`, etc.; private fields do not have an underscore prefix.
- Style recommendations for `this` in field access and file names are not enforced automatically.
- Formatting: four spaces, Allman braces for types and functions, K&R for control flow. A formatter and automatic formatting checks are not yet implemented.
