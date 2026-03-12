////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Critical Section Implementation
//!
//! Provides thread synchronization primitives for the game engine.
//! Implements critical sections, mutexes, and other synchronization mechanisms
//! to ensure thread safety in multi-threaded operations.
//!
//! Rust conversion: 2025

use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Fast critical section for high-performance synchronization
pub struct FastCriticalSection {
    mutex: Mutex<()>,
    is_locked: AtomicBool,
}

impl Default for FastCriticalSection {
    fn default() -> Self {
        Self::new()
    }
}

impl FastCriticalSection {
    /// Create a new fast critical section
    pub fn new() -> Self {
        Self {
            mutex: Mutex::new(()),
            is_locked: AtomicBool::new(false),
        }
    }

    /// Enter the critical section (blocking)
    pub fn enter(&self) -> CriticalSectionGuard<'_> {
        let _guard = self.mutex.lock().unwrap();
        self.is_locked.store(true, Ordering::Release);
        CriticalSectionGuard {
            section: self,
            _guard,
        }
    }

    /// Try to enter the critical section (non-blocking)
    pub fn try_enter(&self) -> Option<CriticalSectionGuard<'_>> {
        if let Ok(_guard) = self.mutex.try_lock() {
            self.is_locked.store(true, Ordering::Release);
            Some(CriticalSectionGuard {
                section: self,
                _guard,
            })
        } else {
            None
        }
    }

    /// Check if the critical section is currently locked
    pub fn is_locked(&self) -> bool {
        self.is_locked.load(Ordering::Acquire)
    }

    /// Leave the critical section (called automatically by guard drop)
    fn leave(&self) {
        self.is_locked.store(false, Ordering::Release);
    }
}

/// RAII guard for critical section
pub struct CriticalSectionGuard<'a> {
    section: &'a FastCriticalSection,
    _guard: std::sync::MutexGuard<'a, ()>,
}

impl<'a> Drop for CriticalSectionGuard<'a> {
    fn drop(&mut self) {
        self.section.leave();
    }
}

/// Read-write critical section for shared/exclusive access
pub struct ReadWriteCriticalSection<T> {
    data: RwLock<T>,
    reader_count: AtomicU32,
    writer_active: AtomicBool,
}

impl<T> ReadWriteCriticalSection<T> {
    /// Create a new read-write critical section
    pub fn new(data: T) -> Self {
        Self {
            data: RwLock::new(data),
            reader_count: AtomicU32::new(0),
            writer_active: AtomicBool::new(false),
        }
    }

    /// Enter for reading (shared access)
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, T> {
        let guard = self.data.read().unwrap();
        self.reader_count.fetch_add(1, Ordering::AcqRel);
        guard
    }

    /// Enter for writing (exclusive access)
    pub fn write(&self) -> WriteLockGuard<'_, T> {
        let guard = self.data.write().unwrap();
        self.writer_active.store(true, Ordering::Release);
        WriteLockGuard {
            guard,
            section: self,
        }
    }

    /// Try to enter for reading
    pub fn try_read(&self) -> Option<std::sync::RwLockReadGuard<'_, T>> {
        self.data.try_read().ok().map(|guard| {
            self.reader_count.fetch_add(1, Ordering::AcqRel);
            guard
        })
    }

    /// Try to enter for writing
    pub fn try_write(&self) -> Option<WriteLockGuard<'_, T>> {
        self.data.try_write().ok().map(|guard| {
            self.writer_active.store(true, Ordering::Release);
            WriteLockGuard {
                guard,
                section: self,
            }
        })
    }

    /// Get the current number of readers
    pub fn reader_count(&self) -> u32 {
        self.reader_count.load(Ordering::Acquire)
    }

    /// Check if a writer is currently active
    pub fn is_writer_active(&self) -> bool {
        self.writer_active.load(Ordering::Acquire)
    }

    fn on_read_exit(&self) {
        self.reader_count.fetch_sub(1, Ordering::AcqRel);
    }

    fn on_write_exit(&self) {
        self.writer_active.store(false, Ordering::Release);
    }
}

/// RAII guard for write lock
pub struct WriteLockGuard<'a, T> {
    guard: std::sync::RwLockWriteGuard<'a, T>,
    section: &'a ReadWriteCriticalSection<T>,
}

impl<'a, T> Drop for WriteLockGuard<'a, T> {
    fn drop(&mut self) {
        self.section.on_write_exit();
    }
}

impl<'a, T> std::ops::Deref for WriteLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a, T> std::ops::DerefMut for WriteLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

/// Scoped critical section lock for RAII-style locking
pub struct ScopedCriticalSection {
    section: Arc<FastCriticalSection>,
}

impl ScopedCriticalSection {
    /// Create a new scoped critical section
    pub fn new() -> Self {
        Self {
            section: Arc::new(FastCriticalSection::new()),
        }
    }

    /// Create a scoped lock
    pub fn lock(&self) -> ScopedLock<'_> {
        ScopedLock {
            _guard: self.section.enter(),
        }
    }

    /// Try to create a scoped lock
    pub fn try_lock(&self) -> Option<ScopedLock<'_>> {
        self.section.try_enter().map(|_guard| ScopedLock { _guard })
    }
}

impl Default for ScopedCriticalSection {
    fn default() -> Self {
        Self::new()
    }
}

/// Scoped lock guard
pub struct ScopedLock<'a> {
    _guard: CriticalSectionGuard<'a>,
}

/// Timeout-aware critical section
pub struct TimeoutCriticalSection {
    section: FastCriticalSection,
}

impl TimeoutCriticalSection {
    /// Create a new timeout critical section
    pub fn new() -> Self {
        Self {
            section: FastCriticalSection::new(),
        }
    }

