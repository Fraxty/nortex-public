English translation of the German language reference (docs/language). The German text is authoritative when they differ.

# NORTEX Language Reference (0.3.7-dev)

Official language reference for NORTEX. All statements and code examples were checked against the actual compiler (`nortexc` 0.3.7-dev, Windows x64). Verified examples are in [`examples/language/`](../../../examples/language/).

> **Key point about maturity:** The *frontend* (lexer, parser, semantic checking) accepts a broad C#-like syntax. The *native backend path* (`nortexc build` → Windows x64 EXE) supports a limited subset that also includes heap classes. Each chapter therefore distinguishes what only passes `check` from what actually runs as an EXE.

## Contents

| Chapter | File |
|---|---|
| 1 – Introduction | [01-introduction.md](01-introduction.md) |
| 2 – Installation and Setup | [02-installation.md](02-installation.md) |
| 3 – Language Basics | [03-language-basics.md](03-language-basics.md) |
| 4 – Data Types and Variables | [04-data-types.md](04-data-types.md) |
| 5 – Operators | [05-operators.md](05-operators.md) |
| 6 – Control Flow | [06-control-flow.md](06-control-flow.md) |
| 7 – Functions and Methods | [07-functions-and-methods.md](07-functions-and-methods.md) |
| 8 – Structs and Data Structures | [08-structs-and-data-structures.md](08-structs-and-data-structures.md) |
| 9 – Memory Management | [09-memory-management.md](09-memory-management.md) |
| 10 – Input and Output | [10-input-output.md](10-input-output.md) |
| 11 – Errors and Debugging | [11-errors-and-debugging.md](11-errors-and-debugging.md) |
| 12 – Compiler Architecture | [12-compiler-architecture.md](12-compiler-architecture.md) |
| 13 – Practical Examples | [13-examples.md](13-examples.md) |
| 14 – Language Reference | [14-language-reference.md](14-language-reference.md) |
| 15 – FAQ and Best Practices | [15-faq-and-best-practices.md](15-faq-and-best-practices.md) |

## Feature Overview (Implementation Status)

Legend: **Native** = `nortexc build` produces a runnable EXE. **Frontend** = `nortexc check` accepts it, but `build` rejects it (usually `NX2001`/`NX2002`).

### 1. Fully implemented (native)

- Free functions: parameters, return values, direct calls, recursion, mutual recursion; modifiers `public`, `private`, `static`.
- Scalar types `int`, `bool` (also `uint`, `short`, `ulong`, `long` as parameters/returns/locals; see Chapter 4 for limitations).
- Variables: immutable by default, `mut` for mutable, `const`.
- Operators: `+ - * / %`, comparisons, `&& || !` (short-circuiting), unary `-`, `& | ^ << >>`, assignments and `+= -= *= /= %=`, ternary `?:`.
- Control flow: `if`/`else if`/`else`, `while`, `for`, `break`, `continue`, `return`.
- `//` and `/* */` comments, decimal/`0x`/`0b` integer literals.
- String literals, local variables, internal string parameters and return values; `Console.write` / `Console.writeLine` with **exactly one string value**.
- Overflow checking (trap) enabled by default; `--overflow-checks=false` enables wraparound.
- Project manifest `nortex.toml` and directory builds.
- Program result = exit code returned by `int main()`.

### 2. Partially implemented

Native classes support heap references, fields, constructors, methods, object passing/returning/assignment, single inheritance, and `virtual`/`override` dispatch. Closed generic classes and generic instance methods with explicit concrete type arguments are monomorphized. The runtime releases its object arena on normal return from `main`; individual reclamation and tracing GC are not implemented. Parameterized base constructors, multiple inheritance, implicit method type-argument inference, and bounds are unsupported. `struct`, `record`, `interface`, `trait`, `enum`; `namespace`/`using`; `var`; `string`/`char`/`float` variables; arrays `T[]`; `switch`; `match`; `try`/`catch`; `async`/`await`; other generics; postfix `++`/`--`, `~`; `foreach` are partial or frontend-only.

### 3. Planned, not implemented

Individual object deallocation and tracing GC, `extern "C"`/FFI, `unsafe` blocks, lambdas, attributes, destructors, generic free functions, standard library integration (`.nx` drafts in `standard-library/`), async state machines, Linux/ARM64 targets.

### 4. Not available

`fn`/`let` syntax (`let` is reserved and produces `NX0100`), `do … while`, cast syntax (`(int)x`, `x as int` are not parsed), `null` in safe code (`NX1102`), `mut` parameters, console input (`Console.readLine` cannot be resolved), string interpolation/formatting, `readonly` on local variables.

## Further design documents (target state, **not** implementation status)

[Implemented specification](SPECIFICATION_IMPLEMENTED.md) · [EBNF](../grammar.ebnf) · [Ownership](ownership.md) · [Open decisions](15-faq-and-best-practices.md#open-decisions-project-owner)

These documents describe the language target. In case of conflict, this reference together with [`BUILD_STATUS.md`](../../../BUILD_STATUS.md) is authoritative.
