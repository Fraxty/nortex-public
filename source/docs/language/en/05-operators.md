# Chapter 5 – Operators

← [Chapter 4](04-data-types.md) · [Contents](README.md) · Next: [Chapter 6](06-control-flow.md)

## Overview

| Category | Operators | Native |
|---|---|---|
| Arithmetic | `+ - * / %` (binary), `-` (unary) | yes |
| Comparison | `== != < > <= >=` | yes |
| Logical | `&& \|\| !` | yes (short-circuiting) |
| Bitwise | `& \| ^ << >>` | yes |
| Bitwise NOT | `~` | no (`NX2002`) |
| Assignment | `=` `+= -= *= /= %=` | yes |
| Other assignments | `&= \|= ^= <<= >>=` | frontend |
| Increment/decrement | `++ --` (pre/post) | no (`NX2002`); use `a += 1` instead |
| Ternary | `condition ? a : b` | yes |
| Other tokens | `. , ; : :: -> => ( ) [ ] { }` | syntax |

## Precedence (high → low), associativity

| Level | Operators | Associativity |
|---|---|---|
| 11 | `* / %` | left |
| 10 | `+ -` | left |
| 9 | `<< >>` | left |
| 8 | `< > <= >=` | left |
| 7 | `== !=` | left |
| 6 | `&` | left |
| 5 | `^` | left |
| 4 | `\|` | left |
| 3 | `&&` | left |
| 2 | `\|\|` | left |
| 2/1 | `?:` (ternary) | right |
| 1 | `=` and compound assignments | right |

Unary operators (`- ! ~`) bind more tightly than all binary operators. Source: parser (`compiler/src/parser/mod.rs`).

## Behavior

```nortex
public int main() {
    int a = 17;
    return a % 5 + (-a) / 4;     // 2 + (-4) = -2 (division truncates toward 0)
}
```

- Integer division rounds toward zero: `-7 / 2` yields `-3`.
- `&&`/`||` evaluate the right operand only when needed: `a != 0 && 10 / a > 1` is safe when `a == 0`.
- **Overflow:** Default is trap. `add(2147483647, 1)` terminates the program with exit code `-1073741795` (`0xC000001D`). With `--overflow-checks=false`, arithmetic wraps. Division by zero also traps.

## Type compatibility

Both operands of an arithmetic/comparison operator must have the same numeric type (`NX1102`). Conditions must be `bool`. Only `== != < > <= >=` may be overloaded (static, two parameters, `bool` return; frontend only).

## Common errors

- `a++` in native code → `NX2002 Operator 'post++' wird im aktuellen MIR-Subset nicht unterstützt`.
- Mixing `int` and `uint`/`long` without a cast.

## Operator overloading

Only `==`, `!=`, `<`, `>`, `<=`, and `>=` can be overloaded. Each overload is `static`, has exactly two parameters, and returns `bool`:

```nortex
public class Vector3
{
    public static bool operator ==(Vector3 left, Vector3 right)
    {
        return left.x == right.x && left.y == right.y && left.z == right.z;
    }
}
```

The frontend checks the operator list, `static`, parameter count, and `bool` return type. **Planned:** checking the relationship of the parameters to the defining type and consistency (`==`/`!=`, sensible ordering); the compiler cannot prove mathematical consistency of arbitrary implementations.

Not overloadable: `+ - * / %`, `& | ^ ~`, `! && ||`, `<< >>`, `++ --`, indexing, calls, member access, assignment, and custom operator symbols. `&&` and `||` retain short-circuit evaluation. `?` is only part of the ternary operator `c ? a : b`; there is no `?` error-propagation operator.
