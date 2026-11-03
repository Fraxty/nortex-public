# NORTEX

NORTEX ist ein experimentelles Projekt (Public Preview 0.1.0) zur Entwicklung einer nativen, statisch und streng typisierten Programmiersprache, einer eigenen Runtime, einer Standardbibliothek und der offiziellen Entwicklungsumgebung NORTEX Code.

## Projektbereiche

- `compiler/` – Lexer, Parser, AST, semantische Analyse und native Windows-x64-Codegenerierung für das dokumentierte Sprachsubset
- `runtime/` – Speicher-, Threading-, Async-, IO-, Netzwerk- und Plattformdienste
- `standard-library/` – NORTEX-Module und öffentliche APIs
- `graphics/` – Graphics API, RHI, D3D12-Backend und Renderer
- `ide/` – NORTEX-Code-Desktop-IDE mit Editor, Plugin-Host und lokalen Entwicklerwerkzeugen
- `tools/` – CLI, Formatter, Language Server, Paketverwaltung und Dokumentationsgenerator
- `docs/` – Sprach-, Compiler-, Runtime-, Grafik-, IDE- und Sicherheitsdokumentation
- `examples/` – NORTEX-Beispiele
- `tests/` – Compiler-, Runtime- und Integrationstestfälle

Ordnerstruktur: `compiler/`, `runtime/`, `ide/`, `assets/` (inkl. `assets/source/` JPG-Vorlagen), `scripts/`, `dist/` (nur ZIP-Releases); `build/` und `bin/` sind generiert. IDE starten: `.\run_nortex_code.ps1` (baut release und legt `bin\nortex-code.exe` ab; `-NoBuild` startet nur).

## Voraussetzungen

- Rust 1.88 oder neuer (Edition 2024)
- Windows SDK und ein unterstützter Windows-Linker für native Windows-Ziele
- DXC und Windows Graphics SDK für das spätere Direct3D-12-Backend

## Bauen und prüfen

