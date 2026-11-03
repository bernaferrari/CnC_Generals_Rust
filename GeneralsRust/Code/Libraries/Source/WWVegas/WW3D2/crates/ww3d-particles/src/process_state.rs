//! Owned particles session state (formerly process PARTICLES LazyLock).
use std::cell::RefCell;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub const LOD_BUCKETS: usize = 17;

pub struct ParticlesRuntime {
    pub default_remove_on_complete: AtomicBool,
    pub lod_max_screen_sizes: Mutex<[f32; LOD_BUCKETS]>,
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
            lod_max_screen_sizes: Mutex::new([f32::MAX; LOD_BUCKETS]),
        }
    }
}

thread_local! {
    static ACTIVE_STACK: RefCell<Vec<Arc<ParticlesRuntime>>> = const { RefCell::new(Vec::new()) };
    static FALLBACK: RefCell<Option<Arc<ParticlesRuntime>>> = const { RefCell::new(None) };
}

pub fn particles() -> Arc<ParticlesRuntime> {
    ACTIVE_STACK.with(|stack| {
        if let Some(top) = stack.borrow().last().cloned() {
            return top;
        }
        FALLBACK.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(Arc::new(ParticlesRuntime::new()));
            }
            slot.clone().expect("particles fallback")
        })
    })
}

pub fn install_particles(services: Arc<ParticlesRuntime>) {
    ACTIVE_STACK.with(|stack| stack.borrow_mut().push(services));
}

pub fn with_particles<R>(services: &Arc<ParticlesRuntime>, f: impl FnOnce() -> R) -> R {
    install_particles(Arc::clone(services));
    struct Pop(Arc<ParticlesRuntime>);
    impl Drop for Pop {
        fn drop(&mut self) {
            ACTIVE_STACK.with(|stack| {
                let mut stack = stack.borrow_mut();
                if stack.last().is_some_and(|top| Arc::ptr_eq(top, &self.0)) {
                    stack.pop();
                }
            });
        }
    }
    let _pop = Pop(Arc::clone(services));
    f()
}

#[cfg(test)]
mod independence_tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc as StdArc, Barrier};
    use std::thread;

    #[test]
    fn two_particles_runtimes_on_two_threads_are_independent() {
        let a = Arc::new(ParticlesRuntime::new());
        let b = Arc::new(ParticlesRuntime::new());
        let barrier = StdArc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = StdArc::clone(&barrier);
        let bar2 = StdArc::clone(&barrier);
        let t1 = thread::spawn(move || {
            with_particles(&a2, || {
                particles()
                    .default_remove_on_complete
                    .store(false, Ordering::SeqCst);
                bar1.wait();
                assert!(!particles().default_remove_on_complete.load(Ordering::SeqCst));
            });
        });
        let t2 = thread::spawn(move || {
            with_particles(&b2, || {
                particles()
                    .default_remove_on_complete
                    .store(true, Ordering::SeqCst);
                bar2.wait();
                assert!(particles().default_remove_on_complete.load(Ordering::SeqCst));
            });
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert!(!a.default_remove_on_complete.load(Ordering::SeqCst));
        assert!(b.default_remove_on_complete.load(Ordering::SeqCst));
    }
}
