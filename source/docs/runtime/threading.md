# NORTEX Runtime – Threadpool und Shutdown (0.3.5-dev)

## Ausführung und Fairness

Der Threadpool besitzt eine gemeinsame `VecDeque` und eine `Condvar`.
Gewöhnliche Rust-Jobs, aus Worker-Jobs gestartete Arbeit und Async-
Fortsetzungen werden ausnahmslos hinten in dieselbe FIFO-Queue eingereiht.
Es gibt keine `push_front`-Priorisierung und keine Inline-Ausführung fremder
Jobs auf dem Stack eines wartenden Tasks. Selbst-Weckungen während eines
laufenden Polls werden im Task-Zustand `RUNNING_NOTIFIED` zusammengefasst; nach
Rückkehr aus `poll()` wird höchstens eine Fortsetzung hinten eingereiht.

Damit wird die zuvor beobachtete unbegrenzte LIFO-Bevorzugung vermieden.
Eine formale Fairnessgarantie unter beliebiger Dauerlast oder Work-Stealing
existiert noch nicht. Die gemeinsame Queue ist ein Bootstrap-Design und kann
unter hoher Last zum Mutex-Engpass werden. Worker-lokale Queues und
Work-Stealing sind spätere Optimierungen.

## Shutdown-Vertrag

`shutdown` beendet die Annahme neuer Arbeit, verwirft noch eingereihte
synchronen Jobs und schließt registrierte asynchrone Tasks zunächst ohne Wake
ab. Erst nachdem die registrierten Tasks terminal sind, werden ihre
gesammelten Waker ausgelöst. Die Task-Abbruchphase arbeitet iterativ und löst
keine rekursive Wake-Kaskade aus.

Laufende `FnOnce`-Jobs können nicht sicher zwangsbeendet werden. Normales
`Drop` wartet nicht auf solche Jobs; Worker, die gerade eine Closure ausführen,
werden detached und beenden sich, sobald die Closure zurückkehrt.
`ThreadPool::detached_worker_count()` zählt diese Worker bis zu ihrem Ende.
Das ist eine Zählung, keine automatische Warnung oder Cancellation.

`shutdown_and_join` wartet ausdrücklich auf laufende Jobs. Diese Methode darf
für nicht kooperative Jobs nicht als zeitbegrenzter Shutdown verstanden
werden. Aus einem Worker heraus wird aus Deadlock-Schutz nicht gejoint.

## Task- und Pool-Lebensdauer

Der Pool hält aktive Task-Zellen bis zu ihrem Abschluss oder Shutdown stark.
Die Task-Zellen und der öffentliche `Spawner` halten den Pool selbst nur
schwach. Dadurch hält ein verworfenes Benutzer-Task-Handle den Task nicht
unbeabsichtigt an; der Pool kann aber aktive Futures bis zur Fertigstellung
verwalten. Futures sollen `Spawner` erfassen und nicht `Arc<ThreadPool>`.
Eine Future, die absichtlich selbst `Arc<ThreadPool>` einfängt, kann weiterhin
einen Eigentumszyklus erzeugen; diese Nutzung ist zu vermeiden.

Das Verwerfen eines `Task`-Handles bricht den Task nicht ab. Explizite
Cancellation-Tokens, strukturierte Task-Scopes, Timer-Rad und IOCP sind noch
nicht implementiert.
