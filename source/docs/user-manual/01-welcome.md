# 1 Willkommen bei Nortex Code

[Inhaltsverzeichnis](README.md) · [Weiter: Installation](02-installation.md)

## Was ist Nortex Code?

**Nortex Code** ist die grafische Entwicklungsumgebung (IDE, „Integrated Development Environment“) für die Programmiersprache **Nortex**. Eine IDE vereint Texteditor, Dateiübersicht, Compiler-Aufruf und Ausgabe in einem Fenster.

Nortex Code ist ein eigenständiges Windows-Programm. Zum Bauen ruft es den Nortex-Compiler **`nortexc`** auf, der `.nx`-Dateien in eine Windows-Programmdatei (EXE) übersetzt.

## Wofür wird es verwendet?

- Nortex-Quelltext (`.nx`) schreiben,
- per Knopfdruck bauen und starten (**Run**, Strg+F5),
- Compiler-Meldungen mit Position lesen,
- mehrere Dateien in Tabs und geteilten Ansichten bearbeiten.

## Überblick der wichtigsten Funktionen

| Bereich | Funktion |
|---|---|
| Editor | Syntaxhervorhebung, Zeilennummern, Autovervollständigung, Suchen/Ersetzen, Split-Ansicht |
| Build | Run-Knopf, Overflow-Checks, `nortex.toml`-Projekte |
| Ausgabe | Compiler Log, Runtime Output, Terminal (PowerShell) |
| Anpassung | Theme, Dichte, UI- und Editor-Zoom |

Nicht enthalten sind u. a. ein Debugger, Projekt-Assistenten und Datei-Umbenennen/Löschen (siehe [Funktionsstand](README.md#funktionsstand-der-ide)).

## Zusammenhang mit der Sprache

Die Sprache bestimmt, was kompiliert werden kann. Heute baut der Compiler nativ nur ein kleines Teilset (u. a. `int`/`bool`, `if`/`while`/`for`, Funktionen und UTF-8-String-Slices für Variablen, Funktionsaufrufe und `Console.writeLine`). Die IDE erkennt mehr Syntax, als der Build tatsächlich erzeugen kann; Details: [Sprachreferenz](../language/README.md).
