# NORTEX – Implementierungsfahrplan

> **Stand:** Meilensteine wurden mit 0.3.5-dev formuliert. Seit 0.3.7-dev ist u. a. M3 (native Windows-x64-EXE, Cranelift + MSVC-Linker) für einen Sprachsubset erreicht. Aktuelle Priorisierung und offene Punkte: [gap-analysis.md](gap-analysis.md); Ist-Stand: [BUILD_STATUS.md](../../BUILD_STATUS.md).

**Zweck:** Verbindliche Reihenfolge für die Umsetzung der beschlossenen Phasen 2 bis 9. Dieser Plan unterscheidet abgeschlossene Spezifikationsarbeit von noch ausstehender Implementierung.

## Freigabestufen

| Meilenstein | Ziel | Mindestnachweis |
|---|---|---|
| M0 – Sprachkern | Parser und semantischer Kern konsistent | Grammatiktests, AST-Tests, Positiv-/Negativtests |
| M1 – Typchecker | Statische Typprüfung und Inferenz | Negativtests für jeden unzulässigen Typvorgang |
| M2 – Ownership | Borrowing, Moves und Drops | CFG-/Dataflowtests, keine unsicheren sicheren Pfade |
| M3 – Native Hello World | Windows-x64-EXE | echte COFF-Datei, Linkerprotokoll, Laufzeittest |
| M4 – Runtime/Stdlib | Kern-APIs stabil | API-/Integrationstests, Fehler- und Ressourcentests |
| M5 – Async/Exceptions | Kontrollfluss korrekt | Unwind-, Cancellation-, Scheduling- und Panic-Tests |
| M6 – Toolchain/Packages | Reproduzierbare Projekte und `.ntx` | deterministischer Doppelbuild, Paketvalidierung und Signaturtest |
| M7 – Graphics | Erste D3D12-Anwendung | Device-/Swapchain-/Present-Test und Ressourcenvalidierung |
| M8 – IDE | produktiver Editor-Workflow | LSP- und UI-Integrationstests, Crash-/Recoverytests |
| M9 – Release | Sicherheits- und Qualitätsfreigabe | CI, Fuzzing, Windows-Releasematrix, Security-Review |

## Reihenfolge der Umsetzung

### M0 – Frontend abschließen

1. Grammatik und Parser auf eine einzige normative Syntax bringen.
2. Lexikalische Fehler, String-/Character-Escapes, Suffixe und Source-Spans vollständig testen.
3. Lexikalische Scopes, Shadowing-Regeln und Namespace-/Modulauflösung implementieren.
4. Konstruktoren, Tupel, Traits über die deklarierte Basistypliste, `unsafe`-Blöcke und vollständige Memberdeklarationen integrieren.
5. Diagnosecodes stabilisieren und Parser-Recovery testen.

**Abnahme:** Alle dokumentierten Grammatikproduktionen besitzen Parser-Positivtests; ungültige Formen liefern Diagnosen statt Panic oder stiller Tokenverwerfung.

### M1 – Typsystem und semantische Analyse

1. Kanonische Typ-IDs für Primitive, nominale Typen, Arrays, Tupel, Referenzen, Funktionen, `Option` und `Result` einführen.
2. Typinferenz mit Unification-Variablen und nachvollziehbaren Fehlerursachen implementieren.
3. Numerische Konvertierungen, Literalsuffixe, Überladungsauflösung und Operatorregeln exakt prüfen.
4. Generics mit Constraints und deterministischer Monomorphisierung implementieren.
5. `must_use`, `match`-Exhaustiveness, Rückgabepfade und definite assignment prüfen.
6. Trait-Coherence, Interface-Verträge und Sichtbarkeit über Module testen.

**Abnahme:** Jeder Typfehler aus der Spezifikation hat mindestens einen Negativtest; keine erfolgreiche `check`-Ausgabe bei ungelöster Typvariable.

### M2 – Ownership, Borrowing und MIR

1. Kontrollflussgraphen aus Funktionen erzeugen.
2. Move-/Initialisierungszustand je Basic Block verfolgen.
3. Shared-/Mutable-Borrow-Konflikte und Lifetimes prüfen.
4. Destruktor- und Cleanup-Punkte bei Scope-Ende, `return`, `break`, `continue` und Unwinding einfügen.
5. MIR-Verifikation vor jedem Optimierungs- oder Backend-Aufruf ausführen.

**Abnahme:** Use-after-move, Dangling Borrow, konkurrierende mutable Borrows und unvollständige Initialisierung werden in sicheren Programmen stets abgewiesen.

### M3 – Native Windows-x64-Toolchain

1. Vollständiges typisiertes HIR/MIR festschreiben.
2. NORTEX-MIR in die interne Maschinen-IR des eigenen Backends überführen.
3. COFF-Objektdateien für `x86_64-pc-windows-msvc` direkt aus dem NORTEX-Backend erzeugen.
4. Relocations, Calling Convention, Stack-Alignment, Shadow Space und Unwind-Daten prüfen.
5. Mit `lld-link` PE-EXE und DLL erzeugen.
6. PDB-/Debuginformationen und reproduzierbare Build-Metadaten ergänzen.

**Abnahme:** Ein minimales NORTEX-Programm wird tatsächlich in eine Windows-x64-EXE übersetzt, gestartet und sein erwarteter Rückgabewert/Output automatisiert geprüft.

