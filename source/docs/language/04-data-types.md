# Kapitel 4 – Datentypen und Variablen

← [Kapitel 3](03-language-basics.md) · [Inhalt](README.md) · Weiter: [Kapitel 5](05-operators.md)

## Primitive Typen

| Typ | Breite | Nativ (Build) |
|---|---|---|
| `int` | 32 Bit, vorzeichenbehaftet | ja |
| `bool` | `true`/`false` | ja |
| `uint`, `short`, `ulong` | 32/16/64 Bit | Locals und Parameter/Rückgabe ja (`uint`-Funktion getestet) |
| `long` | 64 Bit | Parameter/Rückgabe ja; **Locals mit Division/Vergleich können im Codegen abstürzen** (Cranelift-Panic, bekannt) |
| `sbyte`, `byte`, `ushort`, `i128`, `u128`, `nint`, `nuint`, `isize`, `usize` | siehe diese Seite (Abschnitt „Geplant“) | Frontend; nicht einzeln nativ geprüft |
| `float`, `double`, `decimal` | IEEE-754 32/64 bzw. 128 Bit | **nein** (`NX2001`) |
| `char` | Unicode-Skalar | **nein** (`NX2002`) |
| `string` | UTF-8-Slice (Datenzeiger + Byte-Länge) | ja, als Literal, lokale Variable, interner Parameter/Rückgabewert und `Console`-Argument |
| `void` | kein Wert | nur als Rückgabetyp |

## Literale

String-Literale und ihre 16-Byte-Slice-Deskriptoren liegen schreibgeschützt im COFF-Datenbereich. Die Länge zählt UTF-8-Bytes.

- **Konkatenation:** `a + b` ruft `nortex_string_concat` der Runtime auf. Sie reserviert per `HeapAlloc` einen Block aus Deskriptor, Bytes und abschließendem NUL und liefert den neuen Slice. Schlägt die Allokation fehl, bricht das Programm mit einem Trap ab.
- **Globals:** `const string` und `mut string` sind auf Top-Level erlaubt. Der Initialisierer ist ein Stringliteral oder eine `+`-Verkettung von Literalen (zur Compilezeit gefaltet). Das Global ist ein Zeiger auf einen Deskriptor in `.rdata`; `mut string` ersetzt nur den Slice.
- **`extern "C"`:** String-Argumente werden als `char*` (Datenzeiger) übergeben. Alle Strings sind NUL-terminiert; ein eingebettetes `\0` beendet den String aus C-Sicht früher. Externe Funktionen dürfen keinen `string` zurückgeben.
- **Vergleich:** `==` und `!=` rufen `nortex_string_eq` (Längen- und Byte-Vergleich) auf und liefern `bool`. Ordnungsoperatoren (`<`, `>`, …) auf Strings sind nicht implementiert.
- **Drop und Move:** Heap-Strings (Ergebnisse von `+`) gehören genau einer Variable. Beim Scope-Ende, bei `return` und vor dem Überschreiben ruft der Codegen `nortex_string_free` (`HeapFree`) auf, außer der Wert wurde verschoben. Zuweisung, Initialisierung, Rückgabe und Übergabe an interne Funktionen verschieben den Quellstring; jede weitere Verwendung meldet NX1202. Konkatenation, Vergleich, `Console` und `extern "C"`-Aufrufe leihen nur. Statische Literale und `const string` werden nie freigegeben.
- **Grenzen:** Beim Verschieben eines `mut string`-Globals (`string b = name;`, Rückgabe, Argument, Ternary-Zweig) wird der Wert kopiert (`name + ""`); das Global behält seinen Puffer. Beim Neuzuweisen wird der alte Heap-Puffer freigegeben.

```nortex
int a = 42;
int h = 0xFF;          // hexadezimal
int b = 0b1010;        // binär
double d = 1.5e3;      // Exponent (Frontend)
float f = 2.5f;        // Suffix f nötig; 2.5 ist double
char c = 'x';
string s = "text";
bool t = true;
```

Literale werden gegen den Zieltyp geprüft: `int a = 3000000000;` → `NX1102 … liegt außerhalb des Wertebereichs von 'int'`. Integer-Suffixe wie `5L` lexen, scheitern aber im nativen Lowering (`NX2002`). Escapes in Strings: `\n \r \t \0 \" \\`; unbekannte (`\q`) → `NX0100`.

## Variablen

```nortex
int a = 1;          // unveränderlich
mut int b = 2;      // veränderlich
const int LIMIT = 3;
```

- **Unveränderlich ist Standard.** `a = 2;` → `NX1200 Der unveränderliche Wert 'a' darf nicht neu zugewiesen werden.`
- Globale Werte dürfen auch auf oberster Ebene stehen. Sie benötigen einen compile-time auswertbaren Initialisierer; `mut` erlaubt spätere Änderungen. Beispiel: `public const int MAX_PLAYERS = 100;` und `mut int playerCount = 0;`.
- Globale Ganzzahl- und `bool`-Werte werden nativ in schreibgeschützten bzw. beschreibbaren COFF-Datensektionen abgelegt. Andere global deklarierbare Typen werden im Frontend geprüft, sind im nativen MIR-Subset aber noch nicht verfügbar (`NX2002`).
- Auch Parameter und Schleifenvariablen sind unveränderlich; für `for` muss `mut int i` verwendet werden.
- `const` verlangt `UPPER_SNAKE_CASE` (sonst Warnung `NX0004`).
- Jede Variable braucht Typ **und** Initialisierer. `let` ist reserviert und nicht nutzbar. `readonly int a = 1;` ist als lokale Deklaration ungültig.
- `null` ist verboten (`NX1102`).

