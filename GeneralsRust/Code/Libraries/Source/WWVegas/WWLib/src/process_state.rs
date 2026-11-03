//! Packed WWLib process façade (argv / mouse / iff / registry lock).
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use crate::wwfile::WWFile;
use crate::xmouse::Mouse;

pub struct ArgvStorage {
    pub argc: usize,
    pub argv: Vec<String>,
}

impl ArgvStorage {
    pub fn new() -> Self {
        Self {
            argc: 0,
            argv: Vec::with_capacity(256),
        }
    }
}

pub struct WwLibRuntime {
    pub registry_locked: AtomicBool,
    pub argv: Mutex<ArgvStorage>,
    pub mouse: Mutex<Option<Arc<dyn Mouse>>>,
    pub iff_files: Mutex<Vec<Option<WWFile>>>,
    pub iff_buffer: Mutex<Option<Vec<u8>>>,
}

pub static WWLIB: LazyLock<WwLibRuntime> = LazyLock::new(|| WwLibRuntime {
    registry_locked: AtomicBool::new(false),
    argv: Mutex::new(ArgvStorage::new()),
    mouse: Mutex::new(None),
    iff_files: Mutex::new(Vec::new()),
    iff_buffer: Mutex::new(None),
});

/// OnceLock façade used where prior code expected get_or_init semantics for mouse.
pub fn mouse_slot() -> &'static Mutex<Option<Arc<dyn Mouse>>> {
    &WWLIB.mouse
}
