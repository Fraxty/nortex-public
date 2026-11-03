# Kapitel 14 – Sprachreferenz

← [Kapitel 13](13-examples.md) · [Inhalt](README.md) · Weiter: [Kapitel 15](15-faq-and-best-practices.md)

## Schlüsselwörter

Quelle: `is_keyword` in `compiler/src/lexer/mod.rs`.

| Gruppe | Wörter | Nativ nutzbar |
|---|---|---|
| Deklarationen | `using namespace class struct record interface trait enum` | nur Deklaration akzeptiert |
| Modifier | `public private protected internal static abstract sealed partial extern unsafe const virtual override async readonly` | `public private static const virtual override` |
| Veränderlichkeit | `var mut ref out params` | `mut` |
| Kontrollfluss | `return throw if else while for in switch case default break continue try catch finally match` | `return if else while for break continue` |
| Literale | `true false null` | `true false` (`null` verboten) |
| Ausdrücke | `new await this base operator is as typeof sizeof where` | nein |
| Reserviert | `let` | nein (`NX0100`) |
| Typen | `sbyte short int long i128 byte ushort uint ulong u128 nint nuint isize usize float double decimal bool char string void` | `int bool uint short ulong long` (Einschränkungen: Kap. 4) |

Nicht-Schlüsselwörter: `Console`, `System`, `get`, `set`, `do`, `foreach`, `fn`.

## Token

`Identifier`, `Keyword`, `Number` (dezimal, `0x`, `0b`, Exponent, Suffix), `StringLiteral`, `CharacterLiteral`, `Symbol`, `EndOfFile`. Kommentare werden separat erfasst.

## Operatoren und Symbole

`+ - * / %` · `== != < > <= >=` · `&& || !` · `& | ^ ~ << >>` · `= += -= *= /= %= &= |= ^= <<= >>=` · `++ --` · `? :` · `. , ; ( ) [ ] { }` · `-> => ::`. Rangfolge: [Kapitel 5](05-operators.md).

## Grammatikalische Konstruktionen (nativ)

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

Das ist eine vereinfachte Sicht des nativ unterstützten Teils. Die vollständige Frontend-Grammatik: [grammar.ebnf](grammar.ebnf).

## Unterstützte Anweisungen

Lokale Deklaration, Zuweisung (einfach/zusammengesetzt), Funktionsaufruf, `Console.write(Line)`, `if`/`else`, `while`, `for`, `break`, `continue`, `return`, Block.

## Built-in-Funktionen

| Funktion | Signatur | Einschränkung |
|---|---|---|
| `Console.write` | `(string)` | genau ein Stringwert |
| `Console.writeLine` | `(string)` | genau ein Stringwert |

Weitere eingebaute Funktionen existieren nicht (`Console.readLine` nicht auflösbar).

## Diagnosecodes (beobachtet)

`NX0004 NX0100 NX1001 NX1003 NX1004 NX1005 NX1100 NX1102 NX1103 NX1107 NX1200 NX1201 NX1301 NX1302 NX2001 NX2002`. Siehe [Kapitel 11](11-errors-and-debugging.md).

## Relevante Einschränkungen

`intrinsic` ist ein kontextabhängiger Modifier ausschließlich für Dateien unter `standard-library/`; in Anwendungscode wird er mit `NX1005` abgelehnt. Ein Intrinsic-/Runtime-Symbolbinder existiert nicht.

Siehe [Feature-Übersicht](README.md#feature-übersicht-stand-der-implementierung). Zusätzlich: nur Windows x64; `main` muss `public int main()` ohne Parameter sein; Modulsystem ohne qualifizierte Namen und Sichtbarkeit (`using` mit flachem Namensraum); `long`-Locals mit Division können den Codegen abstürzen lassen.
