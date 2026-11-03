//! Fixed-size worker pool with FIFO scheduling and explicit shutdown behavior.
//!
//! Work submitted by a worker is appended to the same FIFO queue as external
//! work. Async continuations use the same rule, preventing self-waking futures
//! from starving ordinary jobs. The async runtime never runs an unrelated job
//! on a waiting task's stack.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::task::Waker;
use std::thread::{self, JoinHandle};

const WORKER_EXITED: u8 = 0b01;
const WORKER_DETACHED: u8 = 0b10;
static DETACHED_WORKERS: AtomicUsizeCompat = AtomicUsizeCompat::new();

// A tiny wrapper keeps this module's public surface independent of a second
// global statistics dependency.
struct AtomicUsizeCompat(std::sync::atomic::AtomicUsize);
impl AtomicUsizeCompat {
    const fn new() -> Self {
        Self(std::sync::atomic::AtomicUsize::new(0))
    }
    fn load(&self) -> usize {
        self.0.load(Ordering::Acquire)
    }
    fn fetch_add(&self, value: usize) {
        self.0.fetch_add(value, Ordering::AcqRel);
    }
    fn fetch_sub(&self, value: usize) {
        self.0.fetch_sub(value, Ordering::AcqRel);
    }
}

struct WorkerState(AtomicU8);

impl WorkerState {
    fn new() -> Self {
        Self(AtomicU8::new(0))
    }

    fn mark_detached(&self) {
        let previous = self.0.fetch_or(WORKER_DETACHED, Ordering::AcqRel);
        if previous & WORKER_EXITED == 0 {
            DETACHED_WORKERS.fetch_add(1);
        }
    }

    fn mark_exited(&self) {
        let previous = self.0.fetch_or(WORKER_EXITED, Ordering::AcqRel);
        if previous & WORKER_DETACHED != 0 {
            DETACHED_WORKERS.fetch_sub(1);
        }
    }
}

struct Worker {
    handle: JoinHandle<()>,
    state: Arc<WorkerState>,
}

struct Job {
    run: Box<dyn FnOnce() + Send + 'static>,
}

impl Job {
    fn run(self) {
        // The panic hook reports the original payload. Catching keeps a single
        // failing job from terminating the worker thread.
        let _ = catch_unwind(AssertUnwindSafe(self.run));
    }
}

pub(crate) trait ActiveTask: Send + Sync {
    /// Completes a registered task without waking dependants. The pool wakes
    /// the collected waiters only after every task has reached a terminal state.
    fn cancel_for_shutdown(&self) -> Vec<Waker>;
}

struct PoolInner {
    queue: Mutex<VecDeque<Job>>,
    active_tasks: Mutex<HashMap<u64, Arc<dyn ActiveTask>>>,
    next_task_id: AtomicU64,
    available: Condvar,
    shutdown: AtomicBool,
}

thread_local! {
    static CURRENT_POOL: RefCell<Option<Arc<PoolInner>>> = const { RefCell::new(None) };
}