    /// Enter with a timeout
    pub fn enter_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<CriticalSectionGuard<'_>, TimeoutError> {
        let start = Instant::now();

        loop {
            if let Some(guard) = self.section.try_enter() {
                return Ok(guard);
            }

            if start.elapsed() >= timeout {
                return Err(TimeoutError::Timeout);
            }

            std::thread::yield_now();
        }
    }

    /// Regular enter (blocking)
    pub fn enter(&self) -> CriticalSectionGuard<'_> {
        self.section.enter()
    }
}

impl Default for TimeoutCriticalSection {
    fn default() -> Self {
        Self::new()
    }
}

/// Timeout error
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeoutError {
    Timeout,
}

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeoutError::Timeout => write!(f, "Critical section lock timeout"),
        }
    }
}

impl std::error::Error for TimeoutError {}

/// Spin lock for very short-duration locks
pub struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    /// Create a new spin lock
    pub fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    /// Acquire the spin lock
    pub fn lock(&self) -> SpinLockGuard<'_> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.locked.load(Ordering::Relaxed) {
                std::hint::spin_loop();
            }
        }
        SpinLockGuard { lock: self }
    }

    /// Try to acquire the spin lock
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(SpinLockGuard { lock: self })
        } else {
            None
        }
    }

    /// Unlock the spin lock
    fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

impl Default for SpinLock {
    fn default() -> Self {
        Self::new()
    }
}

/// Spin lock guard
pub struct SpinLockGuard<'a> {
    lock: &'a SpinLock,
}

impl<'a> Drop for SpinLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

/// Critical section manager for organizing multiple critical sections
pub struct CriticalSectionManager {
    sections: std::collections::HashMap<String, Arc<FastCriticalSection>>,
    global_section: Arc<FastCriticalSection>,
}

impl CriticalSectionManager {
    /// Create a new critical section manager
    pub fn new() -> Self {
        Self {
            sections: std::collections::HashMap::new(),
            global_section: Arc::new(FastCriticalSection::new()),
        }
    }

    /// Get or create a named critical section
    pub fn get_section(&mut self, name: &str) -> Arc<FastCriticalSection> {
        self.sections
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(FastCriticalSection::new()))
            .clone()
    }

    /// Get the global critical section
    pub fn global_section(&self) -> Arc<FastCriticalSection> {
        self.global_section.clone()
    }

    /// Remove a named critical section
    pub fn remove_section(&mut self, name: &str) {
        self.sections.remove(name);
    }

    /// Get the number of managed sections
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }
}

impl Default for CriticalSectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global critical section manager instance
static CRITICAL_SECTION_MANAGER: OnceCell<std::sync::Mutex<CriticalSectionManager>> =
    OnceCell::new();

/// Initialize the global critical section manager
pub fn init_critical_section_manager() {
    if CRITICAL_SECTION_MANAGER.get().is_none() {
        let _ = CRITICAL_SECTION_MANAGER.set(std::sync::Mutex::new(CriticalSectionManager::new()));
    } else if let Some(manager) = CRITICAL_SECTION_MANAGER.get() {
        if let Ok(mut guard) = manager.lock() {
            *guard = CriticalSectionManager::new();
        }
    }
}

/// Get a reference to a named critical section
pub fn get_critical_section(name: &str) -> Option<Arc<FastCriticalSection>> {
    CRITICAL_SECTION_MANAGER
        .get()
        .and_then(|manager| manager.lock().ok())
        .map(|mut manager| manager.get_section(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_fast_critical_section() {
        let section = FastCriticalSection::new();
        assert!(!section.is_locked());

        let _guard = section.enter();
        assert!(section.is_locked());
    }

    #[test]
    fn test_critical_section_drop() {
        let section = FastCriticalSection::new();
        {
            let _guard = section.enter();
            assert!(section.is_locked());
        }
        assert!(!section.is_locked());
    }

    #[test]
    fn test_try_enter() {
        let section = Arc::new(FastCriticalSection::new());
        let section_clone = section.clone();

        let _guard1 = section.enter();
        assert!(section_clone.try_enter().is_none());
    }

    #[test]
    fn test_read_write_critical_section() {
        let section = ReadWriteCriticalSection::new(42);

        let read_guard = section.read();
        assert_eq!(*read_guard, 42);
        assert_eq!(section.reader_count(), 1);
        drop(read_guard);

        let mut write_guard = section.write();
        *write_guard = 100;
        assert!(section.is_writer_active());
        drop(write_guard);

        let read_guard = section.read();
        assert_eq!(*read_guard, 100);
    }

    #[test]
    fn test_spin_lock() {
        let lock = SpinLock::new();
        let _guard = lock.lock();
        assert!(lock.try_lock().is_none());
    }

    #[test]
    fn test_timeout_critical_section() {
        let section = TimeoutCriticalSection::new();
        let timeout = Duration::from_millis(10);

        let _guard = section.enter_with_timeout(timeout).unwrap();

        // This should timeout since the section is already locked
        let result = section.enter_with_timeout(Duration::from_millis(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_critical_section_manager() {
        let mut manager = CriticalSectionManager::new();
        assert_eq!(manager.section_count(), 0);

        let section1 = manager.get_section("test1");
        let section2 = manager.get_section("test2");
        assert_eq!(manager.section_count(), 2);

        manager.remove_section("test1");
        assert_eq!(manager.section_count(), 1);
    }

    #[test]
    fn test_multithreaded_access() {
        let section = Arc::new(FastCriticalSection::new());
        let counter = Arc::new(AtomicU32::new(0));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let section = section.clone();
                let counter = counter.clone();
                thread::spawn(move || {
                    let _guard = section.enter();
                    let current = counter.load(Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(1));
                    counter.store(current + 1, Ordering::Relaxed);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::Relaxed), 10);
    }
}
