//! Smart Handles for Pool-Allocated Objects
//!
//! Provides RAII handles that automatically manage object lifetimes
//! and prevent dangling references through generational checking.

use super::generation::GenerationalIndex;
use super::pool::ObjectPool;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, Weak};

/// A strong handle to a pool-allocated object.
///
/// This handle keeps the pool alive and automatically deallocates its object
/// when dropped. Clones identify the same object: dropping or freeing any one
/// invalidates the others. Generation checks detect stale indices until the
/// counter wraps.
pub struct PoolHandle<T> {
    /// The pool this object belongs to.
    pool: Arc<ObjectPool<T>>,
    /// Generational index of the object.
    index: GenerationalIndex,
    /// Whether dropping this handle should attempt to remove the object.
    remove_on_drop: bool,
    /// Marker for proper variance.
    _marker: PhantomData<T>,
}

impl<T> PoolHandle<T> {
    /// Create a new handle (called by ObjectPool).
    pub(crate) fn new(pool: Arc<ObjectPool<T>>, index: GenerationalIndex) -> Self {
        Self {
            pool,
            index,
            remove_on_drop: true,
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
    /// Checks the generation and occupancy while holding the pool lock.
    /// Do not access the same pool or drop one of its handles inside `f`:
    /// its mutex is not reentrant.
    #[inline]
    pub fn with<F, R>(&self, f: F) -> Result<R, PoolAccessError>
    where
        F: FnOnce(&T) -> R,
    {
        self.pool.with(self.index, f)
    }

    /// Access the object mutably with a closure.
    ///
    /// As with [`Self::with`], the pool lock is held for the whole closure.
    #[inline]
    pub fn with_mut<F, R>(&self, f: F) -> Result<R, PoolAccessError>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.pool.with_mut(self.index, f)
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
    /// A stale handle returns the same error as a pool removal; in either case
    /// consuming it releases its reference to the pool.
    pub fn free(mut self) -> Result<T, PoolAccessError> {
        self.remove_on_drop = false;
        self.pool.remove(self.index)
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
}

impl<T> Drop for PoolHandle<T> {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = self.pool.remove(self.index);
        }
    }
}

impl<T> Clone for PoolHandle<T> {
    fn clone(&self) -> Self {
        // Note: This creates a new handle to the SAME object
        // Be careful with this, as dropping either handle will free the object
        Self::new(Arc::clone(&self.pool), self.index)
    }
}

impl<T: fmt::Debug> fmt::Debug for PoolHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.with(|obj| write!(f, "PoolHandle({}, {:?})", self.index, obj)) {
            Ok(result) => result,
            Err(_) => write!(f, "PoolHandle({}, <stale>)", self.index),
        }
    }
}

