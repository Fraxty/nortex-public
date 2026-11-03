# Lückenanalyse & Roadmap (Stand 2026-11-02, 0.3.7-dev)

Grundlage: Code-Audit des Workspace, `BUILD_STATUS.md`, `docs/language/15-faq-and-best-practices.md`. Verifiziert: fmt, check, Clippy (`-D warnings`) und 337 Tests grün (0 fehlgeschlagen, 3 ignoriert).

## Audit-Ergebnis Produktionscode

- Keine `todo!`/`unimplemented!`/`TODO`-Marker, keine `allow(dead_code)` in `compiler/src`, `runtime/src`, `ide/src`.
- Verbleibende `expect`/`unreachable!` sind dokumentierte Invarianten (MIR-Validierung, Scope-Stack, Piece-Table, Task-Zustände).
- Bekannte Ausnahme: `ThreadPool::new` (`runtime/src/threading/mod.rs`) paniert, wenn das Betriebssystem keinen Worker-Thread startet. Sauber wäre ein `Result`; das ändert die öffentliche API und ist nicht erfolgt.
- Pfade: IDE und Linker lösen `nortexc.exe` bzw. `nortex_runtime.lib` über `current_exe()`; `NORTEX_RUNTIME_LIB` überschreibt.

## IDE `nortex-code`

| Bereich | Status |
|---|---|
| Theme-System, Fonts (eingebettet), Settings-Persistenz | fertig |
| Editor mit Zeilennummern, Drag & Drop, Undo/Redo | fertig |
| Konsole (Compiler Log / Runtime Output), Build-Feedback | fertig |
| Command Palette, Zen Mode | fertig |
| Branding (N-Logo, App-Icon, Exe-Icon via `winres`, Fenstericon, Datei-Icons), Activity Bar | fertig |
| Dashboard (`ViewMode::Dashboard`, Logo = Home-Button), Settings-Ansicht | fertig |
| Layout-Limits (Explorer/Sidecar/Console, Auto-Collapse, Zen-Modus, `Strg+Shift+L`) | fertig, `ui/layout.rs` |
| Erweiterte Edge-Case-Suite (12k Zeilen, Unicode/BOM, Schreibschutz, Settings-Reparatur, CLI) | fertig |
| Layout-Fixes (Explorer-Breite, `\\?\`-Pfadpräfix, Dashboard-Karten, Recents-Duplikate) | fertig |
| Search (Textsuche über Projekt-Quelldateien, max. 200 Treffer) | fertig, ohne Regex/Ersetzen (Regex/Ersetzen gibt es nur im Editor: `Strg+F`/`Strg+H`) |
| Explorer | **flache Dateiliste** (`.nx`/`.nortex`, relative Pfade), Icon-Farbe je Dateityp; keine Baumstruktur, keine Führungslinien. Geplant: Baum, Kontextmenü, Umbenennen/Löschen/Neuer Ordner |
| Build & Run | `Strg+F5` und `Strg+Shift+B` lösen denselben Prozess „Build & Run“ aus (kein getrennter reiner Build) |
| Toolchain-Suche | `nortexc.exe`/`nortex_runtime.lib`: EXE-Ordner, Elternordner, `<Eltern>/target/release`, `<Eltern>/target/debug` (IDE und Linker); `NORTEX_RUNTIME_LIB` hat Vorrang |
| Git-Panel | nur Statusanzeige (`git status --porcelain=v1 --branch`); ohne installiertes `git` Fallback „nicht verfügbar“. Parsing per Test, kein Live-Test (Git auf Dev-Maschine fehlt). **Zukünftig:** Stage/Commit/Push/Diff |
| Debugger-Panel | **zukünftiges Feature, nicht implementiert**; Panel zeigt „Nicht implementiert“. Nächste Phase: Debug-Adapter/Debuginfo im Codegen, dann Breakpoints und Stack |
| Mehrfachcursor, Klammer-Autovervollständigung | **zukünftig**, nicht vorhanden |
| KI-Sidecar | nur Platzhalter, kein Anbieter; klappt bei schmalen Fenstern ein |
| Mehrere Editoren / Split-Workspace | fertig (Modell + UI, `editor/split.rs`); offen: inaktive Panes nur Vorschau, Drag&Drop/Splitter nicht manuell geprüft |
| Minimap, Breadcrumbs, F2-Rename (Datei-lokal) | vorhanden |
| LSP, Folding, projektweites Rename, Semantic Highlighting | fehlt |
| Sicht-/Klicktest der GUI | Screenshot-Prüfung unter Windows erfolgt; kein automatisierter Klick-Test |

## Compiler / Toolchain

- Backend nur für skalares Subset (Integer/Bool, `if`/`while`/`for`, direkte Aufrufe) auf `x86_64-pc-windows-msvc`.
- Fehlend: Strings außer Literal-Intrinsic, Floats, Referenzen, Aggregates/Structs im Codegen (Layout-Berechnung existiert), Klassen, Drops, Generics (Monomorphisierung), Async-Codegen, qualifizierte Modulnamen/Sichtbarkeit, Zeiger-/String-Parameter in `extern "C"`, Standardbibliothek-Anbindung. (Vorhanden: `using`-Module, `extern "C"` für int/bool.)
- `nortex_print_i32`/`nortex_print_str` sind exportiert, aber nicht im Codegen verdrahtet.
- Offene Sprachentscheidungen (Überlauf-Policy, Details in `docs/language/15-faq-and-best-practices.md`).
- Linux: nur Cross-Check; Laufzeit nur Windows.

## Empfohlene Reihenfolge

1. Compiler: Print-Intrinsics verdrahten, Strings/Floats, dann Structs im Codegen.
2. Überlauf-Policy entscheiden, Generics-Monomorphisierung.
3. IDE: Diagnose-Navigation, Git-Aktionen, Debugger, KI-Anbieter.
4. `ThreadPool::new` auf `Result` umstellen.