```powershell
cargo fmt --all -- --check
cargo build --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Alternativ führt `.\scripts\verify.ps1` Versionsabgleich, Formatierung, Workspace-Check, Build, Tests und Clippy unter Windows der Reihe nach aus und bricht beim ersten Fehler ab.
`.\build_app.ps1` führt dieselben grundlegenden Prüfungen sowie einen Release-Build aus und kopiert anschließend die Release-Binaries in den Projektstamm. Alle Cargo-Prüfungen verwenden `--locked`.

Eine geprüfte Quellcode-Distribution kann mit `python .\scripts\package.py` erstellt werden. Die Release-Konfiguration in `packaging.toml` verlangt ein sauberes Git-Repository, erfolgreiche Cargo-Prüfungen und einen Build aus dem entpackten Archiv. `python .\scripts\measure.py` zeigt Inventurwerte ohne generierte `target/`-Dateien.

`nortexc check <datei.nx>` führt den aktuellen Frontend-Prototyp aus. Es wird dabei keine native EXE-Datei erzeugt.
`nortexc mir <datei.nx>` senkt das unterstützte skalare Funktionssubset über HIR nach MIR ab, optimiert es und gibt das validierte MIR aus. Unterstützt sind Integer-/Bool-Locals, lokale `&T`-/`&mut T`-Bindings mit skalaren Referenten, Zuweisungen, skalare sowie verschachtelte bedingte/kurzschließende Ausdrücke, `if`, `while`, klassische `for`-Schleifen, `break`, `continue` und `return`. MIR prüft definite assignment und verfolgt Loan-Liveness CFG-basiert bis zum letzten Use; vollständige Lifetime-Inferenz ist nicht implementiert. Nicht unterstützte Konstrukte, einschließlich Referenzrückgaben ohne Lifetime-Signatur, werden diagnostiziert.

`nortexc build <datei.nx> --output <datei.exe>` erzeugt für das unterstützte native Sprachsubset ein Windows-x64-Programm über Cranelift und den MSVC-Linker. Dazu gehören auch native Klassen mit Heap-Referenzen, Konstruktoren, Methoden, Einzelvererbung, `virtual`/`override`, geschlossenen generischen Klassen und explizit spezialisierten generischen Instanzmethoden. `Console.write` und `Console.writeLine` leiten Stringwerte über die Runtime-ABI an Windows-stdout weiter; andere Skalarwerte lassen sich mit `Convert.toString` formatieren. Die Runtime hält Klassenobjekte in einer Arena bis zur normalen Rückkehr aus `main`; sie bietet noch keine Einzelobjektfreigabe oder Tracing-GC. Explizite Generic-Typargumente sind erforderlich, und Basiskonstruktoren müssen parameterlos sein. Details und weitere Grenzen: [Klassen](docs/language/08-structs-and-data-structures.md), [Speicherverwaltung](docs/language/09-memory-management.md) und [Codegenerierung](docs/compiler/codegen.md). Baue dafür zuvor `cargo build -p nortex-runtime`. Ein ausführbares Beispiel liegt unter [examples/native-sum](examples/native-sum/README.md).

`nortexc build <datei.nx> -o build\app.exe [--overflow-checks[=true|false]]` legt das Ausgabeverzeichnis an. Ohne Flag trappt der Overflow-Check nur in Debug-Builds des Compilers (`0xC000001D`), in Release wrappt die Arithmetik. Die IDE `nortex-code` (`target\debug\nortex-code.exe`, nach `build_app.ps1` im Projektstamm als `NORTEX-Code.exe`) ruft `nortexc.exe` auf und hat eine Overflow-Checks-Checkbox.

`.\scripts\build_release.ps1` baut den Release-Stand und legt `nortexc.exe`, `nortex-code.exe`, `nortex_runtime.lib` sowie das Beispiel `build\app.exe` (aus `examples\native-sum\main.nx`, per `-Example` änderbar) in `build\` ab.

### IDE „NORTEX Code“ (Phase 1 und 2)

- **Theme- und Komponentensystem** (`ide/src/ui/theme/`, `ide/src/ui/components.rs`): Industrial Midnight (Standard), Professional Dark und High Contrast, Dichteprofile Compact/Comfortable/Spacious, getrennter UI-/Editor-/Terminal-Zoom, eigene Editor-Palette, WCAG-Kontrasttests. Wiederverwendbar: `IndustrialButton` (eckig, Hover-Übergang), `file_tab` (Aktiv-, Änderungs-, Fehlerzustand, Schließen) und `panel_header` („01 / EXPLORER“ mit Statuspunkt).
- **Einstellungen**: Theme, Dichte, UI- und Editor-Zoom stehen im Menü „Ansicht“ und werden als `%APPDATA%\NORTEX Code\settings.json` gespeichert und beim Start geladen (fehlende oder defekte Datei → Standardwerte).
- **Eingebettete Schriften**: Inter und JetBrains Mono (beide SIL OFL 1.1) liegen in `ide/assets/fonts/` und werden per `include_bytes!` ins Binary eingebunden. Der Build braucht keine externen Asset-Kopiervorgänge; `build_release.ps1` kopiert keine Schriften.
- **Editor**: Zeilennummernspalte (scrollt mit), Zeilenhöhe nach Dichteprofil, Drag & Drop von `.nx`-Dateien. Es gibt kein Soft-Wrapping, damit Zeilennummern und Zeilen übereinstimmen.
- **Konsole („03 / CONSOLE“)**: Tabs „Compiler Log“ (Diagnosen und `nortexc`-Ausgabe) und „Runtime Output“ (nur Programmausgabe). Der Run-Button zeigt während des Builds Spinner und „Kompiliere...“ und ist gesperrt; der Statuspunkt ist Amber (läuft), Grün (Erfolg) oder Rot (Compilerfehler).
- **Command Palette** (`ide/src/ui/command_palette.rs`): `Strg+Shift+P` oder `F1`, Fuzzy-/Substring-Filter, Pfeiltasten, Enter führt aus, Escape schließt. Befehle: Build & Run, Toggle Theme, Clear Console, Toggle Zen Mode (blendet Explorer und Sidecar aus), Open File (Dateidialog via `rfd`), Save File.
- Noch nicht umgesetzt: Folding, LSP, Debugger.

### IDE „NORTEX Code“ (Branding, Activity Bar, Dashboard, Layout)

- **Branding-Pipeline**: Die vier JPG-Vorlagen im Projektstamm werden mit `python scripts/generate_brand_assets.py` (benötigt `pillow`, `numpy`, `potracer`) per potrace vektorisiert und nach `assets/` geschrieben: `branding/nortex-symbol.svg`, `icons/app/nortex-code.svg` (Master), `nortex-code-256.png`, `nortex-code.ico` (7 Auflösungen: 16/24/32/48/64/128/256), `icons/files/nortex-file.svg` und `nortex-code-file.svg` sowie Laufzeit-PNGs in `icons/runtime/`. Build-Time-Asset: die `.ico` (eingebettet per `winres` in `ide/build.rs`, Explorer-Icon der `nortex-code.exe`). Laufzeit-Assets: PNGs per `include_bytes!` (Fenster-/Taskleisten-Icon über `eframe::NativeOptions`, Explorer-Dateiicons: `.nx` → Code-Icon, `.nortex`/`.ntx` → allgemeines Icon).
- **Activity Bar** (50 px, nur Icons mit Tooltips): Explorer, Search (echte Textsuche über Projektquellen), Git (Statusanzeige über `git status`, nur wenn `git` installiert ist), Debugger (**nicht implementiert**, das Panel sagt das), Settings (eigene Ansicht im Arbeitsbereich).
- **Dashboard**: Das N-Logo oben links wechselt in den `ViewMode::Dashboard` (Projektübersicht, Toolchain-Status, Build-Status, Recents, Quick Actions), ohne Dateien oder Editorzustand zu verlieren.
- **Layout-Limits** (`ide/src/ui/layout.rs`, reine Funktion mit Tests): Explorer 180–300 px (≤ 22 %), Sidecar 220–350 px (≤ 25 %), Console 120–280 px (≤ 30 % der Höhe, so begrenzt, dass ≥ 15 Codezeilen sichtbar bleiben). Der Editor behält mindestens 500 px und 50 % der Breite; reicht der Platz nicht, klappt der Sidecar automatisch ein. `Strg+Shift+L` oder Menü „Ansicht → Sidecar“ holt ihn zurück (er verdrängt dann den Explorer). Zen-Modus blendet Explorer, Sidecar, Console und Activity Bar aus.
- **Tests**: 337 bestanden, 0 fehlgeschlagen, 3 ignoriert (`cargo test --workspace`). Erweiterte Suite: Edge Cases (12.000-Zeilen-Editor, Zeilen > 2.000 Zeichen, UTF-8-BOM, Umlaut-/Leerzeichenpfade, Emoji, gelöschte/schreibgeschützte Dateien), Settings-Reparatur bei korruptem JSON, Zoom-Clamp, Fenstergrößen ab 400×300, CLI-Tests für `--overflow-checks` und fehlende Runtime-Lib.
- Sichtprüfung unter Windows per Screenshot (Logo, Fenstericon, Activity Bar, Dashboard, Settings, 1280×720-Layout). Kein automatisierter Klick-Test.

## Dokumentation

- [Dokumentationsindex](docs/README.md)
- [Sprachreferenz (15 Kapitel)](docs/language/README.md)
- [Benutzerhandbuch NORTEX Code (13 Kapitel)](docs/user-manual/README.md)
- [Architektur](docs/architecture.md), [Roadmap](docs/development/roadmap.md), [Lückenanalyse](docs/development/gap-analysis.md)
- [Implementierungs- und Verifikationsstatus](BUILD_STATUS.md)
- [Zielentwürfe (Phasen 3–8, Sprachentwurf)](docs/design/README.md)

## Änderungsprotokoll

- [CHANGELOG](CHANGELOG.md)

## Projektstatus

Die Architektur und Sprachentscheidungen sind dokumentiert. Der Compiler erzeugt für einen Sprachsubset native Windows-x64-EXEs (HIR/MIR, Cranelift-COFF, MSVC-Linking), besitzt aber keinen vollständigen Borrow-/Lifetime-Checker. `nortex-code` ist ein egui-Desktop-Prototyp mit Build/Run über `nortexc.exe`; LSP, Debugger und echtes Piece-Table-Viewport-Rendering fehlen. Kein nicht ausgeführter Test wird als bestanden gewertet.


## Parser-Unterstützung und Status

- [Syntax-Unterstützungsmatrix](docs/compiler/syntax-support-matrix.md)
- [Build- und Implementierungsstatus](BUILD_STATUS.md)
- [Offene Sprachentscheidungen](docs/language/15-faq-and-best-practices.md#offene-entscheidungen-projektinhaber)
- [Versionierungsregeln](docs/development/versioning.md)
- [Manifest-Schema](docs/build-system/manifest.md)
- [Lückenanalyse & Roadmap](docs/development/gap-analysis.md) (aktuell, 337 Tests grün)


## IDE-Core

Der IDE-Kern (Editor-Core) unterstützt LF/CRLF/CR, sicheres UTF-8-Runden bei Löschbereichen, Piece-Chunk-Ausgabe, Speicherbudget für Undo/Redo und gestreamtes atomisches Speichern. Die egui-GUI nutzt ihn bereits; LSP ist nicht implementiert. Piece- und Zeilenindex-Arrays bleiben O(p)/O(l) zu aktualisieren; ein Rope/Piece-Tree und die 100.000-Edits-Leistungsabnahme fehlen. Siehe [Editor-Core-Dokumentation](docs/ide/editor-core.md).
