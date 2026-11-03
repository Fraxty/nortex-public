# 8 Fehleranalyse und Debugging

[Zurück](07-terminal-and-consoles.md) · [Inhaltsverzeichnis](README.md) · [Weiter: Einstellungen](09-settings-and-customization.md)

## Fehler erkennen

- Der Editor prüft den Text kurz nach dem Tippen und markiert Fehler direkt im Code.
- Der Reiter **Compiler Log (n)** listet alle Diagnosen; die Statusleiste zeigt „n Fehler“.
- Ein roter Punkt **●** im aktiven Tab bedeutet: Datei hat Fehler.
- Nach einem Build rot/grün in der Statusmeldung.

## Diagnosen lesen

Format im Compiler Log: `Code [Datei:Zeile:Spalte] Meldung`, darunter ggf. `Hilfe: …`. Rot = Fehler, Amber = Warnung.

Beispiel (Tippfehler `summee`, Zeile 3, Spalte 12):

```
NX1004 [main.nx:3:12] Unbekannter Bezeichner 'summee'. Der Wert wurde weder deklariert noch importiert.
  Hilfe: Prüfe die Schreibweise oder deklariere den Wert vor seiner Verwendung.
```

Behebung: Name korrigieren (`summe`). Ein fehlendes Semikolon meldet `NX0100` („Nach einer Variablendeklaration wird ';' erwartet.“); die Position zeigt auf das nächste Token, das Semikolon fehlt also meist in der Zeile **davor**.

Code-Bedeutungen: [Diagnosecodes](../compiler/diagnostic-codes.md) und [Sprachreferenz, Kapitel 11](../language/11-errors-and-debugging.md).

## Typische Probleme

| Meldung | Ursache / Lösung |
|---|---|
| NX2001 / NX2002 | Konstrukt wird geprüft, aber nativ noch nicht erzeugt (z. B. `float`, `string`-Variablen, Strukturen). Auf unterstützten Umfang beschränken |
| Linker-Fehler LNK2019 | Es fehlt `public int main()` |
| „Compiler … konnte nicht gestartet werden“ | `nortexc.exe` nicht gefunden ([Kapitel 2](02-installation.md)) |
| „Build fehlgeschlagen (Exitcode …)“ | Fehler stehen im Compiler Log darüber |

## Debugging-Funktionen

**Echtes Debuggen (Anhalten, Schrittbetrieb, Variablen inspizieren) ist nicht verfügbar.** Der Grund: `nortexc` erzeugt keine Debug-Informationen (z. B. Zeilentabellen). Ohne sie kann die IDE ein laufendes Programm nicht an einer Quellzeile anhalten. Die IDE meldet das offen, statt eine Sitzung vorzutäuschen.

Vorhanden ist die Verwaltung von Haltepunkten und die Oberfläche dafür:

| Funktion | Stand |
|---|---|
| **Haltepunkt setzen/entfernen** | **F9** in der Cursorzeile. Die Datei muss gespeichert sein (neue Dokumente ohne Pfad: Hinweis in der Statusleiste). Die Statusleiste meldet „Breakpoint in Zeile n gesetzt/entfernt“. |
| **Markierung im Editor** | Ein **●** in der Spalte links neben der Zeilennummer markiert Zeilen mit Haltepunkt. |
| **Haltepunkt-Liste** | Debugger-Panel (Activity Bar → Debugger oder Strg+Umschalt+D): „Breakpoints (n)“ mit `Datei:Zeile`. Klick springt zur Zeile, **×** entfernt einen Eintrag, *Alle entfernen* leert die Liste. |
| **Aufrufliste / Variablen** | Das Panel enthält die Bereiche *Aufrufliste* und *Variablen*. Sie zeigen „Keine aktive Sitzung.“, weil noch kein Debugger angebunden ist. |
| **F5 / ▶ Debuggen** | Öffnet das Debugger-Panel und meldet, dass schrittweises Debugging nicht verfügbar ist. Die Sitzung bleibt inaktiv. |
| **■ Stopp** | Beendet den (nicht vorhandenen) Sitzungszustand. |
| **Umschalt+F5** | Meldet „Abbrechen laufender Programme ist nicht implementiert.“ |

Haltepunkte werden nur im laufenden Programm der IDE gehalten und nicht gespeichert; nach einem Neustart sind sie weg. Sie haben keinen Einfluss auf **Strg+F5** (Build & Run).

Fehlersuche funktioniert derzeit über Diagnosen, Programmausgabe und den Exitcode (z. B. `return` eines Zwischenergebnisses, siehe [Kapitel 11](11-first-project.md)).