fn worker_loop(inner: Arc<PoolInner>, state: Arc<WorkerState>, _index: usize) {
    CURRENT_POOL.with(|slot| *slot.borrow_mut() = Some(Arc::clone(&inner)));

    loop {
        let job = {
            let mut queue = inner
                .queue
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            loop {
                if inner.shutdown.load(Ordering::Acquire) {
                    break None;
                }
                if let Some(job) = queue.pop_front() {
                    break Some(job);
                }
                queue = inner
                    .available
                    .wait(queue)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        };

        match job {
            Some(job) => job.run(),
            None => break,
        }
    }

    CURRENT_POOL.with(|slot| *slot.borrow_mut() = None);
    state.mark_exited();
}

/// A weak reference to a worker pool's scheduling state. Store this in async
/// futures instead of capturing `Arc<ThreadPool>`; it does not keep the pool
/// alive and therefore cannot create a pool/task ownership cycle.
#[derive(Clone)]
pub(crate) struct PoolHandle {
    inner: Weak<PoolInner>,
}

impl PoolHandle {
    pub(crate) fn register_task(&self, task: Arc<dyn ActiveTask>) -> Result<u64, ThreadPoolError> {
        let inner = self.inner.upgrade().ok_or(ThreadPoolError::ShutDown)?;
        if inner.shutdown.load(Ordering::Acquire) {
            return Err(ThreadPoolError::ShutDown);
        }
        let mut tasks = inner
            .active_tasks
            .lock()
            .map_err(|_| ThreadPoolError::ShutDown)?;
        if inner.shutdown.load(Ordering::Acquire) {
            return Err(ThreadPoolError::ShutDown);
        }
        let id = inner.next_task_id.fetch_add(1, Ordering::Relaxed);
        tasks.insert(id, task);
        Ok(id)
    }

    pub(crate) fn unregister_task(&self, id: u64) {
        if let Some(inner) = self.inner.upgrade()
            && let Ok(mut tasks) = inner.active_tasks.lock()
        {
            tasks.remove(&id);
        }
    }

    pub(crate) fn execute<F>(&self, task: F) -> Result<(), ThreadPoolError>
    where
        F: FnOnce() + Send + 'static,
    {
        let inner = self.inner.upgrade().ok_or(ThreadPoolError::ShutDown)?;
        if inner.shutdown.load(Ordering::Acquire) {
            return Err(ThreadPoolError::ShutDown);
        }
        let mut queue = inner.queue.lock().map_err(|_| ThreadPoolError::ShutDown)?;
        if inner.shutdown.load(Ordering::Acquire) {
            return Err(ThreadPoolError::ShutDown);
        }
        queue.push_back(Job {
            run: Box::new(task),
        });
        drop(queue);
        inner.available.notify_one();
        Ok(())
    }
}

/// A fixed-size pool. `Drop` cancels queued jobs and suspended async tasks,
/// then detaches still-running workers. A running arbitrary closure cannot be
/// force-stopped safely; detached workers are counted until they exit.
///
/// Use `shutdown_and_join` when waiting for every worker is required. It can
/// block forever if a user-supplied synchronous job never returns.
pub struct ThreadPool {
    inner: Arc<PoolInner>,
    workers: Vec<Worker>,
}

impl ThreadPool {
    pub fn new(worker_count: usize) -> Self {
        let worker_count = worker_count.max(1);
        let inner = Arc::new(PoolInner {
            queue: Mutex::new(VecDeque::new()),
            active_tasks: Mutex::new(HashMap::new()),
            next_task_id: AtomicU64::new(1),
            available: Condvar::new(),
            shutdown: AtomicBool::new(false),
        });
        let mut workers = Vec::with_capacity(worker_count);

        for index in 0..worker_count {
            let worker_inner = Arc::clone(&inner);
            let state = Arc::new(WorkerState::new());
            let worker_state = Arc::clone(&state);
            let handle = thread::Builder::new()
                .name(format!("nortex-worker-{index}"))
                .spawn(move || worker_loop(worker_inner, worker_state, index))
                .expect("NORTEX worker thread could not be started");
            workers.push(Worker { handle, state });
        }

        Self { inner, workers }
    }

    /// Creates a pool sized to the number of logical processors reported by
    /// the operating system. The count is always at least one.
    pub fn available() -> Self {
        let count = thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1);
        Self::new(count)
    }

    pub(crate) fn handle(&self) -> PoolHandle {
        PoolHandle {
            inner: Arc::downgrade(&self.inner),
        }
    }

    /// Queues a fire-and-forget job at the back of the FIFO queue.
    pub fn execute<F>(&self, task: F) -> Result<(), ThreadPoolError>
    where
        F: FnOnce() + Send + 'static,
    {
        self.handle().execute(task)
    }

    /// Stops accepting work, cancels queued jobs and suspended async tasks,
    /// and wakes every worker. Jobs already running are allowed to finish.
    pub fn shutdown(&self) {
        stop_pool(&self.inner);
    }

    /// Stops the pool and waits for running jobs to finish.
    pub fn shutdown_and_join(mut self) {
        stop_pool(&self.inner);
        if self.is_current_worker() {
            self.detach_workers();
            self.workers.clear();
            return;
        }
        for worker in self.workers.drain(..) {
            let _ = worker.handle.join();
        }
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Number of detached worker threads that have not exited yet.
    pub fn detached_worker_count() -> usize {
        DETACHED_WORKERS.load()
    }

    /// Reports whether the current thread is executing on this exact pool.
    pub fn is_current_worker(&self) -> bool {
        CURRENT_POOL.with(|slot| {
            slot.borrow()
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &self.inner))
        })
    }

    #[cfg(test)]
    pub(crate) fn inner_has_no_queued_jobs(&self) -> bool {
        self.inner
            .queue
            .lock()
            .map(|queue| queue.is_empty())
            .unwrap_or(true)
    }

    fn detach_workers(&self) {
        for worker in &self.workers {
            worker.state.mark_detached();
        }
    }
}

pub(crate) fn is_worker_thread() -> bool {
    CURRENT_POOL.with(|slot| slot.borrow().is_some())
}

