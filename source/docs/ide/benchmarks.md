# IntelliSense-Benchmarks

Messungen für Lexer, Parser und Completion (inklusive Import-Auflösung, Symbolauflösung und `TypeEnv`). Alle Werte stammen aus dem unten genannten Lauf; es wurden keine Werte geschätzt und keine älteren Versionen verglichen (es existieren keine Vergleichsmessungen).

## Wiederholen

```powershell
cargo test -p nortex-ide --release --test intellisense_bench -- --ignored --nocapture --test-threads=1
```

- Quelle: [ide/tests/intellisense_bench.rs](../../ide/tests/intellisense_bench.rs), nur Standardbibliothek.
- Durchläufe je Messung: 15 (`NORTEX_BENCH_RUNS` überschreibt den Wert). Angegeben sind Min, Median, Mittel und Max.
- Speicher: Ein zählender globaler Allokator misst den Spitzenwert des Heaps während der Messung abzüglich des Stands vor der Messung ("Peak-Heap"). Er erfasst nur Rust-Heap-Allokationen, keinen Stack und keinen Prozess-Working-Set.
- Eingaben werden generiert (siehe unten). "Kalt" heißt: frischer Projektordner je Durchlauf, damit der prozessweite Modul-Cache nicht trifft; Anlegen der Dateien zählt nicht zur Zeit. "Warm" heißt: gleicher Ordner, Cache gefüllt.
- Jede Completion-Messung prüft, dass genau die drei erwarteten Member (`value`, `label`, `total`) geliefert werden; ein Lauf mit leerem Ergebnis bricht ab.

## Testbedingungen

| | |
|---|---|
| CPU | AMD Ryzen 9 7900X, 12 Kerne / 24 Threads |
| Betriebssystem | Windows 10.0.26300 |
| Rust | 1.88.0 |
| Build | `--release` (Workspace-Standardprofil, ohne zusätzliche Einstellungen) |
| Ausführung | ein Testthread, Einzelmaschine, Hintergrundlast nicht kontrolliert |

## Eingaben

- Große Dateien: `N/4` Mal `public class ShapeN { public int width; public int area(int scale) { … } }`.
- Syntaxfehler: 2000 Mal `int valueN = ;` und `class { public int`.
- Projekte: `m0.nx … m(n-1).nx`, je Modul eine Klasse mit zwei Feldern, einem privaten Feld und einer Methode plus eine Fabrikfunktion. "Kette": Modul *i* importiert *i-1*, die Hauptdatei nur das letzte. "Direkt": die Hauptdatei importiert alle. Completion nach `item.` mit `item` aus der Fabrikfunktion des letzten Moduls.

## Ergebnisse (ein Lauf, 15 Durchläufe je Zeile)

| Messung | Eingabe | Min | Median | Mittel | Max | Peak-Heap |
|---|---|---:|---:|---:|---:|---:|
| Lexer (voll) | 1000 Zeilen, 23 KiB | 594.2 µs | 700.8 µs | 699.9 µs | 816.4 µs | 787.7 KiB |
| Parser (voll, ohne Lexen) | 1000 Zeilen, 23 KiB | 792.9 µs | 866.2 µs | 888.7 µs | 1.03 ms | 1.24 MiB |
| Lexer (voll) | 10000 Zeilen, 238 KiB | 6.48 ms | 6.63 ms | 6.64 ms | 6.93 ms | 6.19 MiB |
| Parser (voll, ohne Lexen) | 10000 Zeilen, 238 KiB | 8.16 ms | 8.75 ms | 8.74 ms | 9.36 ms | 13.07 MiB |
| Lexer (voll) | 50000 Zeilen, 1197 KiB | 35.79 ms | 36.37 ms | 36.51 ms | 38.07 ms | 48.98 MiB |
| Parser (voll, ohne Lexen) | 50000 Zeilen, 1197 KiB | 50.03 ms | 50.83 ms | 50.76 ms | 51.65 ms | 63.47 MiB |
| Lexer + Parser, viele Syntaxfehler | 4000 Zeilen | 4.04 ms | 4.21 ms | 4.24 ms | 4.71 ms | 3.40 MiB |
| Completion, lokale Symbole, ohne Datei | 5 Zeilen | 14.4 µs | 15.0 µs | 19.6 µs | 74.7 µs | 18.2 KiB |
| Completion Member, aktive Datei | 10000 Zeilen, 2500 Klassen | 30.99 ms | 31.66 ms | 32.33 ms | 38.14 ms | 41.48 MiB |
| Completion pro Tastendruck, aktive Datei | 10000 Zeilen, Zeichen anhängen | 31.61 ms | 33.72 ms | 34.30 ms | 39.91 ms | 41.48 MiB |
| Completion Member, Imports kalt | 1 Modul | 738.6 µs | 863.5 µs | 1.48 ms | 6.53 ms | 16.8 KiB |
| Completion Member, Imports warm | 1 Modul | 161.8 µs | 199.5 µs | 199.7 µs | 234.0 µs | 9.6 KiB |
| Completion Member, Imports kalt | 10 Module (Kette) | 2.64 ms | 3.47 ms | 3.55 ms | 6.20 ms | 126.4 KiB |
| Completion Member, Imports warm | 10 Module (Kette) | 868.1 µs | 914.2 µs | 909.6 µs | 950.2 µs | 9.6 KiB |
| Completion Member, Imports kalt | 10 Module (direkt) | 2.60 ms | 2.81 ms | 3.01 ms | 6.11 ms | 145.1 KiB |
| Completion Member, Imports warm | 10 Module (direkt) | 872.3 µs | 908.8 µs | 919.0 µs | 1.01 ms | 19.5 KiB |
| Completion Member, Imports kalt | 100 Module (Kette) | 22.77 ms | 23.87 ms | 24.17 ms | 28.97 ms | 1.05 MiB |
| Completion Member, Imports warm | 100 Module (Kette) | 8.32 ms | 8.47 ms | 8.50 ms | 8.90 ms | 25.1 KiB |
| Completion Member, Imports kalt | 100 Module (direkt) | 22.46 ms | 24.83 ms | 24.46 ms | 26.51 ms | 1.03 MiB |
| Completion Member, Imports warm | 100 Module (direkt) | 8.54 ms | 9.22 ms | 9.45 ms | 14.36 ms | 676.0 KiB |
| Completion Member, Imports kalt | 400 Module (Kette) | 89.88 ms | 97.09 ms | 96.75 ms | 102.90 ms | 4.19 MiB |
| Completion Member, Imports warm | 400 Module (Kette) | 33.78 ms | 36.73 ms | 36.26 ms | 39.46 ms | 98.4 KiB |

