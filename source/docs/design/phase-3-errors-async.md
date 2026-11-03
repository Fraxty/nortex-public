# NORTEX Phase 3 – Fehlerbehandlung und Asynchronität

**Spezifikationsstatus:** Zielvertrag 0.3.5-dev; Exception- und Unwind-Freigaben sind teilweise offen  
**Implementierungsstatus:** Runtime-Prototyp (`runtime/src/async`) vorhanden; Frontend meldet `NX1108` (verworfenes `Result`) und `NX1109` (unvollständiges Enum-`match`); Compiler-Lowering von `async`/`await`, `discard`, `Option`-`must_use` und Exception-Unwinding fehlen. Ist-Stand: [Sprachreferenz Kapitel 11](../language/11-errors-and-debugging.md).

## 3.1 Fehlerbehandlungsmodell

NORTEX unterscheidet erwartbare Fehler, optionale Werte und außergewöhnliche Programm- bzw. Systemzustände. Es gibt kein implizites Nullable-Zweitsystem und keinen `?`-Operator.

| Mechanismus | Zweck | Typische Verwendung |
|---|---|---|
| `Option<T>` | Wert kann fehlen | Suche, Konfiguration, optionales Feld |
| `Result<T, E>` | erwartbares Scheitern | Datei, Netzwerk, Parsing, Validierung |
| Exception | außergewöhnlicher Kontrollfluss | beschädigter Zustand, unerwarteter Laufzeitfehler, explizites `throw` |

### 3.1.1 `Result<T, E>` und `Option<T>`

`Result<T, E>` besitzt die Varianten `Ok(T)` und `Err(E)`. `Option<T>` besitzt `Some(T)` und `None`. Beide sind algebraische, statisch bekannte Summentypen. Der Compiler kennt ihre Varianten und kann `match`-Vollständigkeit prüfen.

```nortex
public Result<int, string> divide(int first, int second)
{
    if (second == 0)
    {
        return Err("Division by zero");
    }

    return Ok(first / second);
}

public void calculate()
{
    Result<int, string> result = divide(12, 3);
    match (result)
    {
        Ok(value) => print(value);
        Err(message) => print(message);
    }
}
```

**Verbindliche Regeln:**

1. `Result<T, E>` und `Option<T>` sind `must_use`-Typen. Ein nicht verwendetes Ergebnis erzeugt mindestens einen Compilerfehler, sobald der Wert am Ende einer Anweisung verworfen würde.
2. Ein Ergebnis gilt als verwendet, wenn es geprüft, weitergegeben, zurückgegeben oder mit einer ausdrücklich benannten Discard-Funktion verworfen wurde.
3. Die Standardbibliothek stellt `discard(result)` als bewusst sichtbaren Verwerfungsweg bereit. Der Aufruf muss im Quellcode stehen.
4. `?` ist kein Operator und darf nicht als Fehlerweitergabe interpretiert werden.
5. `match` muss alle Varianten behandeln oder einen `_`-Fallback besitzen. Bei `Result<T,E>` sind `Ok` und `Err` die vollständigen Varianten.
6. Es gibt keine automatische Umwandlung zwischen `Option<T>` und `Result<T,E>`.
7. Fehlertexte sind Diagnoseinformationen, aber kein Ersatz für einen stabilen Fehlercode oder einen typisierten Fehlertyp.

### 3.1.2 Exceptions

Zielentwurf: Exceptions sind für außergewöhnliche Situationen vorgesehen und nicht die bevorzugte Schnittstelle für erwartbare Fehler. Erwartbare Fehler werden mit `Result` gemeldet. Checked-/Unchecked-Semantik und das konkrete native Unwind-Verfahren benötigen noch eine ausdrückliche Freigabe.

```nortex
try
{
    loadRequiredConfiguration();
}
catch (FileNotFoundException error)
{
    print(error.message);
}
finally
{
    releaseTemporaryResources();
}
```

**Sprachregeln:**

