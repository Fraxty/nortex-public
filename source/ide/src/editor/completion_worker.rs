//! Completion auf einem eigenen Thread, damit das Tippen bei großen Dateien nicht auf die
//! Analyse wartet.
//!
//! Die Mailbox hat genau einen Platz: Eine neue Anfrage ersetzt die noch nicht gestartete
//! ältere, die Warteschlange kann also nicht wachsen. Eine bereits laufende Analyse lässt
//! sich nicht unterbrechen (Lexer, Parser und Typauflösung haben keine Abbruchpunkte); ihr
//! Ergebnis wird über die Revision verworfen, sobald eine neuere Anfrage existiert.

use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;

use super::completion::Completion;

pub struct CompletionRequest {
    /// Index des aktiven Tabs zum Zeitpunkt der Anfrage.
    pub document: usize,
    pub text: String,
    pub cursor: usize,
    pub path: Option<PathBuf>,
    pub project_files: Vec<PathBuf>,
    /// Strg+Leertaste: öffnet auch ohne Präfix.
    pub manual: bool,
}

pub enum CompletionOutcome {
    Found(Completion),
    Empty,
    Failed,
}

pub struct CompletionReply {
    pub revision: u64,
    pub request: CompletionRequest,
    pub outcome: CompletionOutcome,
}

impl CompletionReply {
    /// Ein Ergebnis darf nur angezeigt werden, wenn Tab, Text und Cursor noch genau
    /// denen der Anfrage entsprechen.
    pub fn matches(&self, document: usize, text: &str, cursor: usize) -> bool {
        self.request.document == document
            && self.request.cursor == cursor
            && self.request.text == text
    }
}

struct Mailbox {
    pending: Option<(u64, CompletionRequest)>,
    shutting_down: bool,
}

pub struct CompletionWorker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    replies: mpsc::Receiver<CompletionReply>,
    revision: u64,
}

fn run_request(request: &CompletionRequest) -> Option<Completion> {
    let path = request.path.as_deref();
    if request.manual {
        Completion::manual_for_file(&request.text, request.cursor, path, &request.project_files)
    } else {
        Completion::open_for_file(&request.text, request.cursor, path, &request.project_files)
    }
}

impl CompletionWorker {
    /// `wake` wird nach jedem Ergebnis aufgerufen, damit die Oberfläche neu zeichnet.
    pub fn spawn(wake: impl Fn() + Send + 'static) -> io::Result<Self> {
        Self::spawn_with(wake, run_request)
    }

    fn spawn_with(
        wake: impl Fn() + Send + 'static,
        analyze: impl Fn(&CompletionRequest) -> Option<Completion> + Send + 'static,
    ) -> io::Result<Self> {
        let shared = Arc::new((
            Mutex::new(Mailbox {
                pending: None,
                shutting_down: false,
            }),
            Condvar::new(),
        ));
        let worker_shared = Arc::clone(&shared);
        let (sender, replies) = mpsc::channel();
        // Der Thread wird bewusst nicht gejoint: Eine laufende Analyse soll das Schließen der
        // IDE nicht aufhalten.
        thread::Builder::new()
            .name("nortex-completion".into())
            .spawn(move || {
                loop {
                    let (revision, request) = {
                        let (lock, condition) = &*worker_shared;
                        let mut mailbox = lock.lock().unwrap_or_else(|error| error.into_inner());
                        while mailbox.pending.is_none() && !mailbox.shutting_down {
                            mailbox = condition
                                .wait(mailbox)
                                .unwrap_or_else(|error| error.into_inner());
                        }
                        if mailbox.shutting_down {
                            break;
                        }
                        match mailbox.pending.take() {
                            Some(job) => job,
                            None => continue,
                        }
                    };
                    let outcome = match catch_unwind(AssertUnwindSafe(|| analyze(&request))) {
                        Ok(Some(completion)) => CompletionOutcome::Found(completion),
                        Ok(None) => CompletionOutcome::Empty,
                        Err(_) => CompletionOutcome::Failed,
                    };
                    let reply = CompletionReply {
                        revision,
                        request,
                        outcome,
                    };
                    if sender.send(reply).is_err() {
                        break;
                    }
                    wake();
                }
            })?;
        Ok(Self {
            shared,
            replies,
            revision: 0,
        })
    }

