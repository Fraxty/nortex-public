# Kapitel 15 – FAQ und Best Practices

← [Kapitel 14](14-language-reference.md) · [Inhalt](README.md)

## Häufige Anfängerfehler

| Fehler | Ursache / Lösung |
|---|---|
| `NX1200` bei `i += 1` in `for` | Laufvariable `mut` deklarieren: `for (mut int i = 0; …)` |
| `NX1200` bei Parameter ändern | Parameter sind unveränderlich; lokale `mut`-Kopie anlegen. |
| `NX2001`/`NX2002` beim Build | Konstrukt ist nur Frontend (Klassen, Heap-Strings, `var`, `switch`, …). |
| „genau eine parameterlose int-main-Funktion“ | `public int main()` verwenden. |
| `a++` scheitert | `a += 1;` |
| Keine Zahlenausgabe | Ergebnis als Exitcode zurückgeben. |
| `let x = 1;` | Typ angeben: `int x = 1;` |
| `NX1102` bei `uint + int` | Gleiche Typen verwenden; es gibt keine Casts. |

## FAQ

**Warum läuft meine Klasse nicht?** `class`/`struct`/`new` sind im Frontend, aber nicht im nativen Lowering.
**Kann ich Strings speichern?** Ja. Literale, lokale Variablen, Top-Level-`const`/`mut string`, interne Parameter und Rückgaben verwenden UTF-8-Slices; `a + b` erzeugt Heap-Strings (ohne Freigabe).
**Wie debugge ich?** `nortexc check`, `nortexc mir`, Exitcodes; ein Debugger existiert nicht.
**Gibt es Linux?** Nein, nur Windows x64.
**Gibt es Module?** Ja: `using util;` löst `util.nx` bzw. `util/mod.nx` auf (flacher Namensraum), siehe [Ist-Spezifikation](SPECIFICATION_IMPLEMENTED.md).

## Empfohlene Praktiken

- **Namen:** Funktionen/Variablen `camelCase`, Konstanten `UPPER_SNAKE_CASE`, Typen `PascalCase` (entspricht den `NX0004`-Warnungen).
- **Veränderlichkeit:** Nur dort `mut`, wo nötig.
- **Struktur:** Kleine Funktionen mit einem `return`-Pfad; `main` nur als Orchestrator.
- **Fehlerprüfung:** Vor Division auf `0` prüfen (Trap statt Fehlermeldung).
- **Typen:** Im nativen Code `int`/`bool` bevorzugen; explizite Typen statt `var`.
- **Verifikation:** Beispiele nach jeder Änderung bauen und Exitcode prüfen.

## Performance

Der Backend-Optimierer arbeitet auf MIR; Rekursion (`fib`) ist nicht optimiert (kein Tail-Call-Nachweis). Overflow-Prüfung kostet etwas; `--overflow-checks=false` nur bewusst einsetzen, da dann Wraparound ohne Warnung auftritt.

## Größere Projekte

Mit `nortex.toml` bauen ([Kapitel 2](02-installation.md#projekt-mit-manifest)). Mehrere `.nx`-Dateien lassen sich über `using` zusammenführen (flacher Namensraum, keine Sichtbarkeitsprüfung zwischen Modulen; siehe [Ist-Spezifikation](SPECIFICATION_IMPLEMENTED.md)).

## Offene Entscheidungen (Projektinhaber)

Diese Punkte sind nicht freigegeben; der Compiler darf sie nicht als endgültig behandeln:

1. **Lizenz:** Der Repository-Header lautet MIT; das ist keine bestätigte rechtliche Freigabe.
2. **Exceptions:** checked oder unchecked und das native Unwind-Verfahren sind offen. Exceptions dürfen nie unkontrolliert über eine C-ABI-Grenze laufen.
3. **Arithmetik:** Ob Checked-Arithmetik im Release-Standard ist, ist offen (derzeit konfigurierbar über Overflow-Checks, siehe [Manifest](../build-system/manifest.md)).
4. **Methodenreceiver:** `&self`, `&mut self` oder ein anderer Vertrag für Instanzmethoden ist offen.
5. **Backend:** Das Backend ist Cranelift mit MSVC `link.exe`; LLVM ist nicht verbindlich und höchstens ein nicht-normativer Prototyp.

Frühere Entwurfsregeln, die der Ist-Stand überholt hat (Bindungen „standardmäßig veränderbar“, Manifest `.nortex`, Linker `lld-link`, „keine native EXE“), gelten nicht mehr.
