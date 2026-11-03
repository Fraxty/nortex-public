# NORTEX Phase 6 – Standardbibliothek

**Spezifikationsstatus:** Modulaufteilung und API-Grundsätze beschlossen  
**Implementierungsstatus:** nur API-Skelette unter `standard-library/` ohne Compilerbindung; gebunden ist ausschließlich die Console-Ausgabe (`Console.write`/`writeLine`) über die C-ABI-Runtime. Keine vollständige Standardbibliothek.

## 6.1 Grundprinzipien

Die Standardbibliothek ist modular, versioniert und mit der Sprach-/Runtime-Version kompatibel. Sie baut auf dokumentierten Runtime-Schnittstellen auf. Öffentliche APIs geben erwartbare Fehler als `Result<T,E>` und optionale Werte als `Option<T>` zurück. Keine API verschluckt Fehler stillschweigend.

Die Basissprache benötigt keine implizite .NET-Abhängigkeit. `using System;` importiert den NORTEX-Namespace `System`.

## 6.2 Module

| Namespace | Verantwortungsbereich | Kernbestandteile |
|---|---|---|
| `System` | Basisfunktionen | `Console`, `Environment`, `ExitCode`, `Version` |
| `System.Collections` | Sammlungen | `List<T>`, `Vec<T>`, `Dictionary<K,V>`, `HashSet<T>`, `Queue<T>`, `Stack<T>` |
| `System.IO` | Dateisystem und Streams | `File`, `Directory`, `FileStream`, `Stream`, `Path` |
| `System.Net` | Netzwerk | `IPAddress`, `Endpoint`, `TcpClient`, `TcpListener`, `UdpClient`, `HttpClient` |
| `System.Threading` | Threads und Synchronisation | `Thread`, `Mutex<T>`, `RwLock<T>`, `CancellationToken` |
| `System.Async` | Tasks | `Task<T>`, `Future<T>`, Task-Gruppen, Scheduler-API |
| `System.Text` | Text und Kodierung | `string`, UTF-8-Encoder/Decoder; kein `StringBuilder` im Sprachziel 1.0 |
| `System.Math` | Mathematik | Konstanten, Integer-/Float-Funktionen, Decimal128-Funktionen |
| `System.Time` | Zeit | `Date`, `Time`, `Duration`, `Instant`, Zeitzonen-API |
| `System.Diagnostics` | Diagnose | `Stopwatch`, `Trace`, `Debug`, Stacktrace |
| `System.Process` | Prozesse | `Process`, Startoptionen, Exitcodes |
| `System.Memory` | Speicher | `Span<T>`, `Memory<T>`, Allocator-Verträge |
| `System.Security` | Sicherheit | Kryptografie, Hashes, Zufallsquellen, Zertifikate |

## 6.3 Collection-Verträge

`List<T>` und `Vec<T>` bleiben eigenständige nominale Typen und sind nicht implizit ineinander konvertierbar.

- `List<T>` ist eine komfortable dynamische Sammlung auf Basis eines zusammenhängenden, vergrößerbaren Puffers. Sie bietet Indexzugriff und amortisiert konstantes Anhängen.
- `Vec<T>` ist ein eigenständiger nominaler Vektortyp mit eigener Low-Level-API (`push`, `pop`, `reserve`, `capacity`, `shrinkToFit`). Beide Typen dürfen intern eine gemeinsame Puffertechnik verwenden, bleiben aber typverschieden.
- Längen und Indizes verwenden `usize`; `get(index)` liefert `Option<&T>`, verändernder Zugriff `Option<&mut T>`.
- `Dictionary<K,V>` benötigt für Schlüssel einen definierten Hash-/Equality-Vertrag.
- `HashSet<T>` verwendet denselben Hash-/Equality-Vertrag.
- `Queue<T>` folgt FIFO, `Stack<T>` LIFO.
- Indexzugriffe mit möglichem Fehler erhalten eine geprüfte API wie `get(index) -> Option<&T>`; ein Indexoperator darf nur dann panicen, wenn dies ausdrücklich als potenziell panikender Zugriff dokumentiert ist.

## 6.4 Fehler, Speicher und Ownership

Allokationsfehler werden nicht durch stilles Zurückgeben leerer Container maskiert. APIs, deren Kapazitätswachstum fehlschlagen kann, verwenden `Result`. `Span<T>` ist eine nicht besitzende, lebenszeitgebundene Sicht. `Memory<T>` ist ein besitzender Handle mit eigenem Freigabevertrag und keine zweite Schreibweise für `Span<T>`.

## 6.5 API- und Versionsregeln

Öffentliche API-Änderungen folgen semantischer Versionierung. Entfernte oder geänderte APIs erhöhen die Hauptversion; additive, kompatible APIs erhöhen die Nebenversion. Experimentelle APIs stehen in einem ausdrücklich als experimentell markierten Namespace und dürfen nicht unbemerkt in den stabilen Kern gelangen.
