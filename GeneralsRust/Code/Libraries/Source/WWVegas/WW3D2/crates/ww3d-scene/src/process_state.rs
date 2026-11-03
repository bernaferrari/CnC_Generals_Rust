//! Owned ww3d-scene session state (formerly process SCENE LazyLock).
use std::cell::RefCell;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

pub struct SceneRuntime {
    pub lighting_lod_cutoff: Mutex<f32>,
    pub task_id_counter: AtomicU64,
}

impl Default for SceneRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneRuntime {
    pub fn new() -> Self {
        Self {
            lighting_lod_cutoff: Mutex::new(0.5),
            task_id_counter: AtomicU64::new(0),
        }
    }
}

thread_local! {
    static ACTIVE_STACK: RefCell<Vec<Arc<SceneRuntime>>> = const { RefCell::new(Vec::new()) };
    static FALLBACK: RefCell<Option<Arc<SceneRuntime>>> = const { RefCell::new(None) };
}

pub fn scene() -> Arc<SceneRuntime> {
    ACTIVE_STACK.with(|stack| {
        if let Some(top) = stack.borrow().last().cloned() {
            return top;
        }
        FALLBACK.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(Arc::new(SceneRuntime::new()));
            }
            slot.clone().expect("scene fallback")
        })
    })
}

pub fn install_scene(services: Arc<SceneRuntime>) {
    ACTIVE_STACK.with(|stack| stack.borrow_mut().push(services));
}

pub fn with_scene<R>(services: &Arc<SceneRuntime>, f: impl FnOnce() -> R) -> R {
    install_scene(Arc::clone(services));
    struct Pop(Arc<SceneRuntime>);
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
    fn two_scene_runtimes_on_two_threads_are_independent() {
        let a = Arc::new(SceneRuntime::new());
        let b = Arc::new(SceneRuntime::new());
        let barrier = StdArc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = StdArc::clone(&barrier);
        let bar2 = StdArc::clone(&barrier);
        let t1 = thread::spawn(move || {
            with_scene(&a2, || {
                scene().task_id_counter.store(11, Ordering::SeqCst);
                bar1.wait();
                assert_eq!(scene().task_id_counter.load(Ordering::SeqCst), 11);
            });
        });
        let t2 = thread::spawn(move || {
            with_scene(&b2, || {
                scene().task_id_counter.store(22, Ordering::SeqCst);
                bar2.wait();
                assert_eq!(scene().task_id_counter.load(Ordering::SeqCst), 22);
            });
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(a.task_id_counter.load(Ordering::SeqCst), 11);
        assert_eq!(b.task_id_counter.load(Ordering::SeqCst), 22);
    }
}
