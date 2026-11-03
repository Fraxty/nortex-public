//! Waker-driven NORTEX task runtime bootstrap.
//!
//! Futures are polled without holding a mutex across user `poll()` code. A
//! compact atomic state machine prevents duplicate concurrent polls. All jobs
//! and continuations use FIFO scheduling so a self-waking future cannot starve
//! ordinary work. NORTEX compiler lowering from `async` syntax is separate.

use std::any::Any;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, TryLockError};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

use crate::threading::{ActiveTask, PoolHandle, ThreadPool, ThreadPoolError};

const TASK_IDLE: u8 = 0;
const TASK_SCHEDULED: u8 = 1;
const TASK_RUNNING: u8 = 2;
const TASK_RUNNING_NOTIFIED: u8 = 3;
const TASK_COMPLETE: u8 = 4;

struct ThreadWake(thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockOnError {
    /// `block_on` cannot be called from a NORTEX worker. Use `.await` there.
    CalledFromWorker,
}

/// Drives a future on the calling thread. Calling this from a NORTEX worker
/// returns an error instead of panicking. It is intended for process entry
/// points and tests; async tasks must use `.await`.
pub fn block_on<F: Future>(future: F) -> Result<F::Output, BlockOnError> {
    if crate::threading::is_worker_thread() {
        return Err(BlockOnError::CalledFromWorker);
    }

    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);

    loop {
        match Pin::as_mut(&mut future).poll(&mut context) {
            Poll::Ready(output) => return Ok(output),
            Poll::Pending => thread::park(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    /// The submitted future panicked. The original panic payload is converted
    /// to text once; errors are never recursively embedded in panic messages.
    Panicked(String),
    /// A completed Task was polled again after its result was consumed.
    AlreadyCompleted,
    /// The executor was shut down before the task could be scheduled.
    ExecutorShutdown,
}

struct TaskState<T> {
    result: Option<Result<T, TaskError>>,
    waiters: Vec<Waker>,
}

struct TaskCell<T> {
    future: Mutex<Option<Pin<Box<dyn Future<Output = T> + Send + 'static>>>>,
    state: Mutex<TaskState<T>>,
    pool: PoolHandle,
    poll_state: AtomicU8,
    completed: AtomicBool,
    registry_id: AtomicU64,
}

impl<T: Send + 'static> TaskCell<T> {
    fn schedule(self: &Arc<Self>) -> Result<(), ThreadPoolError> {
        loop {
            match self.poll_state.load(Ordering::Acquire) {
                TASK_COMPLETE | TASK_SCHEDULED | TASK_RUNNING_NOTIFIED => return Ok(()),
                TASK_RUNNING => {
                    if self
                        .poll_state
                        .compare_exchange(
                            TASK_RUNNING,
                            TASK_RUNNING_NOTIFIED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return Ok(());
                    }
                }
                TASK_IDLE => {
                    if self
                        .poll_state
                        .compare_exchange(
                            TASK_IDLE,
                            TASK_SCHEDULED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return self.enqueue_claimed();
                    }
                }
                _ => unreachable!("invalid task scheduling state"),
            }
        }
    }

    /// Enqueues a task after its state has already transitioned to SCHEDULED.
    /// All work goes to the back of the FIFO queue to preserve fairness.
    fn enqueue_claimed(self: &Arc<Self>) -> Result<(), ThreadPoolError> {
        let run_cell = Arc::clone(self);
        match self.pool.execute(move || run_cell.poll_once()) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.complete(Err(TaskError::ExecutorShutdown));
                Err(error)
            }
        }
    }

    fn complete(&self, result: Result<T, TaskError>) {
        let waiters = self.complete_without_waking(result);
        for waiter in waiters {
            waiter.wake();
        }
    }

    /// Marks the task complete and returns its waiters without waking them.
    /// Pool shutdown uses this in a first pass, then wakes all waiters only
    /// after every registered task is terminal; this makes shutdown iterative.
    fn complete_without_waking(&self, result: Result<T, TaskError>) -> Vec<Waker> {
        if self.completed.swap(true, Ordering::AcqRel) {
            return Vec::new();
        }
        self.poll_state.store(TASK_COMPLETE, Ordering::Release);
        let registry_id = self.registry_id.swap(0, Ordering::AcqRel);
        if registry_id != 0 {
            self.pool.unregister_task(registry_id);
        }
        let waiters = match self.state.lock() {
            Ok(mut state) => {
                state.result = Some(result);
                std::mem::take(&mut state.waiters)
            }
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.result = Some(Err(TaskError::Panicked("Task state lock poisoned".into())));
                std::mem::take(&mut state.waiters)
            }
        };
        self.drop_future_if_idle();
        waiters
    }

    fn drop_future_if_idle(&self) {
        let future = match self.future.try_lock() {
            Ok(mut future) => future.take(),
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner().take(),
            Err(TryLockError::WouldBlock) => return,
        };
        if let Some(future) = future {
            let _ = catch_unwind(AssertUnwindSafe(|| drop(future)));
        }
    }

    fn poll_once(self: Arc<Self>) {
        if self
            .poll_state
            .compare_exchange(
                TASK_SCHEDULED,
                TASK_RUNNING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return;
        }

        if self.completed.load(Ordering::Acquire) {
            self.poll_state.store(TASK_COMPLETE, Ordering::Release);
            return;
        }

        // Move the Future out while polling. No worker can block on a mutex
        // held by another worker executing user-provided Future::poll code.
        let mut future = {
            let mut slot = self
                .future
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            slot.take()
        };
        let Some(mut future) = future.take() else {
            self.complete(Err(TaskError::AlreadyCompleted));
            return;
        };

        if self.completed.load(Ordering::Acquire) {
            let _ = catch_unwind(AssertUnwindSafe(|| drop(future)));
            return;
        }

        let waker = Waker::from(Arc::clone(&self));
        let mut context = Context::from_waker(&waker);
        let polled = catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(&mut context)));

        match polled {
            Ok(Poll::Ready(value)) => {
                let _ = catch_unwind(AssertUnwindSafe(|| drop(future)));
                self.complete(Ok(value));
            }
            Err(payload) => {
                let message = panic_message(payload.as_ref());
                let _ = catch_unwind(AssertUnwindSafe(|| drop(future)));
                self.complete(Err(TaskError::Panicked(message)));
            }
            Ok(Poll::Pending) => {
                let discarded = {
                    let mut slot = self
                        .future
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if self.completed.load(Ordering::Acquire) {
                        Some(future)
                    } else {
                        *slot = Some(future);
                        None
                    }
                };
                if let Some(future) = discarded {
                    // The Future is still local (it was moved out before poll).
                    // Catch a panicking destructor so shutdown cannot unwind a worker.
                    let _ = catch_unwind(AssertUnwindSafe(|| drop(future)));
                    self.drop_future_if_idle();
                    return;
                }

                loop {
                    match self.poll_state.load(Ordering::Acquire) {
                        TASK_RUNNING => {
                            if self
                                .poll_state
                                .compare_exchange(
                                    TASK_RUNNING,
                                    TASK_IDLE,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                )
                                .is_ok()
                            {
                                return;
                            }
                        }
                        TASK_RUNNING_NOTIFIED => {
                            if self
                                .poll_state
                                .compare_exchange(
                                    TASK_RUNNING_NOTIFIED,
                                    TASK_SCHEDULED,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                )
                                .is_ok()
                            {
                                let _ = self.enqueue_claimed();
                                return;
                            }
                        }
                        TASK_COMPLETE => {
                            self.drop_future_if_idle();
                            return;
                        }
                        TASK_SCHEDULED | TASK_IDLE => return,
                        _ => unreachable!("invalid task polling state"),
                    }
                }
            }
        }
    }
}

