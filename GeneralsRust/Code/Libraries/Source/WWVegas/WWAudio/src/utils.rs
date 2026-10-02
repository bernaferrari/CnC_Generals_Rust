//! Utility helpers mirroring WWAudio Utils.h/.cpp.

use std::cell::Cell;

thread_local! {
    /// Recursion depth of the Miles-style lock held on this thread.
    static MSS_LOCK_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// RAII lock matching MMSLockClass behavior.
///
/// The C++ original guards Miles Sound System calls with a reentrant
/// CRITICAL_SECTION that is only ever taken while loading sound data, so the
/// Rust port models it as a per-thread recursion counter. Unlike a global
/// lock this is safely reentrant (C++ CRITICAL_SECTION semantics), which
/// matters because `SoundBufferClass::load_from_reader` acquires the lock and
/// then calls `determine_stats`, which takes it again.
pub struct MMSLockClass {
    _guard: MMSLockGuard,
}

/// Marker proving the depth counter was incremented.
struct MMSLockGuard;

impl MMSLockClass {
    pub fn new() -> Self {
        MSS_LOCK_DEPTH.with(|depth| depth.set(depth.get() + 1));
        Self {
            _guard: MMSLockGuard,
        }
    }
}

impl Default for MMSLockClass {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MMSLockClass {
    fn drop(&mut self) {
        MSS_LOCK_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Extracts filename from a path (Windows-style, as in C++).
pub fn get_filename_from_path(path: &str) -> &str {
    if let Some(idx) = path.rfind('\\') {
        &path[idx + 1..]
    } else {
        path
    }
}
