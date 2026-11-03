# NORTEX Language Reference (0.3.7-dev)

Offizielle Sprachreferenz für NORTEX. Alle Aussagen und Codebeispiele wurden gegen den tatsächlichen Compiler (`nortexc` 0.3.7-dev, Windows x64) geprüft. Verifizierte Beispiele liegen in [`examples/language/`](../../examples/language/).

> **Kernaussage zum Reifegrad:** Der *Frontend*-Teil (Lexer, Parser, semantische Prüfung) akzeptiert eine breite C#-ähnliche Syntax. Der *native Backend-Pfad* (`nortexc build` → Windows-x64-EXE) unterstützt ein begrenztes, aber über skalare Werte hinausgehendes Subset einschließlich Heap-Klassen. Jedes Kapitel kennzeichnet deshalb getrennt, was nur `check` besteht und was wirklich als EXE läuft.

## Inhaltsverzeichnis

| Kapitel | Datei |
|---|---|
| 1 – Einführung | [01-introduction.md](01-introduction.md) |
| 2 – Installation und Einrichtung | [02-installation.md](02-installation.md) |
| 3 – Grundlagen der Syntax | [03-language-basics.md](03-language-basics.md) |
| 4 – Datentypen und Variablen | [04-data-types.md](04-data-types.md) |
| 5 – Operatoren | [05-operators.md](05-operators.md) |
| 6 – Kontrollstrukturen | [06-control-flow.md](06-control-flow.md) |
| 7 – Funktionen und Methoden | [07-functions-and-methods.md](07-functions-and-methods.md) |
| 8 – Structs und Datenstrukturen | [08-structs-and-data-structures.md](08-structs-and-data-structures.md) |
| 9 – Speicherverwaltung | [09-memory-management.md](09-memory-management.md) |
| 10 – Ein- und Ausgabe | [10-input-output.md](10-input-output.md) |
| 11 – Fehlermeldungen und Debugging | [11-errors-and-debugging.md](11-errors-and-debugging.md) |
| 12 – Compiler-Architektur | [12-compiler-architecture.md](12-compiler-architecture.md) |
| 13 – Praktische Beispiele | [13-examples.md](13-examples.md) |
| 14 – Sprachreferenz | [14-language-reference.md](14-language-reference.md) |
| 15 – FAQ und Best Practices | [15-faq-and-best-practices.md](15-faq-and-best-practices.md) |

## Feature-Übersicht (Stand der Implementierung)

Legende: **Nativ** = `nortexc build` erzeugt eine lauffähige EXE. **Frontend** = `nortexc check` akzeptiert, `build` lehnt ab (meist `NX2001`/`NX2002`).

### 1. Vollständig implementiert (nativ)

- Freie Funktionen: Parameter, Rückgabewerte, direkte Aufrufe, Rekursion, wechselseitige Rekursion; Modifier `public`, `private`, `static`.
- Skalare Typen `int`, `bool` (sowie `uint`, `short`, `ulong`, `long` als Parameter/Rückgabe/Locals, siehe Kapitel 4 für Grenzen).
- Variablen: unveränderlich per Default, `mut` für veränderlich, `const`.
- Operatoren: `+ - * / %`, Vergleiche, `&& || !` (kurzschließend), unäres `-`, `& | ^ << >>`, Zuweisung und `+= -= *= /= %=`, Ternär `?:`.
- Kontrollfluss: `if`/`else if`/`else`, `while`, `for`, `break`, `continue`, `return`.
- Kommentare `//` und `/* */`, Ganzzahlliterale dezimal/`0x`/`0b`.
- String-Literale, lokale Variablen sowie interne String-Parameter und -Rückgaben; `Console.write` / `Console.writeLine` mit **genau einem Stringwert**.
- Overflow-Prüfung (Trap) standardmäßig an, `--overflow-checks=false` für Wraparound.
- Projektmanifest `nortex.toml` und Ordner-Builds.
- Programmergebnis = Exitcode von `int main()`.

### 2. Teilweise implementiert

Native Klassen unterstützen heap-basierte Referenzen, Felder, Konstruktoren, Methoden, Objektübergabe/-rückgabe/-zuweisung, Einzelvererbung und `virtual`/`override`-Dispatch. Geschlossene generische Klassen und generische Instanzmethoden mit expliziten konkreten Typargumenten werden monomorphisiert. Die Runtime gibt die Objektarena bei normaler Rückkehr aus `main` frei; Einzelobjektfreigabe und Tracing-GC fehlen. Parameterisierte Basiskonstruktoren, Mehrfachvererbung, implizite Methoden-Typargument-Inferenz und Bounds sind nicht unterstützt. `struct`, `record`, `interface`, `trait`, `enum`; `namespace`/`using`; `var`; `string`/`char`/`float`-Variablen; Arrays `T[]`; `switch`; `match`; `try`/`catch`; `async`/`await`; übrige Generics; Postfix `++`/`--`, `~`; `foreach` sind nur teilweise oder frontendseitig implementiert.

### 3. Geplant, nicht implementiert

Objektfreigabe und Tracing-GC, `extern "C"`/FFI, `unsafe`-Blöcke, Lambdas, Attribute, Destruktoren, generische Instanziierung, Standardbibliothek-Anbindung (`.nx`-Entwürfe in `standard-library/`), Async-Zustandsmaschinen, Linux/ARM64-Ziele.

### 4. Nicht vorhanden

`fn`/`let`-Syntax (`let` ist reserviert und erzeugt `NX0100`), `do … while`, Cast-Syntax (`(int)x`, `x as int` werden nicht geparst), `null` in sicherem Code (`NX1102`), `mut`-Parameter, Konsoleneingabe (`Console.readLine` nicht auflösbar), String-Interpolation/Formatierung, `readonly` bei lokalen Variablen.

## Weiterführende Entwurfsdokumente (Zielzustand, **nicht** Implementierungsstand)

[Ist-Spezifikation](SPECIFICATION_IMPLEMENTED.md) · [EBNF](grammar.ebnf) · [Ownership](ownership.md) · [Offene Entscheidungen](15-faq-and-best-practices.md#offene-entscheidungen-projektinhaber)

Diese Dokumente beschreiben das Sprachziel. Bei Widersprüchen gilt diese Referenz in Verbindung mit [`BUILD_STATUS.md`](../../BUILD_STATUS.md).
