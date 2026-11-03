use crate::process_state::WwAudioRuntime;
use std::sync::Arc;

/// then calls `determine_stats`, which takes it again.
pub struct MMSLockClass {
    runtime: Arc<WwAudioRuntime>,
}

impl MMSLockClass {
    pub fn new(runtime: Arc<WwAudioRuntime>) -> Self {
        {
            let mut depth = runtime.mss_lock_depth.lock().unwrap_or_else(|p| p.into_inner());
            *depth += 1;
        }
        Self { runtime }
    }
}

impl Drop for MMSLockClass {
    fn drop(&mut self) {
        let mut depth = self
            .runtime
            .mss_lock_depth
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        *depth = depth.saturating_sub(1);
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
