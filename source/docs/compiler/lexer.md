# Lexer

Der Lexer verarbeitet NORTEX-Quelltext zeichenweise und erzeugt Tokens mit Byteoffset, Zeile und Spalte.

## Festgelegte lexikalische Regeln

- Case-sensitive Identifier
- ASCII-Identifier: `[A-Za-z_][A-Za-z0-9_]*`
- Unicode ist in Strings und Kommentaren zulässig, nicht in Identifiern
- `//`- und `/* ... */`-Kommentare
- Escape-Sequenzen `\\`, `\"`, `\n`, `\r`, `\t`, `\0`
- Zahlen mit optionalem `_` als Zifferntrenner und einfacher Dezimalpunkt-Notation
- längste Übereinstimmung für mehrstellige Operatoren

## Status

Implementiert sind Identifier, Zahlen, Stringliterale, Symbole, Kommentare, EOF und Positionsdiagnosen. Der Lexer unterscheidet Bezeichner exakt nach Groß-/Kleinschreibung. Vollständige numerische Literalformen (Exponenten, Suffixe, Hex/Binär/Oktal), Zeichenliterale, Raw-Strings und Interpolationsstrings sind noch nicht Bestandteil des Prototyps.
