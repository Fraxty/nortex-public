# Parser

Der Parser verwendet Recursive Descent mit Pratt-Parsing für Ausdrücke. Die einzige normative Funktionsdeklarationsform ist C#-ähnlich.

## Im Prototyp geparst

- `using` und `namespace`
- Klassen, Structs, Records, Interfaces, Traits und Enums
- Felder, Methoden, Operatorüberladungsdeklarationen und einfache Konstruktoren
- Generische Typen, Parameterlisten und generische Konstruktoraufrufe
- Borrow-/Raw-Pointer-Typannotationen `&T`, `&mut T`, `*const T`, `*mut T`
- dynamische Arrays `T[]` und feste Arraytypen `[T; N]`
- `if`, `else`, `while`, C-artige `for`, `switch`, `match`, `return`, `throw`, `try`, `catch`, `finally`, `break` und `continue`
- `new`, `await`, Aufrufe, Member-/Indexzugriff, Präfix-/Postfixoperationen, zusammengesetzte Zuweisungen und Ternärausdrücke
- Character-Literale; Dezimal-/Hexzahlen, Exponenten und Suffixe auf Lexer-Ebene

## Noch nicht unterstützt oder unvollständig

`foreach`, Tuple-Ausdrücke/-typen, Lambdas, Attribute wie `[repr(C)]`, `extern "C"`, `unsafe {}`-Blöcke, vollständige Pointer-Ausdrücke, Properties, Events, Makros, Guards und vollständige Pattern-Unterstützung sind noch nicht verfügbar. Numeric suffix/range validation ist noch offen. Match-Ausdrücke sind syntaktisch teilweise vorhanden; Exhaustiveness ist nicht implementiert. Die genauen Grenzen stehen in [der Support-Matrix](syntax-support-matrix.md).
