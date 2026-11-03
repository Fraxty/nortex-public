# NORTEX Phase 4 – Native Ausführung, Plattformanbindung und Runtime-Kern

**Spezifikationsstatus:** beschlossen für den Zielentwurf 0.3.5-dev  
**Implementierungsstatus:** `nortexc build` erzeugt für ein skalares Subset (Ganzzahlen, `bool`, Kontrollfluss, `Console`-Ausgabe, einfaches `extern "C"`) native Windows-x64-EXEs (Cranelift, MSVC `link.exe`, Runtime `nortex_runtime.lib`). ARM64, Unwind-Daten (`.pdata`/`.xdata`), Debug-Informationen und der Großteil der Runtime-Module sind nicht angebunden.

## 4.1 Zielplattformen

| Priorität | Betriebssystem | Architektur | Zielstatus |
|---|---|---|---|
| 1 | Windows 11 | x86-64 | Primäres Releaseziel |
| 2 | Windows 10 | x86-64 | Kompatibilitätsziel ohne zugesicherten aktuellen Microsoft-Support |
| 3 | Windows 11 | ARM64 | Zweite Architektur |
| 4 | Linux | x86-64 | Spätere Plattform |
| 5 | Linux | ARM64 | Spätere Plattform |
| 6 | macOS | ARM64 | Spätere Plattform |

Windows 10 hat am 14. Oktober 2025 das reguläre Supportende erreicht. NORTEX kann die technische Ausführung auf kompatiblen Windows-10-Systemen ermöglichen; Sicherheitsstatus und Support werden jedoch nicht mit Windows 11 gleichgesetzt.

Jede Plattform erhält ein eigenes Target-Profil. Ein Target besteht aus Betriebssystem, Architektur, ABI, Objektformat, Linkerprofil und Runtime-Variante. Compiler-Host und Compiler-Target sind voneinander getrennt.

## 4.2 ABI-Vertrag

Für Windows x64 wird die Microsoft x64 Calling Convention verwendet. Das Target ist little-endian und besitzt 64-Bit-Pointer. `nint` und `nuint` sind auf Windows x64 und Windows ARM64 jeweils 64 Bit. Die öffentlichen Layoutregeln werden nicht aus zufälligen Rust-Layouts abgeleitet.

### 4.2.1 Windows-x64-Aufrufkonvention im Detail

- Die ersten vier ganzzahligen oder Pointer-Argumente werden positionsabhängig in `RCX`, `RDX`, `R8` und `R9` übergeben.
- Fließkommaargumente in den ersten vier Argumentpositionen werden in `XMM0` bis `XMM3` übergeben.
- Weitere Argumente werden über den Stack übergeben.
- Der Aufrufer reserviert einen 32-Byte-Shadow-Store für die vier Registerargumentpositionen.
- Der Stack muss an den vorgeschriebenen Call-Grenzen 16-Byte-ausgerichtet sein; Prolog/Epilog und Stack-Allocation müssen unwindbar bleiben.
- Skalar-Rückgaben bis 64 Bit erfolgen in `RAX`; Fließkomma-Rückgaben in `XMM0`. Aggregate folgen den Microsoft-ABI-Größen- und Indirektionsregeln.
- Volatile Register dürfen durch den Aufgerufenen verändert werden; nichtvolatile Register müssen nach dem Aufruf wiederhergestellt sein.

Die Details sind Teil des Target-Adapters und müssen durch ABI-Tests gegen einen Microsoft-kompatiblen Compiler verifiziert werden. Es wird kein selbst erfundenes abweichendes Windows-x64-Aufrufprotokoll verwendet.

### 4.2.2 Windows-ARM64-Aufrufkonvention im Detail

Das Windows-ARM64-Target verwendet die AArch64-Windows-ABI auf Little-Endian-Systemen. Sie basiert für normale, nicht variadische Funktionen auf AAPCS64 und enthält Windows-spezifische Regeln. Der Compiler darf deshalb nicht einfach die x64-Regeln oder eine generische Rust-Aufrufkonvention übernehmen.

