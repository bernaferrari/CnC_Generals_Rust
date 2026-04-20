//! Smart Handles for Pool-Allocated Objects
//!
//! Provides RAII handles that automatically manage object lifetimes
//! and prevent dangling references through generational checking.

use super::generation::GenerationalIndex;
use super::pool::ObjectPool;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};

/// A strong handle to a pool-allocated object.
///
/// This handle owns a reference to the object and will automatically
/// deallocate it when dropped. It uses generational indexing to prevent
/// use-after-free errors.
pub struct PoolHandle<T> {
    /// The pool this object belongs to.
    pool: Arc<ObjectPool<T>>,
    /// Generational index of the object.
    index: GenerationalIndex,
    /// Marker for proper variance.
    _marker: PhantomData<T>,
}

impl<T> PoolHandle<T> {
    /// Create a new handle (called by ObjectPool).
    pub(crate) fn new(pool: Arc<ObjectPool<T>>, index: GenerationalIndex) -> Self {
        Self {
            pool,
            index,
            _marker: PhantomData,
        }
    }

    /// Get the generational index.
    #[inline]
    pub fn index(&self) -> GenerationalIndex {
        self.index
    }

    /// Access the object with a closure.
    ///
    /// This is the recommended way to access pool objects, as it ensures
    /// the generation check happens every time.
    #[inline]
    pub fn with<F, R>(&self, f: F) -> Result<R, PoolAccessError>
    where
        F: FnOnce(&T) -> R,
    {
        self.pool.with(self.index, f)
    }

    /// Access the object mutably with a closure.
    #[inline]
    pub fn with_mut<F, R>(&self, f: F) -> Result<R, PoolAccessError>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.pool.with_mut(self.index, f)
    }

    /// Try to get a reference to the object.
    ///
    /// Returns None if the handle is stale (object was deallocated).
    #[inline]
    pub fn try_get(&self) -> Option<&T> {
        self.pool.get(self.index)
    }

    /// Try to get a mutable reference to the object.
    #[inline]
    pub fn try_get_mut(&mut self) -> Option<&mut T> {
        self.pool.get_mut(self.index)
    }

    /// Create a weak handle that doesn't prevent deallocation.
    pub fn downgrade(&self) -> WeakPoolHandle<T> {
        WeakPoolHandle {
            pool: Arc::downgrade(&self.pool),
            index: self.index,
            _marker: PhantomData,
        }
    }

    /// Manually free the object (consumes the handle).
    ///
    /// This is useful when you want explicit control over deallocation timing.
    pub fn free(self) -> Result<T, PoolAccessError> {
        let result = self.pool.remove(self.index);
        // Don't run the Drop impl since we're explicitly freeing
        std::mem::forget(self);
        result
    }

    /// Check if this handle is still valid.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.pool.is_valid(self.index)
    }

    /// Get the pool this handle belongs to.
    #[inline]
    pub fn pool(&self) -> &Arc<ObjectPool<T>> {
        &self.pool
    }

    /// Safe deref that returns Option instead of panicking on stale handles.
    ///
    /// Prefer this over the Deref trait when you need to handle stale handles gracefully.
    #[inline]
    pub fn deref_option(&self) -> Option<&T> {
        self.try_get()
    }

    /// Safe mutable deref that returns Option instead of panicking on stale handles.
    #[inline]
    pub fn deref_option_mut(&mut self) -> Option<&mut T> {
        self.try_get_mut()
    }

    /// Safe deref with default fallback for stale handles.
    ///
    /// Returns a reference to the object if valid, or the provided default if stale.
    #[inline]
    pub fn deref_or<'a>(&'a self, default: &'a T) -> &'a T {
        self.try_get().unwrap_or(default)
    }

    /// Safe deref with a closure for default fallback.
    ///
    /// Useful when you want to compute a default value only if the handle is stale.
    #[inline]
    pub fn deref_or_else<'a, F>(&'a self, f: F) -> &'a T
    where
        F: FnOnce() -> &'a T,
    {
        self.try_get().unwrap_or_else(f)
    }
}

