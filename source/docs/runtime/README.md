# NORTEX Runtime (0.3.5-dev)

Die Runtime ist als Rust-Crate angelegt. Derzeit ausführbar ist ein begrenzter
Threadpool-Bootstrap mit einem Waker-basierten Future-Executor. Speicher,
Allocator, Exceptions/Unwinding, IO, Networking und Plattformdienste sind
weiterhin nicht vollständig implementiert.

## Im Rust-Prototyp vorhanden

- Fixed-size Threadpool mit gemeinsamer Job-Queue und `Condvar`.
- `spawn` für Rust-Futures mit `Ready`-/`Pending`-Polling und Waker-Fortsetzungen.
- Externe Jobs, Worker-Starts und Continuations werden gemeinsam hinten in die FIFO-Queue eingereiht.
- `block_on` als äußere Einstiegshilfe mit `park`/`unpark`.
- Task-Ergebnisse als `Result<T, TaskError>`; Fehlerfälle `Panicked`,
  `AlreadyCompleted` und `ExecutorShutdown`.
- Pool-Shutdown cancelt eingereihte Jobs und registrierte suspendierte Tasks.
- Normales Drop wartet nicht auf beliebige laufende Job-Closures;
  `shutdown_and_join` ist die ausdrücklich blockierende Variante.

## Nicht implementiert

- NORTEX-`async`-/`await`-Lowering zu Zustandsmaschinen.
- Work-Stealing, lokale Worker-Queues und garantierte Fairness.
- Einzel-Task-Cancellation, Cancellation-Tokens und strukturierte Task-Scopes.
- Timer-Rad und Windows-IOCP-Reaktor.
- Vollständiger Allocator, Memory-/Drop-Glue, natives Unwinding,
  Socket-I/O und stabiler FFI-/Runtime-Startvertrag.

## Aktuelle Verträge

Ein Worker pollt ein Future nur bis `Ready` oder `Pending`. Bei `Pending` wird
der Worker freigegeben. Der Executor führt keine beliebigen fremden Jobs auf
dem Stack eines wartenden Tasks aus. Ein Waker reiht die Fortsetzung erneut ein.

`block_on` ist nur für Programmeinstieg und Tests vorgesehen; es weist einen
Aufruf aus einem NORTEX-Worker zurück. Das Verwerfen eines `Task<T>`-Handles
bricht den Task nicht ab. Die Destruktoren suspendierter Future-Objekte müssen
bei Pool-Shutdown kurz und nicht blockierend sein.

Weitere Details:

- [Async-Executor](async-executor.md)
- [Threadpool und Shutdown](threading.md)
- [Phase 3: Fehlerbehandlung und Async](../design/phase-3-errors-async.md)
