# Parser- und Frontend-Unterstützung – Status 0.3.5-dev

> **Hinweis:** Diese Matrix ist ein Frontend-Snapshot von 0.3.5-dev. Seither implementiert: `extern "C"` (int/bool, Win32), Mehrdatei-Module (`using`), `mut`-Präfix, native Codegenerierung für einen Subset. Maßgeblich: [Sprachreferenz](../language/README.md) und [Ist-Spezifikation](../language/SPECIFICATION_IMPLEMENTED.md). Die Zeilen unten zu `extern "C"`, `using` und Importauflösung sind überholt.

Diese Tabelle beschreibt den tatsächlich vorhandenen Frontend-Subset, nicht das langfristige Sprachziel. „Parser vorhanden“ bedeutet ausdrücklich nicht, dass die vollständige Semantik oder Codegenerierung implementiert ist.

| Syntax | Status im aktuellen Frontend |
|---|---|
| C#-ähnliche Typ-/Funktionsdeklarationen | Teilweise unterstützt |
| Klassen, Structs, Records, Interfaces, Traits, Enums | Teilweise unterstützt |
| Native Klassen (Windows x64) | Runtime-eigene Heap-Referenzen; skalare/String/Klassenreferenz-Felder, Konstruktoren, Methoden, Objektübergabe/-rückgabe/-zuweisung; Arena-Freigabe bei normaler Rückkehr aus `main` |
| Geschlossene generische Klassen und generische Instanzmethoden | Monomorphisiert für konkrete `new`-Instanziierungen bzw. explizite Methoden-Typargumente; keine implizite Methoden-Typargument-Inferenz |
| Einzelvererbung und `virtual`/`override` | Nativ unterstützt; flache Feldlayouts, parameterloser Basiskonstruktor, Dispatch anhand dynamischer Klassen-ID |
| Basistypliste `: Base, IContract, Trait` | Eine Basisklasse im nativen Pfad; Mehrfachvererbung, Interfaces/Traits und deren Vertragsprüfung fehlen |
| Konstruktoren gleichen Namens wie Typ | Grundform wird geparst; `this.field = parameter` kollidiert nicht mehr mit dem gleichnamigen unveränderlichen Parameter; Signatur- und Überladungsprüfung fehlen |
| `get` und `set` als Methoden-/Bezeichnernamen | Normale Identifier, keine reservierten Keywords |
| `let` | Reserviertes Keyword, aber keine unterstützte Deklarationsform |
| Primitive, generische und Arraytypen `T[]`, `[T; N]` | Teilweise unterstützt; Array-Erzeugung `new T[n]` fehlt |
| Hexadezimale, binäre, Exponent- und Suffix-Zahlen | Lexing unterstützt; vollständige Suffix-, Wertebereichs- und Overflow-Prüfung offen |
| Zeichenliterale | Lexing und AST vorhanden; Escape-Validierung begrenzt |
| Präfix-/Postfix- und Infixoperatoren | Pratt-Parser; Präzedenzregressionen ergänzt; vollständige Typprüfung offen |
| Zuweisungen `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=` | Lexing/Parsing im Frontend vorhanden |
| Ternär `condition ? a : b` | AST und Parser vorhanden |
| `await` | Syntaktisch geparst; State-Machine-Lowering fehlt |
| `async Task<T>` mit `return T` | Einfache Rückgabe-Literale werden gegen `T` geprüft; Async-Semantik und Executor fehlen |
| `Box.new(...)` | Membername `new` wird als Ausdruck geparst; Box-Ownership und Konstruktorsemantik fehlen |
| `using` vor oder nach `namespace` | Beide Header-Reihenfolgen werden geparst; allgemeine Importauflösung fehlt |
| `match` mit Block oder Ausdrucksarm | Teilweise unterstützt; Exhaustiveness und vollständige Enum-Auflösung fehlen |
| `foreach` / `for (T value in sequence)` | Parser, AST und erste Elementtypprüfung für Arrays sowie `Iterator<T>` vorhanden |
| Lambdas, Attribute, `extern "C"`, `unsafe {}` | Nicht unterstützt |
| Generische freie Funktionen, `where`, Destruktoren | Nicht unterstützt; generische Instanzmethoden nur mit expliziten konkreten Typargumenten |
| Vollständige `&`/`&mut`-Borrow-Semantik | Nicht unterstützt |
| Vollständige Ausdruckstypprüfung und Constructor overload resolution | Nicht unterstützt |

Die Grammatikdatei wird parallel zum Parser gepflegt. Parserunterstützung ist kein Nachweis dafür, dass die Standardbibliothek, Runtime oder ein nativer Backendpfad die Funktion bereits ausführen können.


## Explizite Methodendeklarationen (0.3.5-dev)

Konkrete Klassenmethoden und freie Funktionen benötigen einen Implementierungsblock.
Eine reine Signatur ohne Rumpf wird mit `NX1005` abgelehnt. Zulässig sind
Signaturen in Interfaces und Traits sowie `abstract`-Methoden in abstrakten
Klassen. `intrinsic` ist ein kontextabhängiger Modifier und wird nur zugelassen,
wenn der Compiler die Prüfung einer compilerverwalteten Quelle unter
`standard-library/` ausdrücklich freigibt. Anwendungscode darf `intrinsic`
nicht verwenden. Ein vollständiger Binder von Intrinsic-Deklarationen auf
Runtime-Symbole ist noch nicht vorhanden.

`extern` ohne explizite ABI-Angabe wird mit `NX1005` abgelehnt. Die Syntax
`extern "C"` und ihre ABI-/Linker-Anbindung sind noch nicht implementiert.
Abstrakte Methoden sind weder `private` noch `static` zulässig und müssen in
einer `abstract`-Klasse stehen. `abstract`, `extern` und `intrinsic` schließen
sich gegenseitig aus; Konstruktoren dürfen keinen dieser Modifier tragen.

## Erste Ausdruckstypprüfung (0.3.5-dev)

Der semantische Frontend-Pass prüft einen begrenzten Ausdrucks-Subset: lokale Initialisierungen, Rückgaben, Bedingungen, Operatoren, ternäre Ausdrücke, bekannte Funktions-/Methoden-/Konstruktoraufrufe, Member, Indexzugriffe und `await`. `for (T value in sequence)` prüft, ob das Iterable ein unterstützter Array- oder `Iterator<T>`-Typ ist. Genericsubstitution, Sichtbarkeit, vollständige Modulauflösung, Rückgabepfade, Ownership und Lifetimes sind noch nicht implementiert.
