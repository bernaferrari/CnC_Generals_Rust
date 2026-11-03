//! Utility helpers mirroring WWAudio Utils.h/.cpp.

use std::cell::Cell;

struct MssLockDepthBag {
    mss_lock_depth: std::sync::Mutex<u32>,
}
static MSS_LOCK_DEPTH_TLS_BAG: std::sync::LazyLock<MssLockDepthBag> = std::sync::LazyLock::new(|| MssLockDepthBag {
        mss_lock_depth: std::sync::Mutex::new(0),
});


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
        { let mut depth = MSS_LOCK_DEPTH_TLS_BAG.mss_lock_depth.lock().unwrap_or_else(|p| p.into_inner()); *depth = depth.clone( + 1) };
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
        { let mut depth = MSS_LOCK_DEPTH_TLS_BAG.mss_lock_depth.lock().unwrap_or_else(|p| p.into_inner()); *depth = depth.clone(.saturating_sub(1)) };
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
