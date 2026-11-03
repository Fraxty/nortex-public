# Chapter 14 – Language Reference

← [Chapter 13](13-examples.md) · [Contents](README.md) · Next: [Chapter 15](15-faq-and-best-practices.md)

## Keywords

Source: `is_keyword` in `compiler/src/lexer/mod.rs`.

| Group | Words | Usable natively |
|---|---|---|
| Declarations | `using namespace class struct record interface trait enum` | declarations accepted only |
| Modifiers | `public private protected internal static abstract sealed partial extern unsafe const virtual override async readonly` | `public private static const` |
| Mutability | `var mut ref out params` | `mut` |
| Control flow | `return throw if else while for in switch case default break continue try catch finally match` | `return if else while for break continue` |
| Literals | `true false null` | `true false` (`null` forbidden) |
| Expressions | `new await this base operator is as typeof sizeof where` | no |
| Reserved | `let` | no (`NX0100`) |
| Types | `sbyte short int long i128 byte ushort uint ulong u128 nint nuint isize usize float double decimal bool char string void` | `int bool uint short ulong long` (limitations: Chapter 4) |

Not keywords: `Console`, `System`, `get`, `set`, `do`, `foreach`, `fn`.

## Tokens

`Identifier`, `Keyword`, `Number` (decimal, `0x`, `0b`, exponent, suffix), `StringLiteral`, `CharacterLiteral`, `Symbol`, `EndOfFile`. Comments are captured separately.

## Operators and symbols

`+ - * / %` · `== != < > <= >=` · `&& || !` · `& | ^ ~ << >>` · `= += -= *= /= %= &= |= ^= <<= >>=` · `++ --` · `? :` · `. , ; ( ) [ ] { }` · `-> => ::`. Precedence: [Chapter 5](05-operators.md).

## Grammar constructs (native)

```text
Datei        = { Using } { Funktion } ;
Using        = "using" Name ";" ;
Funktion     = { Modifier } Typ Name "(" [ Typ Name { "," Typ Name } ] ")" Block ;
Block        = "{" { Anweisung } "}" ;
Anweisung    = Deklaration | Zuweisung ";" | Aufruf ";" | If | While | For
             | "break" ";" | "continue" ";" | "return" [ Ausdruck ] ";" | Block ;
Deklaration  = [ "mut" | "const" ] Typ Name "=" Ausdruck ";" ;
If           = "if" "(" Ausdruck ")" Anweisung [ "else" Anweisung ] ;
While        = "while" "(" Ausdruck ")" Anweisung ;
For          = "for" "(" Deklaration Ausdruck ";" Zuweisung ")" Anweisung ;
Ternär       = Ausdruck "?" Ausdruck ":" Ausdruck ;
```

This is a simplified view of the natively supported subset. The complete frontend grammar: [grammar.ebnf](../grammar.ebnf).

## Supported statements

Local declaration, assignment (simple/compound), function call, `Console.write(Line)`, `if`/`else`, `while`, `for`, `break`, `continue`, `return`, block.

## Built-in functions

| Function | Signature | Limitation |
|---|---|---|
| `Console.write` | `(string)` | exactly one string value |
| `Console.writeLine` | `(string)` | exactly one string value |

There are no other built-in functions (`Console.readLine` cannot be resolved).

## Diagnostic codes (observed)

`NX0004 NX0100 NX1001 NX1003 NX1004 NX1005 NX1100 NX1102 NX1103 NX1107 NX1200 NX1201 NX1301 NX1302 NX2001 NX2002`. See [Chapter 11](11-errors-and-debugging.md).

## Relevant limitations

`intrinsic` is a context-sensitive modifier only for files under `standard-library/`; in application code it is rejected with `NX1005`. No intrinsic/runtime symbol binder exists.

See the [feature overview](README.md#feature-overview-implementation-status). Also: Windows x64 only; `main` must be `public int main()` without parameters; module system has no qualified names or visibility (`using` uses a flat namespace); `long` locals with division can crash codegen.
