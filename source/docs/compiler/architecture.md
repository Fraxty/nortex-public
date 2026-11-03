# Compilerarchitektur

Die verbindliche Zielpipeline und Toolchain sind in [Phase 5](../design/phase-5-compiler-build.md) als Zielentwurf spezifiziert; die tatsächliche Pipeline (AST → HIR → MIR → Optimizer → Cranelift → COFF → MSVC `link.exe`) beschreibt [../language/12-compiler-architecture.md](../language/12-compiler-architecture.md). Aussagen unten, der Compiler erzeuge noch keine native EXE, sind überholt.

## Pipeline

1. Projekt-/Quelltexteinlesen
2. Lexer
3. Parser und AST
4. Modul- und Namensauflösung
5. Typprüfung und Typinferenz
6. Ownership-/Borrow-/Definite-Assignment-Analyse
7. HIR
8. MIR
9. Datenflussanalyse und Optimierung
10. Target-Codegenerierung
11. COFF-Objektdateien und Linker
12. PE-EXE oder DLL samt Debug-/Unwind-Daten

## Implementierter Stand

- Lexer: ASCII-Bezeichner, Case-Sensitivity, Unicode in Strings/Kommentaren, Zahlen, Operatoren und Quelltextpositionen.
- Parser: Teilgrammatik für Imports, Namespace, Typ-/Memberdeklarationen, Generics, Funktionen, Variablen, Kontrollfluss, Exceptions und Ausdrücke.
- Semantic: erste Prüfung für doppelte Deklarationen, bekannte Typen, generische Arität, Naming-Konventionen, zulässige Operatorüberladungen und einen Teil der Mutabilitätsregeln.
- CLI: `nortexc check <datei.nx>` führt Lexer, Parser und die vorhandene Erstprüfung aus.
- Phase-2-Pipeline: `nortexc mir <datei.nx>` senkt Funktionen und Methoden des skalaren Subsets AST→HIR→MIR ab. Unterstützt sind Integer-/Bool-Locals (mutables ohne Initialwert werden durch Definite Assignment geprüft), lokale `&T`-/`&mut T`-Bindings mit skalaren Referenten, bedingte/kurzschließende Ausdrücke, Zuweisungen, `if`, `while`, klassische `for`-Schleifen, `break`, `continue` und `return`. Der Driver validiert MIR samt CFG-basierter Definite Assignment und konservativer Loan-Prüfung vor/nach Optimierung. Loan-Enden sind lexikalisch, Referenzrückgaben und komplexe Referenten werden abgewiesen.
- Nicht unterstützte Typen, Calls, weitere Statements und Ausdrücke werden beim Lowering ausdrücklich diagnostiziert; es gibt keinen erfolgreichen Fallback.

## Noch nicht fertig

Vollständige Typinferenz, Overload Resolution, Trait-Coherence, vollständige Block-/Move-Semantik, breites HIR-/MIR-Lowering für Calls, Felder, Exceptions und übrige Kontrollflüsse, natives Windows-Backend, COFF-Objekterzeugung, Linkeraufruf, Debugsymbole und vollständige LSP-Integration. `nortexc mir` erzeugt keine EXE; `nortexc check` bleibt eine reine Frontend-Prüfung.
