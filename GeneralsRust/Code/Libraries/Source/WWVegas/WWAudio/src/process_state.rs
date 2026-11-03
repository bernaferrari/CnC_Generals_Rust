//! Packed WWAudio process façade.
use std::sync::atomic::AtomicBool;
use std::sync::{LazyLock, Mutex};

pub struct WwAudioRuntime {
    pub assert_handler: Mutex<Option<Box<dyn crate::assert::AssertHandler + Send + Sync>>>,
    pub main_window_handle: Mutex<Option<isize>>,
    pub mss_lock_depth: Mutex<u32>,
}

pub static WWAUDIO: LazyLock<WwAudioRuntime> = LazyLock::new(|| WwAudioRuntime {
    assert_handler: Mutex::new(None),
    main_window_handle: Mutex::new(None),
    mss_lock_depth: Mutex::new(0),
});