## Was die Werte zeigen

- Lexer und Parser skalieren etwa linear (50× mehr Zeilen ≈ 52× Lexer-Zeit).
- Der Modul-Cache spart gegenüber kalt etwa 60–65 % der Zeit, entfernt die Kosten aber nicht: Auch warm wird jedes Modul pro Anfrage auf Änderungen geprüft (Dateistempel). Das ergibt rund 85 µs je Modul (8,5 ms bei 100, 36 ms bei 400 Modulen).
- Bei der aktiven Datei bringt der inkrementelle Lex-/Parse-Pfad im Tastendruck-Szenario keinen messbaren Vorteil: 33,7 ms je Tastendruck gegenüber 31,7 ms für eine einzelne Anfrage.

## Completion: Phasen, Cache und asynchroner Worker

Messung mit `ide/tests/completion_phases.rs` (Median aus 15, Release, ms je Anfrage; Phasen wurden mit `analysis::timed` gemessen):

| Szenario | Wand | Lexen | Parsen | Symbole/Typen |
|---|---:|---:|---:|---:|
| 10 000 Zeilen, unverändert, vor Cache-Fix | 31,4 | 6,0 | 11,5 | 9,4 |
| 10 000 Zeilen, unverändert, nach Cache-Fix | 10,8 | – | – | – |
| 50 000 Zeilen, unverändert, vor / nach Cache-Fix | 347,7 / 269,8 | | | |
| 50 000 Zeilen, erste Anfrage | 374,6 | 45,5 | 81,1 | 225,9 |
| 1000 Zeilen, unverändert, vor / nach Cache-Fix | 2,13 / 0,31 | | | |

- Befund: Der Analyse-Cache hatte vorher nie einen Treffer. `close_open_blocks` analysiert je Anfrage zwei Texte (Original und Original plus `\n}`), und der Ein-Eintrag-Cache wurde abwechselnd überschrieben. Der Cache hält jetzt zwei Einträge (LRU).
- Der Fix hilft nur bei unverändertem Text (z. B. Strg+Leertaste ohne Änderung). Beim Tippen ist es nicht schneller: 10 000 Zeilen am Ende 31,2 → 36,9 ms, in der Mitte 37,2 → 40,9 ms. Dieser Anstieg ist nicht untersucht (Rauschen oder Overhead).
- Symbol- und Typauflösung ist der größte Block (≈ 10 ms bei 10 000, ≈ 226–260 ms bei 50 000 Zeilen).

Die Completion läuft seit dieser Änderung in `editor/completion_worker.rs` auf einem eigenen Thread. Messung mit `ide/tests/completion_async.rs` (Median aus 15, Release, Tippen eines Bezeichners am Dateiende, ms):

