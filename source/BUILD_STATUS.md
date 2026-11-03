# NORTEX – Build- und Implementierungsstatus

**Repository-Stand:** `0.3.7-dev`
**Auditgrundlage:** Prüfbericht „0.3.2-dev Audit3 Runtime-Korrekturen“ sowie die vorherigen NORTEX-Audits.

## 0.3.7-dev – Technische Grundlage

- **Stand 2026-11-03 (Compiler-Job, asynchrone Completion):** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets` und `cargo test --workspace` laufen unter Windows ohne Fehler und Warnungen. Windows-Smoke-Tests (`confine`, `sandbox`, `plugin_ipc`, Bins; mit `--include-ignored --test-threads=1`) bestanden. Linux/macOS: Plugin- und Compiler-Sandbox fail-closed, **nicht verifiziert** (kein Linux-Toolchain für die C-Abhängigkeiten). Das Repository hat keinen Baseline-Commit, ein Diff ist nicht möglich. Compiler-Confinement: Lesen und Netzwerk unbeschränkt. Die Completion läuft asynchron; die Analysekosten beim Tippen sind unverändert (siehe `docs/ide/benchmarks.md`). Die UI wurde nicht manuell geprüft.
- Cargo hat das Lockfile mit Rust/Cargo 1.88 für alle fünf Workspace-Pakete und die IDE-Abhängigkeiten neu erzeugt; der Versionsabgleich gegen Manifeste und Lockfile ist automatisiert.
- `scripts/verify.ps1` lief nach den aktuellen Ownership-/Regions- und MIR-Lowering-Änderungen unter Windows/MSVC vollständig erfolgreich: Formatierung, Versionsabgleich, Workspace-Check, Build, Workspace-Tests und Clippy mit `-D warnings`. Die Compiler-Suites bestanden mit 25 Bibliotheks-, 74 Frontend-, 1 Ownership-Fixture-, 10 Typ-, 15 Pipeline- und 34 Typprüfungs-Regressionstests; IDE und Runtime ließen zusammen drei bestehende Tests explizit ignoriert.
- Aktueller Gesamtstand (Audit 2026-11-02): `cargo fmt --all --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` und `cargo test --workspace` sind grün: **337 Tests bestanden, 0 fehlgeschlagen, 3 ignoriert** (davon 104 in der IDE inkl. 7 Edge-Case-Tests in `ide/tests/edge_cases.rs`; Compiler-CLI-Tests in `compiler/tests/cli_build.rs`).
- Branding/IDE: JPG-Vorlagen in `assets/source/` → `python scripts/generate_brand_assets.py` (benötigt pillow, numpy, potracer) erzeugt `assets/branding`, `assets/icons/app` (SVG/PNG/ICO 16–256), `assets/icons/files` (SVG) und `assets/icons/runtime` (PNG, per `include_bytes!`). Build-Time: `.ico` via `winres` (`ide/build.rs`); Laufzeit: Fenstericon über `eframe::NativeOptions`. Activity Bar (Explorer/Search/Git/Debugger/Settings), Dashboard und Settings-Ansicht sind angebunden; Debugger ist **nicht implementiert**, Git-Panel ist nur Statusanzeige (benötigt installiertes `git`). Sichtprüfung per Screenshot unter Windows durchgeführt. Offene Punkte und Priorisierung: [docs/development/gap-analysis.md](docs/development/gap-analysis.md).
- **Suchen/Ersetzen (Strg+F/Strg+H):** `ide/src/editor/find_replace.rs` (Kern: Case/Whole-Word/RegEx, `$1`/`${name}`, Ersetzen einzeln/alle, ungültige RegEx → `FindError`, kein Panic) und `ide/src/ui/find_bar.rs` (Overlay oben rechts, Counter, Amber-Treffer, Auto-Scroll). Tests: `ide/tests/find_replace.rs` (12). Per Screenshot geprüft: Öffnen, Counter, Treffer-Highlight, Ersetzen-Zeile, roter Rand bei ungültiger RegEx. **Nicht** manuell geprüft: Esc-Fokusrückgabe, Ersetzen-Buttons, Scrollen in langen Dateien. Neue Abhängigkeit: `regex`.
- **Projekt-Manifest `nortex.toml`:** `compiler/src/manifest.rs` (`[package]`, `[build]` mit `main`/`output_name`/`target`/`overflow_checks`/`opt_level`/`linker_flags`, `[dependencies]` mit `path` oder `lib`; Defaults, Validierung, nur Target `x86_64-pc-windows-msvc`). `nortexc build|check <Ordner|nortex.toml>` löst `main` relativ zum Manifest auf, schreibt `build/<output_name>` und reicht `/LIBPATH`, Libs und `linker_flags` an den MSVC-Linker. Imports anderer `.nx`-Dateien (`using util;`) werden vom Resolver aufgelöst (`compiler/src/semantic/resolver.rs`). IDE: Statusleiste/Fenstertitel zeigen Projekt und Ausgabe; Build&Run übergibt das Manifest. Tests: `compiler/tests/manifest_test.rs` (5); End-to-End `nortexc build <Ordner>` manuell geprüft (Exitcode 7). Die IDE-Manifest-Anzeige ist nicht per Screenshot geprüft. Neue Abhängigkeiten: `toml`, `serde`.
- **Split-Editor (Stand 2026-11-03):** `ide/src/editor/split.rs` (`SplitMode` Single/TwoColumns/TwoRows/FourGrid, `PaneGrid`, je Pane eigene Tabs/Cursor/Scroll, Mindestbreite 250 px, Tab-Drag&Drop mit Edge-/Center-Drop, leere Panes schließen sich), `ide/src/ui/keymap.rs` (VS-Code-Keymap), `ide/src/editor/lines.rs` (Kommentieren, Verschieben, Duplizieren). Menü Ansicht → Editor-Layout und Command Palette bieten die Split-Aktionen. Ein Dokument existiert je Datei nur einmal (Zustand über Panes synchron); nur das fokussierte Pane ist editierbar, inaktive Panes zeigen eine schreibgeschützte Vorschau. Tests: `ide/tests/split_keymap.rs` (17), `ide/tests/terminal.rs` (3), `ide/tests/inline_diagnostics.rs` (13). `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` und `cargo test --workspace` grün. Manuell per Screenshot geprüft: Zwei-Spalten-Split über das Menü. **Nicht** manuell geprüft: Tab-Drag&Drop, Splitter-Ziehen, Tastenkürzel im Fenster. Nicht implementiert: Debugger/F5, Stop, Edge-Split in bereits geteiltem Layout (wirkt wie Verschieben), eigener Build ohne Run (Strg+Shift+B startet Build & Run), echtes PTY im Terminal.
- **Ordnerstruktur:** `compiler/` (nortexc), `runtime/` (nortex_runtime.lib), `ide/` (nortex-code), `assets/` (Icons, Branding, `assets/source/` = JPG-Vorlagen), `scripts/` (Build/Asset/Packaging), `dist/` (nur ZIP-Releases); dazu `graphics/`, `tools/`, `standard-library/`, `examples/`, `tests/`, `docs/` als Workspace-/Inhaltsordner. `build/` und `bin/` sind generiert (nicht im ZIP). `bin\nortex-code.exe` entsteht durch `run_nortex_code.ps1`, `build_app.ps1` und `scripts\build_release.ps1`. Asset-Pfade: `ide/build.rs` (winres, `../assets/icons/app/nortex-code.ico`) und `ide/src/branding.rs` (`include_bytes!` auf `assets/icons/...`) wurden geprüft und kompilieren.
- Linux wurde auf diesem Windows-Host für `x86_64-unknown-linux-gnu` erfolgreich mit `cargo check --workspace --all-targets --locked` und Clippy mit `-D warnings` cross-kompiliert. Linux-Tests konnten hier nicht ausgeführt werden; die konfigurierte Ubuntu-CI bleibt der native Laufzeitnachweis.
- Die IDE aktiviert X11- und Wayland-Unterstützung explizit. Die Windows-spezifische Runtime ist auf Windows beschränkt; auf Nicht-Windows-Zielen meldet der Initialisierungseinstieg fehlende native Panic-TLS-Unterstützung ausdrücklich.
- Move-Zustände des Typprüfers sind an lokale Bindungs-IDs statt bloße Namen gebunden; gleichnamige Variablen in getrennten Blocks beeinflussen sich nicht mehr.
- Der MIR-Optimizer faltet lokale Integer-/Bool-Konstanten und konstante `SwitchInt`-Terminatoren und entfernt unerreichbare Blöcke sowie tote, nebenwirkungsfreie Assignments. Moves, Borrows, potenziell trap-auslösende Operationen und CFGs mit ungültigen Kanten bleiben erhalten.
- Phase 2 hat begonnen: Die AST→HIR→MIR-Pipeline unterstützt ein begrenztes skalares Subset mit Integer-/Bool-Locals, Zuweisungen, geschachtelten bedingten und kurzschließenden Ausdrücken, `if`, `while`, klassischem `for`, `break`, `continue` und `return`. Mutable uninitialisierte Locals werden durch CFG-basierte Definite Assignment geprüft; konstante Switch-Kanten gelten bei Reachability und Datenfluss als exklusiv. Der Driver validiert MIR vor und nach Optimierung; `nortexc mir <datei.nx>` macht den Pfad nutzbar. Nicht unterstützte Konstrukte werden explizit diagnostiziert.
- Generische Parameter und Typargumente sind teilweise syntaktisch vorhanden; eine Instanziierung/Substitution generischer Funktionen sowie integrierte Trait-Bound-Prüfung existieren nicht. Generische Aufrufe werden beim HIR-Lowering ausdrücklich zurückgewiesen und nicht als monomorphisiert ausgegeben.
- Phase 3 hat begonnen: Cranelift erzeugt COFF für ein begrenztes skalares MIR-Subset auf `x86_64-pc-windows-msvc`; `nortexc build <datei.nx> -o <ausgabe.obj|ausgabe.exe>` und der direkte Aufruf `nortexc <datei.nx> -o <ausgabe.obj|ausgabe.exe>` sind angebunden. MIR-Gotos, `SwitchInt`-Verzweigungen, Schleifen und direkte interne Aufrufe nicht-generischer freier Funktionen werden gelowered und durch einen Windows-E2E-Test ausgeführt. `.exe`-Ausgaben werden über die erkannte MSVC-Toolchain gelinkt. Registerallokation und ABI-Details delegiert Cranelift. Aggregates, Referenzen, Strings, Floatwerte, Drops, Runtime-Integration und vollständige Compiler-Unterstützung sind weiterhin offen; Phase 3 ist nicht abgeschlossen.
- Native Konsolenausgabe ist für `Console.write`/`Console.writeLine` mit genau einem `string`-Wert integriert; der Wert darf ein Literal, eine Variable oder ein Ausdruck sein. Skalarwerte können zuvor etwa mit `Convert.toString` umgewandelt werden. HIR/MIR senkt Aufrufe auf einen Runtime-ABI-Call mit UTF-8-Zeiger, Byte-Länge und Newline-Flag; dadurch funktionieren auch dynamische/heap-allozierte Strings sowie eingebettete NUL-Bytes. Die Windows-Runtime schreibt nach stdout, meldet Fehler über stderr und der generierte Code trappt bei Fehlerstatus. `.exe`-Builds mit diesem Intrinsic linken `nortex_runtime.lib` sowie `ws2_32`, `userenv` und `ntdll`; die Runtime muss vorher gebaut sein oder über `NORTEX_RUNTIME_LIB` auffindbar sein. Windows-E2E verifiziert UTF-8-Ausgabe, `write` ohne Newline und String-Slices.
- Überlauf-Traps: `CodeGenerator::with_overflow_checks(true)` bzw. `nortexc build --overflow-checks[=true|false]` prüft vorzeichenbehaftete `+`, `-`, `*` bei `int`/`long` per `*_overflow` + `trapnz`; unter Windows endet das Programm mit `0xC000001D` (Exitcode `-1073741795`). Ohne Flag gilt `cfg!(debug_assertions)` des Compilers (Debug = Traps, Release = Wraparound). Die Überlauf-Policy der Sprache bleibt eine offene Entscheidung. E2E: `compiler/tests/codegen_e2e.rs` (Wraparound-Standard und Trap).
- Runtime-C-ABI: `runtime/src/io/console.rs` exportiert zusätzlich `nortex_print_i32(i32) -> i32` und `nortex_print_str(ptr, len) -> i32` (Fehlercode, Newline); sie sind noch nicht im Codegen verdrahtet.
- Struct-Layout: `compiler/src/type_system/struct_layout.rs` berechnet C-kompatible Layouts (Padding, Tail-Padding, verschachtelte Structs, Fehler) mit Unit-Tests; MIR/Codegen verwenden es noch nicht, Aggregates bleiben im Codegen unterstützt-nicht.
- IDE: Build/Run ruft jetzt `nortexc.exe` (zuvor fälschlich `nortex.exe`) auf, legt Ausgabeverzeichnisse an und bietet eine Checkbox „Overflow-Checks“. `nortexc` legt fehlende Ausgabeverzeichnisse selbst an.
- Release-Skript: `.\scripts\build_release.ps1` führt `cargo build --release --workspace --locked` aus, kopiert `nortexc.exe`, `nortex-code.exe` und `nortex_runtime.lib` nach `build\` und baut `build\app.exe` aus `examples\native-sum\main.nx` (anderes Beispiel per `-Example <pfad>`). Verifiziert: `app.exe` gibt die Summe aus und endet mit Exitcode 55. Die IDE-Binary heißt `nortex-code.exe`. `build\` ist von Git und ZIP ausgeschlossen. Die IDE schreibt Start-Panics und -Fehler nach `ide_crash.log`.
- IDE Phase 1 und 2: Theme- und Komponentensystem (`ide/src/ui/theme/`, `components.rs`; Industrial Midnight, Professional Dark, High Contrast; Dichteprofile; WCAG-Kontrasttests), Einstellungen in `%APPDATA%\NORTEX Code\settings.json` (Theme, Dichte, UI-/Editor-Zoom; defekte Datei → Standardwerte), Inter und JetBrains Mono (OFL) per `include_bytes!` aus `ide/assets/fonts/` eingebettet — der Build kommt ohne externe Asset-Kopiervorgänge aus. Editor mit Zeilennummernspalte, Zeilenhöhe nach Dichte, Drag & Drop für `.nx`; Konsole mit Tabs „Compiler Log“ und „Runtime Output“; Run-Button mit Spinner und Sperre während des Builds; Statuspunkt Amber/Grün/Rot. Verifiziert durch Unit-Tests und einen Startcheck (Prozess bleibt offen); keine Sichtprüfung und kein Klick-Test der GUI.
- IDE Command Palette (`ide/src/ui/command_palette.rs`): `Strg+Shift+P`/`F1`, Fuzzy-Filter, Pfeiltasten/Enter/Escape, Befehle Build & Run, Toggle Theme, Clear Console, Toggle Zen Mode (blendet Explorer und Sidecar aus), Open File (`rfd`-Dialog), Save File; Farben folgen dem aktiven Theme. Tastenfluss per Headless-Test geprüft. Branding/Logo, Activity Bar und Dashboard sind **noch nicht** umgesetzt.
- Lexer: Operator-Erkennung ohne `unwrap`; veraltete Audit-/Remediation-Dokumente ohne Verweise (`docs/audit/`) wurden entfernt.
- Diagnosen aus `check`, `mir` und `build` werden mit Datei-/Zeilenkontext und Quellzeilen ausgegeben; ANSI-Farben erscheinen nur in Terminals und sind mit `NO_COLOR` deaktivierbar. Der Build akzeptiert `--output` als Alias für `-o` und `--emit-obj` für explizite Objektdateiausgabe (`.obj`/`.o`).
- Ownership-Erweiterung: Der MIR-Validator verfolgt Loans und Reborrows über CFG-Zusammenführungen, löst Dereferenzierungen für Schreib-/Move-Prüfungen auf und sperrt Elternreferenzen bei aktiven Reborrows. Eine CFG-Fixpunktanalyse berechnet lokale Liveness einschließlich Schleifen-Backedges und lässt Loans nach ihrer letzten Verwendung auslaufen. MIR-Feldprojektionen unterstützen disjunkte Loans, partielle Moves und Feld-Reinitialisierung mit expliziten Feldtyp-Metadaten. Referenzargumente an MIR-Calls werden auf Alias-Konflikte geprüft; Referenzrückgaben aus Referenzparametern sind zulässig, unbekannte Call-Rückgaben bleiben mangels Lifetime-Metadaten gesperrt. Das ist keine vollständige Regions-/Lifetime-Inferenz. Im Quellsprach-Codegen sind nur direkte freie Funktionsaufrufe ohne Generics und überladene Mehrdeutigkeit unterstützt.
- CLI-Regressionsprüfungen decken `nortexc check` für gültige und syntaktisch ungültige Dateien ab; der Codegen-E2E-Test prüft eine echte Windows-Ausführung mit internem Call, Bedingung und Schleifen-Backedge.
- Das ausführbare Beispiel `examples/native-sum/main.nx` berechnet die Summe 1 bis 10 mithilfe einer Schleife, schreibt einen Konsolentext und gibt 55 als Prozess-Exitcode zurück. Der Windows-E2E-Test linkt dieselbe Quelldatei mit der Runtime und prüft stdout sowie Exitcode.
- Dereferenzierte Zuweisungen wie `*value = ...` werden durch HIR/MIR unterstützt und erfordern eine `&mut`-Referenz; die Parser-Erkennung unterscheidet diese Ausdrucksform jetzt von Pointer-Variablendeklarationen.
- Die Move-/Borrow-Regressionsprüfungen behandeln Moves pro Binding-ID und erkennen bei Borrow-Argumenten desselben Aufrufs bekannte disjunkte Feldprojektionen. Rückgabepfade berücksichtigen exhaustive Enum-Matches über Enum-Parameter.
- Ein byteidentischer doppelter historischer Auditbericht wurde entfernt und seine Verweise auf die erhaltene kanonische Kopie umgestellt.
- Der native Windows-Runtime-Einstieg ist als `native-entrypoint`-Feature optional; reguläre Workspace-, DLL- und Test-Builds benötigen keinen unresolved Programmeinstiegspunkt.
- Die zwei 100k-Edit-IDE-Benchmarks sind explizit ignoriert und müssen für Performance-Messungen manuell gestartet werden.
- Die Phasen 2 bis 7 bleiben ein mehrstufiger Entwicklungsumfang; dieser Versionsstand behauptet keinen vollständigen Compiler, keine vollständige Runtime oder IDE.

## IDE-Core 0.3.6-dev – Korrekturen nach dem IDE-Audit

- Löschbereiche normalisieren UTF-8-Endpunkte asymmetrisch (Start abrunden, Ende aufrunden); der Cursor hinter einem Löschbereich wird relativ verschoben.
- Piece-Ranges werden beim Materialisieren/Lesen nicht mehr stillschweigend ausgelassen.
- CR-only-Zeilenenden werden im Zeilenindex und in der Cursorbewegung erkannt; CRLF bleibt ein einzelner logischer Zeilenumbruch, auch bei getrennten Eingaben.
- Der Puffer bietet `for_each_chunk()` und zählt Unicode-Skalare bei der Zeilen-/Spaltenumrechnung ohne temporären Gesamt- oder Zeilen-String.
- Backing-Storage wird bei hoher Fragmentierung bzw. großem Speicherüberhang kompakt zusammengeführt. Undo/Redo ist standardmäßig auf 1.024 Operationen und 32 MiB Historientext begrenzt.
- Datei-I/O unterstützt explizites verlustbehaftetes UTF-8-Öffnen mit Bestätigung vor dem Speichern sowie Nullbyte-Binärerkennung. Atomisches Speichern streamt Pieces in eine Temp-Datei; Unix-Modusrechte werden vor dem Schreiben gesetzt.
- Temp-Dateinamen werden bei Kollision erneut gewählt. Bei einem Schreibfehler bleibt die Zieldatei erhalten.
- Highlighting-Cache vermeidet Re-Lexing bei unveränderter Revision und kann vorhandene Spans auf Viewportbereiche filtern. Ein Edit re-lexed weiterhin das gesamte Dokument.

### Grenzen und Risiken

- Die Piece-Tabelle bleibt `Vec<Piece>`; `split_pieces`, `merge_adjacent` und Präfixoffset-Neuaufbau sind weiterhin O(p). Die Zeilenindex-Aktualisierung verschiebt nachfolgende Zeilenoffsets und kann O(l) sein. Der Prefixindex beschleunigt nur Lesezugriffe. Ein balancierter Piece-Tree/Rope ist nicht enthalten.
- Die Eigenimplementierung der Graphemsegmentierung deckt häufige Sequenzen ab, ist aber nicht vollständig UAX #29-konform.
- Windows-ACL-Übernahme beim atomischen Austausch wurde nicht praktisch geprüft. Der Workspace-Build unter Windows/MSVC ist erfolgreich.
- Die Leistungsabnahme mit 100.000 verteilten Edits in 10 MiB ist als ignorierter Benchmark-Test enthalten, wurde aber nicht ausgeführt.
- `HighlightCache` ist revisionsbasiert, nicht inkrementell. Für Hintergrund-Highlighting/LSP fehlt weiterhin die Prozess- und Synchronisationsschicht.
- Eine GUI, LSP, Terminal und Debugger sind nicht implementiert. Eine Plugin-Sandbox existiert nur für Windows (IDE, AppContainer); siehe `docs/ide/plugins.md`.

### Historische Verifikation 0.3.6-dev

In dieser damaligen Runde waren `cargo`, `rustc`, `rustfmt` und Clippy nicht verfügbar; deshalb wurden seinerzeit keine Rust-Builds oder -Tests ausgeführt. Das Python-Referenzmodell war kein Nachweis für den Rust-Code. Der aktuelle Verifikationsstand steht am Anfang dieser Datei.

## In dieser Runde geändert

### Runtime / Async

- Die gemeinsame Job-Queue verwendet durchgehend FIFO. Externe Jobs, `spawn` aus Workern und Waker-Fortsetzungen werden hinten eingereiht. Es gibt keine unbegrenzte LIFO-Priorisierung.
- Der Task-Zustandsautomat `IDLE`, `SCHEDULED`, `RUNNING`, `RUNNING_NOTIFIED` und `COMPLETE` verhindert parallele Polls derselben Future. Die Future wird vor `poll()` aus dem Mutex-Slot genommen; während fremdem Future-Code wird dieser Mutex nicht gehalten.
- Selbst-Weckungen während `RUNNING` markieren nur `RUNNING_NOTIFIED`. Nach `Pending` wird höchstens eine erneute Ausführung hinten in die Queue gestellt.
- Shutdown schließt registrierte Tasks zunächst ohne Wakes ab und löst die gesammelten Waker erst danach aus. Damit wird die beobachtete rekursive Wake-Kaskade vermieden.
- `Spawner` hält nur ein schwaches Pool-Handle. Futures sollen einen `Spawner` und nicht `Arc<ThreadPool>` erfassen. Der Pool registriert ausstehende Task-Zellen bis zu Abschluss oder Shutdown mit einem starken Task-Handle; dadurch wird ein verworfenes Benutzer-Handle nicht mit Cancellation verwechselt. Eine Future, die selbst ein starkes `Arc<ThreadPool>` einfängt, kann weiterhin einen Eigentumszyklus erzeugen und ist zu vermeiden.
- `block_on` im Worker liefert `BlockOnError::CalledFromWorker` statt eines Assert-Panics.
- Detached Worker werden über `ThreadPool::detached_worker_count()` gezählt. Normales Drop detachiert laufende beliebige Closures; `shutdown_and_join` ist ausdrücklich blockierend. Das ist eine Zählung, keine automatische Warnung. Nicht-kooperative synchrone Jobs können nach Drop weiterlaufen.

### Compiler / Typprüfung / Parser

- Eine erste T1/T2-Ausdruckstypprüfung ist als eigener Pass unter `compiler/src/semantic/type_checker.rs` angelegt.
- Sie prüft lokale Initialisierungen, Rückgabeausdrücke, Bedingungen, unäre und binäre Operatoren, ternäre Ausdrücke, Aufrufe, bekannte Überladungen, Konstruktorargumente, Member, Indexzugriffe, `await` und den Elementtyp von Iterationen.
- Für bekannte Klassen werden Felder und Methoden über die Basisklassenkette aufgelöst. Aufrufargumente werden hinsichtlich Anzahl und exaktem Typ abgeglichen.
- `using`-Direktiven werden gegen die bekannte Standard-Namespace-Liste geprüft. `System.Collections.Generic` macht die Collection-Typen sichtbar.
- `for (T value in sequence)` ist im Parser, AST und in der Typprüfung als `ForEach` abgebildet. `string.chars()` liefert im Frontend `Iterator<char>`; damit ist das Plan-Beispiel p13 im vorgesehenen Frontend-Subset adressiert.
- Unbekannte Typnamen erhalten für bekannte Alias-Schreibweisen konkrete Vorschläge. `let` erzeugt nun eine gezielte Meldung, dass es reserviert, aber nicht unterstützt ist.
- Interface-Verträge berücksichtigen Parameter- und Rückgabetyp. Die Basisklassenkette wird für die Erfüllung abstrakter Verträge untersucht. `Console.write`, `Console.writeLine` und `Console.readLine` besitzen erste explizite Signatur-Overloads im Checker.

### K7 / Deklarationsregeln

- Konkrete Klassenmethoden und freie Funktionen ohne Rumpf werden abgelehnt, sofern sie nicht zu einer unterstützten Deklarationsform gehören.
- Abstrakte Klassen dürfen nicht instanziiert werden. Konkrete Unterklassen müssen abstrakte Methoden und Interface-Verträge mit passender Signatur erfüllen.
- `abstract` ist für private oder statische Methoden unzulässig. Widersprüchliche und doppelte Deklarationsmodifikatoren werden gemeldet.
- `intrinsic` ist kontextabhängig und nur innerhalb des kanonisierten compilerverwalteten `standard-library/`-Quellverzeichnisses zulässig. Die frühere öffentliche boolesche Freigabe-API ist entfernt; ein gleichnamiges Benutzerverzeichnis reicht nicht aus. Es ist weiterhin nur eine Deklarationsmarkierung; der Runtime-Symbolbinder fehlt.
- `extern` ohne explizite ABI wird abgelehnt. Die Syntax `extern "C"` ist noch nicht implementiert.

## Regressionstests im Quelltext

Der aktuelle Quellstand enthält 19 Compiler-Bibliothekstests (darunter fünf
MIR-Optimizer-Tests), 74 Frontend-Integrationstests, 10 Phase-2-Typentests,
14 Pipeline-Smoke-Tests, 1 Ownership-Negativtest-Suite mit sechs Fixtures,
34 aktive Typprüfungs-Regressionstests (`typecheck_regressions.rs`), 29 aktive
und 1 ignorierten Runtime-Test sowie 37 aktive
und 2 ignorierte IDE-Tests. Die vormals ignorierten Compiler-Gap-Regressionen sind
aktiviert und bestehen. Alle aktiven Tests bestanden `scripts/verify.ps1`; die
ignorierten Tests sind weiterhin die zwei expliziten IDE-Performancebenchmarks und
ein bekannter Runtime-Grenzfall.

Neu bzw. erweitert sind Tests für:

- Yield-Fairness unter FIFO, Selbst-Weckung während eines laufenden Polls und iterative Shutdown-Ketten.
- Typfehler bei Variablen, Operatoren, Rückgaben, Argumenten, Konstruktoren, Membern und `await`.
- Imports aus `System.Collections.Generic` und nicht auflösbare Namespaces.
- Das `foreach`-Beispiel mit `string.chars()`.
- Abstrakte Klassen, Interface-Rückgabetypen, Intrinsic-Scope einschließlich Verzeichnis-Spoofing, widersprüchliche Modifikatoren und nicht unterstütztes `let`.
- Return-Path-Abdeckung durch exhaustive Enum-Matches sowie Abweisung einer vermeintlichen Exhaustivität aus einem anderen Enum.
- Borrow-Aliasing anhand bekannter Feldprojektionen; dynamische/ungeklärte Speicherorte bleiben konservativ.


## Bewusst offen

- Der Typchecker und die Ownership-Prüfung sind Teilimplementierungen. Generische Methoden-Substitution, ausgewählte Sichtbarkeitsregeln, Rückgabepfad-/Erreichbarkeitsprüfungen und Enum-Match-Vollständigkeit sind abgedeckt, aber keine vollständige Sprachsemantik; Trait-/Interface-Mehrdeutigkeiten, vollständige Symboltabellen und `must_use` bleiben offen.
- `using a.b;` löst dateiübergreifend auf (`a/b.nx` bzw. `a/b/mod.nx`, flacher Namensraum, Zyklen-/Diamant-Erkennung); `using` auf Standard-Namespaces wird gegen eine begrenzte Liste geprüft. Qualifizierte Namen und Modul-Sichtbarkeit fehlen.
- Move-/Borrow-Regeln erkennen ausgewählte Use-after-move-, Reinitialisierungs- und Aufrufkonflikte; disjunkte Feldprojektionen werden innerhalb eines Aufrufs unterschieden. MIR validiert definite assignment und Move-Zustände an CFG-Zusammenführungen. Vollständiges Loan-/Lifetime-Tracking über Aufrufgrenzen, partielle Move-Zustände, Drop-/Copy-Semantik und Borrow-Datenflussanalyse fehlen weiterhin.
- Der Scheduler verwendet weiterhin eine gemeinsame Mutex-/Condvar-FIFO-Queue. Work-Stealing, lokale Worker-Queues, Cancellation-Tokens, strukturierte Task-Scopes, Timer-Rad und IOCP fehlen.
- Der Rust-Executor ist noch nicht mit NORTEX-`async`/`await`-Lowering verbunden.
- Die AST-to-HIR-/HIR-to-MIR-Lowering-Pipeline unterstützt derzeit nur ein begrenztes skalares Subset samt `if`, `while`, klassischem `for`, lokalen Zuweisungen, `break` und `continue`; Aufrufe, Felder, vollständige Typübertragung und MIR-Interpreter fehlen. Der Driver optimiert und validiert diese MIR-Funktionen. COFF-Backend, Linker, native NORTEX-EXE, Runtime-Allocator und ein vollständiger ABI-/Unwind-Vertrag fehlen.
- Die Standardbibliothek hat noch keinen Intrinsic-Binder. D3D12-Backend, GUI-IDE, LSP und Plugin-Isolation fehlen.

## Historische Verifikation (externe Prüfung 0.3.4-dev)

Die sechs Prüfkommandos wurden mit Rust 1.99.1 unter Linux ausgeführt.

| Kommando | Ausgelieferter Stand | Nach Korrekturen |
|---|---|---|
| `cargo fmt --all -- --check` | 559 Abweichungen | OK |
| `cargo check --workspace --all-targets --locked` | Fehler E0382 in `runtime/src/async/mod.rs` | OK |
| `cargo build --workspace --all-targets --locked` | Fehler E0382 | OK |
| `cargo test --workspace --all-targets --locked` | nicht kompilierbar | OK: 9 + 72 + 9 + 20 Compiler, 25 Runtime, 6 IDE; 12 ignoriert |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 5 Fehler im Compiler, Runtime nicht kompilierbar | OK |
| `cargo test --workspace --locked -- --ignored` | – | 11 erwartete Fehlschläge (dokumentierte Lücken) |

Die Windows-CI wurde nicht ausgeführt. Ignorierte Tests beschreiben bekannte Lücken und müssen bei deren Behebung aktiviert werden.

**Releaseurteil zum damaligen Stand:** `0.3.4-dev` war ein erweiterter Frontend-/Runtime-Prototyp. Es ist kein releasefähiger Compiler und erzeugt weiterhin keine native NORTEX-EXE.


## IDE-Core 0.3.5-dev – Änderungen in dieser Runde

- `EditorBuffer` verwendet Änderungsoperationen für Undo/Redo statt vollständiger Piece-Listen-Kopien. Tippen am Cursor wird gruppiert; die Historie ist standardmäßig auf 1.024 Einträge begrenzt.
- Der Gesamttext wird nicht nach jeder Änderung aufgebaut. `slice()` und `line()` lesen nur benötigte Bereiche; `materialize()` ist explizit.
- Auswahl, UTF-8-sichere Editbereiche, Cursorbewegung, CRLF-Behandlung, Zeilenindex sowie Zeile-/Spalte-Konvertierung sind vorhanden. Die Graphemsegmentierung deckt häufige Emoji-/Combining-Sequenzen ab, ist aber nicht vollständig UAX-29-konform.
- Dokumentdateien bewahren UTF-8-BOM und vorhandene LF-/CRLF-Zeilenenden. Der erste Zeilenumbruchstil wird erkannt und über `newline_text()` für neu eingefügte Zeilen bereitgestellt. Der Dateiersatz erhält vorhandene Dateiberechtigungen, sofern Metadaten gelesen werden können.
- NORTEX-Syntaxspannen kommen aus dem Compiler-Lexer; Kommentarspannen werden separat bereitgestellt, damit die Parser-Tokenfolge unverändert bleibt.
- Der Manifest-Loader (`nortex.toml`) liest und validiert das Kernschema und sperrt den Einstiegspfad auf das Projektverzeichnis. Abhängigkeiten (`path`/`lib`) werden als Linker-/Quellangaben verwendet, es gibt keine Paketauflösung.
- Anwendungszustand verwaltet Dokumenttabs; geänderte Dokumente werden ohne ausdrückliches Verwerfen nicht geschlossen. Diagnoseauszüge können mit Quellzeile und Caret-Markierung dargestellt werden.
- Der IDE-Quellcode enthält 22 Testdefinitionen. Diese wurden in dieser Umgebung nicht ausgeführt. Statische Prüfung: 7 TOML-/Lockdateien valide, 81 Markdown-Dateien mit 102 relativen Verweisen geprüft, 7 geänderte Rust-Dateien auf ausgeglichene Delimiter geprüft und 11 IDE-Core-Invarianten erfüllt. Ein separates Python-Referenzmodell bestand 50.000 gemischte UTF-8-/Zeilenumbruch-Edits; dies ist kein Rust-Build- oder Rust-Testnachweis.
- Die UI-Framework-Auswahl ist offen. Eine grafische Oberfläche, LSP-Prozess, Maus-/Tastatureingabe, Clipboard, vollständige IME-Unterstützung, Terminal und Debugger fehlen weiterhin.

### Historische Verifikation der Änderung 0.3.5-dev

In dieser damaligen Runde war die Rust-Toolchain (`cargo`, `rustc`, `rustfmt`, Clippy`) nicht verfügbar; daher wurden die Rust-Prüfungen nicht ausgeführt. Der aktuelle Verifikationsstand steht am Anfang dieser Datei.

