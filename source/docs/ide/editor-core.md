# NORTEX Code – Editor-Core

## Datenmodell und Komplexität

Der Puffer besteht aus einem Original-String, einem Additions-String und einer geordneten Piece-Liste. Ein Piece verweist auf einen UTF-8-Bytebereich eines der beiden Quellpuffer. `materialize()` baut den Gesamttext nur auf ausdrückliche Anforderung auf; `write_to()` schreibt die Pieces beim Speichern direkt in den Ausgabestream. `slice()` nutzt einen Präfixoffset-Index und beginnt die Suche per Binärsuche beim ersten betroffenen Piece. `for_each_chunk()` liefert dieselben Bereiche stückweise an einen Callback, ohne einen Zwischen-String anzulegen. Zeile-/Spaltenumrechnungen zählen Zeichen direkt über Pieces; sie müssen für eine sehr lange Zeile weiterhin deren Inhalt linear durchlaufen.

Die Piece-Liste ist weiterhin ein `Vec<Piece>`, kein Piece-Tree. Änderungen müssen die Piece-Liste aufteilen und neu zusammensetzen; diese Operation bleibt O(p) mit p = Piece-Anzahl. Der Präfixoffset-Index beschleunigt Lesezugriffe auf Byteoffsets, beseitigt aber die lineare Edit-Komplexität nicht. Bei mehr als 2.048 Pieces oder wenn die Quellpuffer mehr als das Vierfache der sichtbaren Dokumentlänge belegen, wird der sichtbare Text kompakt in den Originalpuffer geschrieben und der alte Additionspuffer freigegeben. Das begrenzt Fragmentierung und totes Backing-Storage, ersetzt aber keinen balancierten Piece-Tree oder Rope.

`line_starts` bleibt ein `Vec<usize>`. Abfragen von Zeilenanfängen verwenden Binärsuche; nachfolgende Offsets müssen bei einem Edit weiterhin angepasst werden. Das Verschieben dieser Einträge ist O(l) mit l = Anzahl nachfolgender Zeilenstarts. Eine gewichtete Rope-/B-Tree-Struktur ist für große, stark fragmentierte Dateien weiterhin der nächste Architektur-Schritt.

## Undo und Redo

Die Historie speichert Edit-Operationen (`start`, entfernter Text, eingefügter Text und Cursorzustände), keine Kopien vollständiger Piece-Listen. Benachbarte Einfügungen am Cursor werden gruppiert. Die Historie ist standardmäßig auf 1.024 Operationen und zusätzlich auf 32 MiB gespeicherte Edit-Texte begrenzt. Beide Grenzwerte können konfiguriert werden. Werden Grenzen überschritten, werden die ältesten Undo-/Redo-Einträge verworfen. Sehr große Einzeländerungen können deshalb nicht rückgängig gemacht werden, wenn sie das Bytebudget überschreiten.

Zustands-IDs erlauben es, nach Undo bis zum letzten gespeicherten Zustand zurückzukehren, ohne den Gesamtinhalt zu hashen. Redo stellt denselben Zustandspfad wieder her.

## UTF-8, Cursor und Auswahl

Alle öffentlichen Editbereiche sind Byteoffsets. Bei einem nichtleeren Löschbereich wird der Anfang auf die vorherige und das Ende auf die nächste UTF-8-Grenze erweitert. Ein leerer Bereich bleibt leer. CRLF wird bei normalen Cursor-/Auswahlbereichen als zusammengehöriger Zeilenumbruch behandelt; interne Undo-/Redo-Operationen können die beiden Bytes gezielt getrennt wiederherstellen, wenn sie aus zwei Eingabeoperationen stammen.

Die Cursorbewegung liest für übliche lange Zeilen nur ein lokales Textfenster und vergrößert es, wenn ein Graphem über den Fensterrand hinausreicht. Die Graphemsegmentierung ist weiterhin eine eigene, unvollständige Implementierung. Sie deckt gängige kombinierende Zeichen, Variation Selector, Emoji-Hauttöne, ZWJ-Sequenzen und Regionalindikator-Paare ab, ist aber nicht vollständig konform zu Unicode UAX #29. Hangul-, Indic- und weitere Unicode-Graphemregeln müssen noch durch eine etablierte Segmentierungsbibliothek oder vollständige Property-Tabellen abgedeckt werden.

