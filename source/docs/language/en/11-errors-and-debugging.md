# Chapter 11 – Errors and Debugging

← [Chapter 10](10-input-output.md) · [Contents](README.md) · Next: [Chapter 12](12-compiler-architecture.md)

## Format

```text
error[NX1102]: Typfehler in Variableninitialisierung: erwartet 'float', erhalten 'double'.
 --> datei.nx:7:19
   |
 7 |     float f = 2.5;
   |               ^^^
```

Messages are in German; codes (`NXnnnn`) are stable. Severity: `error` or `warning`. `nortexc` exits with code ≠ 0 on errors and does not write an EXE.

## Error classes with codes actually observed

| Class | Code | Example / fix |
|---|---|---|
| Lexical/syntax | `NX0100` | `int a = ;` · `@` · missing `;` · unterminated string · unknown escape `\q`. Fix the syntax. |
| Duplicate declaration | `NX1001` | `int a` twice in one scope → rename it. |
| Name resolution | `NX1003`, `NX1004` | Unknown namespace/identifier → declare it or correct the name. |
| Member declaration | `NX1005` | Function without a body → add `{ ... }`. |
| Types | `NX1100`, `NX1102` | `int a = "text";` → use a matching type; literal out of range. |
| Return | `NX1103` | Not all paths return → add `return`. |
| Overload/call | `NX1107` | Wrong number of arguments. |
| Mutability | `NX1200`, `NX1201` | Assignment to immutable binding → make it `mut`. |
| Control flow | `NX1301`, `NX1302` | `break`/`continue` outside a loop. |
| Style warning | `NX0004` | Constant not `UPPER_SNAKE_CASE`, variable not `camelCase`. |
| Backend subset | `NX2001`, `NX2002` | Construct not supported by native lowering (see below). |
| Codegen/linker | (no code) | “exactly one parameterless int-main function”; `LNK2019` if `main` is missing. |

> **Difference from the code table:** [`diagnostic-codes.md`](../../compiler/diagnostic-codes.md) reserves `NX0001`–`NX0003` for lexer errors. The current lexer actually reports these cases as `NX0100`. Observed behavior is authoritative.

## Runtime errors

There are no runtime error messages or exceptions. An overflow or division-by-zero trap terminates the program with exit code `-1073741795` (`0xC000001D`, illegal instruction) and no text. `--overflow-checks=false` disables the overflow trap.

## Known compiler bug

Programs with `long` locals and division/comparison against an `int` literal can trigger a **panic in the Cranelift frontend** (`frontend.rs`) instead of a diagnostic. Workaround: use `long` only as a parameter/return type.

## Strategies

1. `nortexc check file.nx` for frontend-only errors.
2. `NX2001`/`NX2002` mean “not native yet,” not “invalid code”: switch to supported constructs ([README](README.md#feature-overview-implementation-status)).
3. `nortexc mir file.nx` displays optimized MIR.
4. Return results through the exit code ([Chapter 10](10-input-output.md)).

## Error handling (design and frontend status)

NORTEX distinguishes optional values (`Option<T>`: `Some`/`None`), expected errors (`Result<T,E>`: `Ok`/`Err`), and exceptions (`throw`, `try`/`catch`/`finally`). There is no `?` operator. `Option` and `Result` are intended to be `must_use`; an unused result must be handled, propagated, or discarded with `discard(value)`.

```nortex
public Result<int, string> divide(int first, int second)
{
    if (second == 0) { return Err("Division by zero"); }
    return Ok(first / second);
}
```

Current frontend status: using a `Result<T,E>` as a standalone expression statement reports `NX1108`; non-exhaustive enum `match` arms report `NX1109` (`_` is the fallback). `Option<T>` is not checked for `must_use`. **Planned:** `discard`, native exception unwinding, RAII cleanup, complete `match`, exceptions as status values at the C-ABI boundary; checked/unchecked behavior is undecided ([Chapter 15](15-faq-and-best-practices.md#open-decisions-project-owner)).

## Asynchrony (planned)

Goal: `async` functions become suspendable state machines; `await` does not block a worker. `Future<T>` is the computation, `Task<T>` the execution scheduled by the runtime.

```nortex
public async Task<string> loadText(string path)
{
    return await File.readText(path);
}
```

The parser knows `async`/`await`; the compiler does **not** lower them into the runtime. The Rust prototype `runtime/src/async/` has a Waker-based scheduler with a FIFO queue and `block_on` (for entry points/tests only). Work stealing, timers, a cancellation API, an IOCP reactor, and `Send`/`Sync` analysis are missing.
