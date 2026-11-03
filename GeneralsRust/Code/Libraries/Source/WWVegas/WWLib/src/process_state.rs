//! Owned WWLib session state (argv / mouse / iff / registry lock).
//! Host constructs [`WwLibRuntime`] and passes `&` / `Arc` — no process LazyLock.
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

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
    pub registry_locked: Arc<AtomicBool>,
    pub argv: Arc<Mutex<ArgvStorage>>,
    pub mouse: Mutex<Option<Arc<dyn Mouse>>>,
    pub iff_files: Mutex<Vec<Option<WWFile>>>,
    pub iff_buffer: Mutex<Option<Vec<u8>>>,
}

impl Default for WwLibRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WwLibRuntime {
    pub fn new() -> Self {
        Self {
            registry_locked: Arc::new(AtomicBool::new(false)),
            argv: Arc::new(Mutex::new(ArgvStorage::new())),
            mouse: Mutex::new(None),
            iff_files: Mutex::new(Vec::new()),
            iff_buffer: Mutex::new(None),
        }
    }
}

#[cfg(test)]
mod independence_tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn two_wwlib_runtimes_on_two_threads_are_independent() {
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
