# NORTEX Code – Handbuch (0.3.7-dev)

Kurzanleitung für die Bedienung von NORTEX Code. Dieses einzelne Dokument ist über **Hilfe → NORTEX Code-Handbuch** direkt in der Anwendung verfügbar. Die ausführlichen Kapitel stehen weiterhin im [Benutzerhandbuch-Inhaltsverzeichnis](./README.md).

## 1. Start

- Installer (falls gebaut, siehe `scripts\build_installer.ps1`, benötigt Inno Setup 6 oder 7) oder direkt `bin\nortex-code.exe`.
- Doppelklick auf eine `.nx`-Datei öffnet sie (die Datei wird als Startargument übergeben).
- `nortexc.exe` und `nortex_runtime.lib` müssen neben der IDE-EXE, im Parent oder in `target\release`/`target\debug` liegen.

## 2. Erstes Programm

1. Datei → Ordner öffnen → `examples\language\01_hello_world`.
2. `main.nx` öffnen, **Strg+F5** (Build & Run). Ausgabe im Terminal/Ausgabe-Panel: `Hallo, NORTEX!`.
3. Mit `nortex.toml` im Ordner baut die IDE über das Manifest; Ergebnis: `build\<name>.exe`.
4. Neue Dokumente speicherst du über **Datei → Speichern unter…** oder Strg+Umschalt+S.

## 3. Tastenkarte

| Taste | Aktion |
|---|---|
| Strg+F5 / Strg+Shift+B | Build & Run |
| F2 | Symbol umbenennen (Enter übernimmt, Esc bricht ab) |
| Strg+Alt+M | Minimap ein/aus (`editor.minimap.enabled`) |
| Strg+Mausrad | UI-Zoom in 10%-Schritten |
| Strg+Shift+M | Probleme |
| Strg+Shift+U | Ausgabe |
| Strg+Shift+L | Sidecar |
| Strg+Shift+P | Befehlspalette |

F5/Shift+F5 (Debuggen) sind Platzhalter, es gibt keinen Debugger.

## 4. Editor

- **Breadcrumbs**: Pfad aus Datei und umschließenden Scopes (z. B. `struct Vector3 › fn main()`); Klick springt zur Zeile.
- **Minimap**: 60 px rechts; Klick scrollt.
- **F2-Rename**: ersetzt Bezeichner token-basiert in der aktuellen Datei, lässt Kommentare, Strings und Schlüsselwörter unberührt. Nur ASCII-Namen; kein projektweites Rename.
- **Suchen/Ersetzen**: Groß-/Kleinschreibung, ganze Wörter, RegEx; im Ersatz `$1`, `${name}`.
- **Split-Editor** und Tab-Drag&Drop mit Drop-Zonen.
- Tabs lassen sich am Dateinamen ziehen; **×** schließt einen Tab. Bei ungespeicherten Änderungen kannst du speichern, verwerfen oder abbrechen.
- Speichern schreibt atomar (temporäre Datei, dann Umbenennen).

## 5. Terminal

Ein integriertes Terminal (`powershell.exe`). Mehrere Terminal-Tabs gibt es nicht.

## 6. Einstellungen

`%APPDATA%\NORTEX Code\settings.json`.

## 7. Grenzen (Stand 0.3.7-dev)

- Build für `int`/`bool`, UTF-8-String-Slices, Kontrollfluss, freie Funktionen und `Console.write(line)` mit einem Stringwert; Module per `using` (flacher Namensraum) und `extern "C"` mit int/bool.
- Kein Debugger, kein LSP, kein External-File-Watcher (extern geänderte Dateien werden nicht erkannt).
- Spezifikation: [Ist-Stand](../language/SPECIFICATION_IMPLEMENTED.md).