impl<T: fmt::Display> fmt::Display for PoolHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.with(|obj| write!(f, "{}", obj)) {
            Ok(result) => result,
            Err(_) => write!(f, "<stale handle>"),
        }
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
    ///
    /// Validity is checked at the time of the call. Another handle can still
    /// remove the object before the returned handle is used.
    pub fn upgrade(&self) -> Option<PoolHandle<T>> {
        let pool = self.pool.upgrade()?;
        if pool.is_valid(self.index) {
            Some(PoolHandle::new(pool, self.index))
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

    #[test]
    fn explicit_free_releases_pool_and_returns_the_undropped_value() {
        use std::cell::Cell;

        struct DropProbe<'a>(&'a Cell<usize>);
        impl Drop for DropProbe<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let drops = Cell::new(0);
        let pool = ObjectPool::new(PoolConfig::new("free lifetime")).unwrap();
        let handle = pool.alloc(DropProbe(&drops)).unwrap();
        let weak_pool = Arc::downgrade(&pool);
        drop(pool);

        let value = handle.free().unwrap();
        assert!(weak_pool.upgrade().is_none());
        assert_eq!(drops.get(), 0);
        drop(value);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn dropping_a_clone_invalidates_the_others_and_releases_the_pool() {
        let pool = ObjectPool::new(PoolConfig::new("clone drop")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let clone = handle.clone();
        let weak = handle.downgrade();
        let weak_pool = Arc::downgrade(&pool);
        drop(pool);

        drop(clone);
        assert!(!handle.is_valid());
        assert_eq!(handle.with(|value| *value), Err(PoolAccessError::GenerationMismatch));
        assert!(!weak.is_valid());
        assert!(weak.upgrade().is_none());
        assert!(weak_pool.upgrade().is_some());
        drop(handle);
        assert!(weak_pool.upgrade().is_none());
    }

    #[test]
    fn failed_explicit_free_also_releases_pool() {
        let pool = ObjectPool::new(PoolConfig::new("stale free lifetime")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let stale = handle.clone();
        let weak_pool = Arc::downgrade(&pool);
        drop(handle);
        drop(pool);

        assert_eq!(stale.free(), Err(PoolAccessError::GenerationMismatch));
        assert!(weak_pool.upgrade().is_none());
    }

    #[test]
    fn concurrent_frees_leave_reused_slot_intact_for_stale_handles() {
        use std::sync::Barrier;

        let pool = ObjectPool::new(PoolConfig::new("concurrent free")).unwrap();
        let first = pool.alloc(42).unwrap();
        let second = first.clone();
        let stale = first.clone();
        let stale_for_free = first.clone();
        let weak = first.downgrade();
        let old_index = first.index();
        let barrier = Barrier::new(3);

        let (first_result, second_result) = std::thread::scope(|scope| {
            let first_barrier = &barrier;
            let first_thread = scope.spawn(move || {
                first_barrier.wait();
                first.free()
            });
            let second_barrier = &barrier;
            let second_thread = scope.spawn(move || {
                second_barrier.wait();
                second.free()
            });
            barrier.wait();
            (first_thread.join().unwrap(), second_thread.join().unwrap())
        });

        assert!(
            (first_result == Ok(42) && second_result == Err(PoolAccessError::GenerationMismatch))
                || (second_result == Ok(42)
                    && first_result == Err(PoolAccessError::GenerationMismatch))
        );
        assert!(!stale.is_valid());
        assert!(!weak.is_valid());
        assert!(weak.upgrade().is_none());

        let replacement = pool.alloc(99).unwrap();
        assert_eq!(replacement.index().index(), old_index.index());
        assert_ne!(replacement.index().generation(), old_index.generation());
        drop(stale);
        assert_eq!(stale_for_free.free(), Err(PoolAccessError::GenerationMismatch));
        assert_eq!(replacement.with(|value| *value), Ok(99));
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn weak_handle_cannot_upgrade_a_vacant_matching_generation() {
        let pool = ObjectPool::new(PoolConfig::new("vacant weak")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let index = handle.index();
        assert_eq!(handle.free(), Ok(42));

        let vacant = PoolHandle::new(
            Arc::clone(&pool),
            GenerationalIndex::new(index.index(), index.generation().next()),
        );
        let weak = vacant.downgrade();
        assert!(!vacant.is_valid());
        assert!(!weak.is_valid());
        assert!(weak.upgrade().is_none());
        drop(vacant);
    }

    // Checked handle access tests.

    #[test]
    fn test_handle_creation() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();
        assert!(handle.is_valid());
        assert_eq!(handle.with(|value| *value), Ok(42));
    }

    #[test]
    fn test_handle_with() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();

        let result = handle.with(|val| *val * 2).unwrap();
        assert_eq!(result, 84);
    }

    #[test]
    fn test_weak_handle() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let weak = handle.downgrade();

        assert!(weak.is_valid());
        assert!(weak.upgrade().is_some());

        drop(handle);
        assert!(!weak.is_valid());
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_handle_access() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();

        assert_eq!(handle.with(|value| *value), Ok(42));
    }

    #[test]
    fn test_stale_handle_access_returns_error() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let index = handle.index();

        // Manually free the object
        handle.free().unwrap();

        // Create a new (stale) handle
        let stale_handle = PoolHandle::new(pool, index);
        assert_eq!(stale_handle.with(|value| *value), Err(PoolAccessError::GenerationMismatch));
    }

    // Checked access and caller-provided fallback values.

    #[test]
    fn test_checked_access_valid_handle() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(99).unwrap();

        assert_eq!(handle.with(|value| *value), Ok(99));
    }

    #[test]
    fn test_checked_access_stale_handle() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(99).unwrap();
        let index = handle.index();

        // Free the object
        handle.free().unwrap();

        // Create a stale handle
        let stale_handle = PoolHandle::new(pool, index);
        assert_eq!(stale_handle.with(|value| *value), Err(PoolAccessError::GenerationMismatch));
    }

    #[test]
    fn test_checked_mutable_access_valid_handle() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(50).unwrap();

        let result = handle.with_mut(|value| {
            assert_eq!(*value, 50);
            *value = 51;
        });
        assert_eq!(result, Ok(()));
        assert_eq!(handle.with(|value| *value), Ok(51));
    }

    #[test]
    fn test_fallback_valid_handle() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(123).unwrap();

        let result = handle.with(|value| *value).unwrap_or(0);
        assert_eq!(result, 123);
    }

    #[test]
    fn test_fallback_stale_handle() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(123).unwrap();
        let index = handle.index();

        // Free the object
        handle.free().unwrap();

        // Create a stale handle
        let stale_handle = PoolHandle::new(pool, index);
        let default_val = 0u64;
        let result = stale_handle.with(|value| *value).unwrap_or(default_val);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_lazy_fallback_valid_handle() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(456).unwrap();

        let mut closure_called = false;
        let result = handle.with(|value| *value).unwrap_or_else(|_| {
            closure_called = true;
            0
        });
        assert_eq!(result, 456);
        assert!(
            !closure_called,
            "Closure should not be called for valid handle"
        );
    }

    #[test]
    fn test_lazy_fallback_stale_handle() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(456).unwrap();
        let index = handle.index();

        // Free the object
        handle.free().unwrap();

        // Create a stale handle
        let stale_handle = PoolHandle::new(pool, index);

        let default_val = 999u64;
        let mut closure_called = false;
        let result = stale_handle.with(|value| *value).unwrap_or_else(|_| {
            closure_called = true;
            default_val
        });
        assert_eq!(result, 999);
        assert!(closure_called);
    }

    #[test]
    fn test_handle_is_valid_after_creation() {
        // Handle should be valid immediately after creation
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(77).unwrap();

        assert!(handle.is_valid());
    }

    #[test]
    fn test_handle_is_invalid_after_free() {
        // Handle should be invalid after freeing
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(77).unwrap();

        assert!(handle.is_valid());
        let freed = handle.free().unwrap();
        assert_eq!(freed, 77);
    }

    #[test]
    fn test_handle_access_returns_error_after_drop() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(88).unwrap();
        let index = handle.index();

        drop(handle);

        // Try to access with new handle (which will be stale)
        let stale_handle = PoolHandle::new(Arc::clone(&pool), index);
        assert_eq!(stale_handle.with(|value| *value), Err(PoolAccessError::GenerationMismatch));
    }

    #[test]
    fn test_handle_clone_identifies_same_object() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle1 = pool.alloc(555).unwrap();
        let handle2 = handle1.clone();

        assert_eq!(handle1.with(|value| *value), Ok(555));
        assert_eq!(handle2.with(|value| *value), Ok(555));
        assert_eq!(handle2.with_mut(|value| *value += 1), Ok(()));
        assert_eq!(handle1.with(|value| *value), Ok(556));
        assert_eq!(handle1.index(), handle2.index());
    }

    #[test]
    fn test_multiple_handle_accesses() {
        // Multiple handles can safely access the same object
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle1 = pool.alloc(777).unwrap();
        let handle2 = handle1.clone();
        let handle3 = handle1.clone();

        assert_eq!(handle1.with(|value| *value), Ok(777));
        assert_eq!(handle2.with(|value| *value), Ok(777));
        assert_eq!(handle3.with(|value| *value), Ok(777));
    }

    #[test]
    fn test_weak_handle_upgrade_valid() {
        // Weak handle should upgrade successfully when object is alive
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(111).unwrap();
        let weak = handle.downgrade();

        let upgraded = weak.upgrade();
        assert!(upgraded.is_some());
        assert_eq!(upgraded.unwrap().with(|value| *value), Ok(111));
    }

    #[test]
    fn test_weak_handle_upgrade_fails_after_free() {
        // Weak handle upgrade should fail after object is freed
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(222).unwrap();
        let weak = handle.downgrade();

        handle.free().unwrap();

        let upgraded = weak.upgrade();
        assert!(upgraded.is_none());
    }

    #[test]
    fn test_handle_with_mut_modifies_object() {
        // with_mut should allow modification of the object
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(333).unwrap();

        let result = handle.with_mut(|val| {
            *val = 666;
            *val
        });

        assert!(result.is_ok());
        assert_eq!(handle.with(|value| *value), Ok(666));
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
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(444).unwrap();
        let mut scoped = ScopedHandle::new(&pool, handle.index());

        assert_eq!(scoped.with(|val| *val * 2), Ok(888));
        assert_eq!(scoped.with_mut(|val| *val += 1), Ok(()));
        assert_eq!(handle.with(|val| *val), Ok(445));

        // Original handle should still be valid
        assert!(handle.is_valid());
    }

    #[test]
    fn test_handle_debug_valid() {
        // Debug formatting for valid handle
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(555).unwrap();

        let debug_str = format!("{:?}", handle);
        assert!(debug_str.contains("PoolHandle"));
        assert!(!debug_str.contains("stale"));
        assert!(debug_str.contains("555"));
        assert_eq!(handle.to_string(), "555");
    }

    #[test]
    fn test_handle_debug_stale() {
        // Debug formatting for stale handle
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(555).unwrap();
        let index = handle.index();

        handle.free().unwrap();

        let stale_handle = PoolHandle::<u64>::new(pool, index);
        let debug_str = format!("{:?}", stale_handle);
        assert!(debug_str.contains("stale"));
        assert_eq!(stale_handle.to_string(), "<stale handle>");
    }

    #[test]
    fn test_repeated_handle_access() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(777).unwrap();

        for _ in 0..5 {
            let result = handle.with(|value| *value).unwrap();
            assert_eq!(result, 777);
        }
    }
}
