//! Packed particles process façade.
use std::sync::atomic::AtomicBool;
use std::sync::{LazyLock, Mutex};

pub const LOD_BUCKETS: usize = 17;
pub struct ParticlesRuntime {
    pub default_remove_on_complete: AtomicBool,
    pub lod_max_screen_sizes: Mutex<[f32; LOD_BUCKETS]>,
}

pub static PARTICLES: LazyLock<ParticlesRuntime> = LazyLock::new(|| ParticlesRuntime {
    default_remove_on_complete: AtomicBool::new(true),
    // Match prior bag init: filled with NO_MAX_SCREEN_SIZE from buffer.rs
    lod_max_screen_sizes: Mutex::new([f32::MAX; LOD_BUCKETS]),
});
