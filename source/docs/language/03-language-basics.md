# Kapitel 3 – Grundlagen der Syntax

← [Kapitel 2](02-installation.md) · [Inhalt](README.md) · Weiter: [Kapitel 4](04-data-types.md)

## Aufbau eines Programms

Eine Quelldatei (`.nx`) besteht aus optionalen `using`-/`namespace`-Zeilen und Deklarationen. Funktionen dürfen direkt auf oberster Ebene stehen. Der native Einstiegspunkt ist **genau eine parameterlose Funktion `public int main()`**; ihr Rückgabewert wird zum Exitcode.

```nortex
using System;

public int main() {
    Console.writeLine("Hallo");
    return 0;
}
```

`void main()`, `main(int a)` und `bool main()` werden beim Build mit „Windows-Programme benötigen derzeit genau eine parameterlose int-main-Funktion“ abgelehnt; ohne `main` scheitert der Linker (`LNK2019`).

`using System;` ist für `Console` im getesteten Pfad nicht nötig, aber üblich. `using` mit unbekanntem Namespace (`using Foo.Bar;`) ergibt `NX1003`.

## Anweisungen, Ausdrücke, Blöcke

- Eine **Anweisung** endet mit `;` (Deklaration, Zuweisung, Aufruf, `return`, `break`, `continue`).
- Ein **Ausdruck** liefert einen Wert (`a * b`, `f(x)`, `c ? 1 : 2`).
- Ein **Block** `{ ... }` gruppiert Anweisungen und bildet einen eigenen Gültigkeitsbereich.

```nortex
public int main() {
    int a = 1;            // Deklaration
    mut int b = a + 2;    // Ausdruck als Initialisierung
    if (b > 2) {          // Block
        b += 1;
    }
    return b;
}
```

Fehlendes Semikolon: `error[NX0100]: Nach einer Variablendeklaration wird ';' erwartet.` Nach Blöcken (`}`) steht kein `;`.

## Kommentare

```nortex
// Zeilenkommentar
/* Blockkommentar */
```

Kommentare dürfen Unicode enthalten.

## Bezeichner

`[A-Za-z_][A-Za-z0-9_]*`, nur ASCII. Groß-/Kleinschreibung ist signifikant (`name`, `Name`, `NAME` sind verschieden). Stilwarnungen `NX0004`: Konstanten sollen `UPPER_SNAKE_CASE`, Variablen/Member `camelCase` sein (Warnung, kein Fehler).

## Schlüsselwörter

Alle reservierten Wörter stehen in [Kapitel 14](14-language-reference.md#schlüsselwörter). `get`/`set` sind normale Bezeichner.

## Typische Fehler

| Fehler | Meldung |
|---|---|
| Unbekanntes Zeichen (`@`) | `NX0100 Unbekanntes Zeichen '@'` |
| Reserviertes `let` | `NX0100 'let' ist reserviert, aber in NORTEX nicht unterstützt` |
| Unbekannter Bezeichner | `NX1004` |
| Doppelte Deklaration im selben Scope | `NX1001` |

## Namenskonventionen (Empfehlung)

Bezeichner bestehen aus ASCII-Buchstaben, Ziffern und `_`; Unicode ist nur in Kommentaren und String-/Char-Literalen erlaubt. Typen `PascalCase` (ohne `I`-Präfix bei Interfaces), Methoden und Variablen `camelCase`, Konstanten `UPPER_SNAKE_CASE`, Modul-/Dateinamen `snake_case`. Zwei-Buchstaben-Akronyme bleiben groß (`IOStream`, `getID`), längere werden wie Wörter behandelt (`XmlParser`). Private Felder tragen keine führenden Unterstriche. Verstöße sind Warnungen (`NX0004`), keine Fehler.

## Geplant (Zielentwurf)

- `using` importiert nur NORTEX-Namespaces; es gibt keine Verbindung zu .NET.
- Sichtbarkeiten `public`, `private`, `protected`, `internal` werden geparst; ihre Durchsetzung über Modulgrenzen fehlt (siehe [Kapitel 14](14-language-reference.md)).
- Das Paketformat `.ntx` ist Teil des Entwurfs, siehe [Kapitel 12](12-compiler-architecture.md#paketformat-ntx-geplant).