- Die allgemeinen Parameterregister sind `x0` bis `x7`; die SIMD-/Floating-Point-Parameterregister sind `v0` bis `v7`. Die Zuordnung folgt den AAPCS64-Klassifizierungsregeln und ist positions- und typabhängig.
- `x8` ist das Register für eine indirekte Rückgabeadresse, wenn der Rückgabetyp nicht direkt in den Ergebnisregistern zurückgegeben werden kann.
- Ganzzahlige und Pointer-Skalarwerte werden grundsätzlich in `x0` zurückgegeben; Gleitkommawerte in `v0`. Kleine Aggregate können mehrere Ergebnisregister belegen. Homogene Floating-Point-Aggregate folgen den HFA-Regeln. Größere oder anderweitig indirekt zurückzugebende Aggregate verwenden den von der ABI vorgesehenen Rückgabepointer.
- `x0` bis `x17` sind volatile Register. `x18` ist ein reserviertes Plattformregister und darf vom NORTEX-Codegenerator nicht als allgemeines temporäres Register verwendet werden.
- `x19` bis `x28` sind nichtvolatile Register und müssen bei Verwendung gesichert und wiederhergestellt werden. `x29` ist der Frame Pointer; `x30` enthält die Link Register-Rücksprungadresse.
- Bei SIMD-Registern sind `v0` bis `v7` volatile. Bei `v8` bis `v15` müssen die unteren 64 Bit erhalten bleiben; die oberen 64 Bit sind volatile. `v16` bis `v31` sind volatile.
- Der Stack muss jederzeit 16-Byte-ausgerichtet sein. Stack-Frames und Prolog-/Epilog-Sequenzen müssen mit den Windows-ARM64-Unwind-Regeln zusammenpassen.
- Variadische Funktionen erhalten eine gesonderte ABI-Behandlung. Sie werden nicht automatisch wie normale Funktionen klassifiziert und sind erst nach expliziter Implementierung für FFI freizugeben.

Die genaue Klassifizierung von Aggregaten, Homogeneous Floating-point Aggregates (HFA), Homogeneous Vector Aggregates (HVA), variadischen Argumenten und indirekten Übergaben muss der Target-Adapter anhand der Microsoft-ABI-Spezifikation implementieren. Die oben genannten Regeln sind der Architekturvertrag, aber noch kein Nachweis eines funktionierenden ARM64-Codegenerators.

### 4.2.3 Interne und externe Layouts

- Normale NORTEX-Structs verwenden ein compilerdefiniertes Layout. Felder dürfen für Alignment und Optimierung umgeordnet werden, sofern keine beobachtbare Sprachregel verletzt wird.
- `extern "C"`-Deklarationen und mit `[repr(C)]` markierte Datentypen folgen dem ABI des konkreten Targets.
- Bitfelder, implizite gepackte Structs und nicht dokumentierte Enum-Layouts sind nicht Teil des C-ABI-Vertrags.
- Enum- und Summentypen werden an FFI-Grenzen nur in ausdrücklich freigegebenen Repräsentationen übertragen.
- `bool` wird am C-ABI-Rand als festgelegter 8-Bit-Wert mit den Werten 0 und 1 abgebildet.
- `string`, Klassenreferenzen, `Task<T>`, `Future<T>` und Exceptions sind keine direkt übertragbaren C-ABI-Typen.

### 4.2.4 C-ABI-Funktionen

```nortex
extern "C"
public int calculate(int first, int second)
{
    return first + second;
}
```

Eine FFI-Funktion verwendet ausschließlich ABI-fähige Parameter und Rückgabewerte. Für Text wird ein explizites Paar aus UTF-8-Zeiger und Byte-Länge verwendet. Eine Funktion, die Speicher zurückgibt, muss eine passende Freigabefunktion derselben Runtime bzw. Bibliothek bereitstellen. Allokation in einer CRT und Freigabe in einer anderen CRT ist verboten, sofern kein gemeinsamer Allocator-Vertrag vereinbart wurde.

### 4.2.5 Symbolnamen und Versionen

C-ABI-Exporte behalten den vereinbarten externen Symbolnamen. Interne NORTEX-Symbole werden deterministisch gemangelt und enthalten Modul, Typ, Funktionsname und gegebenenfalls generische Instanziierung. Das Mangle-Format ist versionsgebunden und wird nicht als stabile Fremd-ABI beworben. Ein ABI-Break erhöht die ABI-Hauptversion.

## 4.3 Runtime-Kern

