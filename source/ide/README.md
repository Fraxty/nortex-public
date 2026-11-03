# NORTEX Code – IDE-Core und Desktop-Prototyp

`nortex-code` startet einen nativen egui/eframe-Desktop-Prototyp. Er enthält eine Projektdateiliste, Editor-Tabs, Lexer-basierte Syntaxfarben, einen rechten Sidecar-Bereich sowie ein unteres Ausgabe-/Diagnosepanel. Compilerdiagnosen laufen über den bestehenden Compiler-Frontend-API auf einem Hintergrund-Worker. Das Build-/Run-Menü versucht den benachbarten `nortexc.exe`-Compiler aufzurufen und meldet explizit, wenn keine ausführbare Datei erzeugt wurde.

## Implementierter Kern

- `EditorBuffer` speichert Edit-Operationen statt vollständiger Piece-Listen-Kopien. Undo/Redo ist auf 1.024 Operationen und standardmäßig 32 MiB Historientext begrenzt.
- `slice()` beginnt per Binärsuche am ersten überlappenden Piece. `for_each_chunk()` gibt Bereiche stückweise ohne zusätzlichen Gesamt-String aus.
- Der Piece-Puffer kompaktifiziert Additions- und Originalspeicher bei übermäßiger Fragmentierung bzw. überdimensioniertem Backing-Storage.
- UTF-8-Löschbereiche runden Anfang und Ende auf gültige Grenzen. Cursor, Auswahl und Zeilen-/Spaltenumrechnung verwenden Byteoffsets und Unicode-Skalarspalten.
- LF, CRLF und CR werden als Zeilenenden erkannt. Getrennt eingefügte CR-/LF-Bytes werden im Zeilenindex als ein CRLF behandelt.
- `EditorDocument` unterstützt striktes UTF-8-Öffnen und ausdrücklich verlustbehaftetes `open_lossy()`. Ein verlustbehaftet geöffnetes Dokument darf erst nach Bestätigung überschrieben werden.
- Das Speichern streamt Pieces in eine temporäre Datei im selben Verzeichnis und ersetzt das Ziel atomar. Unter Unix werden vorhandene POSIX-Modusrechte bereits bei der Anlage der temporären Datei gesetzt. Windows-ACL-Übernahme wurde nicht praktisch verifiziert.
- Die Syntax-Hervorhebung nutzt den Compiler-Lexer. `HighlightCache` vermeidet erneutes Lexing beim Neuzeichnen derselben Revision und filtert Spans für einen Viewport. Änderungen lösen aktuell weiterhin ein vollständiges Re-Lexing aus.
- `.nortex`-Projektmanifest laden und grundlegende Pflichtfelder sowie den Einstiegspfad validieren.

## Bekannte Grenzen

Die Piece-Liste und der Zeilenindex sind weiterhin flache `Vec`-Strukturen. Ein Edit bleibt O(p) mit p = Piece-Anzahl; das Aktualisieren nachfolgender Zeilenoffsets bleibt O(l). Der Präfixoffset-Index beschleunigt nur Lesezugriffe. Ein gewichteter Piece-Tree oder Rope ist noch nicht implementiert. Die Leistungsabnahme mit 100.000 verteilten Edits in einer 10-MiB-Datei ist als ignorierter Benchmark-Test vorhanden, wurde aber nicht ausgeführt.

Die Graphemsegmentierung ist eine unvollständige Eigenimplementierung und nicht vollständig UAX-#29-konform. Unicode- und Windows-Edge-Cases müssen noch auf der Zielplattform geprüft werden.

## Noch nicht vorhanden

Ein virtueller Piece-Table-Editor mit Zeilennummern und viewport-basiertem Rendering,
echte Squiggles, LSP/JSON-RPC, Terminal/ConPTY mit Live-Streaming, Debugger und
Plugin-Sandbox sind nicht implementiert. Das Frontend nutzt das TextEdit-Widget
und führt noch keine vollständige NORTEX-Build-Pipeline aus.