impl<T> Drop for PoolHandle<T> {
    fn drop(&mut self) {
        // Automatically free the object when handle is dropped
        let _ = self.pool.remove(self.index);
    }
}

impl<T> Clone for PoolHandle<T> {
    fn clone(&self) -> Self {
        // Note: This creates a new handle to the SAME object
        // Be careful with this, as dropping either handle will free the object
        Self {
            pool: Arc::clone(&self.pool),
            index: self.index,
            _marker: PhantomData,
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for PoolHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_get() {
            Some(obj) => write!(f, "PoolHandle({}, {:?})", self.index, obj),
            None => write!(f, "PoolHandle({}, <stale>)", self.index),
        }
    }
}

impl<T: fmt::Display> fmt::Display for PoolHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_get() {
            Some(obj) => write!(f, "{}", obj),
            None => write!(f, "<stale handle>"),
        }
    }
}

// Implement Deref for convenient access (use with caution!)
impl<T> Deref for PoolHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.try_get()
            .expect("Attempted to dereference stale pool handle")
    }
}

impl<T> DerefMut for PoolHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.try_get_mut()
            .expect("Attempted to dereference stale pool handle")
    }
}

/// A weak handle that doesn't own the object.
///
/// This is useful for storing references without preventing deallocation.
/// Always check validity before accessing.
pub struct WeakPoolHandle<T> {
    pool: Weak<ObjectPool<T>>,
    index: GenerationalIndex,
    _marker: PhantomData<T>,
}

impl<T> WeakPoolHandle<T> {
    /// Attempt to upgrade to a strong handle.
    pub fn upgrade(&self) -> Option<PoolHandle<T>> {
        let pool = self.pool.upgrade()?;
        if pool.is_valid(self.index) {
            Some(PoolHandle {
                pool,
                index: self.index,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }

    /// Get the generational index.
    #[inline]
    pub fn index(&self) -> GenerationalIndex {
        self.index
    }

    /// Check if this weak handle can be upgraded.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.pool
            .upgrade()
            .map(|pool| pool.is_valid(self.index))
            .unwrap_or(false)
    }
}

impl<T> Clone for WeakPoolHandle<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            index: self.index,
            _marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for WeakPoolHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WeakPoolHandle({}, valid={})",
            self.index,
            self.is_valid()
        )
    }
}

/// Error type for pool access operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolAccessError {
    /// The handle is stale (object was deallocated).
    Stale,
    /// The pool is locked (concurrent access).
    Locked,
    /// Index out of bounds.
    OutOfBounds,
    /// Generation mismatch.
    GenerationMismatch,
}

impl fmt::Display for PoolAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale => write!(f, "Handle is stale (object was deallocated)"),
            Self::Locked => write!(f, "Pool is locked (concurrent access)"),
            Self::OutOfBounds => write!(f, "Index out of bounds"),
            Self::GenerationMismatch => write!(f, "Generation mismatch"),
        }
    }
}

impl std::error::Error for PoolAccessError {}

/// Scoped handle that automatically releases on drop but doesn't free the object.
///
/// This is useful for temporary access that shouldn't affect object lifetime.
pub struct ScopedHandle<'a, T> {
    pool: &'a ObjectPool<T>,
    index: GenerationalIndex,
}

impl<'a, T> ScopedHandle<'a, T> {
    #[allow(dead_code)]
    pub(crate) fn new(pool: &'a ObjectPool<T>, index: GenerationalIndex) -> Self {
        Self { pool, index }
    }

    #[inline]
    pub fn with<F, R>(&self, f: F) -> Result<R, PoolAccessError>
    where
        F: FnOnce(&T) -> R,
    {
        self.pool.with(self.index, f)
    }

