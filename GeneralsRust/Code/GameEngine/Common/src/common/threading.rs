//! Threading Utilities Module
//!
//! Provides high-performance threading primitives using parking_lot:
//! - ScopedMutex: RAII-style scoped mutex locks
//! - FastMutex: Optimized mutex using parking_lot
//! - ScopedReadWriteLock: Scoped RwLock for shared/exclusive access
//! - ThreadPool: Simple thread pool for parallel task execution

use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Fast mutex using parking_lot for better performance than std::sync::Mutex
pub type FastMutex<T> = Mutex<T>;

/// Fast RwLock using parking_lot for better performance
pub type FastRwLock<T> = RwLock<T>;

/// RAII-style scoped mutex lock guard
///
/// Automatically releases the lock when dropped, ensuring exception safety
pub struct ScopedMutex<T> {
    mutex: Arc<FastMutex<T>>,
}

impl<T> ScopedMutex<T> {
    /// Create a new scoped mutex
    pub fn new(value: T) -> Self {
        Self {
            mutex: Arc::new(FastMutex::new(value)),
        }
    }

    /// Create from an existing Arc<Mutex>
    pub fn from_arc(mutex: Arc<FastMutex<T>>) -> Self {
        Self { mutex }
    }

    /// Lock the mutex and return a guard
    pub fn lock(&self) -> parking_lot::MutexGuard<'_, T> {
        self.mutex.lock()
    }

    /// Try to lock the mutex without blocking
    pub fn try_lock(&self) -> Option<parking_lot::MutexGuard<'_, T>> {
        self.mutex.try_lock()
    }

    /// Try to lock with a timeout
    pub fn try_lock_for(&self, duration: Duration) -> Option<parking_lot::MutexGuard<'_, T>> {
        self.mutex.try_lock_for(duration)
    }

    /// Check if the mutex is currently locked
    pub fn is_locked(&self) -> bool {
        self.mutex.is_locked()
    }

    /// Clone the inner Arc for sharing across threads
    pub fn clone_arc(&self) -> Arc<FastMutex<T>> {
        Arc::clone(&self.mutex)
    }
}

impl<T> Clone for ScopedMutex<T> {
    fn clone(&self) -> Self {
        Self {
            mutex: Arc::clone(&self.mutex),
        }
    }
}

/// RAII-style scoped read-write lock
///
/// Provides shared (read) and exclusive (write) access with automatic unlocking
pub struct ScopedReadWriteLock<T> {
    rwlock: Arc<FastRwLock<T>>,
}

impl<T> ScopedReadWriteLock<T> {
    /// Create a new scoped read-write lock
    pub fn new(value: T) -> Self {
        Self {
            rwlock: Arc::new(FastRwLock::new(value)),
        }
    }

    /// Create from an existing Arc<RwLock>
    pub fn from_arc(rwlock: Arc<FastRwLock<T>>) -> Self {
        Self { rwlock }
    }

    /// Acquire a read (shared) lock
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.rwlock.read()
    }

    /// Try to acquire a read lock without blocking
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        self.rwlock.try_read()
    }

    /// Try to acquire a read lock with a timeout
    pub fn try_read_for(&self, duration: Duration) -> Option<RwLockReadGuard<'_, T>> {
        self.rwlock.try_read_for(duration)
    }

    /// Acquire a write (exclusive) lock
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.rwlock.write()
    }

    /// Try to acquire a write lock without blocking
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        self.rwlock.try_write()
    }

    /// Try to acquire a write lock with a timeout
    pub fn try_write_for(&self, duration: Duration) -> Option<RwLockWriteGuard<'_, T>> {
        self.rwlock.try_write_for(duration)
    }

    /// Clone the inner Arc for sharing across threads
    pub fn clone_arc(&self) -> Arc<FastRwLock<T>> {
        Arc::clone(&self.rwlock)
    }
}

impl<T> Clone for ScopedReadWriteLock<T> {
    fn clone(&self) -> Self {
        Self {
            rwlock: Arc::clone(&self.rwlock),
        }
    }
}

/// Simple thread pool for executing tasks in parallel
pub struct ThreadPool {
    workers: Vec<thread::JoinHandle<()>>,
    sender: crossbeam_channel::Sender<Task>,
}

