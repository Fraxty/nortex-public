# Kapitel 11 – Fehlermeldungen und Debugging

← [Kapitel 10](10-input-output.md) · [Inhalt](README.md) · Weiter: [Kapitel 12](12-compiler-architecture.md)

## Format

```text
error[NX1102]: Typfehler in Variableninitialisierung: erwartet 'float', erhalten 'double'.
 --> datei.nx:7:19
   |
 7 |     float f = 2.5;
   |               ^^^
```

Meldungen sind deutsch; Codes (`NXnnnn`) sind stabil. Severity: `error` oder `warning`. `nortexc` bricht bei Fehlern mit Exitcode ≠ 0 ab und schreibt keine EXE.

## Fehlerklassen mit tatsächlich beobachteten Codes

| Klasse | Code | Beispiel / Behebung |
|---|---|---|
| Lexikalisch/Syntax | `NX0100` | `int a = ;` · `@` · fehlendes `;` · nicht abgeschlossener String · unbekannte Escape `\q`. Syntax korrigieren. |
| Doppelte Deklaration | `NX1001` | `int a` zweimal im Scope → umbenennen. |
| Namensauflösung | `NX1003`, `NX1004` | Unbekannter Namespace/Bezeichner → deklarieren oder korrigieren. |
| Memberdeklaration | `NX1005` | Funktion ohne Rumpf → `{ ... }` ergänzen. |
| Typen | `NX1100`, `NX1102` | `int a = "text";` → passenden Typ nutzen; Literal außerhalb des Wertebereichs. |
| Rückgabe | `NX1103` | Nicht alle Pfade geben zurück → `return` ergänzen. |
| Überladung/Aufruf | `NX1107` | Falsche Argumentzahl. |
| Veränderlichkeit | `NX1200`, `NX1201` | Zuweisung an unveränderliche Bindung → `mut` setzen. |
| Kontrollfluss | `NX1301`, `NX1302` | `break`/`continue` außerhalb einer Schleife. |
| Stil-Warnung | `NX0004` | Konstante nicht `UPPER_SNAKE_CASE`, Variable nicht `camelCase`. |
| Backend-Subset | `NX2001`, `NX2002` | Konstrukt vom nativen Lowering nicht unterstützt (siehe unten). |
| Codegen/Linker | (ohne Code) | „genau eine parameterlose int-main-Funktion“, `LNK2019` bei fehlender `main`. |

> **Abweichung zur Codetabelle:** [`diagnostic-codes.md`](../compiler/diagnostic-codes.md) reserviert `NX0001`–`NX0003` für Lexerfehler. Der aktuelle Lexer meldet diese Fälle tatsächlich als `NX0100`. Maßgeblich ist das beobachtete Verhalten.

## Laufzeitfehler

Es gibt keine Laufzeit-Fehlermeldungen oder Exceptions. Ein Overflow- oder Division-durch-0-Trap beendet das Programm mit Exitcode `-1073741795` (`0xC000001D`, illegale Instruktion) ohne Text. Mit `--overflow-checks=false` entfällt der Overflow-Trap.

## Bekannter Compilerfehler

Programme mit `long`-Locals und Division/Vergleich gegen `int`-Literal können einen **Panic im Cranelift-Frontend** auslösen (`frontend.rs`), statt einer Diagnose. Workaround: `long` nur als Parameter/Rückgabe verwenden.

## Strategien

1. `nortexc check datei.nx` für reine Frontend-Fehler.
2. `NX2001`/`NX2002` bedeuten „noch nicht nativ“, nicht „falscher Code“: auf unterstützte Konstrukte umstellen ([README](README.md#feature-übersicht-stand-der-implementierung)).
3. `nortexc mir datei.nx` zeigt das optimierte MIR.
4. Ergebnisse über den Exitcode ausgeben ([Kapitel 10](10-input-output.md)).

## Fehlerbehandlung (Entwurf und Frontend-Stand)

NORTEX trennt optionale Werte (`Option<T>`: `Some`/`None`), erwartbare Fehler (`Result<T,E>`: `Ok`/`Err`) und Exceptions (`throw`, `try`/`catch`/`finally`). Es gibt keinen `?`-Operator. `Option` und `Result` sollen `must_use` sein; ein nicht verwendetes Ergebnis muss behandelt, weitergegeben oder mit `discard(value)` verworfen werden.

```nortex
public Result<int, string> divide(int first, int second)
{
    if (second == 0) { return Err("Division by zero"); }
    return Ok(first / second);
}
```

Ist-Stand (Frontend): Ein `Result<T,E>` als eigenständiges Ausdrucksstatement meldet `NX1108`; nicht vollständige Enum-`match`-Arme melden `NX1109` (`_` ist der Fallback). `Option<T>` wird nicht auf `must_use` geprüft. **Geplant:** `discard`, native Exception-Unwinding, RAII-Bereinigung, vollständiges `match`, Exceptions als Statuswerte an der C-ABI-Grenze; checked/unchecked ist offen ([Kapitel 15](15-faq-and-best-practices.md#offene-entscheidungen-projektinhaber)).

## Asynchronität (geplant)

Ziel: `async`-Funktionen werden zu suspendierbaren Zustandsmaschinen; `await` blockiert keinen Worker. `Future<T>` ist die Berechnung, `Task<T>` die von der Runtime eingeplante Ausführung.

```nortex
public async Task<string> loadText(string path)
{
    return await File.readText(path);
}
```

Der Parser kennt `async`/`await`; der Compiler senkt sie **nicht** in die Runtime ab. Der Rust-Prototyp `runtime/src/async/` hat einen Waker-basierten Scheduler mit FIFO-Queue und `block_on` (nur für Einstieg/Tests). Es fehlen Work-Stealing, Timer, Cancellation-API, IOCP-Reaktor und `Send`/`Sync`-Analyse.
