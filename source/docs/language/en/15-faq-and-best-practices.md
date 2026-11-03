# Chapter 15 – FAQ and Best Practices

← [Chapter 14](14-language-reference.md) · [Contents](README.md)

## Common beginner mistakes

| Error | Cause / fix |
|---|---|
| `NX1200` for `i += 1` in `for` | Declare the loop variable `mut`: `for (mut int i = 0; …)` |
| `NX1200` when changing a parameter | Parameters are immutable; create a local `mut` copy. |
| `NX2001`/`NX2002` during build | Construct is frontend-only (classes, heap strings, `var`, `switch`, …). |
| “exactly one parameterless int-main function” | Use `public int main()`. |
| `a++` fails | Use `a += 1;` |
| Cannot print a number | Return the result as an exit code. |
| `let x = 1;` | Specify the type: `int x = 1;` |
| `NX1102` for `uint + int` | Use matching types; there are no casts. |

## FAQ

**Why doesn't my class run?** `class`/`struct`/`new` are supported by the frontend, but not by native lowering.
**Can I store strings?** Yes. Literals, local variables, top-level `const`/`mut string`, internal parameters, and returns use UTF-8 slices; `a + b` creates heap strings (without deallocation).
**How do I debug?** Use `nortexc check`, `nortexc mir`, and exit codes; there is no debugger.
**Is Linux supported?** No, Windows x64 only.
**Are there modules?** Yes: `using util;` resolves `util.nx` or `util/mod.nx` (flat namespace); see the [implemented specification](SPECIFICATION_IMPLEMENTED.md).

## Recommended practices

- **Names:** functions/variables `camelCase`, constants `UPPER_SNAKE_CASE`, types `PascalCase` (matches `NX0004` warnings).
- **Mutability:** use `mut` only where needed.
- **Structure:** small functions with a single return path; keep `main` as an orchestrator.
- **Error checking:** check for `0` before division (otherwise it traps rather than reporting an error).
- **Types:** prefer `int`/`bool` in native code; use explicit types instead of `var`.
- **Verification:** build examples after each change and check the exit code.

## Performance

The backend optimizer operates on MIR; recursion (`fib`) is not optimized (no tail-call proof). Overflow checking has some cost; use `--overflow-checks=false` only deliberately, as wraparound then occurs without a warning.

## Larger projects

Build with `nortex.toml` ([Chapter 2](02-installation.md)). Multiple `.nx` files can be combined with `using` (flat namespace, no visibility checks between modules; see the [implemented specification](SPECIFICATION_IMPLEMENTED.md)).

## Open decisions (project owner)

These items have not been approved; the compiler must not treat them as final:

1. **License:** The repository header says MIT; this is not confirmed legal approval.
2. **Exceptions:** checked or unchecked behavior and the native unwinding mechanism are undecided. Exceptions must never escape uncontrollably across a C-ABI boundary.
3. **Arithmetic:** Whether checked arithmetic is the release default is undecided (currently configurable through overflow checks; see the [manifest](../../build-system/manifest.md)).
4. **Method receiver:** `&self`, `&mut self`, or another instance-method contract is undecided.
5. **Backend:** The backend is Cranelift with MSVC `link.exe`; LLVM is not binding and at most a non-normative prototype.

Earlier design rules superseded by the current implementation (bindings “mutable by default,” `.nortex` manifest, `lld-link` linker, “no native EXE”) no longer apply.