impl<T: Send + 'static> ActiveTask for TaskCell<T> {
    fn cancel_for_shutdown(&self) -> Vec<Waker> {
        self.complete_without_waking(Err(TaskError::ExecutorShutdown))
    }
}

impl<T> Drop for TaskCell<T> {
    fn drop(&mut self) {
        let id = self.registry_id.swap(0, Ordering::AcqRel);
        if id != 0 {
            self.pool.unregister_task(id);
        }
    }
}

impl<T: Send + 'static> Wake for TaskCell<T> {
    fn wake(self: Arc<Self>) {
        let _ = self.schedule();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let _ = self.schedule();
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// A scheduled asynchronous computation. Its result is consumed exactly once.
#[must_use = "ignoring a Task discards its result and may hide a task failure"]
pub struct Task<T> {
    shared: Arc<TaskCell<T>>,
    completed: bool,
}

impl<T> Task<T> {
    fn new(shared: Arc<TaskCell<T>>) -> Self {
        Self {
            shared,
            completed: false,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.completed || self.shared.completed.load(Ordering::Acquire)
    }
}

impl<T: Send + 'static> Future for Task<T> {
    type Output = Result<T, TaskError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.completed {
            return Poll::Ready(Err(TaskError::AlreadyCompleted));
        }
        let mut state = match this.shared.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(result) = state.result.take() {
            this.completed = true;
            Poll::Ready(result)
        } else {
            if !state
                .waiters
                .iter()
                .any(|waiter| waiter.will_wake(context.waker()))
            {
                state.waiters.push(context.waker().clone());
            }
            Poll::Pending
        }
    }
}