- Checked-/Unchecked-Semantik und die Frage nach `throws`-Deklarationen sind noch nicht freigegeben; das Beispiel beschreibt den Zielmechanismus, nicht die endgültige Signaturregel.
- `throw expression;` löst eine typisierte Exception aus.
- `catch (T value)` fängt `T` und dessen abgeleitete Exception-Typen, sofern `T` ein Exception-Typ ist.
- Mehrere `catch`-Blöcke werden in Quelltextreihenfolge geprüft. Ein allgemeiner Basistyp muss nach spezifischeren Typen stehen.
- `finally` wird beim normalen Verlassen sowie beim Verlassen durch eine Exception ausgeführt.
- `return`, `break` und `continue` sind in `finally` verboten. Dadurch kann Cleanup den ursprünglichen Kontrollfluss nicht verdecken.
- Destruktoren sind `noexcept`: Sie dürfen keine NORTEX-Exception auslösen. Ein Verstoß führt zu einem nicht wiederherstellbaren Runtime-Abbruch.
- Während Stack-Unwinding werden Destruktoren in umgekehrter Reihenfolge der Initialisierung ausgeführt.
- Löst `finally` selbst eine Exception aus, wird die ursprüngliche Exception als `cause` an die neue Exception angehängt.
- Ein nicht abgefangener Exception-Pfad beendet den aktuellen Thread kontrolliert über den Runtime-Fehlerpfad und erzeugt Diagnoseinformationen.
- Das Windows-x64-Backend erzeugt für unwindbare Funktionen die erforderlichen PE-Unwind-Daten (`.pdata`/`.xdata`). Ohne diese Metadaten dürfen native Exceptions nicht als unterstützt gelten.

**C-ABI-Grenze:** Eine NORTEX-Exception darf niemals über eine C-ABI-Grenze propagieren. Der Exportadapter fängt sie ab und übersetzt sie in einen festgelegten Statuscode oder `Result`-kompatiblen Rückgabewert. Beim Aufruf fremder C-Funktionen werden Fehlercodes nach dem jeweiligen API-Vertrag in NORTEX-Fehlerwerte übersetzt.

### 3.1.3 Ressourcenbereinigung

RAII ist die verbindliche Ressourcenstrategie. Jeder erfolgreiche Ressourcenaufbau erzeugt einen Owner, dessen Destruktor beim regulären Scope-Ende und beim Unwinding ausgeführt wird. Destruktoren dürfen nicht blockierend auf fremde Tasks warten und dürfen keine Exceptions auslösen. Teure oder fehlerbehaftete Freigaben erhalten zusätzlich eine explizite `close()`- oder `dispose()`-Methode mit `Result`-Rückgabe; der Destruktor bleibt ein sicherer Fallback.

## 3.2 Asynchronität und Threading

### 3.2.1 Typen und Zuständigkeiten

- `Future<T>` ist ein noch nicht zwingend eingeplanter asynchroner Berechnungszustand.
- `Task<T>` ist die von der Runtime verwaltete und eingeplante Ausführung eines Futures.
- `async` erzeugt eine vom Compiler als Zustandsmaschine dargestellte asynchrone Funktion.
- `await` wartet innerhalb einer `async`-Funktion auf `Future<T>` oder `Task<T>` und suspendiert die aktuelle Zustandsmaschine, statt einen Worker blockierend schlafen zu legen.
- `CancellationToken` transportiert kooperative Abbruchanforderungen.

`Task<T>` und `Future<T>` sind unterschiedliche nominale Typen. Eine Task kann einen Future ausführen; eine Task ist kein Alias für Future.

```nortex
public async Task<string> downloadData(CancellationToken cancellation)
{
    Result<string, NetworkError> response = await network.get(
        "https://example.com",
        cancellation
    );

    match (response)
    {
        Ok(value) => return value;
        Err(error) => throw NetworkException(error.message);
    }
}
```

### 3.2.2 Ausführung und Scheduling

Die Runtime verwendet einen begrenzten Worker-Pool. Die Standardgröße ist die Zahl der verfügbaren logischen Prozessoren, mindestens jedoch ein Worker; eine Projekt- oder Runtime-Konfiguration darf die Zahl ausdrücklich überschreiben. Der aktuelle Prototyp verwendet eine gemeinsame FIFO-Queue für externe Jobs, Worker-Starts und Waker-Fortsetzungen. Selbst-Weckungen werden nach Ende des aktuellen Polls hinten eingereiht. Es gibt keinen priorisierten LIFO-Slot. Work-Stealing und lokale Worker-Queues bleiben spätere Optimierungen.

