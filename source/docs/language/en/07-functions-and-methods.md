# Chapter 7 – Functions and Methods

← [Chapter 6](06-control-flow.md) · [Contents](README.md) · Next: [Chapter 8](08-structs-and-data-structures.md)

## Function definition

```nortex
[Modifier] Rückgabetyp name(Typ param1, Typ param2) { Rumpf }
```

```nortex
public int add3(int a, int b, int c) {
    return a + b + c;
}
```

- Modifiers: `public`, `private`, and `static` are tested natively (`private int helper()`, `public static int twice(int n)`).
- Parameters are **immutable** (`a = a + 1;` → `NX1200`); `mut` parameters do not exist (`NX0100 Typname erwartet`).
- Every function with a non-`void` return type needs a body and a `return` on every path (`NX1103`). A signature without a body → `NX1005`.

## Calls and recursion

Natively, calls are limited to **direct calls to free, non-generic functions**. Definition order does not matter (mutual recursion `even`/`odd` tested).

```nortex
public int fib(int n) {
    if (n < 2) { return n; }
    return fib(n - 1) + fib(n - 2);
}
```

Wrong number of arguments: `NX1107 Keine Überladung von 'f' akzeptiert 2 Argumente. Kandidaten: f(int).` Unknown function: `NX1004`.

## Return values

Exactly one value is returned via `return`. String literals can be assigned to local variables, passed as parameters, and returned by internal functions. `void` functions cannot be called in native builds, **except** `Console.write/writeLine` (`NX2002 Void-Aufrufe sind im MIR-Subset nur für Console.write und Console.writeLine verfügbar`). String parameters/returns and `bool`, `int`, `uint` are supported natively; the string ABI in `extern "C"` is not supported.

## Local variables

They are in scope from their declaration to the end of the block and are separate for each call ([Chapter 4](04-data-types.md#scopes)).

## Methods and visibility (frontend only)

Methods may appear in `class`/`struct`; `public`/`private`/`protected`/`internal` are parsed. The call `c.inc()` passes `check`, but the build rejects classes/`new` (`NX2001`; see [Chapter 8](08-structs-and-data-structures.md)). Constructors are named after the type; signature and overload checks are missing.

## Limitations

No generics, lambdas, native overload resolution, function pointers, or default parameters.

## Design rules and frontend status

- The C#-like declaration (`public int add(int a, int b)`) is the only form; `fn name(...) -> T` is not part of the language. Constructors are named after the type and have no return type.
- Generic parameters follow the name: `public T identity<T>(T value)`.
- Overloads differ by parameter types, not only by return type. For otherwise valid overloads, unsuffixed integer literals prefer integer parameters over floating-point parameters; equally ranked candidates remain ambiguous (`NX1107`).
- The frontend detects missing return paths, generic method calls, and type-argument substitution in simple signatures; a non-static method call through the type name is rejected.
- **Planned:** full generic inference, constraint checking, cross-module resolution, and code generation.