fn stop_pool(inner: &Arc<PoolInner>) {
    if inner.shutdown.swap(true, Ordering::AcqRel) {
        return;
    }

    // Drop queued closures instead of invoking their cancellation callbacks.
    // All registered async tasks are completed in one batch below.
    let queued = {
        let mut queue = inner
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        queue.drain(..).collect::<Vec<_>>()
    };
    drop(queued);

    let active_tasks = {
        let mut tasks = inner
            .active_tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::take(&mut *tasks)
            .into_values()
            .collect::<Vec<_>>()
    };

    // First mark every task completed without waking dependants. Only after
    // the entire set is terminal do we issue wakes. This avoids recursive
    // parent->child->parent cancellation chains during shutdown.
    let mut waiters = Vec::new();
    for task in &active_tasks {
        if let Ok(mut task_waiters) = catch_unwind(AssertUnwindSafe(|| task.cancel_for_shutdown()))
        {
            waiters.append(&mut task_waiters);
        }
    }
    drop(active_tasks);
    for waiter in waiters {
        waiter.wake();
    }

    inner.available.notify_all();
}

impl Default for ThreadPool {
    fn default() -> Self {
        Self::available()
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        stop_pool(&self.inner);
        self.detach_workers();
        self.workers.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadPoolError {
    ShutDown,
}

pub fn spawn<F, T>(task: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    thread::spawn(task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn pool_executes_a_job() {
        let pool = ThreadPool::new(2);
        let (sender, receiver) = mpsc::channel();
        pool.execute(move || sender.send(42).unwrap()).unwrap();
        assert_eq!(receiver.recv_timeout(Duration::from_secs(2)).unwrap(), 42);
    }

    #[test]
    fn panicking_job_does_not_kill_worker() {
        let pool = ThreadPool::new(1);
        let (sender, receiver) = mpsc::channel();
        pool.execute(|| panic!("expected test panic")).unwrap();
        pool.execute(move || sender.send(7).unwrap()).unwrap();
        assert_eq!(receiver.recv_timeout(Duration::from_secs(2)).unwrap(), 7);
        assert_eq!(pool.worker_count(), 1);
    }

    #[test]
    fn two_workers_execute_jobs_concurrently() {
        let pool = ThreadPool::new(2);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let (sender, receiver) = mpsc::channel();
        for _ in 0..2 {
            let barrier = Arc::clone(&barrier);
            let sender = sender.clone();
            pool.execute(move || {
                sender.send(()).unwrap();
                barrier.wait();
            })
            .unwrap();
        }
        drop(sender);
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    }

    #[test]
    fn worker_membership_is_pool_specific() {
        let pool_a = ThreadPool::new(1);
        let pool_b = Arc::new(ThreadPool::new(1));
        let (sender, receiver) = mpsc::channel();
        let b = Arc::clone(&pool_b);
        pool_a
            .execute(move || {
                let (inner_sender, inner_receiver) = mpsc::channel();
                b.execute(move || inner_sender.send(thread::current().id()).unwrap())
                    .unwrap();
                let id = inner_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
                sender.send(id != thread::current().id()).unwrap();
            })
            .unwrap();
        assert!(receiver.recv_timeout(Duration::from_secs(3)).unwrap());
    }

    #[test]
    fn detached_worker_is_counted_until_it_exits() {
        let pool = ThreadPool::new(1);
        let worker_state = Arc::clone(&pool.workers[0].state);
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        pool.execute(move || {
            started_tx.send(()).unwrap();
            let _ = release_rx.recv();
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        drop(pool);
        assert_ne!(worker_state.0.load(Ordering::Acquire) & WORKER_DETACHED, 0);
        assert!(ThreadPool::detached_worker_count() >= 1);
        release_tx.send(()).unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while worker_state.0.load(Ordering::Acquire) & WORKER_EXITED == 0
            && std::time::Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(2));
        }
        assert_ne!(
            worker_state.0.load(Ordering::Acquire) & WORKER_EXITED,
            0,
            "detached worker must eventually exit after its blocking job returns"
        );
    }

    #[test]
    fn drop_cancels_queued_jobs_without_waiting_for_them() {
        let pool = ThreadPool::new(1);
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        pool.execute(move || {
            started_tx.send(()).unwrap();
            let _ = release_rx.recv();
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        pool.execute(|| {}).unwrap();

        let (dropped_tx, dropped_rx) = mpsc::channel();
        let drop_thread = thread::spawn(move || {
            drop(pool);
            let _ = dropped_tx.send(());
        });
        dropped_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("dropping a pool must not wait for a blocked job");
        release_tx.send(()).unwrap();
        drop_thread.join().unwrap();
    }
}