## Typinferenz

`var c = a * b;` besteht `check`, wird aber im Build abgelehnt: „Variablentyp-Inferenz wird in dieser Lowering-Stufe nicht unterstützt“ (`NX2001`). Im nativen Code immer explizite Typen verwenden.

## Typkonvertierungen

Es gibt **keine implizite Konvertierung zwischen Ganzzahltypen**: `uint + int` → `NX1102 Die Operanden von '+' müssen denselben numerischen Typ besitzen`; `return` eines `byte` aus `int`-Funktion → `NX1102`. Eine funktionierende Cast-Syntax ist nicht vorhanden (`(int)x` und `x as int` werden nicht geparst).

## Gültigkeitsbereiche

Ein Block `{}` bildet einen Scope. Variablen sind ab der Deklaration bis zum Blockende sichtbar; Verwendung außerhalb → `NX1004`. Doppelte Namen im selben Scope → `NX1001`. Ein verschachtelter Scope kann einen äußeren Namen im getesteten Fall nicht neu zuweisen, wenn dieser unveränderlich ist (`NX1200`).

```nortex
public int main() {
    mut int a = 1;
    if (a == 1) {
        int b = 5;
        a = b;          // ok: a ist mut
    }
    return a;           // 5; b ist hier nicht sichtbar
}
```

## Einschränkungen

Nativ nur skalare Ganzzahl-/Bool-Werte. Strings, Gleitkomma, Arrays, Structs und Klassen sind noch nicht ausführbar.

## Geplant (Zielentwurf, nicht ausführbar)

Primitive Typnamen sind case-sensitive und haben keine Aliasse (`Int32`, `String`, `Bool`, `Unit`, `Never` sind ungültig). Implizite verlustbehaftete numerische Konvertierungen sind ausgeschlossen. `char` ist ein 32-Bit-Unicode-Skalarwert; `string` ist UTF-8, `length()` zählt Bytes. `void` ist kein speicherbarer Werttyp; der divergierende Typ heißt `!`. `nint`/`nuint`/`isize`/`usize` sind pointerbreit (Windows x64/ARM64: 64 Bit).

**Typinferenz:** `var x = …;` leitet den Typ aus dem Initialisierer ab (Ist: unveränderlich, `mut var` für veränderbar). Eine globale Inferenz über Parameter, Rückgaben und Generics ist nicht implementiert; mehrdeutige Inferenz soll ein Fehler sein. `const` verlangt einen Compile-Time-Wert.

**Decimal128 (Entwurf):** `decimal` soll ein eigenes 128-Bit-Festkommaformat sein: 1 Vorzeichenbit, 7 Bit Scale (0–38), 120-Bit-Koeffizient (0 bis 10³⁶−1); Wert `(-1)^sign × coefficient / 10^scale`, Rundung *nearest, ties to even*; Überlauf/Division durch null sollen `ArithmeticException` auslösen. Keine stille Umwandlung in `double`. Nur das Format ist spezifiziert, es gibt keine Arithmetik.

**Zusammengesetzte Typen (Entwurf):** `T[]` (dynamisch), `[T; N]` (fest), Tupel `(T1, T2, …)` mit mindestens zwei Elementen, `struct`, `class`, `record` (unveränderlich, strukturelle Gleichheit), `enum` (Summentyp), `interface`, `trait`. `Option<T>` (`Some`/`None`) und `Result<T,E>` (`Ok`/`Err`); `null` gibt es im sicheren Modell nicht.

**Weitere Typkonzepte (Entwurf, nur als Namen bekannt):**

| Typ | Vorgesehene Semantik |
|---|---|
| `object` | explizites Type-Erasure/Boxing, kein universeller Basistyp |
| `dynamic` | dynamische Auflösung mit Laufzeitprüfung (`DynamicDispatchException`) |
| `&T`, `&mut T` | geteilter bzw. exklusiver Borrow |
| `*const T`, `*mut T` | Raw-Pointer, Dereferenzierung nur in `unsafe` |
| `Box<T>` | eindeutiger Heap-Owner |
| `Rc<T>`, `Arc<T>` | geteiltes Eigentum (nicht threadsicher / atomar) |
| `Cell<T>`, `RefCell<T>` | interne Veränderlichkeit (RefCell mit Laufzeitprüfung) |
| `Mutex<T>`, `RwLock<T>` | Synchronisation; `Send`/`Sync` markieren Thread-Sicherheit |
| `Span<T>` | nicht besitzende Sicht auf zusammenhängenden Speicher |
| `Memory<T>` | besitzender Runtime-Handle |
| `Func<…,R>`, `Action<…>` | aufrufbare Werte; Captures folgen Move-/Borrow-Regeln |
| `Future<T>`, `Task<T>` | Berechnung bzw. eingeplante Ausführung, siehe [Kapitel 11](11-errors-and-debugging.md#asynchronität-geplant) |

Die Registrierung eines Typnamens im Compiler belegt keine Implementierung.
