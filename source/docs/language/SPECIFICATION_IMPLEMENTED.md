# NORTEX Sprachspezifikation – Ist-Stand 0.3.7-dev

Beschreibt nur, was Compiler und `nortexc build` heute leisten. Entwurfsinhalte (Zielbild) stehen als „Geplant“ in den Kapiteln 03–15 der [Sprachreferenz](./README.md); Details in der [Sprachreferenz](./README.md). Nicht Umgesetztes ist als **Zukunft** markiert.

## 1. Toolchain

- Ziel: `x86_64-pc-windows-msvc` (einzig unterstützt).
- Pipeline: Quelltext → AST → HIR → MIR → Cranelift → Objektdatei → MSVC-Linker → EXE.
- Befehle: `nortexc check <datei>`, `nortexc build <datei|ordner|nortex.toml> [-o <exe>] [--overflow-checks=<true|false>]`.

## 2. Lexik

- Dateien `.nx`, UTF-8. Kommentare `//` und `/* … */`.
- Bezeichner nur ASCII (`[A-Za-z_][A-Za-z0-9_]*`); Unicode nur in Kommentaren und String-Literalen.
- PascalCase für Typen, camelCase für Funktionen und Werte sowie UPPER_SNAKE_CASE für `const` (Warnung NX0004).
- Es gibt **kein** `fn`, `let`, `i32`, `str`. Stattdessen: `public int name(...)`, `int x = …;`, `mut int x = …;`.

## 3. Typen

| Typ | Build-Status |
|---|---|
| `int` (32 Bit, vorzeichenbehaftet), `bool` | nativ |
| Globale `int`-/`bool`-Werte | nativ; compile-time Initialisierer, unveränderlich ohne `mut` |
| `string` | native Literale, lokale Variablen sowie interne Funktionsparameter/-rückgaben; `Console.write`/`writeLine` akzeptieren einen `string` |
| `double` | nativ im MIR-/COFF-Subset |
| `class` | nativ: Heap-Referenzen mit Arena-Lebensdauer bis zur normalen Rückkehr aus `main`, skalare/String/Klassenfelder, Konstruktoren, Methoden, Übergabe/Rückgabe und Zuweisung |
| Einzelvererbung, `virtual`/`override` | nativ; abgeflachte Feldlayouts, parameterloser Basiskonstruktor, Dispatch anhand dynamischer Klassen-ID |
| geschlossene generische Klassen und generische Instanzmethoden | native Monomorphisierung bei konkretem `new` bzw. expliziten konkreten Methoden-Typargumenten |
| `float`, `struct`, generische freie Funktionen | derzeit kein nativer Backendpfad |

Native Klassenobjekte sind aliasierbare Zeiger; die Runtime-Arena gibt sie bei normaler Rückkehr aus dem generierten `main` gemeinsam frei. Es gibt keine Einzelobjektfreigabe und keinen Tracing-GC, daher kann der Speicher bis dahin anwachsen. Trap/Abbruch umgeht den Shutdown; bei eingebetteter Nutzung muss der Host `nortex_object_arena_shutdown` erst aufrufen, wenn kein Objektzeiger mehr verwendet wird. Programme müssen vor `main`-Rückkehr alle Threads beenden, die Objektzeiger nutzen. Generische Methoden benötigen explizite Typargumente; implizite Inferenz und Bounds fehlen. Siehe [Speicherverwaltung](09-memory-management.md) und [Klassen](08-structs-and-data-structures.md).

Native Stringwerte sind unveränderliche UTF-8-Slices: Ein 16-Byte-Deskriptor enthält Datenzeiger und Byte-Länge; Literaldaten und Deskriptoren liegen schreibgeschützt im COFF-Datenbereich. Intern wird die Adresse des Deskriptors als 64-Bit-Wert übergeben. Heap-Strings entstehen durch `+` (`nortex_string_concat`, `HeapAlloc`, NUL-terminiert) und werden beim Scope-Ende/`return` per `nortex_string_free` freigegeben, sofern nicht verschoben (NX1202 bei Folgeverwendung). `==`/`!=` nutzen `nortex_string_eq`. Top-Level-`const`/`mut string` sind Zeiger auf Deskriptoren in `.rdata`; `extern "C"` erhält `string`-Argumente als `char*`. Der Linker baut `nortex_runtime.lib` neu, wenn Runtime-Quellen jünger sind.

## 4. Anweisungen

```nortex
using System;

public int add(int a, int b)
{
    return a + b;
}

public int main()
{
    mut int sum = 0;
    for (mut int i = 1; i <= 10; i = i + 1)
    {
        sum = sum + i;
    }
    if (sum == 55) { Console.writeLine("ok"); }
    return add(sum, 0) - 55;
}
```

- Einstiegspunkt: genau `public int main()`; der Rückgabewert ist der Exitcode.
- Bindungen sind unveränderlich, außer mit `mut`.
- Kontrollfluss: `if`/`else`, `while`, `for`, `return`; Aufrufe freier Funktionen.
- Globale Skalare können vor Funktionen stehen, z. B. `public const int MAX_PLAYERS = 64;` oder `mut int playerCount = 0;`. Nur Ganzzahl- und `bool`-Globals werden nativ ausgegeben.

## 5. Overflow und Traps

