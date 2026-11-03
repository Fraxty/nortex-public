# NORTEX Runtime – Async-Executor (0.3.5-dev)

## Poll- und Continuation-Modell

`spawn` legt eine Future als Task an und reiht den ersten Poll-Auftrag am Ende
der gemeinsamen FIFO-Warteschlange ein. Ein Worker pollt die Future bis
`Ready` oder `Pending`. Bei `Pending` wird der Worker freigegeben. Ein `Waker`
meldet die Fortsetzung; sie wird ebenfalls hinten in die FIFO-Warteschlange
eingereiht. Selbst-Weckungen während eines Polls erzeugen keinen parallelen
zweiten Poll: Der Task wechselt in `RUNNING_NOTIFIED` und wird nach Rückkehr
aus `poll` höchstens einmal erneut hinten eingereiht.

Es gibt keinen LIFO-Vorrangplatz. `spawn` aus einem Worker und externe Jobs
verwenden dieselbe FIFO-Regel. Das verhindert eine unbegrenzte Bevorzugung
rekursiv erzeugter Aufgaben und erlaubt normalen Jobs, zwischen Yield-Polls
Fortschritt zu machen. Lokale Worker-Queues und Work-Stealing sind noch nicht
implementiert.

`block_on` ist ausschließlich für den äußersten Programmeinstieg und Tests
vorgesehen. Innerhalb eines Tasks wird ausschließlich `.await` verwendet.
Der äußere `block_on` wartet mit `park`; ein `Waker` ruft `unpark` auf. Es
gibt kein periodisches 1-ms-Polling. Ein Aufruf aus einem NORTEX-Worker gibt
`BlockOnError::CalledFromWorker` zurück und löst keinen Panic aus.

## Task-Zustandsautomat

Die Runtime verwendet die Zustände `IDLE`, `SCHEDULED`, `RUNNING`,
`RUNNING_NOTIFIED` und `COMPLETE`. Die Future wird vor dem Aufruf von
`Future::poll` aus ihrem Mutex-Slot genommen. Der Mutex wird nicht während
User-Code gehalten. Ein Wake während `RUNNING` setzt nur
`RUNNING_NOTIFIED`; ein zweiter Worker wartet dadurch nicht auf denselben
Future-Mutex und kann keine parallele Poll-Ausführung starten.

## Task- und Fehlerzustand

Ein Task liefert sein Ergebnis genau einmal. Ein erneutes Polling nach dem
Verbrauch liefert `TaskError::AlreadyCompleted`. Das Executor-Shutdown liefert
`TaskError::ExecutorShutdown`. Panics während des Future-Pollings werden
einmalig in `TaskError::Panicked(String)` überführt. Der Rust-Panic-Hook kann
die ursprüngliche Panic-Meldung weiterhin auf stderr ausgeben. Die Runtime
verpackt Panic-Payloads nicht rekursiv.

## Spawner und Lebensdauer

`Spawner` enthält nur ein schwaches Pool-Handle (`Weak`). Futures sollen diesen
Spawner erfassen und nicht `Arc<ThreadPool>`. So hält eine wartende Aufgabe den
Executor nicht über den vorgesehenen Scheduling-Vertrag am Leben. Eine Future,
die selbst einen starken `Arc<ThreadPool>` erfasst, kann weiterhin einen
Besitzzyklus erzeugen und muss dies vermeiden.

## Shutdown

`ThreadPool::shutdown` stoppt die Annahme neuer Jobs, verwirft noch eingereihte
Jobs und schließt alle registrierten suspendierten Tasks zunächst ohne Wake
ab. Erst nachdem alle registrierten Tasks terminal sind, werden die
gesammelten Waker ausgelöst. Die Verarbeitung ist iterativ; eine tiefe Kette
wartender Tasks erzeugt keine rekursive Wake-Kaskade.

Bereits laufende beliebige Rust-Job-Closures werden nicht gewaltsam beendet.
Beim normalen Drop werden Worker, die noch in einer Closure laufen, detached;
`ThreadPool::detached_worker_count()` zählt diese Worker bis zu ihrem Ende.
`shutdown_and_join` ist die ausdrücklich blockierende Variante. Sie kann
unbegrenzt warten, wenn eine Job-Closure niemals zurückkehrt. Ein sicherer
harter Thread-Abbruch ist in Rust nicht verfügbar. Aufruf aus einem Worker
verwendet aus Deadlock-Schutz die Detach-Variante.

Das Verwerfen eines `Task`-Handles bricht den Task nicht ab. Explizite
Cancellation-Tokens, Task-Bereiche, Timer und IOCP sind noch nicht implementiert.

## Grenzen dieses Entwicklungsstands

Der Rust-Runtime-Executor pollt Futures und unterstützt Waker-Fortsetzungen.
Der NORTEX-Compiler erzeugt jedoch noch keine Futures oder Async-Zustandsmaschinen
aus NORTEX-`async`-/`await`-Syntax. Es gibt noch keinen IOCP-Reaktor, Timer-Rad,
Work-Stealing-Deque oder Cancellation-API.
