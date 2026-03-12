//! Internal thread pool for audio processing (not exposed in public API).

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::Result;

/// Task priority for thread pool
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Audio task for thread pool execution
struct AudioTask {
    priority: TaskPriority,
    task: Option<Box<dyn FnOnce() + Send + 'static>>,
    completion_sender: Option<tokio::sync::oneshot::Sender<Result<()>>>,
}

impl AudioTask {
    fn execute(mut self) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(task) = self.task.take() {
                (task)();
            }
        }));

        let outcome = match result {
            Ok(_) => Ok(()),
            Err(_) => Err(crate::error::Error::Audio("Task panicked".to_string())),
        };

        if let Some(sender) = self.completion_sender.take() {
            let _ = sender.send(outcome);
        }
    }
}

struct TaskQueue {
    inner: Mutex<Vec<AudioTask>>,
    condvar: Condvar,
    shutdown: AtomicBool,
}

impl TaskQueue {
    fn new() -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
            condvar: Condvar::new(),
            shutdown: AtomicBool::new(false),
        }
    }

    fn push(&self, task: AudioTask) {
        let mut guard = self.inner.lock().expect("task queue poisoned");
        guard.push(task);
        guard.sort_by(|a, b| b.priority.cmp(&a.priority));
        self.condvar.notify_one();
    }

    fn pop(&self) -> Option<AudioTask> {
        let mut guard = self.inner.lock().expect("task queue poisoned");
        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(task) = guard.pop() {
                return Some(task);
            }
            guard = self.condvar.wait(guard).expect("task queue wait poisoned");
        }
    }

    fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.condvar.notify_all();
    }
}

/// Thread pool for audio processing
pub(crate) struct AudioThreadPool {
    queue: Arc<TaskQueue>,
    workers: Vec<thread::JoinHandle<()>>,
    delayed_release: DelayedReleaseWorker,
}

impl AudioThreadPool {
    /// Create new audio thread pool
    pub fn new(config: ThreadPoolConfig) -> Self {
        let queue = Arc::new(TaskQueue::new());
        let mut workers = Vec::with_capacity(config.worker_count);
        for index in 0..config.worker_count {
            let queue_clone = Arc::clone(&queue);
            workers.push(thread::spawn(move || worker_loop(index, queue_clone)));
        }

        Self {
            queue,
            workers,
            delayed_release: DelayedReleaseWorker::new(),
        }
    }

    /// Submit task to thread pool
    pub fn submit<F>(
        &self,
        priority: TaskPriority,
        task: F,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<()>>>
    where
        F: FnOnce() + Send + 'static,
    {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let audio_task = AudioTask {
            priority,
            task: Some(Box::new(task)),
            completion_sender: Some(sender),
        };
        self.queue.push(audio_task);
        Ok(receiver)
    }

    /// Submit task without waiting for completion
    pub fn submit_fire_and_forget<F>(&self, priority: TaskPriority, task: F) -> Result<()>
    where
        F: FnOnce() + Send + 'static,
    {
        let audio_task = AudioTask {
            priority,
            task: Some(Box::new(task)),
            completion_sender: None,
        };
        self.queue.push(audio_task);
        Ok(())
    }

    pub fn submit_delay<F>(&self, delay_ms: u64, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.delayed_release.push(delay_ms, Box::new(task));
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.delayed_release.shutdown();
        self.queue.shutdown();
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
        Ok(())
    }

    pub fn submit_decode_task<F>(
        &self,
        task: F,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<()>>>
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit(TaskPriority::Normal, task)
    }

    pub fn submit_mix_task<F>(&self, task: F) -> Result<tokio::sync::oneshot::Receiver<Result<()>>>
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit(TaskPriority::High, task)
    }

    pub fn submit_stream_task<F>(
        &self,
        task: F,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<()>>>
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit(TaskPriority::Critical, task)
    }
}

fn worker_loop(worker_id: usize, queue: Arc<TaskQueue>) {
    log::debug!("Audio worker {} started", worker_id);
    while let Some(task) = queue.pop() {
        task.execute();
    }
    log::debug!("Audio worker {} stopped", worker_id);
}

struct DelayedReleaseWorker {
    queue: Arc<(Mutex<Vec<DelayedTask>>, Condvar)>,
    shutdown: Arc<AtomicBool>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}

struct DelayedTask {
    when: Instant,
    task: Option<Box<dyn FnOnce() + Send + 'static>>,
}

impl DelayedReleaseWorker {
    fn new() -> Self {
        let queue = Arc::new((Mutex::new(Vec::<DelayedTask>::new()), Condvar::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let queue_clone = Arc::clone(&queue);
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || {
            let (lock, cond) = &*queue_clone;
            loop {
                let mut guard = lock.lock().expect("delayed queue poisoned");
                while guard.is_empty() && !shutdown_clone.load(Ordering::Relaxed) {
                    guard = cond.wait(guard).expect("delayed queue wait poisoned");
                }

                if shutdown_clone.load(Ordering::Relaxed) {
                    break;
                }

                guard.sort_by(|a, b| a.when.cmp(&b.when));
                let now = Instant::now();
                if let Some(next) = guard.first() {
                    if next.when <= now {
                        let mut next = guard.remove(0);
                        drop(guard);
                        if let Some(task) = next.task.take() {
                            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task));
                        }
                        continue;
                    } else {
                        let wait_duration = next.when - now;
                        let (guard2, _) = cond
                            .wait_timeout(guard, wait_duration)
                            .expect("delayed queue wait timeout poisoned");
                        drop(guard2);
                        continue;
                    }
                }
            }
        });

        Self {
            queue,
            shutdown,
            thread: Mutex::new(Some(handle)),
        }
    }

    fn push(&self, delay_ms: u64, task: Box<dyn FnOnce() + Send + 'static>) {
        let (lock, cond) = &*self.queue;
        let mut guard = lock.lock().expect("delayed queue poisoned");
        guard.push(DelayedTask {
            when: Instant::now() + Duration::from_millis(delay_ms),
            task: Some(task),
        });
        cond.notify_one();
    }

    fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let (lock, cond) = &*self.queue;
        if let Ok(_guard) = lock.lock() {
            cond.notify_all();
        }
        if let Ok(mut handle_guard) = self.thread.lock() {
            if let Some(handle) = handle_guard.take() {
                let _ = handle.join();
            }
        }
    }
}

/// Thread pool configuration
#[derive(Debug, Clone)]
pub(crate) struct ThreadPoolConfig {
    pub worker_count: usize,
    pub queue_size: usize,
    pub enable_priority_queue: bool,
}

impl Default for ThreadPoolConfig {
    fn default() -> Self {
        Self {
            worker_count: num_cpus::get().max(2).min(8),
            queue_size: 1000,
            enable_priority_queue: true,
        }
    }
}

impl Drop for AudioThreadPool {
    fn drop(&mut self) {
        self.queue.shutdown();
        self.delayed_release.shutdown();
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

// Convenience functions for sharing a global thread pool
use std::sync::OnceLock;
static GLOBAL_POOL: OnceLock<AudioThreadPool> = OnceLock::new();

pub(crate) fn global_thread_pool() -> &'static AudioThreadPool {
    GLOBAL_POOL.get_or_init(|| AudioThreadPool::new(ThreadPoolConfig::default()))
}

pub(crate) fn queue_delayed_release<F>(delay_ms: u64, task: F)
where
    F: FnOnce() + Send + 'static,
{
    global_thread_pool().submit_delay(delay_ms, task);
}