- `overflow_checks = true` (Manifest) bzw. `--overflow-checks=true`: `int`-Überlauf trapt, Exitcode `0xC000001D` (`-1073741795`).
- `false`: Wrap-around.
- Beispiel: [02_math_and_overflows](../../examples/language/02_math_and_overflows/main.nx).

## 6. Runtime

`Console.write`/`writeLine` und Objektallokation binden gegen `nortex_runtime.lib` (ABI-Entwurf: [Kapitel 12](12-compiler-architecture.md#abi-zielentwurf)). Suche: EXE-Ordner → Parent → `target\release` → `target\debug`; `NORTEX_RUNTIME_LIB` hat Vorrang. Objektallokationen werden bei normaler Rückkehr aus `main` gemeinsam freigegeben; es gibt keine Einzelobjektfreigabe oder GC.

## 7. Module und FFI

- **Module**: `using util;` lädt `util.nx` (oder `util\mod.nx`) relativ zur importierenden Datei bzw. zum Ordner der Einstiegsdatei; `using a.b;` → `a\b.nx`. Jede Datei wird einzeln geparst, die Deklarationen werden in Abhängigkeitsreihenfolge zu einem Programm zusammengeführt (flacher Namensraum, keine `util::`-Qualifizierung, keine Sichtbarkeitsprüfung zwischen Modulen). Zyklen werden als Fehler gemeldet, Diamant-Importe laden ein Modul einmal. Semantik-Diagnosen werden der Einstiegsdatei zugeordnet (Parser-Fehler tragen die richtige Datei). Beispiel: [03_multi_file_project](../../examples/language/03_multi_file_project/src/main.nx).
- **extern "C"**: `public extern "C" int GetTickCount();` deklariert eine Win32-/C-Funktion ohne Rumpf. Der Symbolname wird unverändert übernommen (keine Dekoration), Aufrufkonvention ist Windows x64. Bibliotheken kommen aus `[dependencies]` (`lib = "user32.lib"`). Nur `int`/`bool`-Parameter und -Rückgaben; Zeiger/Strings (z. B. `MessageBoxA`) sind **nicht** möglich. Beispiel: [04_win32_demo](../../examples/language/04_win32_demo/src/main.nx).

## 8. Manifest `nortex.toml`

```toml
[package]
name = "demo"             # Pflicht
version = "0.1.0"         # Standard
authors = ["Example Author"]
description = "optional"

[build]
main = "src/main.nx"      # Standard
output_name = "demo"      # Standard: package.name
target = "x86_64-pc-windows-msvc"
overflow_checks = true    # optional
opt_level = 0             # 0..3
linker_flags = []

[dependencies]
kernel32 = { lib = "kernel32.lib" }   # genau eines von path | lib
```

- Ausgabe: `<Manifest-Ordner>\build\<output_name>.exe`.
- `path` → `/LIBPATH:`, `lib` → Linker-Argument.
- `docs/build-system/manifest.md` beschreibt ein älteres Schema (`.nortex`, `[project]`); maßgeblich ist dieses.

## 9. Offene Lücken

Structs/Floats, Freigabe von Heap-Strings (Drop), generische freie Funktionen, implizite Typargument-Inferenz/Bounds, Klassenverwendung ohne konkrete Instanziierung, Mehrfachvererbung und Interfaces/Traits, qualifizierte Modulnamen und Sichtbarkeit zwischen Modulen, Zeiger-Parameter in `extern "C"`, Paketmanager, `nortexc fmt`, LSP.

## 10. Namens- und Layoutkonventionen

Diese Regeln werden teilweise mit `NX0004` geprüft. Es sind Warnungen, keine Syntaxfehler.

| Element | Stil | Beispiele |
|---|---|---|
| Klassen, Structs, Enums, Interfaces, Traits | PascalCase | `Player`, `XmlParser`, `IOStream` |
| Funktionen, Methoden, Variablen, Felder, Parameter | camelCase | `takeDamage`, `isActive`, `targetPosition` |
| `const`-Werte | UPPER_SNAKE_CASE | `MAX_PLAYERS`, `DEFAULT_TIMEOUT_MS` |
| Generische Parameter | PascalCase | `T`, `TKey`, `TValue` |
| Modulnamen und Dateinamen | snake_case (Empfehlung) | `math_utils`, `collision_detector.nx` |

- Typen, Funktionen und Werte dürfen keine Unterstriche enthalten, außer `const`-Namen.
- Zwei-Buchstaben-Akronyme bleiben groß (`IOStream`, `getID`); Akronyme ab drei Buchstaben werden wie normale Wörter geschrieben (`XmlParser`, `parseHttpHeader`).
- Interfaces und Traits erhalten kein vorgeschriebenes `I`-Präfix. Sichtbarkeit wird mit `public`, `private` usw. festgelegt; private Felder erhalten keinen Unterstrich.
- `extern "C"`-Funktionsnamen behalten ihre externe Schreibweise und erhalten keine camelCase-Warnung.
- Stil-Empfehlungen für `this` bei Feldzugriffen sowie Dateinamen werden nicht automatisch erzwungen.
- Formatierung: vier Leerzeichen, Allman-Klammern für Typen und Funktionen, K&R für Kontrollstrukturen. Ein Formatter und automatische Formatprüfung sind noch nicht implementiert.