Mit installiertem Rust 1.88+ kann `.\build_app.ps1` den Workspace prüfen, bauen
und `nortexc.exe` sowie `NORTEX-Code.exe` im Projektstamm ablegen. Das Skript kann
erst nach erfolgreicher Implementierung der Compiler-/Linker-Pipeline zu einer
benutzbaren Programm-EXE führen.

Siehe [IDE-Architektur und Status](../docs/ide/README.md), [Editor-Core](../docs/ide/editor-core.md) und [Phase 8 (Zielentwurf)](../docs/design/phase-8-ide.md).

## Theme

Das Design liegt zentral in ide/src/ui/theme/ (colors, typography, spacing, editor, accessibility). Standard ist Industrial Midnight (Alternativen: Professional Dark, High Contrast), Dichteprofile Compact/Comfortable/Spacious, getrennter UI-/Editor-/Terminal-Zoom. Zoomregler verwenden feste 10%-Schritte; Prozentwerte können separat eingegeben werden und werden erst nach Enter/Fokuswechsel übernommen. Strg+Mausrad ändert den UI-Zoom. Inter und JetBrains Mono (OFL, Lizenztexte in ide/assets/fonts/) sind per include_bytes! eingebettet; die egui-Standardschriften bleiben als Glyph-Fallback. Theme, Dichte und Zoom werden im Menue "Ansicht" gewaehlt und in %APPDATA%\NORTEX Code\settings.json gespeichert. Editor: Zeilennummern, Drag & Drop fuer .nx; Konsole: Tabs Compiler Log / Runtime Output.

Projektdateien und Git-Status werden im Hintergrund aktualisiert, damit Datei-/Git-Aktionen die UI nicht durch synchrones Warten blockieren. *Datei → Speichern unter…* öffnet den nativen Speichern-Dialog; ungespeicherte Tabs bieten Speichern, Verwerfen oder Abbrechen. Das Handbuch ist über *Hilfe → NORTEX Code-Handbuch* erreichbar.

## Branding, Activity Bar, Dashboard, Layout

- **Icons**: `python scripts/generate_brand_assets.py` vektorisiert die JPG-Vorlagen (potrace) zu `assets/branding/nortex-symbol.svg`, `assets/icons/app/nortex-code.{svg,ico}` (ICO mit 16/24/32/48/64/128/256 px), `nortex-code-256.png`, `assets/icons/files/*.svg` und Laufzeit-PNGs (`assets/icons/runtime/`, per `include_bytes!`). `ide/build.rs` bettet die ICO per `winres` in `nortex-code.exe` ein (Fehlschlag nur Warnung, Nicht-Windows leer); das Fenstericon kommt über `eframe::NativeOptions`.
- **Activity Bar** (`ui/activity_bar.rs`, 50 px, Icons + Tooltips): Explorer, Search, Git, Debugger, Settings. Search ist echte Textsuche; Git zeigt nur Status (benötigt installiertes `git`); der Debugger ist **nicht implementiert**.
- **Dashboard** (`ui/dashboard.rs`): Klick auf das N-Logo, `ViewMode::Dashboard` mit Projekt-, Toolchain- und Build-Status, Recents und Quick Actions; Editorzustand bleibt erhalten. Settings-Ansicht: `ViewMode::Settings`.
- **Layout** (`ui/layout.rs`): Explorer 180–300 px, Sidecar 220–350 px, Console 120–280 px bei mindestens 15 sichtbaren Codezeilen; Editor ≥ 500 px und ≥ 50 % Breite, Sidecar klappt bei Platzmangel ein (`Strg+Shift+L`), Zen-Modus blendet alles außer dem Editor aus.
- **Tests** (337 bestanden, 0 fehlgeschlagen, 3 ignoriert; IDE-Teil: Lib-Tests plus `ide/tests/edge_cases.rs`): Edge Cases (12.000 Zeilen, Zeile > 2.000 Zeichen, UTF-8-BOM, Umlaut-/Leerzeichenpfade, Emoji, externe Löschung, Schreibschutz), Settings-Reparatur (kaputtes JSON, falsche Typen, Zoom-Clamp), Command-Palette-Grenzen, Layout-Limits.