Die Runtime ist eine native Unterstützungsbibliothek. Sie definiert keine alternative Sprachsemantik und ersetzt weder Compilerprüfung noch Ownership-Regeln.

| Modul | Verbindliche Verantwortung |
|---|---|
| Memory | Speicherbereiche, Alignment, sichere Views und Laufzeitmetadaten |
| Allocation | Allocator-Schnittstellen, Allokations-/Freigabepfade und Diagnosestatistik |
| Collections | generische Laufzeitoperationen, soweit nicht vollständig inline generiert |
| Exceptions | Exception-Payload, Typmetadaten und Unwinding-Unterstützung |
| Async | Task-Zustände, Scheduler, Continuations und Cancellation |
| Threading | Worker, Mutexes, RW-Locks, Atomics und Thread-Identität |
| IO | Dateien, Streams, Konsolenzugriff und asynchrones I/O |
| Networking | Socket- und Resolver-Abstraktion |
| Process | Prozessstart, Exitcode, Argumente und Umgebungsvariablen |
| Platform | Betriebssystemabstraktion und systemnahe APIs |
| Debug | Stacktraces, Symbolinformationen und Debugger-Hooks |

## 4.4 Speicherverwaltung

NORTEX besitzt keinen Garbage Collector. Der Compiler erzwingt Ownership, Borrowing und Move-Regeln. Die Runtime stellt Allokatoren bereit, übernimmt aber nicht die Entscheidung, wem ein Wert gehört.

- `Box<T>` ist ein eindeutiger Heap-Owner.
- `Rc<T>` ist nicht threadsicher und verwendet Referenzzählung für explizit gemeinsam genutzte Werte innerhalb eines Threads.
- `Arc<T>` ist threadsicher und verwendet atomare Referenzzählung.
- `Clone` ist explizit und kann Ressourcen duplizieren; es ist niemals automatisch gleichbedeutend mit einem Move.
- Destruktoren werden deterministisch und in umgekehrter Initialisierungsreihenfolge ausgeführt.
- `unsafe` kann die statischen Garantien für eine abgegrenzte Operation aufheben; die Verantwortung liegt beim Autor dieses Blocks.

## 4.5 Runtime und Standardbibliothek

Runtime und Standardbibliothek werden getrennt versioniert. Die Runtime stellt ABI-nahe Basisdienste zur Verfügung. Die Standardbibliothek liefert benutzerfreundliche Typen und Funktionen. Die Standardbibliothek darf keine private Runtime-Implementierung voraussetzen, sondern verwendet ausschließlich dokumentierte Runtime-Schnittstellen.

## 4.6 ARM64-Unwinding und Referenzen zur Windows-ABI

Auf Windows ARM64 werden Funktions-Unwind-Informationen nach dem Windows-ARM64-SEH-Modell über `.pdata` und zugehörige `.xdata`-Informationen beschrieben. Prologe und Epiloge müssen vom Unwind-Format darstellbar sein; dynamisch erzeugte oder ungewöhnliche Stack-Frames benötigen eine gesonderte Prüfung. Ein ARM64-Backend darf keine x64-Unwind-Daten erzeugen.

Die ABI-Implementierung ist durch Cross-Compiler-Tests gegen MSVC/clang-cl und durch Stackwalk-/Exception-Tests auf echter Windows-ARM64-Hardware zu verifizieren. Diese Tests sind noch nicht vorhanden.



- [Microsoft Learn – x64 Calling Convention](https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention?view=msvc-170)
- [Microsoft Learn – x64 Exception Handling](https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64?view=msvc-170)
- [Microsoft C++ Documentation – ARM64 ABI conventions](https://github.com/MicrosoftDocs/cpp-docs/blob/main/docs/build/arm64-windows-abi-conventions.md)
- [Microsoft C++ Documentation – ARM64 exception handling](https://github.com/MicrosoftDocs/cpp-docs/blob/main/docs/build/arm64-exception-handling.md)

Der Windows-x64-Codegenerator muss für Nicht-Blattfunktionen die erforderlichen Unwind-Daten in den PE-Abschnitten `.pdata`/`.xdata` bereitstellen. Ohne korrekte Unwind-Informationen sind Exception-Unwinding, Stackwalks und Debugging nicht verlässlich.
