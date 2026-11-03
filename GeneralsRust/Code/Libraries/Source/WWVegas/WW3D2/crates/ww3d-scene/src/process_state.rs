//! Packed ww3d-scene process façade.
use std::sync::atomic::AtomicU64;
use std::sync::{LazyLock, Mutex};

pub struct SceneRuntime {
    pub lighting_lod_cutoff: Mutex<f32>,
    pub task_id_counter: AtomicU64,
}

pub static SCENE: LazyLock<SceneRuntime> = LazyLock::new(|| SceneRuntime {
    lighting_lod_cutoff: Mutex::new(0.5),
    task_id_counter: AtomicU64::new(0),
});
