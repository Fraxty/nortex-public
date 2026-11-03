# Chapter 4 – Data Types and Variables

← [Chapter 3](03-language-basics.md) · [Contents](README.md) · Next: [Chapter 5](05-operators.md)

## Primitive types

| Type | Width | Native (build) |
|---|---|---|
| `int` | 32-bit, signed | yes |
| `bool` | `true`/`false` | yes |
| `uint`, `short`, `ulong` | 32/16/64-bit | yes for locals and parameters/returns (`uint` function tested) |
| `long` | 64-bit | yes for parameters/returns; **locals with division/comparison can crash codegen** (known Cranelift panic) |
| `sbyte`, `byte`, `ushort`, `i128`, `u128`, `nint`, `nuint`, `isize`, `usize` | see this page (section “Planned”) | frontend; not individually tested natively |
| `float`, `double`, `decimal` | IEEE-754 32/64 or 128-bit | **no** (`NX2001`) |
| `char` | Unicode scalar | **no** (`NX2002`) |
| `string` | UTF-8 slice (data pointer + byte length) | yes, as a literal, local variable, internal parameter/return value, and `Console` argument |
| `void` | no value | return type only |

## Literals

String literals and their 16-byte slice descriptors reside read-only in the COFF data section. Length is measured in UTF-8 bytes.

- **Concatenation:** `a + b` calls the runtime's `nortex_string_concat`. It reserves a block containing a descriptor, bytes, and a trailing NUL via `HeapAlloc`, and returns the new slice. If allocation fails, the program terminates with a trap.
- **Globals:** Top-level `const string` and `mut string` are allowed. The initializer must be a string literal or a `+` concatenation of literals (folded at compile time). The global is a pointer to a descriptor in `.rdata`; `mut string` replaces only the slice.
- **`extern "C"`:** String arguments are passed as `char*` (data pointer). All strings are NUL-terminated; an embedded `\0` terminates the string early from C's perspective. External functions may not return `string`.
- **Comparison:** `==` and `!=` call `nortex_string_eq` (length and byte comparison) and return `bool`. Ordering operators (`<`, `>`, …) on strings are not implemented.
- **Drop and Move:** Heap strings (results of `+`) belong to exactly one variable. At scope exit, on `return`, and before overwriting, codegen calls `nortex_string_free` (`HeapFree`), unless the value was moved. Assignment, initialization, return, and passing to internal functions move the source string; any further use reports NX1202. Concatenation, comparison, `Console`, and `extern "C"` calls only borrow. Static literals and `const string` are never freed.
- **Limitations:** Moving a `mut string` global (`string b = name;`, return, argument, ternary branch) copies the value (`name + ""`); the global retains its buffer. Reassigning it frees the old heap buffer.

```nortex
int a = 42;
int h = 0xFF;          // hexadecimal
int b = 0b1010;        // binary
double d = 1.5e3;      // exponent (frontend)
float f = 2.5f;        // f suffix required; 2.5 is double
char c = 'x';
string s = "text";
bool t = true;
```

Literals are checked against the target type: `int a = 3000000000;` → `NX1102 … is outside the value range of 'int'`. Integer suffixes such as `5L` lex successfully, but fail in native lowering (`NX2002`). String escapes: `\n \r \t \0 \" \\`; unknown escapes (`\q`) → `NX0100`.

## Variables

```nortex
int a = 1;          // immutable
mut int b = 2;      // mutable
const int LIMIT = 3;
```

- **Immutable by default.** `a = 2;` → `NX1200 Der unveränderliche Wert 'a' darf nicht neu zugewiesen werden.`
- Global values may also appear at the top level. They require a compile-time-evaluable initializer; `mut` allows later changes. Example: `public const int MAX_PLAYERS = 100;` and `mut int playerCount = 0;`.
- Global integer and `bool` values are emitted natively in read-only or writable COFF data sections. Other globally declarable types are checked by the frontend, but are not yet available in the native MIR subset (`NX2002`).
- Parameters and loop variables are also immutable; `for` requires `mut int i`.
- `const` requires `UPPER_SNAKE_CASE` (otherwise warning `NX0004`).
- Every variable needs both a type **and** an initializer. `let` is reserved and cannot be used. `readonly int a = 1;` is invalid as a local declaration.
- `null` is forbidden (`NX1102`).