## UI/UX-Verfeinerungen (NORTEX Code 0.3.7-dev)

- Statusleiste: Fehlerzahl, Zeile/Spalte, UTF-8, Zeilenende (LF/CRLF/Gemischt), Ziel `x64 MSVC`, Git-Branch, Projekt → Ausgabe, farbige Statusmeldung (`ide/src/ui/status_bar.rs`).
- Tabs: aktiv `#1E2228`, passiv `#14171B`; `•` für ungespeicherte Änderungen, `×` bei Hover.
- Activity Bar: 2-px-Indikatorstreifen am aktiven Icon (bereits vorhanden, unverändert).
- Explorer: Icon-Farbe nach Dateityp (`.nx` `#9670E8`, `.toml` `#D6A64B`, `.json` `#398BFF`, `.md` hellgrau). Der Explorer ist eine flache Dateiliste; Führungslinien für Einrückung sind daher nicht umgesetzt.
- Overlays (Suchen/Ersetzen, Befehlspalette): Hintergrund `#121418`, Rand `#343B43`, Padding 6×10.
- Explorer ist eine flache Dateiliste (keine Führungslinien); `Strg+Shift+B` und `Strg+F5` lösen beide „Build & Run“ aus.
- Debugger, Git-Aktionen, Mehrfachcursor und Explorer-Kontextmenüs sind zukünftige Features (nicht vorhanden).
- Toolchain-Suche relativ zur EXE: EXE-Ordner → Elternordner → `target/release` → `target/debug` (Assets sind per `include_bytes!` eingebettet).
- Benutzerhandbuch mit echten Screenshots: `docs/user-manual/README.md`; Sprachreferenz: `docs/language/README.md`.

