//! Audio-specific locking and synchronization primitives.

use parking_lot::{Condvar, Mutex, RwLock};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Audio-specific mutex with timeout support
pub struct AudioMutex<T> {
    inner: Arc<Mutex<T>>,
    name: String,
}

/// Audio-specific read-write lock
pub struct AudioRwLock<T> {
    inner: Arc<RwLock<T>>,
    name: String,
}

/// Audio condition variable for thread synchronization
pub struct AudioCondvar {
    inner: Arc<Condvar>,
    name: String,
}

/// Lock timeout configuration
#[derive(Debug, Clone)]
pub struct LockConfig {
    pub timeout: Duration,
    pub enable_profiling: bool,
    pub enable_deadlock_detection: bool,
}

impl<T> AudioMutex<T> {
    /// Create new audio mutex
    pub fn new(data: T, name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(data)),
            name: name.into(),
        }
    }

    /// Lock with timeout
    pub fn lock_timeout(&self, timeout: Duration) -> Option<parking_lot::MutexGuard<'_, T>> {
        let start = Instant::now();
        loop {
            if let Some(guard) = self.inner.try_lock() {
                return Some(guard);
            }

            if start.elapsed() > timeout {
                log::warn!("Mutex lock timeout for '{}' after {:?}", self.name, timeout);
                return None;
            }

            std::thread::yield_now();
        }
    }

    /// Try to lock without blocking
    pub fn try_lock(&self) -> Option<parking_lot::MutexGuard<'_, T>> {
        self.inner.try_lock()
    }

    /// Lock (blocking)
    pub fn lock(&self) -> parking_lot::MutexGuard<'_, T> {
        self.inner.lock()
    }

    /// Get mutex name
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<T> AudioRwLock<T> {
    /// Create new audio read-write lock
    pub fn new(data: T, name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(data)),
            name: name.into(),
        }
    }

    /// Read lock with timeout
    pub fn read_timeout(&self, timeout: Duration) -> Option<parking_lot::RwLockReadGuard<'_, T>> {
        let start = Instant::now();
        loop {
            if let Some(guard) = self.inner.try_read() {
                return Some(guard);
            }

            if start.elapsed() > timeout {
                log::warn!("Read lock timeout for '{}' after {:?}", self.name, timeout);
                return None;
            }

            std::thread::yield_now();
        }
    }

    /// Write lock with timeout
    pub fn write_timeout(&self, timeout: Duration) -> Option<parking_lot::RwLockWriteGuard<'_, T>> {
        let start = Instant::now();
        loop {
            if let Some(guard) = self.inner.try_write() {
                return Some(guard);
            }

            if start.elapsed() > timeout {
                log::warn!("Write lock timeout for '{}' after {:?}", self.name, timeout);
                return None;
            }

            std::thread::yield_now();
        }
    }

    /// Read lock (blocking)
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, T> {
        self.inner.read()
    }

    /// Write lock (blocking)
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, T> {
        self.inner.write()
    }

    /// Try read lock
    pub fn try_read(&self) -> Option<parking_lot::RwLockReadGuard<'_, T>> {
        self.inner.try_read()
    }

    /// Try write lock
    pub fn try_write(&self) -> Option<parking_lot::RwLockWriteGuard<'_, T>> {
        self.inner.try_write()
    }

    /// Get lock name
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl AudioCondvar {
    /// Create new audio condition variable
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Condvar::new()),
            name: name.into(),
        }
    }

    /// Wait on condition variable
    pub fn wait<T>(&self, guard: &mut parking_lot::MutexGuard<T>) {
        self.inner.wait(guard);
    }

    /// Wait with timeout
    pub fn wait_timeout<T>(
        &self,
        guard: &mut parking_lot::MutexGuard<T>,
        timeout: Duration,
    ) -> bool {
        self.inner.wait_for(guard, timeout).timed_out()
    }

    /// Notify one waiting thread
    pub fn notify_one(&self) {
        self.inner.notify_one();
    }

    /// Notify all waiting threads
    pub fn notify_all(&self) {
        self.inner.notify_all();
    }

    /// Get condition variable name
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Scoped lock guard that automatically releases on drop
pub struct ScopedLock<'a, T> {
    _guard: parking_lot::MutexGuard<'a, T>,
    name: String,
}

impl<'a, T> Drop for ScopedLock<'a, T> {
    fn drop(&mut self) {
        log::trace!("Released scoped lock: {}", self.name);
    }
}

/// Lock manager for tracking and debugging locks
pub struct LockManager {
    config: LockConfig,
    active_locks: Arc<Mutex<std::collections::HashMap<String, Instant>>>,
}

impl LockManager {
    /// Create new lock manager
    pub fn new(config: LockConfig) -> Self {
        Self {
            config,
            active_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Register lock acquisition
    pub fn register_lock(&self, name: &str) {
        if self.config.enable_profiling {
            let mut locks = self.active_locks.lock();
            locks.insert(name.to_string(), Instant::now());
        }
    }

    /// Unregister lock release
    pub fn unregister_lock(&self, name: &str) {
        if self.config.enable_profiling {
            let mut locks = self.active_locks.lock();
            if let Some(start_time) = locks.remove(name) {
                let duration = start_time.elapsed();
                log::debug!("Lock '{}' held for {:?}", name, duration);
            }
        }
    }

    /// Check for potential deadlocks
    pub fn check_deadlocks(&self) -> Vec<String> {
        let mut deadlocked = Vec::new();

        if self.config.enable_deadlock_detection {
            let locks = self.active_locks.lock();
            let now = Instant::now();

            for (name, start_time) in locks.iter() {
                if now.duration_since(*start_time) > self.config.timeout * 2 {
                    deadlocked.push(name.clone());
                }
            }
        }

        deadlocked
    }
}

impl Default for LockConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(5000), // 5 second timeout
            enable_profiling: cfg!(debug_assertions),
            enable_deadlock_detection: cfg!(debug_assertions),
        }
    }
}

impl Default for LockManager {
    fn default() -> Self {
        Self::new(LockConfig::default())
    }
}

impl<T> Clone for AudioMutex<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            name: self.name.clone(),
        }
    }
}

impl<T> Clone for AudioRwLock<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            name: self.name.clone(),
        }
    }
}

impl Clone for AudioCondvar {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            name: self.name.clone(),
        }
    }
}

/// Convenience macro for creating named locks
#[macro_export]
macro_rules! audio_mutex {
    ($data:expr, $name:expr) => {
        $crate::lock::AudioMutex::new($data, $name)
    };
}

#[macro_export]
macro_rules! audio_rwlock {
    ($data:expr, $name:expr) => {
        $crate::lock::AudioRwLock::new($data, $name)
    };
}