| Zeilen | UI-Thread (Text klonen, `submit`, `poll`) | bis Ergebnis |
|---:|---:|---:|
| 1000 | 0,003 | 1,59 |
| 10 000 | 0,006 | 26,87 |
| 50 000 | 0,161 | 350,10 |

Der UI-Thread zahlt nur noch Klonen und Übergabe. Die Analyse selbst wird dadurch nicht billiger: Das Popup erscheint nach derselben Zeit wie vorher, blockiert aber das Tippen nicht. Die Messung deckt den Worker ab, nicht die gesamte UI (kein Frame-Zeit-Test). Die Latenz enthält ein Polling mit 200 µs Schlafzeit.

Grenzen: Eine laufende Analyse ist nicht unterbrechbar; veraltete Ergebnisse werden nur verworfen. Pro Anfrage werden Text und Dateiliste geklont.

## Einschränkungen

- Eine Maschine, ein Lauf; Rauschen durch Hintergrundprozesse ist nicht kontrolliert. Maxima bei kleinen Eingaben (z. B. 6,53 ms bei "1 Modul kalt") sind Ausreißer durch Dateisystemzugriffe.
- Gemessen werden synthetische, gleichförmige Projekte, keine echten Anwendungen.
- Nicht gemessen: IDE-Gesamtprozess-Speicher, Projektindex über viele Dateien im Explorer, Plugin-Host-Speicher und Sandbox-Startdauer (die IPC-Messung steht im Abschnitt „Plugin-IPC“), Compiler-Phasen nach dem Frontend. Compiler-Build-Zeiten: [docs/release/BENCHMARKS.md](../release/BENCHMARKS.md).
- Es gibt keine automatische Regressionsschwelle; Vergleiche mit künftigen Versionen müssen manuell mit demselben Befehl erfolgen.

## Plugin-IPC

Gemessen mit echten Kindprozessen (`ide/examples/plugin_stub.rs`, Modus `bench`) und dem echten `PluginHost`. Das Plugin misst die Round-Trip-Zeit selbst (Anfrage geschrieben bis Antwort gelesen), der Host misst die Dauer seiner `pump`-Aufrufe.

```text
cargo build --release -p nortex-ide --example plugin_stub
cargo test -p nortex-ide --release --test plugin_ipc_bench -- --ignored --nocapture --test-threads=1
```

Testbedingungen: Windows 10.0.26300 (x86_64), AMD64 Family 25 Model 97 (24 logische Prozessoren), Release-Profil, Median aus 7 Läufen, Zeiten in µs. Für die Messung ist das Rate-Limit des Hosts auf 1 000 000 Anfragen/s angehoben (Standard: 100/s, siehe unten). Jedes Szenario läuft in zwei getrennten Stufen: als normaler Kindprozess und im AppContainer (`launch_sandboxed`, Stub-Kopie pro Plugin in einem eigenen Temp-Verzeichnis). Die Plugin-Spalten stammen aus dem Plugin-Prozess selbst (`GetProcessTimes`, `K32GetProcessMemoryInfo`).

Ohne AppContainer (normaler Kindprozess):

| Szenario | Anfragen/s | min | p50 | p95 | p99 | max | pump max | pump Auslastung % | Plugin-CPU µs/Anfrage | Plugin-Spitzenspeicher KiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| klein (16 Z.), nacheinander, Pump ohne Pause | 61213 | 11 | 14 | 27 | 44 | 109 | 46 | 52.1 | 16 | 4204 |
| groß (1900 Z.), nacheinander, Pump ohne Pause | 45085 | 17 | 19 | 37 | 56 | 137 | 44 | 53.1 | 16 | 4224 |
| Antwort 24 KiB (document.active), nacheinander | 16813 | 45 | 52 | 93 | 125 | 176 | 88 | 67.6 | 31 | 4236 |
| klein, 16 gleichzeitig offen | 280804 | 11 | 53 | 80 | 109 | 167 | 41 | 60.5 | 3 | 4228 |
| klein, 4 Plugins parallel, je nacheinander | 142506 | 13 | 25 | 36 | 44 | 2526 | 55 | 73.1 | 14 | 4208 |
| klein, nacheinander, Pump alle 16 ms (UI-Takt) | 61 | 14106 | 16295 | 16512 | 16562 | 16576 | 52 | 0.1 | 0 | 4180 |
| klein, 16 offen, Pump alle 16 ms (UI-Takt) | 986 | 14142 | 16287 | 16533 | 16561 | 16575 | 42 | 0.2 | 0 | 4180 |

Im AppContainer (`launch_sandboxed`, Low-Integrity-Token, Job-Limits):