    /// Ersetzt eine noch wartende Anfrage und entwertet alle früheren Ergebnisse.
    pub fn submit(&mut self, request: CompletionRequest) {
        self.revision += 1;
        let (lock, condition) = &*self.shared;
        let mut mailbox = lock.lock().unwrap_or_else(|error| error.into_inner());
        mailbox.pending = Some((self.revision, request));
        condition.notify_one();
    }

    /// Verwirft die wartende Anfrage und das Ergebnis einer laufenden.
    pub fn cancel(&mut self) {
        self.revision += 1;
        let (lock, _) = &*self.shared;
        lock.lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending = None;
    }

    /// Das Ergebnis der jüngsten Anfrage, falls es inzwischen vorliegt. Ältere Ergebnisse
    /// werden dabei verworfen.
    pub fn poll(&mut self) -> Option<CompletionReply> {
        let mut latest = None;
        while let Ok(reply) = self.replies.try_recv() {
            if reply.revision == self.revision {
                latest = Some(reply);
            }
        }
        latest
    }
}

impl Drop for CompletionWorker {
    fn drop(&mut self) {
        let (lock, condition) = &*self.shared;
        lock.lock()
            .unwrap_or_else(|error| error.into_inner())
            .shutting_down = true;
        condition.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use super::*;

    fn request(text: &str, cursor: usize) -> CompletionRequest {
        CompletionRequest {
            document: 0,
            text: text.to_owned(),
            cursor,
            path: None,
            project_files: Vec::new(),
            manual: false,
        }
    }

    fn wait_for(worker: &mut CompletionWorker) -> CompletionReply {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(reply) = worker.poll() {
                return reply;
            }
            assert!(Instant::now() < deadline, "no reply within 10 s");
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Hält jede Analyse an, bis der Test sie freigibt.
    struct Gate {
        open: Mutex<bool>,
        changed: Condvar,
    }

    impl Gate {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                open: Mutex::new(false),
                changed: Condvar::new(),
            })
        }

        fn pass(&self) {
            let mut open = self.open.lock().unwrap();
            while !*open {
                open = self.changed.wait(open).unwrap();
            }
        }

