# Kapitel 12 – Kompilierung und technische Funktionsweise

← [Kapitel 11](11-errors-and-debugging.md) · [Inhalt](README.md) · Weiter: [Kapitel 13](13-examples.md)

## Pipeline

```text
.nx → Lexer → Parser/AST → semantische Prüfung → HIR → MIR
    → MIR-Validierung + Optimierung → Cranelift → Windows-x64-COFF (.obj)
    → MSVC-Linker (+ NORTEX-Runtime) → .exe
```

| Stufe | Quelle | Aufgabe |
|---|---|---|
| Lexer | `compiler/src/lexer/mod.rs` | ASCII-Bezeichner, Schlüsselwörter, Zahlen, Strings/Chars, Symbole, Kommentarspannen (für die IDE) |
| Parser/AST | `parser/mod.rs`, `ast/mod.rs` | Pratt-Parser für Ausdrücke, Deklarationen, Anweisungen; Fehler als `NX0100` |
| Semantik | `semantic/mod.rs`, `type_checker.rs` | Namen, Typen, Veränderlichkeit, Rückgabepfade, Move/Borrow-Grundprüfungen, Stilwarnungen |
| HIR | `hir/mod.rs` | aufgelöste, typisierte Zwischenform |
| MIR | `mir/lowering.rs`, `mir/mod.rs` | CFG mit Blöcken; skalares Subset; Definite-Assignment- und Loan-Prüfung; Diagnosen `NX2001`/`NX2002` für nicht unterstützte Konstrukte |
| Optimizer | `optimizer/mod.rs` | Optimierungen auf validiertem MIR |
| Codegen | `codegen/mod.rs` | Cranelift-IR → COFF-Objekt für `x86_64-pc-windows-msvc`; Overflow-Trap |
| Linker | `linker/mod.rs` | ruft MSVC `link.exe`, bindet CRT und Runtime; zusätzliche Argumente aus dem Manifest |
| Driver | `driver/mod.rs` | verbindet die Stufen |

## Kommandos

| Kommando | Wirkung |
|---|---|
| `nortexc check <datei>` | Lexer + Parser + Semantik |
| `nortexc mir <datei>` | gibt validiertes, optimiertes MIR aus |
| `nortexc build <datei\|ordner\|nortex.toml> [-o …] [--emit-obj] [--overflow-checks[=bool]]` | erzeugt `.exe` oder `.obj` |

## Eigenschaften des Backends

- Ziel: ausschließlich Windows x64 (MSVC). Kein Linux/ARM64.
- Einstieg: C-`main` der CRT; NORTEX-`int main()` liefert den Prozess-Exitcode.
- Stringwerte werden als Adresse eines 16-Byte-UTF-8-Slice-Deskriptors repräsentiert; `Console.write(Line)` lädt daraus Datenzeiger und Byte-Länge für den Runtime-ABI-Aufruf.
- Nicht unterstützte Konstrukte werden diagnostiziert, nicht still ignoriert (Ausnahme: im getesteten Fall `async`-Deklaration baut ohne Wirkung; `enum`/`class`-Deklarationen ohne Nutzung werden akzeptiert).

## Hinweis zu älteren Dokumenten

[`docs/compiler/architecture.md`](../compiler/architecture.md) beschreibt einen früheren Stand („natives Backend noch nicht fertig“). Der native Pfad für das skalare Subset ist inzwischen vorhanden und durch End-to-End-Tests belegt (`sum_to(10)` → Exitcode 55).

## ABI (Zielentwurf)

`extern "C"` markiert eine Funktion mit C-ABI; nur ABI-kompatible Parameter/Rückgaben sind zulässig. Structs sind nur mit `[repr(C)]` C-kompatibel; `string`, Klassen-Owner, Generics, Exceptions, `Task`/`Future` und beliebige `Result`/`Option`-Layouts haben keine automatische C-Kompatibilität. Strings gehen als UTF-8-Zeiger plus Byte-Länge; Speicher wird vom bereitstellenden Allocator freigegeben. Symbolnamen, Calling Convention und ABI-Hauptversion gehören zum Schnittstellenvertrag. Eine freigegebene Binärkompatibilitätsgarantie und ein FFI-Testlauf fehlen; Verbindliches steht in [Phase 4](../design/phase-4-native-runtime-abi.md).

## Paketformat `.ntx` (geplant)

`.nx` ist Quelltext, `nortex.toml` das TOML-Manifest, `.ntx` ein eigener deterministischer Binär-Container (kein ZIP, kein Quelltext). **Reader und Writer sind nicht implementiert.** Entwurf (Containerformat 1, little-endian): 48-Byte-Header mit Magic `NORTEXPK`, Major-/Minorversion, Flags (unbekannte Pflichtflags → Ablehnung), Offset/Länge des TOML-Manifests, Index-Offset, Eintragszahl und reservierten Nullbytes; Index je Datei mit Pfadlänge, Typ, Flags, Offsets, Größen und SHA-256, lexikografisch sortiert; 8-Byte-ausgerichtete Nutzdaten, keine Kompression. Validierung: nur relative UTF-8-Pfade mit `/` (kein `..`, keine Duplikate), alle Bereiche innerhalb der Datei, Größenlimits vor Allokation, Integerüberlauf-Prüfung, SHA-256-Prüfung (Schutz vor Beschädigung, keine Authentifizierung), optionale Signaturen (Verfahren offen), atomare Installation ohne Symlinks. Builds sollen byteidentisch reproduzierbar sein.