type Task = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    /// Create a new thread pool with the specified number of threads
    pub fn new(num_threads: usize) -> Self {
        assert!(num_threads > 0, "Thread pool size must be greater than 0");

        let (sender, receiver) = crossbeam_channel::unbounded::<Task>();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(num_threads);

        for id in 0..num_threads {
            let receiver = Arc::clone(&receiver);
            let handle = thread::Builder::new()
                .name(format!("ThreadPool-{}", id))
                .spawn(move || {
                    loop {
                        let task = {
                            let receiver = receiver.lock();
                            receiver.recv()
                        };

                        match task {
                            Ok(task) => task(),
                            Err(_) => break, // Channel closed, exit thread
                        }
                    }
                })
                .expect("Failed to spawn thread");

            workers.push(handle);
        }

        Self { workers, sender }
    }

    /// Execute a task on the thread pool
    pub fn execute<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.sender
            .send(Box::new(task))
            .expect("Failed to send task to thread pool");
    }

    /// Get the number of worker threads
    pub fn thread_count(&self) -> usize {
        self.workers.len()
    }

    /// Shutdown the thread pool and wait for all threads to finish
    pub fn shutdown(self) {
        drop(self.sender); // Close channel to signal threads to exit

        for worker in self.workers {
            worker.join().expect("Failed to join worker thread");
        }
    }
}

impl Default for ThreadPool {
    fn default() -> Self {
        Self::new(num_cpus::get())
    }
}

/// Utility for running code once across multiple threads
#[derive(Clone)]
pub struct Once {
    inner: std::sync::Arc<parking_lot::Once>,
}

impl Once {
    /// Create a new Once instance
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(parking_lot::Once::new()),
        }
    }

    /// Execute the closure only once, even if called from multiple threads
    pub fn call_once<F>(&self, f: F)
    where
        F: FnOnce(),
    {
        self.inner.call_once(f);
    }

    /// Check if the Once has been called
    pub fn is_completed(&self) -> bool {
        self.inner.state().done()
    }
}

impl Default for Once {
    fn default() -> Self {
        Self::new()
    }
}

/// Atomic flag for simple boolean signaling between threads
pub struct AtomicFlag {
    flag: Arc<parking_lot::Mutex<bool>>,
}

impl AtomicFlag {
    /// Create a new atomic flag with initial value
    pub fn new(initial: bool) -> Self {
        Self {
            flag: Arc::new(parking_lot::Mutex::new(initial)),
        }
    }

    /// Set the flag to true
    pub fn set(&self) {
        *self.flag.lock() = true;
    }

    /// Clear the flag to false
    pub fn clear(&self) {
        *self.flag.lock() = false;
    }

    /// Check if the flag is set
    pub fn is_set(&self) -> bool {
        *self.flag.lock()
    }

    /// Toggle the flag and return the new value
    pub fn toggle(&self) -> bool {
        let mut flag = self.flag.lock();
        *flag = !*flag;
        *flag
    }
}

impl Clone for AtomicFlag {
    fn clone(&self) -> Self {
        Self {
            flag: Arc::clone(&self.flag),
        }
    }
}

impl Default for AtomicFlag {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_scoped_mutex() {
        let mutex = ScopedMutex::new(0);

        {
            let mut guard = mutex.lock();
            *guard = 42;
        }

        let guard = mutex.lock();
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_scoped_mutex_try_lock() {
        let mutex = ScopedMutex::new(0);
        let _guard = mutex.lock();

        assert!(mutex.try_lock().is_none());
    }

    #[test]
    fn test_scoped_rwlock() {
        let rwlock = ScopedReadWriteLock::new(vec![1, 2, 3]);

        {
            let read_guard = rwlock.read();
            assert_eq!(read_guard.len(), 3);
        }

        {
            let mut write_guard = rwlock.write();
            write_guard.push(4);
        }

        let read_guard = rwlock.read();
        assert_eq!(read_guard.len(), 4);
    }

    #[test]
    fn test_thread_pool() {
        let pool = ThreadPool::new(4);
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let counter = Arc::clone(&counter);
            pool.execute(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        pool.shutdown();
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_once() {
        let once = Once::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let once = once.clone();
                let counter = Arc::clone(&counter);
                thread::spawn(move || {
                    once.call_once(|| {
                        counter.fetch_add(1, Ordering::SeqCst);
                    });
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(once.is_completed());
    }

    #[test]
    fn test_atomic_flag() {
        let flag = AtomicFlag::new(false);
        assert!(!flag.is_set());

        flag.set();
        assert!(flag.is_set());

        flag.clear();
        assert!(!flag.is_set());

        let result = flag.toggle();
        assert!(result);
        assert!(flag.is_set());
    }

    #[test]
    fn test_scoped_mutex_multithreaded() {
        let mutex = ScopedMutex::new(0);
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let mutex = mutex.clone();
                thread::spawn(move || {
                    let mut guard = mutex.lock();
                    *guard += 1;
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let guard = mutex.lock();
        assert_eq!(*guard, 10);
    }
}