/// Weak task spawner. Futures should capture this handle rather than
/// `Arc<ThreadPool>`, so a task cannot keep its own executor alive.
#[derive(Clone)]
pub struct Spawner {
    pool: PoolHandle,
}

impl Spawner {
    pub(crate) fn from_pool(pool: PoolHandle) -> Self {
        Self { pool }
    }

    pub fn spawn<F, T>(&self, future: F) -> Result<Task<T>, ThreadPoolError>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        spawn_with_handle(self.pool.clone(), future)
    }
}

/// Returns a weak spawner associated with `pool`.
pub fn spawner(pool: &ThreadPool) -> Spawner {
    Spawner::from_pool(pool.handle())
}

/// Schedules a future on the NORTEX worker pool.
pub fn spawn<F, T>(pool: &ThreadPool, future: F) -> Result<Task<T>, ThreadPoolError>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    spawner(pool).spawn(future)
}

fn spawn_with_handle<F, T>(pool: PoolHandle, future: F) -> Result<Task<T>, ThreadPoolError>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let shared = Arc::new(TaskCell {
        future: Mutex::new(Some(Box::pin(future))),
        state: Mutex::new(TaskState {
            result: None,
            waiters: Vec::new(),
        }),
        pool,
        poll_state: AtomicU8::new(TASK_IDLE),
        completed: AtomicBool::new(false),
        registry_id: AtomicU64::new(0),
    });
    let registered_task: Arc<dyn ActiveTask> = shared.clone();
    let registry_id = shared.pool.register_task(registered_task)?;
    shared.registry_id.store(registry_id, Ordering::Release);
    shared.schedule()?;
    Ok(Task::new(shared))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::threading::ThreadPool;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn block_on_returns_ready_future_value() {
        assert_eq!(block_on(async { 7_u32 }), Ok(7));
    }

    #[test]
    fn block_on_from_worker_returns_an_error_without_panicking() {
        let pool = ThreadPool::new(1);
        let (sender, receiver) = mpsc::channel();
        pool.execute(move || {
            let result = block_on(async { 1_u8 });
            sender.send(result).unwrap();
        })
        .unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            Err(BlockOnError::CalledFromWorker)
        );
    }

    #[test]
    fn scheduled_task_returns_its_value() {
        let pool = ThreadPool::new(2);
        let task = spawn(&pool, async { 42_u32 }).expect("task should be queued");
        assert_eq!(block_on(task), Ok(Ok(42)));
    }

    #[test]
    fn task_panic_is_returned_without_killing_the_worker() {
        struct PanickingFuture;
        impl Future for PanickingFuture {
            type Output = u32;
            fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
                panic!("task failed")
            }
        }

        let pool = ThreadPool::new(1);
        let task = spawn(&pool, PanickingFuture).expect("task should be queued");
        assert_eq!(
            block_on(task),
            Ok(Err(TaskError::Panicked("task failed".into())))
        );
        let next = spawn(&pool, async { 9_u32 }).expect("worker should still accept work");
        assert_eq!(block_on(next), Ok(Ok(9)));
    }

    #[test]
    fn nested_async_tasks_suspend_instead_of_blocking_workers() {
        let pool = Arc::new(ThreadPool::new(1));
        let child_spawner = spawner(&pool);
        let parent = spawn(&pool, async move {
            let child = child_spawner
                .spawn(async { 42_u32 })
                .expect("child scheduled");
            child.await.expect("child task completes")
        })
        .expect("parent scheduled");
        assert_eq!(block_on(parent), Ok(Ok(42)));
    }

    #[test]
    fn deeply_nested_tasks_use_heap_continuations_without_panics_or_error_wrapping() {
        fn nested(spawner: Spawner, depth: usize, completed: Arc<AtomicUsize>) -> Task<()> {
            let child_spawner = spawner.clone();
            let child_completed = Arc::clone(&completed);
            spawner
                .spawn(async move {
                    if depth > 0 {
                        let child = nested(child_spawner, depth - 1, child_completed);
                        child.await.expect("nested child task should complete");
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                })
                .expect("nested task should be queued")
        }

        let pool = ThreadPool::new(2);
        let completed = Arc::new(AtomicUsize::new(0));
        let result = block_on(nested(spawner(&pool), 100, Arc::clone(&completed)));
        assert_eq!(result, Ok(Ok(())));
        assert_eq!(completed.load(Ordering::SeqCst), 101);
    }

    #[test]
    fn completed_task_polled_again_returns_already_completed() {
        let pool = ThreadPool::new(1);
        let mut task = Box::pin(spawn(&pool, async { 17_u32 }).expect("task should be queued"));
        let first = block_on(async { task.as_mut().await });
        assert_eq!(first, Ok(Ok(17)));
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        assert_eq!(
            Pin::as_mut(&mut task).poll(&mut context),
            Poll::Ready(Err(TaskError::AlreadyCompleted))
        );
    }

    #[test]
    fn yielding_future_does_not_starve_fifo_jobs() {
        let pool = ThreadPool::new(1);
        let (release_tx, release_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        pool.execute(move || {
            started_tx.send(()).unwrap();
            let _ = release_rx.recv();
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let flag = Arc::new(AtomicBool::new(false));
        let polls = Arc::new(AtomicUsize::new(0));
        let task_flag = Arc::clone(&flag);
        let task_polls = Arc::clone(&polls);
        let yielding = spawn(
            &pool,
            std::future::poll_fn(move |cx| {
                task_polls.fetch_add(1, Ordering::Relaxed);
                if task_flag.load(Ordering::Acquire) {
                    Poll::Ready(())
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }),
        )
        .unwrap();
        let job_flag = Arc::clone(&flag);
        pool.execute(move || job_flag.store(true, Ordering::Release))
            .unwrap();
        release_tx.send(()).unwrap();

        assert_eq!(block_on(yielding), Ok(Ok(())));
        assert!(
            polls.load(Ordering::Relaxed) < 1_000,
            "yielding future starved FIFO work"
        );
    }

    #[test]
    fn self_wake_during_poll_does_not_block_another_worker_on_future_mutex() {
        struct SelfWakeBlockingPoll {
            started: Option<mpsc::Sender<()>>,
            release: mpsc::Receiver<()>,
        }
        impl Future for SelfWakeBlockingPoll {
            type Output = ();
            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if let Some(started) = self.started.take() {
                    cx.waker().wake_by_ref();
                    let _ = started.send(());
                    let _ = self.release.recv();
                    Poll::Ready(())
                } else {
                    Poll::Ready(())
                }
            }
        }

        let pool = ThreadPool::new(2);
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let _task = spawn(
            &pool,
            SelfWakeBlockingPoll {
                started: Some(started_tx),
                release: release_rx,
            },
        )
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let (job_tx, job_rx) = mpsc::channel();
        pool.execute(move || job_tx.send(()).unwrap()).unwrap();
        job_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("second worker blocked on a Future mutex");
        release_tx.send(()).unwrap();
    }

    #[test]
    fn shutdown_of_deep_waiter_graph_is_iterative() {
        let pool = ThreadPool::new(1);
        let handle = pool.handle();
        let mut cells: Vec<Arc<TaskCell<()>>> = Vec::with_capacity(100_000);
        for _ in 0..100_000 {
            let cell = Arc::new(TaskCell {
                future: Mutex::new(None),
                state: Mutex::new(TaskState {
                    result: None,
                    waiters: Vec::new(),
                }),
                pool: handle.clone(),
                poll_state: AtomicU8::new(TASK_IDLE),
                completed: AtomicBool::new(false),
                registry_id: AtomicU64::new(0),
            });
            let registered_task: Arc<dyn ActiveTask> = cell.clone();
            let id = handle.register_task(registered_task).unwrap();
            cell.registry_id.store(id, Ordering::Release);
            cells.push(cell);
        }
        for index in 0..(cells.len() - 1) {
            cells[index + 1]
                .state
                .lock()
                .unwrap()
                .waiters
                .push(Waker::from(Arc::clone(&cells[index])));
        }
        drop(pool);
        assert!(
            cells
                .iter()
                .all(|cell| cell.completed.load(Ordering::Acquire))
        );
    }

    #[test]
    fn active_task_remains_scheduled_after_user_handle_is_dropped() {
        let pool = ThreadPool::new(1);
        let (finished_tx, finished_rx) = mpsc::channel();
        let polls = Arc::new(AtomicUsize::new(0));
        let task_polls = Arc::clone(&polls);
        let task = spawn(
            &pool,
            std::future::poll_fn(move |cx| {
                if task_polls.fetch_add(1, Ordering::SeqCst) == 0 {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else {
                    let _ = finished_tx.send(());
                    Poll::Ready(())
                }
            }),
        )
        .unwrap();
        drop(task);
        finished_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("the pool should retain and drive an active task after its handle is dropped");
        assert!(polls.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn weak_spawner_does_not_keep_pool_alive() {
        let pool = ThreadPool::new(1);
        let task = spawner(&pool).spawn(std::future::pending::<u32>()).unwrap();
        drop(pool);
        assert_eq!(block_on(task), Ok(Err(TaskError::ExecutorShutdown)));
    }

    #[test]
    fn two_yielding_tasks_do_not_starve_a_normal_job() {
        let pool = ThreadPool::new(2);
        let (ready_tx, ready_rx) = mpsc::channel();
        let start_gate = Arc::new(std::sync::Barrier::new(3));
        for _ in 0..2 {
            let ready = ready_tx.clone();
            let gate = Arc::clone(&start_gate);
            pool.execute(move || {
                ready.send(()).unwrap();
                gate.wait();
            })
            .unwrap();
        }
        drop(ready_tx);
        ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let flag = Arc::new(AtomicBool::new(false));
        let polls = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();

        for _ in 0..2 {
            let task_flag = Arc::clone(&flag);
            let task_polls = Arc::clone(&polls);
            tasks.push(
                spawn(
                    &pool,
                    std::future::poll_fn(move |cx| {
                        task_polls.fetch_add(1, Ordering::Relaxed);
                        if task_flag.load(Ordering::Acquire) {
                            Poll::Ready(())
                        } else {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                    }),
                )
                .unwrap(),
            );
        }

        let job_flag = Arc::clone(&flag);
        pool.execute(move || job_flag.store(true, Ordering::Release))
            .unwrap();
        start_gate.wait();
        for task in tasks {
            assert_eq!(block_on(task), Ok(Ok(())));
        }
        assert!(
            polls.load(Ordering::Relaxed) < 2_000,
            "normal work must run between cooperative yields"
        );
    }

    #[test]
    fn children_started_by_a_worker_preserve_fifo_submission_order() {
        let pool = ThreadPool::new(1);
        let child_spawner = spawner(&pool);
        let (sender, receiver) = mpsc::channel();
        let parent_spawner = child_spawner.clone();
        let parent_sender = sender.clone();

        let parent = spawn(&pool, async move {
            let mut children = Vec::new();
            for value in 1..=5 {
                let child_sender = parent_sender.clone();
                children.push(
                    parent_spawner
                        .spawn(async move {
                            child_sender.send(value).unwrap();
                            value
                        })
                        .unwrap(),
                );
            }
            for child in children {
                child.await.expect("child should complete");
            }
        })
        .unwrap();

        assert_eq!(block_on(parent), Ok(Ok(())));
        drop(sender);
        let order = (0..5)
            .map(|_| receiver.recv_timeout(Duration::from_secs(2)).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(order, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn pending_future_is_not_polled_periodically() {
        struct PendingFuture(Arc<AtomicUsize>);
        impl Future for PendingFuture {
            type Output = ();
            fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Poll::Pending
            }
        }

        let pool = ThreadPool::new(1);
        let polls = Arc::new(AtomicUsize::new(0));
        let task = spawn(&pool, PendingFuture(Arc::clone(&polls))).unwrap();
        // Wait for the first poll first; under load it may take longer than a fixed sleep.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while polls.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        drop(task);
        pool.shutdown();
    }

    #[test]
    fn concurrent_external_wakes_never_poll_a_task_twice_at_once() {
        // Eight threads wake one task 2_000 times each while it may be
        // running. RUNNING_NOTIFIED must coalesce wakes without overlap and
        // without losing the final wake.
        struct Probe {
            in_poll: Arc<AtomicUsize>,
            overlaps: Arc<AtomicUsize>,
            done: Arc<AtomicBool>,
            waker: Arc<Mutex<Option<Waker>>>,
        }
        impl Future for Probe {
            type Output = ();
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
                if self.in_poll.fetch_add(1, Ordering::SeqCst) != 0 {
                    self.overlaps.fetch_add(1, Ordering::SeqCst);
                }
                *self.waker.lock().unwrap() = Some(cx.waker().clone());
                std::hint::spin_loop();
                self.in_poll.fetch_sub(1, Ordering::SeqCst);
                if self.done.load(Ordering::SeqCst) {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            }
        }

        for _ in 0..20 {
            let pool = ThreadPool::new(4);
            let overlaps = Arc::new(AtomicUsize::new(0));
            let done = Arc::new(AtomicBool::new(false));
            let waker = Arc::new(Mutex::new(None::<Waker>));
            let task = spawn(
                &pool,
                Probe {
                    in_poll: Arc::new(AtomicUsize::new(0)),
                    overlaps: Arc::clone(&overlaps),
                    done: Arc::clone(&done),
                    waker: Arc::clone(&waker),
                },
            )
            .unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while waker.lock().unwrap().is_none() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "first poll did not happen"
                );
                std::thread::yield_now();
            }
            let wakers = (0..8)
                .map(|_| {
                    let waker = Arc::clone(&waker);
                    std::thread::spawn(move || {
                        for _ in 0..2_000 {
                            if let Some(waker) = waker.lock().unwrap().clone() {
                                waker.wake();
                            }
                        }
                    })
                })
                .collect::<Vec<_>>();
            for handle in wakers {
                handle.join().unwrap();
            }
            done.store(true, Ordering::SeqCst);
            waker.lock().unwrap().clone().unwrap().wake();
            let (result_tx, result_rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = result_tx.send(block_on(task));
            });
            assert_eq!(
                result_rx
                    .recv_timeout(Duration::from_secs(3))
                    .expect("final wake was lost"),
                Ok(Ok(()))
            );
            assert_eq!(
                overlaps.load(Ordering::SeqCst),
                0,
                "a task was polled concurrently"
            );
        }
    }

    #[test]
    fn shutdown_during_poll_cancels_and_drops_the_future_exactly_once() {
        struct CountDrop(Arc<AtomicUsize>);
        impl Drop for CountDrop {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        for _ in 0..50 {
            let pool = ThreadPool::new(2);
            let drops = Arc::new(AtomicUsize::new(0));
            let guard = CountDrop(Arc::clone(&drops));
            let (started_tx, started_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel::<()>();
            let mut started = Some(started_tx);
            let task = spawn(
                &pool,
                std::future::poll_fn(move |cx| {
                    let _keep = &guard;
                    if let Some(started) = started.take() {
                        started.send(()).unwrap();
                        let _ = release_rx.recv();
                        cx.waker().wake_by_ref();
                    }
                    Poll::<()>::Pending
                }),
            )
            .unwrap();
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            pool.shutdown();
            release_tx.send(()).unwrap();
            assert_eq!(block_on(task), Ok(Err(TaskError::ExecutorShutdown)));
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while drops.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            assert_eq!(
                drops.load(Ordering::SeqCst),
                1,
                "future must be dropped exactly once"
            );
        }
    }

    #[test]
    fn wake_after_completion_and_after_pool_drop_is_harmless() {
        let pool = ThreadPool::new(1);
        let saved = Arc::new(Mutex::new(None::<Waker>));
        let task_saved = Arc::clone(&saved);
        let task = spawn(
            &pool,
            std::future::poll_fn(move |cx| {
                *task_saved.lock().unwrap() = Some(cx.waker().clone());
                Poll::Ready(5_u8)
            }),
        )
        .unwrap();
        assert_eq!(block_on(task), Ok(Ok(5)));
        let waker = saved.lock().unwrap().take().unwrap();
        for _ in 0..1_000 {
            waker.wake_by_ref();
        }
        drop(pool);
        waker.wake();
    }

    #[test]
    fn panicking_future_destructor_during_shutdown_does_not_escape() {
        struct PanicOnDrop;
        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                panic!("expected test panic in drop");
            }
        }
        let pool = ThreadPool::new(1);
        let guard = PanicOnDrop;
        let task = spawn(&pool, async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        })
        .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !pool.inner_has_no_queued_jobs() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        pool.shutdown();
        assert_eq!(block_on(task), Ok(Err(TaskError::ExecutorShutdown)));
    }

    #[test]
    #[ignore = "Bekannte Grenze: Arc<ThreadPool> in einem wartenden Task bildet einen Zyklus; Spawner verwenden"]
    fn captured_arc_pool_in_pending_task_is_released() {
        let pool = Arc::new(ThreadPool::new(1));
        let weak = Arc::downgrade(&pool);
        let captured = Arc::clone(&pool);
        let _task = spawn(&pool, async move {
            let _keep = captured;
            std::future::pending::<()>().await;
        })
        .unwrap();
        drop(pool);
        assert!(
            weak.upgrade().is_none(),
            "pool is kept alive by its own task"
        );
    }
}
