# NORTEX Phase 8 – NORTEX Code IDE

**Spezifikationsstatus:** Zielentwurf; die Entscheidung für das UI-Framework ist gefallen (egui/eframe).  
**Implementierungsstatus:** Die egui-basierte IDE „NORTEX Code“ (`ide/`) existiert mit Editor, Explorer, Diagnosen, Build/Run, Terminal und Einstellungen. Debugger (nur Haltepunkt-Verwaltung, kein Backend), Language Server, Plugins/Erweiterungen und Konto-Dienst sind nicht implementiert. Ist-Stand: [IDE-Doku](../ide/README.md) und [Benutzerhandbuch](../user-manual/README.md).

## 8.1 Produktumfang

NORTEX Code ist die offizielle native Desktop-IDE. Hauptbereiche sind Menüleiste, Toolbar, Projekt-Explorer, Editor-Tabs, Code-Editor, Diagnosen, Build-Ausgabe, Terminal, Debugger, Suche, Einstellungen und Pluginverwaltung.

Die IDE verwendet denselben Compiler-Kern wie CLI und Buildsystem. Sie implementiert keine zweite, abweichende Grammatik oder Typprüfung.

## 8.2 UI-Technologie (Entwurf; entschieden: egui/eframe)

Vor einer Entscheidung werden Prototypen mit mindestens zwei, bevorzugt drei Kandidaten verglichen: egui/eframe, Slint und iced. Kriterien sind Textdarstellung, Leistung bei großen Dateien, DPI-Skalierung, Barrierefreiheit, Wartbarkeit, IME-Unterstützung und Lizenzbedingungen. Die Ergebnisse sind mit Messwerten und konkreten Versionsnummern zu dokumentieren.

Die Editor-Engine darf keine UI-Abhängigkeit besitzen. Dadurch bleibt ein späterer Frameworkwechsel möglich.

## 8.3 Editor-Engine

Der Editor-Kern verwendet eine Piece Table mit getrennten Original- und Additionspuffern. Er bietet:

- Änderungsbasierte Undo-/Redo-Historie, zusammengefasste Eingaben und eine konfigurierbare Obergrenze.
- UTF-8-sichere Byteoffsets, Cursor, Auswahl und Zeilen-/Spaltenabbildung.
- Zeilenweisen Zugriff ohne ständige Materialisierung des Gesamtdokuments.
- Öffnen und Speichern von UTF-8-Dateien; BOM und vorhandene LF-/CRLF-/CR-Zeilenenden bleiben erhalten.
- Lexer-basierte Syntaxspannen für NORTEX.

Die aktuelle Graphembewegung deckt häufige kombinierende Zeichen, Emoji-ZWJ-Sequenzen und Flaggenpaare ab, ist aber noch nicht vollständig UAX-#29-konform. Die Piece Table basiert vorerst auf einem Vektor; ein balancierter Baum und inkrementelle Bereichsindizes bleiben mögliche Optimierungen.

Mindestfunktionen der späteren GUI:

- Syntax Highlighting, Zeilennummern, Einrückung und Klammerpaare.
- Cursor, Auswahl, Mehrfachcursor und IME-Eingabe.
- Undo/Redo mit Gruppierung.
- Suchen/Ersetzen und Datei-Tabs.
- Code Folding und Diagnose-Markierungen.
- Autocomplete, Hover und Symbolnavigation.
- DPI-Skalierung, Tastaturbedienbarkeit und barrierearme Kontrastdarstellung.

## 8.4 Language Server

Der NORTEX Language Server verwendet das Language Server Protocol über JSON-RPC und stdio. Er bietet Diagnostics, Completion, Definition, References, Hover, Document Symbols, Formatting und Code Actions. Compilerbibliothek und LSP-Prozess teilen sich Parser und Semantic Analyzer.

Die IDE verwaltet Prozessstart, Neustart, Timeouts und Protokollfehler. Ein Absturz des Language Servers darf ungespeicherte Editorinhalte nicht verlieren. Der aktuelle Stand ruft den Compiler noch direkt im IDE-Prozess auf; ein LSP ist noch nicht vorhanden.

## 8.5 Plugin-Sicherheit

Plugins starten standardmäßig deaktiviert und in einem separaten Prozess. Ein Plugin erhält nur deklarierte Berechtigungen. Der Benutzer kann Dateizugriff, Netzwerk, Prozessstart und Projektänderungen einzeln freigeben. Signaturprüfung und Herkunftsinformation werden angezeigt; nicht signierte Plugins erhalten eine ausdrückliche Warnung.

Plugin-Metadaten umfassen eindeutige ID, Version, Herausgeber, API-Kompatibilität, Berechtigungen, Einstiegspunkt und Prüfsumme. Ein Plugin darf nicht automatisch breiten Prozess- oder Dateisystemzugriff erhalten.

## 8.6 Debugger und Terminal

Zielentwurf (nicht umgesetzt: `nortexc` erzeugt keine Debug-Informationen, es gibt nur die Verwaltung von Haltepunkten): Der Debugger startet Prozesse, setzt Breakpoints, liest Stackframes, Variablen und Threads und unterstützt Step Into/Over/Out. Die erste Version nutzt vorhandene Windows-Debugschnittstellen und PDB-Symbole.

Das integrierte Terminal verwendet einen separaten Prozess (Ist: PowerShell-Sitzung ohne ConPTY, einfache Textanzeige). Shell-Ausführung erfolgt nur auf ausdrückliche Benutzeraktion; Terminalbefehle werden nicht automatisch aus Projektdateien gestartet.

## 8.7 Aktueller Abnahmestand

- **I0 Textkern:** Piece Table, Änderungs-Undo/Redo, Auswahl, Cursor, Zeilenindex und Dokumentdateien sind im Quellcode vorhanden. Automatisierte Unit-Tests sind ergänzt, aber in dieser Arbeitsumgebung nicht mit Cargo ausgeführt.
- **I1 UI-Prototypvergleich:** kein Vergleich dokumentiert; gewählt wurde egui/eframe.
- **I2 grafisches Fenster und Editor:** vorhanden (egui).
- **I3 Lexer-basierte Hervorhebung:** vorhanden.
- **I4 Live-Diagnosen:** Diagnosen im Editor und Compiler Log vorhanden; ein separater LSP fehlt.


## Editor-Core – Stand 0.3.6-dev

Der UI-unabhängige Editor-Kern unterstützt jetzt CR-only-Zeilenenden, explizit bestätigtes verlustbehaftetes UTF-8-Öffnen, gestreamtes atomisches Speichern und ein Bytebudget für Undo/Redo. `for_each_chunk()` ermöglicht stückweise Bereichsausgabe ohne Gesamttext-Allokation; Zeilen-/Spaltenumrechnung zählt direkt über Pieces. Der Piece- und Zeilenindex bleiben dennoch flache Vektoren und damit bei vielen Fragmenten bzw. Zeilen linear zu aktualisieren. Ein balancierter Piece-Tree/Rope und die 100.000-Edits-Leistungsabnahme sind offen. Details: [Editor-Core-Dokumentation](../ide/editor-core.md).
