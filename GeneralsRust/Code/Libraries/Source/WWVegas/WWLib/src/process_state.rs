//! Packed WWLib process façade (argv / mouse / iff / registry lock).
//! Prefer moving fields onto an owned [`WwLibRuntime`] passed by `&`/`Arc`.
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LazyLock, Mutex};

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

impl WwLibRuntime {
    pub fn new() -> Self {
        Self {
            registry_locked: AtomicBool::new(false),
            argv: Mutex::new(ArgvStorage::new()),
            mouse: Mutex::new(None),
            iff_files: Mutex::new(Vec::new()),
            iff_buffer: Mutex::new(None),
        }
    }
}

pub static WWLIB: LazyLock<WwLibRuntime> = LazyLock::new(WwLibRuntime::new);

/// OnceLock façade used where prior code expected get_or_init semantics for mouse.
pub fn mouse_slot() -> &'static Mutex<Option<Arc<dyn Mouse>>> {
    &WWLIB.mouse
}

#[cfg(test)]
mod independence_tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn two_owned_wwlib_runtimes_do_not_alias() {
        let a = Arc::new(WwLibRuntime::new());
        let b = Arc::new(WwLibRuntime::new());
        assert!(!Arc::ptr_eq(&a, &b));
        let barrier = Arc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = Arc::clone(&barrier);
        let bar2 = Arc::clone(&barrier);
        let t1 = thread::spawn(move || {
            a2.registry_locked.store(true, Ordering::SeqCst);
            bar1.wait();
            assert!(a2.registry_locked.load(Ordering::SeqCst));
        });
        let t2 = thread::spawn(move || {
            b2.registry_locked.store(false, Ordering::SeqCst);
            bar2.wait();
            assert!(!b2.registry_locked.load(Ordering::SeqCst));
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert!(a.registry_locked.load(Ordering::SeqCst));
        assert!(!b.registry_locked.load(Ordering::SeqCst));
    }
}
