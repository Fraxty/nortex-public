# Chapter 6 – Control Flow

← [Chapter 5](05-operators.md) · [Contents](README.md) · Next: [Chapter 7](07-functions-and-methods.md)

All constructs in this chapter are verified in `examples/language/03_conditions.nx` and `04_loops.nx` and run natively.

## if / else

```nortex
if (bedingung) { ... } else if (bedingung) { ... } else { ... }
```

The condition is in parentheses and must be `bool`. Braces may be omitted for a single statement (`if (a == 1) return 5;` tested).

```nortex
public int classify(int score) {
    if (score >= 90) {
        return 1;
    } else if (score >= 50) {
        return 2;
    }
    return 3;
}
```

Nesting is unlimited (`nested_if` tested). Common error: non-`bool` condition → `NX1102`.

## while

```nortex
while (bedingung) { ... }
```

Checks the condition before each iteration. `while (true)` with `break` is allowed. There is no **`do … while`**.

## for

```nortex
for (mut int i = 0; i < 10; i += 1) { ... }
```

Classic form: initialization; condition; step. The loop variable must be `mut` (otherwise `NX1200`). `i++` as a step fails natively; use `i += 1`. `foreach` / `for (T v in seq)` is frontend-only.

## break and continue

`break` exits the innermost loop; `continue` jumps to the next iteration. Outside a loop: `NX1301` (`break`) or `NX1302` (`continue`).

## return

`return expression;` ends the function. Functions with a return type require a value (`return;` → `NX1103 Nicht jeder Kontrollfluss ? liefert einen R?ckgabewert`); every control-flow path must return (`NX1103 Nicht jeder Kontrollfluss ? liefert einen R?ckgabewert`).

## Ternary

`int t = sum > 5 ? 1 : 2;` – right-associative and available natively.

## Other constructs (frontend only)

`switch`/`case`/`default` → build error `NX2001 Diese Kontrollflussanweisung wird in der aktuellen Lowering-Stufe nicht unterstützt`; `try`/`catch` → likewise; `match` is partially parsed (arm syntax is not accepted in the test, `NX0100`).