Die Auswahl ist ein einzelner Bytebereich. Mehrfachcursor, Maus-Selektion und IME-Kompositionsbereiche sind nicht vorhanden.

## Zeilenindex und Zeilenenden

Der Zeilenindex erkennt LF, CRLF und CR. CRLF zählt als ein Zeilenumbruch, auch wenn CR und LF in getrennten Edit-Operationen eingefügt werden. Der lokale Indexbereich wird nach Änderungen neu ausgewertet, damit CRLF-Paare an Editgrenzen korrekt bleiben. `line()` liefert den Inhalt ohne Zeilenendbytes. Spalten sind nullbasierte Unicode-Skalarpositionen, keine Bildschirm- oder Graphemspalten.

Bei gemischten Zeilenenden bleiben bestehende Bytes erhalten. `EditorDocument::newline_text()` liefert den zuerst erkannten Zeilenumbruchstil für neu eingefügte Zeilen.

## Dokumentdateien und Speichern

`EditorDocument::open()` akzeptiert gültiges UTF-8, erhält einen vorhandenen UTF-8-BOM und lehnt Dateien mit Nullbytes im ersten 8-KiB-Prüfbereich als Binärdateien ab. `open_lossy()` ist eine ausdrückliche Alternative für nicht valides UTF-8. Ein verlustbehaftet geöffnetes Dokument darf erst nach `confirm_lossy_conversion()` überschrieben werden; dadurch wird eine stillschweigende Umwandlung unbekannter Bytes verhindert.

Das Speichern streamt die Pieces in eine temporäre Datei im selben Verzeichnis, ruft `sync_all()` auf und ersetzt das Ziel atomar. Unter Unix werden die vorhandenen Dateimodusrechte bereits beim Anlegen der Temp-Datei gesetzt, wobei die Prozess-Umask weiterhin einschränken darf. Anschließend wird das Verzeichnis synchronisiert. Unter Windows erfolgt der Austausch über `MoveFileExW` mit Replace- und Write-through-Flags. Die Übernahme individueller Windows-ACLs wurde nicht praktisch verifiziert.

Nicht-UTF-8-Encoding-Erkennung bzw. -Konvertierung für Windows-1252/UTF-16, Reload-Erkennung externer Änderungen, File-Locking und eine Binär-/Hexansicht fehlen weiterhin.

## Syntax-Hervorhebung

`highlight_nortex()` verwendet ausschließlich den NORTEX-Lexer. `HighlightCache` speichert die Spans pro Dokument-ID und Revision; ein erneutes Zeichnen desselben Dokuments führt dadurch nicht erneut den Lexer aus. `visible_spans()` filtert die vorhandenen Spans für den Viewport. Bei jeder neuen Revision wird aktuell noch das gesamte Dokument neu lexikalisch analysiert. Dirty-Range-Relexing und Lexer-State-Übergabe über Zeilen sind nicht implementiert.

## Tests und Abnahme

Der Quelltext enthält Tests für Undo/Redo, Operationen- und Bytebudget, Piece-Kompaktierung, Freigabe toter Quellpuffer, UTF-8-Endpunkt-Rundung, CR/LF/CRLF einschließlich getrennter CR-/LF-Eingaben, lange Zeilen, häufige Graphemfolgen, deterministische Editsequenzen gegen ein `String`-Referenzmodell, lossy-open mit ausdrücklicher Speicherbestätigung, Binärdatei-Erkennung und Unix-Dateirechte. Diese Tests müssen mit `cargo test -p nortex-ide` ausgeführt werden.

Zwei ignorierte Leistungsabnahmen sind vorbereitet: 100.000 verteilte Edits in einer 10-MiB-Ein-Zeilen-Datei sowie in einer 10-MiB-Datei mit 10.240 Zeilen. Sie wurden nicht ausgeführt und gelten nicht als bestanden. Der Mehrzeilenfall ist besonders wichtig, weil das flache `line_starts`-Array weiterhin nachfolgende Offsets linear verschiebt. Wegen des weiterhin flachen Piece- und Line-Index-Vektors wird dieses Ziel nicht zugesichert.