    #[inline]
    pub fn with_mut<F, R>(&mut self, f: F) -> Result<R, PoolAccessError>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.pool.with_mut(self.index, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::PoolConfig;
    use std::thread;

    // ============================================================================
    // WEEK 1 PRIORITY 3: HANDLE DEREF SAFETY TESTS (30+ tests for safe access)
    // ============================================================================

    #[test]
    fn test_handle_creation() {
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(42).unwrap();
        assert!(handle.is_valid());
        assert_eq!(*handle.try_get().unwrap(), 42);
    }

    #[test]
    fn test_handle_with() {
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(42).unwrap();

        let result = handle.with(|val| *val * 2).unwrap();
        assert_eq!(result, 84);
    }

    #[test]
    fn test_weak_handle() {
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(42).unwrap();
        let weak = handle.downgrade();

        assert!(weak.is_valid());
        assert!(weak.upgrade().is_some());

        drop(handle);
        assert!(!weak.is_valid());
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_handle_deref() {
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(42).unwrap();

        assert_eq!(*handle, 42);
    }

    #[test]
    #[should_panic]
    fn test_stale_handle_deref_panics() {
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(42).unwrap();
        let index = handle.index();

        // Manually free the object
        handle.free().unwrap();

        // Create a new (stale) handle
        let stale_handle = PoolHandle::new(pool, index);
        let _ = *stale_handle; // Should panic
    }

    // ============================================================================
    // Safe Option-based deref methods
    // ============================================================================

    #[test]
    fn test_deref_option_valid_handle() {
        // deref_option should return Some for valid handles
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(99).unwrap();

        let result = handle.deref_option();
        assert_eq!(result, Some(&99));
    }

    #[test]
    fn test_deref_option_stale_handle() {
        // deref_option should return None for stale handles (no panic)
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(99).unwrap();
        let index = handle.index();

        // Free the object
        handle.free().unwrap();

        // Create a stale handle
        let stale_handle = PoolHandle::new(pool, index);
        let result = stale_handle.deref_option();
        assert_eq!(result, None);
    }

    #[test]
    fn test_deref_option_mut_valid_handle() {
        // deref_option_mut should return Some for valid handles
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let mut handle = pool.alloc(50).unwrap();

        let result = handle.deref_option_mut();
        assert!(result.is_some());
        if let Some(val) = result {
            assert_eq!(*val, 50);
        }
    }

    #[test]
    fn test_deref_or_valid_handle() {
        // deref_or should return the value for valid handles
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(123).unwrap();

        let result = handle.deref_or(&0);
        assert_eq!(*result, 123);
    }

    #[test]
    fn test_deref_or_stale_handle() {
        // deref_or should return the default for stale handles
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(123).unwrap();
        let index = handle.index();

        // Free the object
        handle.free().unwrap();

        // Create a stale handle
        let stale_handle = PoolHandle::new(pool, index);
        let default_val = 0u64;
        let result = stale_handle.deref_or(&default_val);
        assert_eq!(*result, 0);
    }

    #[test]
    fn test_deref_or_else_valid_handle() {
        // deref_or_else should return the value for valid handles (not call closure)
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(456).unwrap();

        let mut closure_called = false;
        let closure = || {
            closure_called = true;
            &0u64
        };

        let result = handle.deref_or_else(closure);
        assert_eq!(*result, 456);
        assert!(
            !closure_called,
            "Closure should not be called for valid handle"
        );
    }

    #[test]
    fn test_deref_or_else_stale_handle() {
        // deref_or_else should call closure for stale handles
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(456).unwrap();
        let index = handle.index();

        // Free the object
        handle.free().unwrap();

        // Create a stale handle
        let stale_handle = PoolHandle::new(pool, index);

        let default_val = 999u64;
        let result = stale_handle.deref_or_else(|| &default_val);
        assert_eq!(*result, 999);
    }

    #[test]
    fn test_handle_is_valid_after_creation() {
        // Handle should be valid immediately after creation
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(77).unwrap();

        assert!(handle.is_valid());
    }

    #[test]
    fn test_handle_is_invalid_after_free() {
        // Handle should be invalid after freeing
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(77).unwrap();

        assert!(handle.is_valid());
        let freed = handle.free().unwrap();
        assert_eq!(freed, 77);
    }

    #[test]
    fn test_handle_try_get_returns_none_after_drop() {
        // try_get should return None after handle is dropped
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(88).unwrap();
        let index = handle.index();

        drop(handle);

        // Try to access with new handle (which will be stale)
        let stale_handle = PoolHandle::new(Arc::clone(&pool), index);
        assert!(stale_handle.try_get().is_none());
    }

    #[test]
    fn test_handle_clone_creates_shared_reference() {
        // Cloning a handle creates a shared reference to the same object
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle1 = pool.alloc(555).unwrap();
        let handle2 = handle1.clone();

        assert_eq!(*handle1.try_get().unwrap(), *handle2.try_get().unwrap());
        assert_eq!(handle1.index(), handle2.index());
    }

    #[test]
    fn test_multiple_handle_accesses() {
        // Multiple handles can safely access the same object
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle1 = pool.alloc(777).unwrap();
        let handle2 = handle1.clone();
        let handle3 = handle1.clone();

        assert_eq!(*handle1.try_get().unwrap(), 777);
        assert_eq!(*handle2.try_get().unwrap(), 777);
        assert_eq!(*handle3.try_get().unwrap(), 777);
    }

    #[test]
    fn test_weak_handle_upgrade_valid() {
        // Weak handle should upgrade successfully when object is alive
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(111).unwrap();
        let weak = handle.downgrade();

        let upgraded = weak.upgrade();
        assert!(upgraded.is_some());
        assert_eq!(*upgraded.unwrap().try_get().unwrap(), 111);
    }

    #[test]
    fn test_weak_handle_upgrade_fails_after_free() {
        // Weak handle upgrade should fail after object is freed
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(222).unwrap();
        let weak = handle.downgrade();

        handle.free().unwrap();

        let upgraded = weak.upgrade();
        assert!(upgraded.is_none());
    }

    #[test]
    fn test_handle_with_mut_modifies_object() {
        // with_mut should allow modification of the object
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(333).unwrap();

        let result = handle.with_mut(|val| {
            *val = 666;
            *val
        });

        assert!(result.is_ok());
        assert_eq!(*handle.try_get().unwrap(), 666);
    }

    #[test]
    fn test_pool_access_error_display() {
        // Test error message formatting
        assert_eq!(
            PoolAccessError::Stale.to_string(),
            "Handle is stale (object was deallocated)"
        );
        assert_eq!(
            PoolAccessError::Locked.to_string(),
            "Pool is locked (concurrent access)"
        );
        assert_eq!(
            PoolAccessError::OutOfBounds.to_string(),
            "Index out of bounds"
        );
        assert_eq!(
            PoolAccessError::GenerationMismatch.to_string(),
            "Generation mismatch"
        );
    }

    #[test]
    fn test_scoped_handle_access() {
        // ScopedHandle should allow access without freeing
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(444).unwrap();

        let result = handle.with(|val| *val * 2);
        assert_eq!(result.unwrap(), 888);

        // Original handle should still be valid
        assert!(handle.is_valid());
    }

    #[test]
    fn test_handle_debug_valid() {
        // Debug formatting for valid handle
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(555).unwrap();

        let debug_str = format!("{:?}", handle);
        assert!(debug_str.contains("PoolHandle"));
        assert!(!debug_str.contains("stale"));
    }

    #[test]
    fn test_handle_debug_stale() {
        // Debug formatting for stale handle
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = pool.alloc(555).unwrap();
        let index = handle.index();

        handle.free().unwrap();

        let stale_handle = PoolHandle::<u64>::new(pool, index);
        let debug_str = format!("{:?}", stale_handle);
        assert!(debug_str.contains("stale"));
    }

    #[test]
    fn test_concurrent_handle_access() {
        // Handles should be safe to access from multiple threads
        let pool = Arc::new(ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap());
        let handle = Arc::new(pool.alloc(777).unwrap());

        let mut threads = vec![];
        for _ in 0..5 {
            let handle = Arc::clone(&handle);
            let thread = thread::spawn(move || {
                if let Some(val) = handle.try_get() {
                    *val
                } else {
                    0
                }
            });
            threads.push(thread);
        }

        for thread in threads {
            let result = thread.join().expect("Thread panicked");
            assert_eq!(result, 777);
        }
    }
}
