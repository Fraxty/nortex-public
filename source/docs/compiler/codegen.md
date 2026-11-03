# Native Codegenerierung

## Zielarchitektur

NORTEX erhält ein eigenes Backend. Das Backend übersetzt validierte MIR direkt in Windows-COFF-Objektdateien für `x86_64-pc-windows-msvc`. In einem späteren Zielschritt folgt Windows ARM64. Es gibt keine verpflichtende LLVM-Zwischenrepräsentation.

Der externe Link-Schritt verwendet standardmäßig `lld-link`; `link.exe` aus Visual Studio Build Tools ist eine alternative Toolchain-Option. Das Backend bleibt unabhängig vom konkreten Linker.

## Pipeline

`AST → Namensauflösung → Typprüfung → Ownership/Borrow → HIR → MIR → MIR-Optimierung → NORTEX-Codegenerator → COFF-Objekt → Linker → EXE/DLL`

Der Codegenerator muss vor dem ersten ausführbaren Release mindestens Integer-/Float-Arithmetik, Funktionsaufrufe, Kontrollfluss, Datenlayout, globale Daten, Runtime-Aufrufe, Unwind-Informationen und Debugsymbole unterstützen.

**Ist-Stand:** HIR, MIR, Optimierer, Cranelift-Codegenerator (COFF) und MSVC-Linkeraufruf sind für den in [BUILD_STATUS.md](../../BUILD_STATUS.md) beschriebenen Sprachsubset implementiert; `nortexc build` erzeugt native `.exe`. Das Backend ist Cranelift, nicht LLVM.

## Native Klassen und Objektlebensdauer

Klasseninstanzen werden über den Runtime-Import `nortex_object_alloc` im Heap angelegt. MIR-Werte für Klassen sind 64-Bit-Zeiger; Felder sind 8-Byte-Slots. Stringfelder speichern Stringdeskriptoren, Klassenfelder speichern weitere Zeiger. Feldzugriffe prüfen auf Null und trapen, bevor Speicher dereferenziert wird. Konstruktorinitialisierer laufen vor dem Konstruktorrumpf.

Die Runtime besitzt alle Klassenobjekte in einer synchronisierten Arena. Das erlaubt Aliase und Zyklen ohne Referenzzählung; generiertes `main` gibt die Arena bei normaler Rückkehr frei. Einzelobjekte werden nicht während der Ausführung freigegeben, daher kann der Speicherbedarf bis zur Rückkehr aus `main` wachsen. Traps/Prozessabbruch umgehen den expliziten Shutdown und verlassen sich auf die Prozessbereinigung des Betriebssystems. Eingebettete Aufrufer müssen `nortex_object_arena_shutdown` erst nach Ende aller Objektzeiger-Nutzer aufrufen; bei Threads müssen diese vor Rückkehr aus `main` beendet sein. Der Compiler emittiert keine Root-Maps, daher wäre ein sicherer mark-and-sweep-Kollektor mit dem gegenwärtigen ABI nicht möglich.

Explizite geschlossene generische Klasseninstanzen (`new Container<int>(...)`) und generische Instanzmethoden mit konkreten Typargumenten (`value.echo<int>(...)`) werden im HIR monomorphisiert. Einzelvererbung flacht geerbte Feldlayouts ab; abgeleitete Konstruktoren rufen parameterlose Basiskonstruktoren zuerst auf. `virtual`/`override`-Methoden dispatchen nach der dynamischen Klassen-ID. Parameterisierte Basiskonstruktoren, Mehrfachvererbung, generische freie Funktionen, implizite Methoden-Typargument-Inferenz, Bounds und Interfaces/Traits sind nicht unterstützt. Generische Klassen benötigen eine konkrete `new`-Instanziierung im Programm.
