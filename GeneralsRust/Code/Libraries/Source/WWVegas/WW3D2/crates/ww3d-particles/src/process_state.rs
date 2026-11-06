//! Owned particles session state. Host constructs [`ParticlesRuntime`] and passes `&` / `Arc`.
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub struct ParticlesRuntime {
    pub default_remove_on_complete: AtomicBool,
    pub lod_max_screen_sizes: Mutex<[f32; 17]>,
}

impl Default for ParticlesRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ParticlesRuntime {
    pub fn new() -> Self {
        Self {
            default_remove_on_complete: AtomicBool::new(true),
            lod_max_screen_sizes: Mutex::new([1.0e30; 17]),
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
    fn two_particles_runtimes_on_two_threads_are_independent() {
        let a = Arc::new(ParticlesRuntime::new());
        let b = Arc::new(ParticlesRuntime::new());
        let barrier = Arc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = Arc::clone(&barrier);
        let bar2 = Arc::clone(&barrier);
        let t1 = thread::spawn(move || {
            a2.default_remove_on_complete
                .store(false, Ordering::SeqCst);
            bar1.wait();
            assert!(!a2.default_remove_on_complete.load(Ordering::SeqCst));
        });
        let t2 = thread::spawn(move || {
            b2.default_remove_on_complete
                .store(true, Ordering::SeqCst);
            bar2.wait();
            assert!(b2.default_remove_on_complete.load(Ordering::SeqCst));
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert!(!a.default_remove_on_complete.load(Ordering::SeqCst));
        assert!(b.default_remove_on_complete.load(Ordering::SeqCst));
    }
}
