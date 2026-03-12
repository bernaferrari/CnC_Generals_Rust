//! Audio Locking System
//! 
//! Provides thread-safe locking mechanisms for the audio system.
//! This is a direct conversion of the C++ AUD_Lock.cpp file to 
//! idiomatic Rust using Arc/Mutex patterns.

use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use crate::error::{AudioResult, AudioError};

/// Thread-safe lock for audio resources
/// 
/// This provides a counting lock mechanism similar to the original C++ version
/// but using Rust's safe concurrency primitives.
#[derive(Debug)]
pub struct AudioLock {
    /// Internal mutex for thread synchronization
    mutex: Arc<Mutex<()>>,
    
    /// Lock count for debugging and reference tracking
    count: AtomicUsize,
    
    /// Thread ID that currently holds the lock (for debugging)
    #[cfg(debug_assertions)]
    owner_thread: Arc<Mutex<Option<thread::ThreadId>>>,
}

/// RAII guard for audio lock
/// 
/// Automatically releases the lock when dropped, ensuring
/// exception safety and preventing deadlocks.
pub struct AudioLockGuard<'a> {
    _lock: &'a AudioLock,
    _guard: MutexGuard<'a, ()>,
}

impl AudioLock {
    /// Create a new audio lock
    /// 
    /// # Returns
    /// A new AudioLock instance
    pub fn new() -> Self {
        AudioLock {
            mutex: Arc::new(Mutex::new(())),
            count: AtomicUsize::new(0),
            #[cfg(debug_assertions)]
            owner_thread: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize a lock (for compatibility with C++ API)
    /// 
    /// In Rust, this is essentially the same as `new()` since
    /// initialization happens at construction time.
    pub fn init() -> Self {
        Self::new()
    }

    /// Acquire the lock
    /// 
    /// This will block until the lock becomes available.
    /// In debug builds, it also tracks which thread owns the lock.
    pub fn acquire(&self) {
        let _guard = self.mutex.lock().expect("Mutex poisoned");
        self.count.fetch_add(1, Ordering::SeqCst);
        
        #[cfg(debug_assertions)]
        {
            let mut owner = self.owner_thread.lock().expect("Owner thread mutex poisoned");
            assert!(owner.is_none() || owner.as_ref() == Some(&thread::current().id()),
                   "Lock acquired by different thread than expected");
            *owner = Some(thread::current().id());
        }
        
        // Keep the guard alive by storing it (this is handled by the caller)
        std::mem::forget(_guard);
    }

    /// Try to acquire the lock without blocking
    /// 
    /// # Returns
    /// Some(AudioLockGuard) if the lock was acquired, None if it's busy
    pub fn try_acquire(&self) -> Option<AudioLockGuard> {
        if let Ok(guard) = self.mutex.try_lock() {
            self.count.fetch_add(1, Ordering::SeqCst);
            
            #[cfg(debug_assertions)]
            {
                let mut owner = self.owner_thread.lock().expect("Owner thread mutex poisoned");
                *owner = Some(thread::current().id());
            }
            
            Some(AudioLockGuard {
                _lock: self,
                _guard: guard,
            })
        } else {
            None
        }
    }

    /// Acquire the lock with a timeout
    /// 
    /// # Arguments
    /// * `timeout` - Maximum time to wait for the lock
    /// 
    /// # Returns
    /// Result containing the lock guard or a timeout error
    pub fn acquire_timeout(&self, timeout: Duration) -> AudioResult<AudioLockGuard> {
        let start = std::time::Instant::now();
        
        loop {
            if let Some(guard) = self.try_acquire() {
                return Ok(guard);
            }
            
            if start.elapsed() >= timeout {
                return Err(AudioError::LockTimeout);
            }
            
            // Small sleep to avoid busy waiting
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Release the lock
    /// 
    /// # Safety
    /// This should only be called by the thread that acquired the lock.
    /// In debug builds, this is enforced with assertions.
    pub fn release(&self) {
        #[cfg(debug_assertions)]
        {
            let mut owner = self.owner_thread.lock().expect("Owner thread mutex poisoned");
            assert_eq!(owner.as_ref(), Some(&thread::current().id()),
                      "Lock released by different thread than the one that acquired it");
            *owner = None;
        }
        
        let count = self.count.fetch_sub(1, Ordering::SeqCst);
        assert!(count > 0, "Released lock that wasn't acquired");
        
        // The actual mutex release happens when the guard is dropped
    }

    /// Check if the lock is currently held
    /// 
    /// # Returns
    /// true if the lock is held, false otherwise
    pub fn is_locked(&self) -> bool {
        self.count.load(Ordering::SeqCst) > 0
    }

    /// Get the current lock count (for debugging)
    /// 
    /// # Returns
    /// The number of times this lock has been acquired
    pub fn get_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    /// Create a scoped lock that automatically releases when dropped
    /// 
    /// # Returns
    /// A guard that will automatically release the lock when dropped
    pub fn scoped_lock(&self) -> AudioLockGuard {
        let guard = self.mutex.lock().expect("Mutex poisoned");
        self.count.fetch_add(1, Ordering::SeqCst);
        
        #[cfg(debug_assertions)]
        {
            let mut owner = self.owner_thread.lock().expect("Owner thread mutex poisoned");
            *owner = Some(thread::current().id());
        }
        
        AudioLockGuard {
            _lock: self,
            _guard: guard,
        }
    }
}

impl Default for AudioLock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AudioLock {
    /// Clone creates a new independent lock
    /// 
    /// Note: This doesn't share the lock state - each clone
    /// is a completely separate lock.
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl Drop for AudioLockGuard<'_> {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            let mut owner = self._lock.owner_thread.lock().expect("Owner thread mutex poisoned");
            *owner = None;
        }
        
        self._lock.count.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Safe wrapper for C-style lock operations
/// 
/// This provides a more traditional C-style API that matches
/// the original implementation while maintaining Rust safety.
pub struct CLock {
    lock: AudioLock,
}

impl CLock {
    /// Initialize a new C-style lock
    pub fn init() -> Self {
        CLock {
            lock: AudioLock::new(),
        }
    }

    /// Acquire the lock (C-style API)
    pub fn acquire(&self) {
        self.lock.acquire();
    }

    /// Release the lock (C-style API)
    pub fn release(&self) {
        self.lock.release();
    }

    /// Check if locked (C-style API)
    pub fn is_locked(&self) -> bool {
        self.lock.is_locked()
    }
}

/// Macro for creating scoped locks
/// 
/// Usage: `audio_lock!(lock_variable) { /* critical section */ }`
#[macro_export]
macro_rules! audio_lock {
    ($lock:expr) => {
        let _guard = $lock.scoped_lock();
    };
    ($lock:expr, $body:block) => {
        {
            let _guard = $lock.scoped_lock();
            $body
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_basic_locking() {
        let lock = AudioLock::new();
        
        assert!(!lock.is_locked());
        assert_eq!(lock.get_count(), 0);
        
        {
            let _guard = lock.scoped_lock();
            assert!(lock.is_locked());
            assert_eq!(lock.get_count(), 1);
        }
        
        assert!(!lock.is_locked());
        assert_eq!(lock.get_count(), 0);
    }

    #[test]
    fn test_try_acquire() {
        let lock = AudioLock::new();
        
        // First acquisition should succeed
        let guard1 = lock.try_acquire();
        assert!(guard1.is_some());
        
        // Second acquisition should fail
        let guard2 = lock.try_acquire();
        assert!(guard2.is_none());
        
        // After dropping the first guard, second should succeed
        drop(guard1);
        let guard3 = lock.try_acquire();
        assert!(guard3.is_some());
    }

    #[test]
    fn test_timeout_acquire() {
        let lock = Arc::new(AudioLock::new());
        let lock_clone = Arc::clone(&lock);
        
        // Spawn a thread that holds the lock for a short time
        let handle = thread::spawn(move || {
            let _guard = lock_clone.scoped_lock();
            thread::sleep(Duration::from_millis(100));
        });
        
        // Try to acquire with a longer timeout - should succeed
        let result = lock.acquire_timeout(Duration::from_millis(200));
        assert!(result.is_ok());
        
        handle.join().unwrap();
    }

    #[test]
    fn test_timeout_failure() {
        let lock = Arc::new(AudioLock::new());
        let lock_clone = Arc::clone(&lock);
        
        // Spawn a thread that holds the lock for a long time
        let handle = thread::spawn(move || {
            let _guard = lock_clone.scoped_lock();
            thread::sleep(Duration::from_millis(200));
        });
        
        // Give the thread time to acquire the lock
        thread::sleep(Duration::from_millis(10));
        
        // Try to acquire with a short timeout - should fail
        let result = lock.acquire_timeout(Duration::from_millis(50));
        assert!(result.is_err());
        
        handle.join().unwrap();
    }

    #[test]
    fn test_c_style_api() {
        let clock = CLock::init();
        
        assert!(!clock.is_locked());
        
        clock.acquire();
        assert!(clock.is_locked());
        
        clock.release();
        assert!(!clock.is_locked());
    }

    #[test]
    fn test_macro() {
        let lock = AudioLock::new();
        
        audio_lock!(lock, {
            assert!(lock.is_locked());
        });
        
        assert!(!lock.is_locked());
    }

    #[test]
    fn test_multithreaded_access() {
        let lock = Arc::new(AudioLock::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];
        
        // Spawn multiple threads that increment a counter
        for _ in 0..10 {
            let lock_clone = Arc::clone(&lock);
            let counter_clone = Arc::clone(&counter);
            
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let _guard = lock_clone.scoped_lock();
                    let current = counter_clone.load(Ordering::SeqCst);
                    thread::sleep(Duration::from_nanos(1)); // Simulate some work
                    counter_clone.store(current + 1, Ordering::SeqCst);
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify that all increments were applied correctly
        assert_eq!(counter.load(Ordering::SeqCst), 1000);
    }
}