# Chapter 10 – Input and Output

← [Chapter 9](09-memory-management.md) · [Contents](README.md) · Next: [Chapter 11](11-errors-and-debugging.md)

## Console output

```nortex
Console.write("Text without a line break");
Console.writeLine("Text with a line break");
```

Both calls accept **exactly one value of type `string`** in native builds. String literals and string variables can be printed. The runtime receives the UTF-8 data pointer and byte length through an ABI call; this is not a general C-ABI string binding.

```nortex
using System;

public int main()
{
    string greeting = "Hello";
    Console.write(greeting);
    Console.writeLine(" NORTEX");
    return 0;
}
```

Internal string values use a 16-byte slice descriptor (data pointer and byte length), passed as an address according to the Windows x64 ABI. String literal bytes and descriptors reside read-only in the COFF data section. Empty strings and embedded NUL bytes are handled correctly based on the length.

```nortex
using System;

public int main() {
    Console.write("a");
    Console.writeLine("b\tc \"quoted\" \\");
    return 0;
}
```

Output: `ab`⇥`c "quoted" \`. Escapes: `\n \r \t \0 \" \\`. An empty string is allowed. Unicode in a literal is output as UTF-8 bytes; correct rendering depends on the console code page (not systematically tested).

## Not supported (`NX2001`/`NX2002` in each case)

| Attempt | Message |
|---|---|
| `Console.writeLine(5);` | Argument must be of type `string` |
| `Console.readLine()` | only direct free functions and console output |

## String concatenation

`Console.writeLine("Hello " + name);` is allowed: `+` between two `string` values creates a new heap string through `nortex_string_concat` (Windows `HeapAlloc`). It is freed at scope exit through `nortex_string_free` (drop in codegen; see [04-data-types.md](04-data-types.md)). Top-level strings (`const string`/`mut string`) and `extern "C"` calls with a `string` argument (`char*`, NUL-terminated) are also possible; see [04-data-types.md](04-data-types.md).

Console input, formatting, and interpolation are **not implemented**. There is no dedicated API for error output (stderr).

## Return values as an output channel

Since numbers cannot be printed, example programs return results as an **exit code** (`return countPrimes(100);` → `25`). In PowerShell: `$LASTEXITCODE`.

## Standard library

`standard-library/system/console.nx` and `io/mod.nx` are drafts with no compiler integration.

## Strings (design target)

`string` is UTF-8 and owns its memory. `length()` counts bytes (not scalar values or grapheme clusters); indexing is byte-oriented and returns `byte`; `chars()` iterates Unicode scalar values (`char`, 32-bit), while `bytes()` iterates UTF-8 bytes. Strings must remain valid UTF-8. Planned API: `length()`, `isEmpty()`, `contains()`, `startsWith()`, `endsWith()`, `replace()`, `split()`, `trim()`, `toUpperCase()`, `toLowerCase()`, `chars()`, `bytes()`. A public `StringBuilder` is not planned. The type checker accepts `string + string` and `string +` scalar primitives (result `string`); formatting is not yet available end-to-end.