## Ergonomie, Installer, Tests (0.3.7-dev)

- Editor: F2-Rename (token-basiert, nur ASCII, nur aktuelle Datei), Breadcrumbs (Scope-Pfad, Klick springt), Minimap (Strg+Alt+M, Einstellung `editor.minimap.enabled`). Tests: `ide/tests/ergonomics_test.rs`.
- Golden-Tests: `compiler/tests/golden_tests.rs` (`cargo test --workspace --test golden_tests`, nur Windows).
- Installer-Pipeline: `scripts/stage_release.ps1`, `scripts/build_installer.ps1`, `installer/nortex_setup.iss` (benötigt Inno Setup 6 oder 7). **Nicht gebaut/getestet** (ISCC auf der Entwicklungsmaschine nicht installiert).
- CI: Job `release-zip` baut Release und `dist/*.zip` (`scripts/package.py`).
- Doku: `docs/language/SPECIFICATION_IMPLEMENTED.md` (Ist-Stand), `docs/user-manual/USER_GUIDE.md`, Beispiele `examples/language/01–04_*` (alle baubar und per Golden-Test geprüft).
- Multi-Datei-Module: `using util;` (`compiler/src/semantic/resolver.rs`, Zyklen-/Diamant-Erkennung, flacher Namensraum). `extern "C"` für int/bool-Win32-Aufrufe; `.lib`s aus `[dependencies]`. Nicht vorhanden: Zeiger/Strings in extern, qualifizierte Namen, Modul-Sichtbarkeit.
- Nicht umgesetzt: Paketmanager, WASM-Playground, VS-Code-Extension, LSP, `nortexc fmt`, `keybindings.json`, Fuzzing, Semantic Highlighting, Extract Variable/Function, projektweites Rename, External-File-Watcher. Speichern ist atomar.