## Type inference

`var c = a * b;` passes `check`, but is rejected during build: “Variablentyp-Inferenz wird in dieser Lowering-Stufe nicht unterstützt” (`NX2001`). Always use explicit types in native code.

## Type conversions

There are **no implicit conversions between integer types**: `uint + int` → `NX1102 Die Operanden von '+' müssen denselben numerischen Typ besitzen`; returning a `byte` from an `int` function → `NX1102`. No working cast syntax is available (`(int)x` and `x as int` are not parsed).

## Scopes

A block `{}` creates a scope. Variables are visible from their declaration to the end of the block; use outside that scope → `NX1004`. Duplicate names in the same scope → `NX1001`. In the tested case, a nested scope cannot reassign an outer name if it is immutable (`NX1200`).

```nortex
public int main() {
    mut int a = 1;
    if (a == 1) {
        int b = 5;
        a = b;          // ok: a is mutable
    }
    return a;           // 5; b is not visible here
}
```

## Limitations

Only scalar integer/boolean values are native. Strings, floating-point values, arrays, structs, and classes are not yet executable.

## Planned (design target, not executable)

Primitive type names are case-sensitive and have no aliases (`Int32`, `String`, `Bool`, `Unit`, `Never` are invalid). Implicit lossy numeric conversions are excluded. `char` is a 32-bit Unicode scalar value; `string` is UTF-8, and `length()` counts bytes. `void` is not a storable value type; the divergent type is `!`. `nint`/`nuint`/`isize`/`usize` are pointer-sized (64-bit on Windows x64/ARM64).

**Type inference:** `var x = …;` infers the type from the initializer (current design: immutable, use `mut var` for mutable). Global inference across parameters, returns, and generics is not implemented; ambiguous inference should be an error. `const` requires a compile-time value.

**Decimal128 (design):** `decimal` is intended to be a custom 128-bit fixed-point format: 1 sign bit, 7 scale bits (0–38), and a 120-bit coefficient (0 to 10³⁶−1); value `(-1)^sign × coefficient / 10^scale`, with *nearest, ties to even* rounding; overflow/division by zero should raise `ArithmeticException`. No silent conversion to `double`. Only the format is specified; no arithmetic is implemented.

**Composite types (design):** `T[]` (dynamic), `[T; N]` (fixed), tuples `(T1, T2, …)` with at least two elements, `struct`, `class`, `record` (immutable, structural equality), `enum` (sum type), `interface`, `trait`. `Option<T>` (`Some`/`None`) and `Result<T,E>` (`Ok`/`Err`); there is no `null` in the safe model.

**Other type concepts (design, names only):**

| Type | Intended semantics |
|---|---|
| `object` | explicit type erasure/boxing, not a universal base type |
| `dynamic` | dynamic resolution with runtime checking (`DynamicDispatchException`) |
| `&T`, `&mut T` | shared or exclusive borrow |
| `*const T`, `*mut T` | raw pointer, dereference only in `unsafe` |
| `Box<T>` | unique heap owner |
| `Rc<T>`, `Arc<T>` | shared ownership (non-thread-safe / atomic) |
| `Cell<T>`, `RefCell<T>` | interior mutability (`RefCell` with runtime checking) |
| `Mutex<T>`, `RwLock<T>` | synchronization; `Send`/`Sync` mark thread safety |
| `Span<T>` | non-owning view of contiguous memory |
| `Memory<T>` | owning runtime handle |
| `Func<…,R>`, `Action<…>` | callable values; captures follow Move/Borrow rules |
| `Future<T>`, `Task<T>` | computation or scheduled execution; see [Chapter 11](11-errors-and-debugging.md#asynchrony-planned) |

Registering a type name in the compiler does not establish that it is implemented.
