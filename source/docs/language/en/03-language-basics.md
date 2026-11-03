# Chapter 3 – Language Basics

← [Chapter 2](02-installation.md) · [Contents](README.md) · Next: [Chapter 4](04-data-types.md)

## Program structure

A source file (`.nx`) consists of optional `using`/`namespace` lines and declarations. Functions may appear directly at the top level. The native entry point must be **exactly one parameterless function `public int main()`**; its return value becomes the exit code.

```nortex
using System;

public int main() {
    Console.writeLine("Hello");
    return 0;
}
```

`void main()`, `main(int a)`, and `bool main()` are rejected during the build with “Windows-Programme benötigen derzeit genau eine parameterlose int-main-Funktion”; without `main`, the linker fails (`LNK2019`).

`using System;` is not required for `Console` in the tested path, but is conventional. `using` with an unknown namespace (`using Foo.Bar;`) produces `NX1003`.

## Statements, expressions, and blocks

- A **statement** ends with `;` (declaration, assignment, call, `return`, `break`, `continue`).
- An **expression** produces a value (`a * b`, `f(x)`, `c ? 1 : 2`).
- A **block** `{ ... }` groups statements and creates its own scope.

```nortex
public int main() {
    int a = 1;            // declaration
    mut int b = a + 2;    // expression used as initializer
    if (b > 2) {          // block
        b += 1;
    }
    return b;
}
```

Missing semicolon: `error[NX0100]: Nach einer Variablendeklaration wird ';' erwartet.` There is no `;` after blocks (`}`).

## Comments

```nortex
// Line comment
/* Block comment */
```

Comments may contain Unicode.

## Identifiers

`[A-Za-z_][A-Za-z0-9_]*`, ASCII only. Case matters (`name`, `Name`, and `NAME` are different). Style warnings `NX0004`: constants should use `UPPER_SNAKE_CASE`; variables/members should use `camelCase` (warning, not an error).

## Keywords

All reserved words are listed in [Chapter 14](14-language-reference.md#keywords). `get`/`set` are ordinary identifiers.

## Typical errors

| Error | Message |
|---|---|
| Unknown character (`@`) | `NX0100 Unbekanntes Zeichen '@'` |
| Reserved `let` | `NX0100 'let' ist reserviert, aber in NORTEX nicht unterstützt` |
| Unknown identifier | `NX1004` |
| Duplicate declaration in the same scope | `NX1001` |

## Naming conventions (recommendation)

Identifiers consist of ASCII letters, digits, and `_`; Unicode is allowed only in comments and string/character literals. Use `PascalCase` for types (without an `I` prefix for interfaces), `camelCase` for methods and variables, `UPPER_SNAKE_CASE` for constants, and `snake_case` for module/file names. Two-letter acronyms remain uppercase (`IOStream`, `getID`); longer ones are treated like words (`XmlParser`). Private fields do not have leading underscores. Violations are warnings (`NX0004`), not errors.

## Planned (design target)

- `using` imports only NORTEX namespaces; it has no connection to .NET.
- Visibility modifiers `public`, `private`, `protected`, and `internal` are parsed; enforcement across module boundaries is missing (see [Chapter 14](14-language-reference.md)).
- The `.ntx` package format is part of the design; see [Chapter 12](12-compiler-architecture.md#ntx-package-format-planned).