### M4 – Runtime und Standardbibliothek

1. Allocator-Verträge, Fehlerpfade und deterministische Drop-Ausführung bereitstellen.
2. `String`, `List`, `Vec`, `Dictionary`, `HashSet`, `Queue` und `Stack` vervollständigen.
3. Datei-, Stream-, Pfad-, Prozess-, Zeit- und Diagnose-APIs implementieren.
4. C-ABI-Exports und FFI-Imports mit Layout-/Allocator-Vertrag testen.

**Abnahme:** Öffentliche APIs besitzen Dokumentation, Fehlerfälle, Ownership-Vertrag und Integrationstests.

### M5 – Exceptions und Async

1. NORTEX-Exceptionobjekte in Runtime und MIR abbilden.
2. Native Unwind-/Cleanup-Kanten samt `finally` und Drop testen.
3. Async-Funktionen in suspendierbare Zustandsmaschinen senken.
4. Continuations so schedulen, dass `await` keinen Worker blockiert.
5. Cancellation, Task-Gruppen, `Send`/`Sync` und I/O-Reactor integrieren.
6. Unter Windows asynchrones I/O über IOCP ergänzen.

**Abnahme:** Tests für Task-Suspension, Wakeups, Cancellation, Fehlerweitergabe, verschachteltes Unwinding und Shutdown bestehen ohne verlorene Tasks oder blockierte Worker.

### M6 – Projekt-/Pakettoolchain

1. Manifest-Schema und Versionsauflösung validieren.
2. CLI-Befehle `new`, `check`, `build`, `run`, `clean`, `test`, `package`, `format`, `doc` und `version` implementieren.
3. `.ntx`-Reader/Writer mit Pfad- und Größenlimits implementieren.
4. Hashes, optionale Signaturen und Dependency-Lockfile unterstützen.
5. Doppelbuilds unter kontrollierter Toolchain vergleichen.

**Abnahme:** Ein Projekt lässt sich ohne manuelle Sonderbefehle bauen und als validiertes `.ntx` reproduzierbar paketieren.

### M7 – Graphics

1. RHI-Verträge, Ressourcenlebensdauer und Synchronisation unabhängig vom Renderer definieren.
2. D3D12 Device, Queue, Allocator, Command List, Descriptor Heap, Swapchain und Fence implementieren.
3. HLSL über DXC kompilieren und Shader-/Pipeline-States verwalten.
4. Rendergraph-Ressourcenabhängigkeiten und Transitions validieren.
5. Buffer-, Texture-, Mesh-, Material-, Kamera- und Lichtsystem ergänzen.

**Abnahme:** Ein Fenster präsentiert über D3D12 stabil einen Frame; Resize, Device-Verlust, Ressourcenfreigabe und Debug-Layer werden getestet.

### M8 – NORTEX Code

1. UI-Framework anhand der protokollierten Prototypen egui/eframe, Slint und iced auswählen.
2. UI-unabhängigen Piece-Table-Textbuffer samt Änderungs-Undo/Redo, Auswahl und Zeilenindex bereitstellen.
3. Desktop-Shell, Projekt-/Dateiverwaltung und eigenes Editor-Widget erstellen.
4. Compilerbibliothek über einen getrennten LSP/JSON-RPC-Prozess anbinden.
5. Diagnosen, Completion, Navigation, Formatter, Build-Ausgabe und Terminal integrieren.
6. Debugger mit Windows-Debugschnittstellen und PDB-Symbolen anbinden.
7. Plugin-Prozess und minimale Berechtigungssandbox etablieren.

**Abnahme:** Öffnen, Bearbeiten, Prüfen, Bauen, Debuggen und Wiederherstellen nach einem LSP-/Plugin-Absturz funktionieren in automatisierten UI-Tests.

### M9 – Qualität und Veröffentlichung

1. CI unter Windows x64 einrichten; später ARM64/Linux/macOS erweitern.
2. Unit-, Integration-, Golden-, Property-, Fuzz- und Differentialtests automatisieren.
3. Sanitizer und Miri für geeignete Rust-Komponenten verwenden.
4. Abhängigkeiten, Paketformate, Signaturen und Plugin-Sandbox auditieren.
5. Performance- und Speicherprofile veröffentlichen.
6. Release-Kandidaten nur mit vollständiger Toolchain- und Regressionstestmatrix freigeben.

**Abnahme:** Keine kritischen Compiler-/Runtime-Fehler offen; reproduzierbarer Release-Build und dokumentierter Sicherheitsreview vorhanden.

## Aktuelle Position

Der aktuelle Stand ist ein Frontend- und Runtime-Prototyp. Die Rust-Runtime enthält einen Worker-Pool sowie `spawn` mit Waker-basierten Future-Fortsetzungen; `Pending` gibt einen Worker frei. Das ist noch kein vom NORTEX-Compiler erzeugter Async-Executor. Typchecker, Borrow-Checker, HIR/MIR-Lowering, natives COFF-Backend, Linker, Standardbibliothek-Implementierungen, D3D12-Backend und grafische IDE sind weiterhin offen. Die übrigen Meilensteine sind Entwicklungsziele, keine behaupteten Implementierungen. Prüfe den konkreten Status in [`../../BUILD_STATUS.md`](../../BUILD_STATUS.md).
