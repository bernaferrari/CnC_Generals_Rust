//! Utility helpers mirroring WWAudio Utils.h/.cpp.

use std::sync::{Mutex, OnceLock};

static MSS_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn mss_mutex() -> &'static Mutex<()> {
    MSS_LOCK.get_or_init(|| Mutex::new(()))
}

/// RAII lock matching MMSLockClass behavior.
pub struct MMSLockClass {
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl MMSLockClass {
    pub fn new() -> Self {
        let guard = mss_mutex().lock().expect("MSS lock poisoned");
        Self { _guard: guard }
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
