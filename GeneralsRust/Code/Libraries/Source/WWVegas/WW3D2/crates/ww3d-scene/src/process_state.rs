//! Owned ww3d-scene session state. Host constructs [`SceneRuntime`] and passes `&` / `Arc`.
use std::sync::{Arc, Mutex};

pub struct SceneRuntime {
    pub lighting_lod_cutoff: Mutex<f32>,
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
        }
    }
}

#[cfg(test)]
mod independence_tests {
    use super::*;
        use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn two_scene_runtimes_on_two_threads_are_independent() {
        let a = Arc::new(SceneRuntime::new());
        let b = Arc::new(SceneRuntime::new());
        let barrier = Arc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = Arc::clone(&barrier);
        let bar2 = Arc::clone(&barrier);
        let t1 = thread::spawn(move || {
            *a2.lighting_lod_cutoff.lock().unwrap() = 0.11;
            bar1.wait();
            assert!((*a2.lighting_lod_cutoff.lock().unwrap() - 0.11).abs() < 1e-6);
        });
        let t2 = thread::spawn(move || {
            *b2.lighting_lod_cutoff.lock().unwrap() = 0.22;
            bar2.wait();
            assert!((*b2.lighting_lod_cutoff.lock().unwrap() - 0.22).abs() < 1e-6);
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert!((*a.lighting_lod_cutoff.lock().unwrap() - 0.11).abs() < 1e-6);
        assert!((*b.lighting_lod_cutoff.lock().unwrap() - 0.22).abs() < 1e-6);
    }
}