| Szenario | Anfragen/s | min | p50 | p95 | p99 | max | pump max | pump Auslastung % | Plugin-CPU µs/Anfrage | Plugin-Spitzenspeicher KiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| klein (16 Z.), nacheinander, Pump ohne Pause | 56664 | 10 | 15 | 26 | 43 | 126 | 42 | 52.5 | 16 | 4712 |
| groß (1900 Z.), nacheinander, Pump ohne Pause | 47750 | 16 | 17 | 35 | 56 | 131 | 100 | 53.8 | 8 | 4740 |
| Antwort 24 KiB (document.active), nacheinander | 15484 | 50 | 59 | 92 | 128 | 240 | 116 | 63.3 | 47 | 4748 |
| klein, 16 gleichzeitig offen | 250639 | 12 | 56 | 90 | 141 | 178 | 38 | 59.7 | 3 | 4744 |
| klein, 4 Plugins parallel, je nacheinander | 32946 | 14 | 30 | 40 | 54 | 180737 | 122 | 69.4 | 20 | 4720 |
| klein, nacheinander, Pump alle 16 ms (UI-Takt) | 62 | 3948 | 16284 | 16517 | 16543 | 16590 | 53 | 0.1 | 156 | 4800 |
| klein, 16 offen, Pump alle 16 ms (UI-Takt) | 1021 | 4012 | 16268 | 16489 | 16554 | 16556 | 43 | 0.2 | 0 | 4692 |

Hinweise zur Lesart: Die CPU-Zeit stammt aus `GetProcessTimes` mit einer Auflösung von etwa 15,6 ms. Bei kurzen Läufen (100–320 Anfragen im UI-Takt) ist der Wert deshalb quantisiert; „156“ ist ein einzelner Zeittick, „0“ bedeutet unter der Auflösung. Belastbar sind nur die Schleifenszenarien. Der Spitzenspeicher ist der Arbeitssatz des Plugin-Prozesses (Maximum über alle Plugins eines Laufs), nicht der des Hosts.

| Frame (JSON) | Kodieren ns | Parsen + Validieren ns | Zeilenlänge Byte |
|---|---:|---:|---:|
| log, 16 Zeichen | 123 | 478 | 109 |
| log, 2000 Zeichen | 1014 | 759 | 2093 |
| Antwort, 24 KiB Dokument | 10478 | 3631 | 24674 |

| Zeitüberschreitung | konfiguriert | gemessen bis Endzustand |
|---|---:|---:|
| Handshake (Plugin schweigt) | 300 ms | 307 ms |
| Shutdown (Plugin ignoriert) | 300 ms | 302 ms |

Aussagen:

- Die Pipe selbst ist nicht der Engpass: Eine Anfrage kostet im engen Schleifenbetrieb etwa 14–53 µs.
- Im IDE-Betrieb bestimmt die Pump-Kadenz die Latenz. Wird pro UI-Frame (16 ms) einmal gepumpt, liegt der Round-Trip bei etwa 16 ms, und ein Plugin mit je einer offenen Anfrage schafft nur etwa 61 Anfragen/s. Mit 16 offenen Anfragen sind es etwa 988/s. Pipelining lohnt sich also.
- Die `pump`-Dauer bleibt im Maximum bei etwa 0,1 ms (ohne AppContainer ≤ 0,09 ms, im AppContainer ≤ 0,12 ms); die Reaktionsfähigkeit der IDE ist damit nicht gefährdet. Die hohe „Auslastung“ in den Schleifenszenarien entsteht nur durch das Pumpen ohne Pause.
- Der AppContainer kostet im Einzelplugin-Betrieb wenig: p50 15 statt 14 µs (klein), 59 statt 52 µs (24 KiB), Durchsatz bei 16 offenen Anfragen 250 639 statt 280 804/s. Der Arbeitssatz des Plugin-Prozesses liegt etwa 500 KiB höher (4,7 statt 4,2 MiB).
- Auffällig ist „4 Plugins parallel“ im AppContainer: 32 946 statt 142 506 Anfragen/s und ein Maximum von 180 737 µs bei einer einzelnen Anfrage. Die Ursache wurde nicht untersucht; es kann am Start mehrerer Sandbox-Prozesse oder an der Messung liegen. Der Befund ist nicht erklärt und wird hier nicht als Eigenschaft der Sandbox behauptet.
- Das Standard-Rate-Limit (100 Anfragen/s je Plugin) liegt weit unter dem technischen Durchsatz und ist die tatsächliche Obergrenze im Normalbetrieb.
- Der Ausreißer von 2526 µs bei vier Plugins ohne AppContainer ist ein einzelner Scheduler-Ausschlag.

Nicht gemessen: Linux und macOS (dort gibt es keine Sandbox; Plugin-Ausführung ist fail-closed abgelehnt), Speicher und CPU des Hosts selbst, Dauer des Sandbox-Starts. Eine Maschine, kein kontrolliertes Rauschen.
