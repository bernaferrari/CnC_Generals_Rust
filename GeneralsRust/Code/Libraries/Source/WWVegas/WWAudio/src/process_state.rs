//! Owned WWAudio session state. Host constructs [`WwAudioRuntime`] and passes `&`.
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64};

pub struct WwAudioRuntime {
    pub assert_handler: Mutex<Option<Box<dyn crate::assert::AssertHandler + Send + Sync>>>,
    pub main_window_handle: Mutex<Option<isize>>,
    pub mss_lock_depth: Mutex<u32>,
    /// Windows failsafe timer state (formerly process FAILSAFE).
    pub failsafe_time: AtomicU64,
    pub failsafe_calls: AtomicU32,
}

impl Default for WwAudioRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WwAudioRuntime {
    pub fn new() -> Self {
        Self {
            assert_handler: Mutex::new(None),
            main_window_handle: Mutex::new(None),
            mss_lock_depth: Mutex::new(0),
            failsafe_time: AtomicU64::new(0x100000000),
            failsafe_calls: AtomicU32::new(0),
        }
    }
}

#[cfg(test)]
mod independence_tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn two_wwaudio_runtimes_on_two_threads_are_independent() {
        let a = Arc::new(WwAudioRuntime::new());
        let b = Arc::new(WwAudioRuntime::new());
        let barrier = Arc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = Arc::clone(&barrier);
        let bar2 = Arc::clone(&barrier);
        let t1 = thread::spawn(move || {
            *a2.mss_lock_depth.lock().unwrap() = 3;
            bar1.wait();
            assert_eq!(*a2.mss_lock_depth.lock().unwrap(), 3);
        });
        let t2 = thread::spawn(move || {
            *b2.mss_lock_depth.lock().unwrap() = 7;
            bar2.wait();
            assert_eq!(*b2.mss_lock_depth.lock().unwrap(), 7);
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(*a.mss_lock_depth.lock().unwrap(), 3);
        assert_eq!(*b.mss_lock_depth.lock().unwrap(), 7);
    }
}
