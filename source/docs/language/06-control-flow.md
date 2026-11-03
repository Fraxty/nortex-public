# Kapitel 6 – Kontrollstrukturen

← [Kapitel 5](05-operators.md) · [Inhalt](README.md) · Weiter: [Kapitel 7](07-functions-and-methods.md)

Alle Strukturen dieses Kapitels sind in `examples/language/03_conditions.nx` und `04_loops.nx` verifiziert und laufen nativ.

## if / else

```nortex
if (bedingung) { ... } else if (bedingung) { ... } else { ... }
```

Die Bedingung steht in Klammern und muss `bool` sein. Geschweifte Klammern dürfen bei einer einzelnen Anweisung entfallen (`if (a == 1) return 5;` getestet).

```nortex
public int classify(int score) {
    if (score >= 90) {
        return 1;
    } else if (score >= 50) {
        return 2;
    }
    return 3;
}
```

Verschachtelung ist beliebig (`nested_if` getestet). Häufiger Fehler: Nicht-`bool`-Bedingung → `NX1102`.

## while

```nortex
while (bedingung) { ... }
```

Prüft vor jedem Durchlauf. `while (true)` mit `break` ist zulässig. Es gibt **kein** `do … while`.

## for

```nortex
for (mut int i = 0; i < 10; i += 1) { ... }
```

Klassische Form: Initialisierung; Bedingung; Fortschritt. Die Laufvariable muss `mut` sein (sonst `NX1200`). `i++` als Fortschritt scheitert nativ; `i += 1` verwenden. `foreach` / `for (T v in seq)` ist nur im Frontend vorhanden.

## break und continue

`break` verlässt die innerste Schleife, `continue` springt zur nächsten Iteration. Außerhalb: `NX1301` (`break`) bzw. `NX1302` (`continue`).

## return

`return ausdruck;` beendet die Funktion. In Funktionen mit Rückgabetyp ist ein Wert Pflicht (`return;` → `NX1103`); jeder Kontrollpfad muss zurückkehren (`NX1103 Nicht jeder Kontrollfluss … liefert einen Rückgabewert`).

## Ternär

`int t = sum > 5 ? 1 : 2;` – rechts-assoziativ, nativ verfügbar.

## Weitere Konstrukte (nur Frontend)

`switch`/`case`/`default` → im Build `NX2001 Diese Kontrollflussanweisung wird in der aktuellen Lowering-Stufe nicht unterstützt`; `try`/`catch` → ebenso; `match` ist teilweise geparst (Armsyntax im Test nicht akzeptiert, `NX0100`).
