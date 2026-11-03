# NORTEX – Changelog

## Unreleased - Rich PDF export

- Neu: `document.export_pdf` erzeugt Unicode-PDFs (eingebettete TrueType-Schrift, `ToUnicode`). Text mit der ersten Zeile `%%nxpdf 1` wird als NXPDF-Markup gesetzt (#-Ueberschriften, Absaetze, **fett**, *kursiv*, Farben, `@pagebreak`, `@header`/`@footer` mit `{page}`/`{pages}`); anderer Text bleibt unveraendert (Monospace, Seitenzahl). Rechte, Pfad-, Groessen- und Seitenlimits unveraendert. Neue Abhaengigkeiten: `epaint_default_fonts`, `flate2`, `ttf-parser` (bereits transitiv im Lockfile). Siehe `docs/ide/plugins.md`.

## Unreleased – Native Klassen und Generics

- Nativer Windows-x64-Compiler: Heap-allozierte Klassenobjekte mit skalaren, String- und Klassenreferenzfeldern, Aliasierung, Methoden, Konstruktoren, Objektübergabe und -rückgabe.
- Einzelvererbung mit abgeflachten Feldlayouts und parameterlosem Basiskonstruktor; `virtual`/`override` dispatcht nach dynamischem Klassentyp.
- Geschlossene generische Klassen sowie generische Instanzmethoden mit expliziten konkreten Typargumenten werden monomorphisiert.
- Die Runtime-Arena gibt Objekte bei normaler Rückkehr aus `main` frei. Es gibt noch keine Einzelobjektfreigabe oder Tracing-GC; der Speicherbedarf kann während langer Programmläufe wachsen. Siehe `docs/language/08-structs-and-data-structures.md` und `docs/language/09-memory-management.md`.

## Unreleased – 25 NORTEX-Extensions, Plugin-API 1.1, lokaler Marketplace

- 25 Extensions in NORTEX selbst (`extensions/bundled` ×8, `extensions/available` ×17) auf Basis von `extensions/sdk/sdk.nx`.
- Marketplace-Tab zeigt den lokalen Katalog (Suche, Installieren) ohne Server-, Schlüssel- und Kontofelder.
- Plugin-API 1.1: `document.replace`, `file.read/write`, `notification.show`, `search.text`, `git.status`, `theme.current`.
- Compiler: `double`, `IntList`, `StringList`. Git-Panel mit Hunk-Staging und Konfliktauflösung, Diagnostics-Registry mit Quick Fix.

## Unreleased – Compiler-Job, parallele Erweiterungsläufe, IPC-Messung
- Neu: Nativer Compiler unterstützt `double` (Literale, Arithmetik, Vergleiche, `Convert.toDouble/doubleToInt/doubleToString/parseDouble/isDouble`, `Math.sqrt/abs/floor/pow`) sowie `IntList`/`StringList` (Handles, `create/add/get/set/length/free`) und `Text.split/join`. Zu diesem damaligen Entwicklungsstand waren Klassen im nativen Backend noch nicht unterstützt. Siehe `docs/language/builtins.md`.
- Neu: Eingebaute Text-, Konvertierungs- und Dateifunktionen im nativen Compiler (`Text.*`, `Convert.*`, `File.*`, `Directory.*`, `Console.readLine`). Tabelle in `compiler/src/builtins.rs`, Runtime in `runtime/src/text.rs`; Strings zählen UTF-8-Bytes. Siehe `docs/language/builtins.md`. `Console.write/writeLine` verbrauchen ihr Argument nicht mehr.
- Neu: Feedback- und Supportsystem im Hilfe-Menü (`ide/src/feedback/`, `ide/src/shell/feedback.rs`) mit Formular, ausfüllbarem 4-seitigem AcroForm-PDF, `mailto:`-Versand (Anhang manuell), Secret-Erkennung und ausdrücklichem lokalem Entwurf; DE/EN. Kontaktadresse in `CONTACT_EMAIL` konfiguriert. Siehe `docs/ide/feedback.md`.

- `nortexc` läuft beim Bauen von Erweiterungen in einem Windows-Job-Object (Prozess-, Speicher-, CPU-Zeit-Limit, Timeout beendet den ganzen Prozessbaum, Ausgabe-Limit) mit Low-Integrity-Token; Start suspendiert, erst Job-Zuweisung, dann Resume. Schreiben nur im Workspace; Lesen und Netzwerk bleiben unbeschränkt.
- Completion läuft auf einem Worker-Thread (`editor/completion_worker.rs`); der Analyse-Cache hält zwei Einträge und trifft jetzt bei unverändertem Text (10 000 Zeilen: 31,4 → 10,8 ms). Tippen ist nicht schneller. Messungen: `docs/ide/benchmarks.md`.
- 50 fehlende englische Übersetzungen ergänzt; zwei Tests erzwingen vollständige Einträge samt Platzhaltern.
- Plugin-IPC-Messung mit und ohne AppContainer, inkl. CPU/Speicher der Plugin-Prozesse.
- Einstellung `extensions.max_parallel_runs` (1–4, Standard 2); Sandbox-Läufe sind einzeln abbrechbar und haben je Erweiterung einen eigenen Status.
- Messung des Plugin-IPC mit echten Kindprozessen (`ide/tests/plugin_ipc_bench.rs`, Ergebnisse in `docs/ide/benchmarks.md`).

## 0.1.0 – Public Preview

- Versionsstand auf 0.1.0 (Public Preview) zurückgesetzt; MIT-Lizenz und vollständiger Quellcode im öffentlichen Repository.
- Zweisprachige IDE und Compiler-Meldungen, Handbuch und Sprachreferenz als PDF (DE/EN), Installer (pro Benutzer) und portable ZIP, SBOM, Security-/Release-Dokumente.
- Sicherheit: absoluter Pfad für cmd.exe im Linker; Release-Build ohne lokale Pfade (scripts/build_release_clean.ps1).
- Die folgenden Einträge sind die historische Entwicklungsfolge (frühere Versionsnummern).
- IDE: Plugin-Host mit versionierter IPC-API (`ide/src/plugins/{protocol,host}.rs`), interaktive Sandbox-Prozesse (`spawn_sandboxed`), Manifestfelder `ide_api`/`ide_permissions`, Statusliste in Extensions → Installiert. IPC-Tests mit echtem Kindprozess in `ide/tests/plugin_ipc.rs`.
- IDE: IntelliSense mit Importgraph und `TypeEnv`; Benchmark `ide/tests/intellisense_bench.rs`, Ergebnisse in `docs/ide/benchmarks.md`.

## 0.3.7-dev – Technische Grundlage

- Dokumentation konsolidiert: neuer Index `docs/README.md`, `docs/architecture.md`, `docs/getting-started/`, `docs/development/` (Handbuch, Roadmap, Lückenanalyse, Versionsregeln), `docs/security/`; Phasen 3–8 und Sprach-Entwurfskapitel nach `docs/design/` verschoben (als Zielentwurf gekennzeichnet); historische Audit-/Remediation-/Statusberichte, `reports/`, `PROJECT_REFERENCE.md` und `*-pending.md`-Platzhalter entfernt; `nortex.toml`-Manifestdokumentation an den Code angepasst; `scripts/check_md_links.ps1` prüft relative Markdown-Links.
- `Cargo.lock` mit Cargo 1.88 einschließlich der IDE-Abhängigkeiten neu erzeugt.
- MIR-Datenstrukturen verwenden für `f64`-haltige Werte `PartialEq` statt des nicht möglichen `Eq`; der Optimizer verwendet den tatsächlich vorhandenen `MirBody`-Typ.
- Edition-2024-Fehler der Windows-Runtime, CLI-Argumenttypen und Clippy-Verstöße in Compiler/Runtime/CLI behoben.
- Noch nicht implementierte CLI-Befehle (`test`, `fmt`, `lint`, `doc`, `package`, `install`, `publish`) melden nun einen Fehler und liefern keinen falschen Erfolgsstatus.
- Rust-Dateien mit rustfmt formatiert.
- CI prüft die gemeinsame Workspace-Version gegen alle Workspace-Manifeste und `Cargo.lock`.
- Der vollständige Build und die Tests sind lokal noch durch den fehlenden MSVC-Linker blockiert; Erfolg wird erst nach erfolgreicher CI bestätigt.

## 0.3.6-dev – IDE-Core-Korrekturen

### Editor und Dokumentdateien

- UTF-8-Löschbereiche runden den Anfang ab und das Ende auf; leere Bereiche bleiben unverändert.
- Piece-Quellbereiche werden beim Materialisieren und Slicen nicht mehr stillschweigend übersprungen, sondern melden verletzte interne Invarianten.
- Präfixoffset-Index beschleunigt Piece-Lookups und Bereichsslices per Binärsuche.
- Original- und Additionspuffer werden bei übermäßiger Fragmentierung bzw. übergroßem Backing-Storage kompakt zusammengeführt.
- Undo/Redo ist zusätzlich zu 1.024 Einträgen auf 32 MiB Edit-Text begrenzt.
- Graphemnavigation arbeitet auf einem lokalen Fenster und erweitert es bei über den Rand reichenden Sequenzen. Die vollständige UAX-#29-Konformität bleibt offen.
- LF, CRLF und CR werden als Zeilenenden erkannt; getrennt eingefügtes CR+LF wird beim Zeilenindex als ein CRLF behandelt.
- Speichern schreibt Pieces direkt in die Temp-Datei. Unter Unix werden Zielrechte bereits beim Anlegen angewendet; Datei und Verzeichnis werden synchronisiert.
- `open_lossy()` ergänzt einen ausdrücklich bestätigungspflichtigen UTF-8-Fallback; Binärdateien mit Nullbytes im Prüfblock werden abgelehnt.
- `HighlightCache` verhindert erneutes Lexing beim Neuzeichnen derselben Dokumentrevision und bietet Viewport-Filterung. Bei Änderungen wird weiterhin vollständig neu lexikalisch analysiert.

### Bekannte Grenzen

- Piece-Liste und Zeilenindex sind weiterhin flache Vektoren; Edit- und Zeilenindex-Updates können linear sein. Ein Piece-Tree/Rope und die Leistungsabnahme mit 100.000 Edits in 10 MB fehlen.
- Die UI-Framework-Evaluierung und das grafische Fenster sind nicht Teil dieser Korrekturrunde.
- Rust-Build, Tests, rustfmt und Clippy wurden mangels Toolchain nicht ausgeführt.

## 0.3.5-dev – IDE-Core, Dokumente und Projektmanifest

- EditorBuffer ohne vollständigen Text-Cache pro Änderung; Änderungs-Undo/Redo, Gruppierung und begrenzte Historie.
- Cursor- und Auswahlfunktionen, UTF-8-sichere Bereiche, inkrementeller Zeilenindex sowie Zeilen-/Spaltenumrechnung.
- Dokumentdateien mit UTF-8-BOM- und LF-/CRLF-Erhalt sowie temporärem Schreib-/Ersetzungsverfahren.
- NORTEX-Hervorhebung nutzt Compiler-Lexer-Tokens und zusätzlich vom Lexer bereitgestellte Kommentarspannen.
- `.nortex`-Kernschema wird gelesen; Einstiegspfad wird gegen Verlassen des Projektverzeichnisses geprüft.
- IDE-Dokumenttabs und Diagnoseausschnitte als UI-unabhängige Funktionen.
- UI-Framework bleibt bis zum dokumentierten Prototypvergleich offen; keine grafische IDE behauptet.
- Rust-Build, Formatierung und Tests in dieser Arbeitsumgebung nicht ausgeführt (Toolchain nicht installiert).

## 0.3.4-dev – Typechecker, foreach, Runtime-Fairness und Audit-Härtung

- Die Intrinsic-Berechtigung ist nicht mehr über einen öffentlichen booleschen Compiler-Optionsparameter aktivierbar. Nur kanonisch innerhalb des compilerverwalteten `standard-library/`-Verzeichnisses liegende Dateien erhalten diese Berechtigung.
- Ein Regressionstest stellt sicher, dass ein vom Benutzer erzeugtes Verzeichnis namens `standard-library` keine Intrinsic-Rechte gewährt.
- Die Runtime fängt nun auch einen Future-Destruktor ab, wenn ein während `poll()` erfolgter Shutdown die lokale Future verwirft.
- Die bisherigen Änderungen aus 0.3.3-dev bleiben enthalten: FIFO-Fairness, Task-Zustandsautomat, iterativer Shutdown, schwache Spawner-Handles, erste Ausdruckstypprüfung und `foreach`.
- Testquellen: 72 Frontend-Integrationstests, 9 Phase-2-Typentests, 15 Async-Runtime-Tests und 6 Threadpool-Tests. Diese Testdefinitionen sind in dieser Umgebung nicht ausgeführt.
- Rust-Toolchain (cargo/rustc/rustfmt/Clippy) fehlt weiterhin; Build, Formatierung, Tests und Clippy sind nicht verifiziert.

## 0.3.3-dev – Typprüfung, foreach und Runtime-Fairness

### Compiler / Frontend

- Einen ersten eigenständigen Ausdrucks-Typchecker ergänzt: Variableninitialisierungen,
  Rückgaben, Bedingungen, Operatoren, Aufrufe, Konstruktoraufrufe, Member und
  Indexzugriffe erhalten eine begrenzte Typprüfung.
- Aufrufauflösung prüft Argumentanzahl und exakte Parametertypen für bekannte
  Funktionen, Methoden und Konstruktoren. Implizite numerische Konvertierungen
  werden weiterhin nicht angenommen.
- Unbekannte Member auf bekannten Klassen und primitiven Typen erzeugen
  `NX1106`; fehlende oder unpassende Überladungen erzeugen `NX1107`.
- Lokale Variablentypen und `foreach`-Elementtypen werden auf bekannte Typnamen
  geprüft; unbekannte Typen erhalten `NX1100`.
- `for (T value in sequence)` ist als `ForEach` im AST abgebildet.
  `string.chars()` und `string.bytes()` liefern im Frontend `Iterator<char>`
  beziehungsweise `Iterator<byte>`.
- Die bisherige Clippy-`collapsible_if`-Stelle in der Mutationsprüfung wurde
  zusammengeführt. Return-Typfehler verwenden jetzt den einheitlichen Code `NX1102`.
- `intrinsic` bleibt ein kontextabhängiger Modifier und ist außerhalb seiner
  Modifier-Position ein normaler Bezeichner.
- K7- und Typchecker-Regressionstests ergänzt; Beispiele für Grafik und Async
  so angepasst, dass sie keine Intrinsic-Deklarationen im Anwendungscode benötigen.

### Runtime

- Die bestehende Scheduling-Policy ist ausdrücklich FIFO: externe Jobs,
  Worker-Spawn und Waker-Fortsetzungen werden hinten eingereiht. Es gibt keinen
  unbegrenzten LIFO-Vorrang mehr.
- Die Runtime-Dokumentation beschreibt den Task-Zustandsautomaten, den weak
  Spawner-Vertrag, die iterative Shutdown-Behandlung und detached Worker.
- Runtime-Regressionstests prüfen zusätzlich zwei gleichzeitig yieldende Tasks,
  FIFO-Reihenfolge für aus einem Worker gestartete Kinder und das Ende der
  Lebensdauer eines detached Workers.

### Verifikation

- Statische TOML-, Lockfile-, Markdown-Link-, Projektstruktur- und Quelltextprüfungen
  wurden ausgeführt.
- Rust-Build, Cargo-Tests, rustfmt und Clippy konnten in der Arbeitsumgebung nicht
  ausgeführt werden, weil `cargo`, `rustc`, `rustfmt` und `clippy-driver` fehlen.
  Diese Ergebnisse sind ausdrücklich nicht als bestanden markiert.
- `Cargo.toml` und `Cargo.lock` tragen die eindeutige Arbeitsversion `0.3.3-dev`.
  Die Lockfile-Version wurde wegen der fehlenden Toolchain nur auf die fünf
  Workspace-Pakete synchronisiert, nicht durch Cargo neu erzeugt.

## 0.3.2-dev – Runtime-/Audit-Korrekturrunde

### Async-Executor und Threadpool

- Worker-Hilfsausführung auf dem wartenden Task-Stack vollständig entfernt.
- Futures werden über Waker-Fortsetzungen gepollt; `Pending` gibt den Worker frei.
- Task-Fortsetzungen werden vor gewöhnlichen FIFO-Jobs priorisiert, um künstliche Prioritätsumkehr zu reduzieren.
- `block_on` wartet mit `park`/`unpark` statt 1-ms-Polling und weist verschachtelte Aufrufe innerhalb eines Workers ausdrücklich zurück.
- Die künstliche Verschachtelungsgrenze von 64 Ebenen entfällt; tiefe Task-Ketten laufen über Fortsetzungen statt rekursiver Worker-Aufrufe.
- Task-Panics werden einmalig als `TaskError::Panicked` gespeichert; es gibt keine rekursive Panic-String-Verpackung durch den Scheduler.
- Shutdown cancelt eingereihte und registrierte suspendierte Tasks mit `TaskError::ExecutorShutdown`. Normales Drop joint keine laufenden beliebigen Jobs; `shutdown_and_join` ist die ausdrücklich blockierende Variante.
- Regressionstests für Task-Verschachtelung, Fortsetzungspriorität, Shutdown, erneutes Polling und fehlendes periodisches Polling ergänzt.

### Compiler und Diagnosen

- Konkrete Klassenmethoden ohne Implementierungsblock werden mit `NX1005` abgelehnt. Zulässige Deklarationsformen sind Interface-/Trait-Verträge sowie `abstract`, `extern` und `intrinsic`.
- `intrinsic` ist als expliziter Deklarationsmodifier ergänzt. Die tatsächliche Verknüpfung mit Runtime-Symbolen ist weiterhin offen.
- Doppelte Methodensignaturen verwenden `NX1001` statt des Parsercodes `NX0100`.
- Vier verschachtelte Borrow-/Mutationsprüfungen wurden zur Vermeidung von Clippy-`collapsible_if`-Warnungen vereinfacht.
- `Waker::noop()` wird in dem Task-Re-Poll-Test statt eines selbstgebauten No-op-Wakers genutzt.
- Versionskennung auf `0.3.2-dev` angehoben.

### Verifikation 0.3.2-dev

- Quellcode- und Dokumentationsänderungen wurden statisch gegengeprüft.
- `cargo`, `rustc`, `rustfmt` und Clippy sind in der aktuellen Arbeitsumgebung nicht verfügbar; Build, Tests, Formatcheck und Clippy sind deshalb nicht ausgeführt und nicht als bestanden gewertet.
- Die CI-Matrix enthält weiterhin `ubuntu-latest` und `windows-latest`; ein erfolgreicher CI-Lauf steht aus.

## 0.3.1-dev – Audit-Überarbeitung

### Zweite Audit-Korrekturrunde

- IDE-Diagnoseadapter korrigiert: Span-Informationen werden vor dem Verschieben der Meldungsfelder gelesen.
- Basistyplisten konsumieren den Doppelpunkt; Konstruktorinitialisierung `this.field = parameter` trennt Feld- und Parameterzustand.
- `get` und `set` sind normale Bezeichner; `let` ist reserviert, aber keine unterstützte Deklarationsform.
- Async-Rückgaben von `Task<T>` werden bei einfachen Literalen gegen `T` geprüft.
- Threadpool auf Queue-Verteilung und poolbezogene Hilfsausführung beim Warten umgestellt; erneutes Polling eines abgeschlossenen Tasks wird als `AlreadyCompleted` gemeldet.
- Regressionstests für die genannten Fehler ergänzt; Ausführung bleibt mangels Rust-Toolchain unbestätigt.

- Parser-/Lexer-Prototyp um Basistyplisten, Operatorpräzedenz, Konstruktor-Grundformen, generische Konstruktion, lokale Arraydeklarationen, Zeichenliterale und Ternärausdrücke erweitert.
- Erste blockbasierte Namensauflösung sowie begrenzte Prüfungen primitiver Literal-Kategorien ergänzt.
- Diagnosecodes für offensichtliche primitive Typfehler auf `NX1102` und `NX1103` gelegt; `NX1200`/`NX1201` bleiben dem Bereich Ownership/Borrowing vorbehalten.
- Worker-Jobs werden immer über eine gemeinsame Queue verteilt; `block_on` kann während eines blockierenden Worker-Wartens poolbezogen Hilfsarbeit ausführen. Job-Panics werden gemeldet und abgefangen, die Hilfsausführung ist tiefenbegrenzt, und ein zweites Polling eines konsumierten Tasks liefert `AlreadyCompleted`.
- Runtime als `rlib`, `staticlib` und `cdylib` konfiguriert und um Bootstrap-Symbole für ABI-Version, Initialisierung und Shutdown ergänzt.
- `graphics/` und `tools/` als Workspace-Crates ergänzt; CI-Workflow für Windows und Linux angelegt.
- CLI-Diagnosebezeichnungen vereinheitlicht und auf Deutsch gestellt.
- ARM64-Windows-Aufrufkonvention und Unwind-Anforderungen in Phase 4 dokumentiert.
- Dokumentationsversion, Cargo-Paketversion und Auditstand auf `0.3.1-dev` vereinheitlicht.
- Offene Grundsatzentscheidungen (insbesondere Mutabilität, Lizenz, Exceptions, Release-Arithmetik und Methodenreceiver) ausdrücklich als freigabepflichtig markiert.

## 0.3.0 – Sprach- und Architekturentwurf

- Sprachsyntax, Typmodell, Ownership-Ziele und Module für die Phasen 2 bis 9 beschrieben.
- Eigenes NORTEX-Backend als Zielarchitektur und `lld-link`/`link.exe` als externe Windows-Linker vorgesehen.
- `.nortex` als TOML-Projektmanifest und `.ntx` als deterministisches, eigenes Binärpaketformat spezifiziert.
- Runtime-, Standardbibliotheks-, Grafik-, IDE- und Sicherheitsarchitektur dokumentiert.

## Bekannte Grenzen 0.3.0

Diese Versionen sind Entwicklungsstände und keine produktiven Sprachreleases. HIR/MIR-Lowering, native Codegenerierung, Linkerpfad, vollständiger Typ-/Borrow-Checker, asynchroner Executor, vollständige Standardbibliothek, D3D12-Geräte-/Swapchain-Pfad, LSP und Plugin-Sandbox sind nicht vollständig implementiert. `cargo build`, `cargo test`, `cargo fmt` und Clippy sind für die Audit-Überarbeitung in der aktuellen Arbeitsumgebung nicht ausführbar gewesen; Ergebnisse werden daher nicht als bestanden dargestellt.

- CI und `scripts/verify.ps1` auf vollständige Workspace-Targets und Lockfile-Prüfung (`--locked`) abgestimmt.