Ein Worker darf bei `await` nicht blockierend auf die Fortsetzung warten. Die Compiler-Zustandsmaschine registriert eine Continuation, gibt den Worker frei und wird nach Abschluss der abhängigen Operation wieder eingeplant.

### 3.2.3 Cancellation und strukturierte Nebenläufigkeit

Cancellation ist kooperativ. Ein Token kann angefragt und geprüft werden; Runtime und Standardbibliothek prüfen ihn an definierten Unterbrechungspunkten. Eine Cancellation beendet keinen beliebigen nativen Thread gewaltsam.

Tasks gehören standardmäßig zu einer übergeordneten Task-Gruppe. Beim Verlassen des Scope muss die Gruppe abgeschlossen oder explizit mit `detach` aus der strukturierten Lebensdauer gelöst werden. `detach` ist eine bewusste Ausnahme und benötigt eine Fehlerbeobachtung über einen registrierten Handler. Nicht beobachtete Task-Fehler werden als Runtime-Diagnose protokolliert.

### 3.2.4 Thread-Sicherheit

Daten dürfen nur dann zwischen Threads geteilt werden, wenn die Typregeln dies erlauben. `Send` kennzeichnet Werte, die sicher zwischen Threads übertragen werden können; `Sync` kennzeichnet Werte, die sicher über gemeinsame Referenzen verwendet werden können. `Arc<T>`, `Mutex<T>` und `RwLock<T>` sind explizite Synchronisationswerkzeuge. Der Compiler muss Data Races in sicheren NORTEX-Programmen ausschließen.

### 3.2.5 Runtime-Implementierungsreihenfolge

1. Worker-Pool, Job-Queue und geordnetes Shutdown.
2. Task-Zustand, Ergebnisablage und Continuation-Wakeup.
3. Future-Polling und Compiler-Zustandsautomaten.
4. Cancellation und strukturierte Task-Gruppen.
5. Asynchrone Datei- und Netzwerkoperationen.
6. Windows-IOCP als I/O-Reactor.

IOCP ist die Windows-Zielarchitektur für asynchrones I/O, aber nicht Voraussetzung für den ersten CPU-Task-Prototyp.


## 3.3 Tatsächlicher Runtime-Prototyp und Abgrenzung

Der aktuelle Rust-Prototyp stellt `ThreadPool`, `block_on` und `spawn` bereit. `spawn` liefert `Task<T>` als Rust-`Future`. Jeder Worker pollt eine Future kurz bis `Ready` oder `Pending`; bei `Pending` wird der Worker freigegeben. Ein Waker reiht eine Fortsetzung ein. Fremde Queue-Jobs werden nicht auf dem Stack eines wartenden Tasks ausgeführt.

`block_on` ist nur für den äußeren Programmeinstieg und Tests vorgesehen. Es wartet mit `park`/`unpark`, nicht durch periodisches Polling. Ein Aufruf innerhalb eines NORTEX-Worker-Threads wird zurückgewiesen. Task-Panics werden als `TaskError::Panicked(String)` gespeichert; ein wiederholter Poll nach Verbrauch des Ergebnisses liefert `AlreadyCompleted`. Aufrufe von `block_on` innerhalb eines Worker-Threads liefern `BlockOnError::CalledFromWorker`.

**Wichtige Einschränkung:** Dies ist ein Waker-basierter Rust-Future-Executor-Bootstrap mit einer gemeinsamen Mutex-/Condvar-Queue, kein produktiver NORTEX-Async-Executor. Der Compiler erzeugt noch keine Rust-Futures oder Async-Zustandsmaschinen aus NORTEX-`async`-/`await`-Syntax. Work-Stealing, Einzel-Task-Cancellation, strukturierte Task-Gruppen, Timer-Rad und IOCP fehlen. Normales Pool-Drop cancelt registrierte Tasks und wartet nicht auf beliebige laufende Jobs; `shutdown_and_join` ist ausdrücklich blockierend. Details: `docs/runtime/async-executor.md` und `docs/runtime/threading.md`.
