# NORTEX Code – IDE-Dokumentation

NORTEX Code ist eine native Desktop-IDE auf Basis von **egui/eframe** (Entscheidung siehe [ui-framework-evaluation.md](ui-framework-evaluation.md)) mit einer eigenständigen, UI-unabhängigen Editor-Engine. Bedienung: [Benutzerhandbuch](../user-manual/README.md); aktueller Funktionsstand: Tabelle dort und [BUILD_STATUS.md](../../BUILD_STATUS.md).

## Branding

- Produktname: NORTEX Code
- Sprache/Compiler: NORTEX
- Hauptfarbe: Rot (`196, 42, 52`)
- IDE-Akzentfarbe: Blau (`44, 112, 190`)
- Stil: modern, minimalistisch, professionell
- Zielgruppe: Anfänger und professionelle Entwickler

## Architekturprinzip

`editor-core` kennt keine Fenster- oder Renderingbibliothek. Dokumente, Textpuffer, Cursor, Auswahl, Undo/Redo, Zeilenindex und Syntaxspannen sind unabhängig von der Oberfläche testbar. Die GUI ist ein Adapter auf diese APIs.

Die Sprachanalyse soll in einem separaten `nortex-language-server`-Prozess laufen. Die aktuelle IDE-Bibliothek ruft den Compiler noch direkt im Prozess auf; JSON-RPC/LSP, Neustart, Zeitlimits und Prozessisolation sind nicht implementiert.

## Implementierter Kern

- Piece Table mit Original- und Additionspuffer; Edits hängen zunächst an den Additionspuffer an, eine Kompaktierung organisiert die sichtbaren Pieces bei Bedarf neu.
- Änderungsoperationen statt Kopien der gesamten Piece-Liste für Undo/Redo.
- Zusammenfassung direkt aufeinanderfolgender Eingaben sowie Begrenzung auf 1.024 Einträge und 32 MiB Historientext.
- Kompaktierung bei mehr als 2.048 Pieces oder stark überdimensionierten Original-/Additionspuffern; Byteoffset-Lookups verwenden einen Präfixindex.
- UTF-8-sichere Bytebereiche, gekapselter Cursor- und Auswahlzustand, Auswahloperationen sowie Zeilen-/Spaltenumrechnung.
- Inkrementeller Zeilenindex; sichtbare Zeilen können einzeln oder über `for_each_chunk()` stückweise abgefragt werden. Die Aktualisierung nach einem Edit verschiebt weiterhin den nachfolgenden Teil des flachen Zeilenvektors.
- Öffnen/Speichern von UTF-8-Dateien; UTF-8-BOM und vorhandene LF-/CRLF-/CR-Zeilenenden bleiben erhalten. Nicht-UTF-8-Dateien können mit `open_lossy()` ausdrücklich verlustbehaftet geöffnet werden; Speichern erfordert danach eine Bestätigung. Binärdateien mit Nullbytes im ersten Prüfblock werden abgelehnt.
- Syntax-Hervorhebung auf Grundlage des NORTEX-Lexers, mit einem Cache pro Dokumentrevision und Viewport-Filterung. Bei einer neuen Revision wird weiterhin das gesamte Dokument neu lexikalisch analysiert.
- Laden des `nortex.toml`-Kernschemas, einschließlich Prüfung, dass die Einstiegsdatei im Projektverzeichnis bleibt.
- Diagnoseausschnitt mit Quellzeile und Caret-Markierung.

Die Graphembewegung deckt häufige kombinierende Zeichen, Variation Selector, Emoji-Modifikatoren, ZWJ-Sequenzen und regionale Flaggenpaare ab, ist aber nicht vollständig UAX-#29-konform. Die Piece Table verwendet weiterhin einen Vektor und keinen balancierten Baum. Präfixoffsets beschleunigen Lesezugriffe; Piece-Edits bleiben O(p), Zeilenindex-Updates können O(l) sein. Der Additions- und Piece-Speicher wird begrenzt kompakt gehalten, aber die Leistungsabnahme „10 MB / 100.000 Edits“ ist noch nicht gemessen und gilt nicht als bestanden.

## Nicht implementiert

Es gibt keinen LSP-Prozess, kein Terminal über ConPTY (das integrierte Terminal startet `powershell.exe`), keinen Debugger, kein IDE-weites Benutzerkonto (Marketplace-Sitzungen sind separat), kein Code-Folding, keine Mehrfachcursor und keine KI-Anbindung im Sidecar. Eine Plugin-Sandbox gibt es nur unter Windows (siehe unten). Piece-Table-Viewport-Rendering ist nicht abgeschlossen.

## Pluginsicherheit

Plugins sind standardmäßig deaktiviert. Die Extensions-Ansicht verwaltet lokale Freigabe-, Aktivierungs- und Seitenleistenmetadaten. Der API-Client installiert Pakete nach Signatur- und Archivprüfung. Freigegebene und aktivierte Erweiterungen laufen nach Bestätigung pro Lauf unter Windows in einem AppContainer mit Job Object; mit `ide_api` im Manifest werden sie als gehostetes Plugin mit versionierter IPC-API gestartet (siehe [Plugin-System](plugins.md)). Linux und macOS verweigern den Start (kein ungesicherter Fallback), der NORTEX-Compiler für `.nx`-Einträge läuft nicht in der Sandbox. Das selbst hostbare Backend muss separat gestartet und für den Client konfiguriert werden; es werden keine Marketplace-Daten simuliert. Details: [Plugin-System](plugins.md) und [Marketplace-Architektur](marketplace.md).

## Code Completion und IntelliSense

Die aktive Datei wird mit dem NORTEX-Lexer und dem Parser (inkrementelles Lexen/Parsen mit Cache) analysiert; Importe werden über den Importgraphen aufgelöst, Typen über `TypeEnv`. Messwerte stehen in [Benchmarks](benchmarks.md). Der inkrementelle Pfad bringt im Tastendruck-Szenario keinen messbaren Vorteil, und die Typinferenz ist nicht vollständig. Details: [Completion und IntelliSense](intellisense.md).