        fn release(&self) {
            *self.open.lock().unwrap() = true;
            self.changed.notify_all();
        }
    }

    #[test]
    fn computes_a_completion_off_the_calling_thread() {
        let mut worker = CompletionWorker::spawn(|| {}).unwrap();
        let text = "int counter = 1;\nint total = coun";
        worker.submit(request(text, text.chars().count()));
        let reply = wait_for(&mut worker);
        assert!(reply.matches(0, text, text.chars().count()));
        let CompletionOutcome::Found(completion) = reply.outcome else {
            panic!("expected a completion");
        };
        assert!(completion.items.iter().any(|item| item == "counter"));
    }

    #[test]
    fn waiting_requests_are_replaced_so_the_queue_cannot_grow() {
        let gate = Gate::new();
        let analyzed = Arc::new(AtomicUsize::new(0));
        let (worker_gate, worker_count) = (gate.clone(), analyzed.clone());
        let mut worker = CompletionWorker::spawn_with(
            || {},
            move |_| {
                worker_count.fetch_add(1, Ordering::SeqCst);
                worker_gate.pass();
                None
            },
        )
        .unwrap();

        worker.submit(request("first", 5));
        while analyzed.load(Ordering::SeqCst) == 0 {
            thread::sleep(Duration::from_millis(1));
        }
        for index in 0..1_000 {
            worker.submit(request(&format!("typed {index}"), 5));
        }
        let waiting = worker.shared.0.lock().unwrap().pending.is_some();
        assert!(waiting);
        gate.release();

        let reply = wait_for(&mut worker);
        assert_eq!(reply.request.text, "typed 999");
        assert_eq!(analyzed.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn result_of_a_superseded_request_is_dropped() {
        let gate = Gate::new();
        let worker_gate = gate.clone();
        let mut worker = CompletionWorker::spawn_with(
            || {},
            move |request| {
                if request.text == "old" {
                    worker_gate.pass();
                }
                None
            },
        )
        .unwrap();

        worker.submit(request("old", 3));
        thread::sleep(Duration::from_millis(20));
        worker.submit(request("new", 3));
        gate.release();

        let reply = wait_for(&mut worker);
        assert_eq!(reply.request.text, "new");
        thread::sleep(Duration::from_millis(20));
        assert!(worker.poll().is_none());
    }

    #[test]
    fn cancel_discards_a_running_analysis_and_a_waiting_one() {
        let gate = Gate::new();
        let worker_gate = gate.clone();
        let mut worker = CompletionWorker::spawn_with(
            || {},
            move |_| {
                worker_gate.pass();
                None
            },
        )
        .unwrap();

        worker.submit(request("running", 7));
        thread::sleep(Duration::from_millis(20));
        worker.submit(request("waiting", 7));
        worker.cancel();
        gate.release();
        thread::sleep(Duration::from_millis(50));
        assert!(worker.poll().is_none());
        assert!(worker.shared.0.lock().unwrap().pending.is_none());
    }

    #[test]
    fn reply_is_only_valid_for_the_same_tab_text_and_cursor() {
        let mut worker = CompletionWorker::spawn(|| {}).unwrap();
        worker.submit(request("int value = 1;\nva", 17));
        let reply = wait_for(&mut worker);
        assert!(reply.matches(0, "int value = 1;\nva", 17));
        assert!(!reply.matches(1, "int value = 1;\nva", 17), "other tab");
        assert!(!reply.matches(0, "int value = 1;\nvar", 17), "text changed");
        assert!(!reply.matches(0, "int value = 1;\nva", 16), "cursor moved");
    }

    #[test]
    fn a_panicking_analysis_is_reported_and_the_worker_keeps_running() {
        let mut worker = CompletionWorker::spawn_with(
            || {},
            |request| {
                assert_ne!(request.text, "boom", "simulated analysis failure");
                None
            },
        )
        .unwrap();

        worker.submit(request("boom", 4));
        assert!(matches!(
            wait_for(&mut worker).outcome,
            CompletionOutcome::Failed
        ));
        worker.submit(request("fine", 4));
        assert!(matches!(
            wait_for(&mut worker).outcome,
            CompletionOutcome::Empty
        ));
    }

    #[test]
    fn rapid_typing_ends_with_the_result_for_the_final_text() {
        let mut worker = CompletionWorker::spawn(|| {}).unwrap();
        let base = "int amount = 1;\nint total = ";
        let mut text = base.to_owned();
        for letter in "amou".chars() {
            text.push(letter);
            worker.submit(request(&text, text.chars().count()));
        }
        let reply = loop {
            let reply = wait_for(&mut worker);
            if reply.request.text == text {
                break reply;
            }
        };
        assert!(reply.matches(0, &text, text.chars().count()));
        let CompletionOutcome::Found(completion) = reply.outcome else {
            panic!("expected a completion");
        };
        assert!(completion.items.iter().any(|item| item == "amount"));
    }

    #[test]
    fn the_thread_ends_when_the_worker_is_dropped() {
        let alive = Arc::new(());
        let held = alive.clone();
        let worker = CompletionWorker::spawn_with(
            || {},
            move |_| {
                let _ = &held;
                None
            },
        )
        .unwrap();
        assert_eq!(Arc::strong_count(&alive), 2);
        drop(worker);
        let deadline = Instant::now() + Duration::from_secs(5);
        while Arc::strong_count(&alive) > 1 {
            assert!(Instant::now() < deadline, "worker thread did not exit");
            thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn broken_source_yields_a_reply_instead_of_a_crash() {
        let mut worker = CompletionWorker::spawn(|| {}).unwrap();
        let text = "class { public int ( = ; \"unterminated\nfoo.";
        worker.submit(request(text, text.chars().count()));
        let reply = wait_for(&mut worker);
        assert!(!matches!(reply.outcome, CompletionOutcome::Failed));
    }
}
