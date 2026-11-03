//! Owned renderer-3d session state (C++ WW3D2 file-scope / class-static data).
//!
//! Each display/renderer owns an [`Arc<Renderer3dRuntime>`] and passes `&` /
//! `Arc` into free-function bridges. There is no process-wide LazyLock bag and
//! no thread-local active stack: two instances never share config, texture, or
//! id counters.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32};
use std::sync::{Arc, Mutex};

use crate::config::RendererConfig;
use crate::rendering::texture_metrics::TextureDecisionRecord;
use crate::rendering::texture_quality::TextureQualitySettings;
use crate::rendering::texture_system::texfcach::TextureFileCache;
use crate::rendering::texture_system::texturethumbnail::ThumbnailManagerClass;


/// Session shader dirty/cache state (formerly SHADER_STATE_TLS_BAG).
#[derive(Debug)]
pub struct ShaderGlobalState {
    pub dirty: bool,
    pub current_shader: u32,
    pub applied_blend_bits: u32,
    pub applied_fog_bits: u32,
    pub applied_texture_stage_bits: u32,
    pub applied_depth_bits: u32,
    pub applied_cull_bits: u32,
    pub applied_sec_gradient_bits: u32,
    pub applied_npatch_bits: u32,
}

impl ShaderGlobalState {
    pub fn new() -> Self {
        Self {
            dirty: true,
            current_shader: 0,
            applied_blend_bits: 0,
            applied_fog_bits: 0,
            applied_texture_stage_bits: 0,
            applied_depth_bits: 0,
            applied_cull_bits: 0,
            applied_sec_gradient_bits: 0,
            applied_npatch_bits: 0,
        }
    }
}

impl Default for ShaderGlobalState {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-session renderer-3d mutable state (formerly process RENDERER_3D / BOX /
/// vertex-material id statics).
pub struct Renderer3dRuntime {
    pub enable_per_polygon_culling: AtomicBool,
    pub next_logical_decal_id: AtomicU32,
    pub mesh_debug_id_count: AtomicU32,
    pub next_vertex_material_id: AtomicU32,
    pub box_display_mask: AtomicI32,
    pub box_is_initted: AtomicBool,
    pub lighting_lod_cutoff: Mutex<f32>,
    pub config: Mutex<RendererConfig>,
    pub texture_quality: Mutex<TextureQualitySettings>,
    pub texture_decisions: Mutex<Vec<TextureDecisionRecord>>,
    pub thumbnail_manager: Mutex<ThumbnailManagerClass>,
    pub texture_cache: Mutex<Option<TextureFileCache>>,
    pub shader_state: Mutex<ShaderGlobalState>,
    pub static_sort_state: Mutex<crate::rendering::mesh_system::StaticSortState>,
    pub ring_mesh_array: Mutex<Option<crate::rendering::ring_render_obj::RingMeshArray>>,
    /// Session wgpu device/surface wrapper (formerly WGPU_WRAPPER_INSTANCE).
    pub wgpu_wrapper: Mutex<Option<crate::rendering::wgpu_renderer::wgpu_wrapper::WgpuWrapper>>,
}

impl Default for Renderer3dRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer3dRuntime {
    pub fn new() -> Self {
        Self {
            enable_per_polygon_culling: AtomicBool::new(true),
            next_logical_decal_id: AtomicU32::new(1),
            mesh_debug_id_count: AtomicU32::new(0),
            next_vertex_material_id: AtomicU32::new(1),
            box_display_mask: AtomicI32::new(0),
            box_is_initted: AtomicBool::new(false),
            lighting_lod_cutoff: Mutex::new(0.5),
            config: Mutex::new(RendererConfig::default()),
            texture_quality: Mutex::new(TextureQualitySettings::default()),
            texture_decisions: Mutex::new(Vec::new()),
            thumbnail_manager: Mutex::new(ThumbnailManagerClass::new()),
            texture_cache: Mutex::new(None),
            shader_state: Mutex::new(ShaderGlobalState::new()),
            static_sort_state: Mutex::new(crate::rendering::mesh_system::default_static_sort_state()),
            ring_mesh_array: Mutex::new(None),
            wgpu_wrapper: Mutex::new(None),
        }
    }
}


/// Free-function bridges take an explicit [`Renderer3dRuntime`] (or `&Arc`).
/// There is no process-wide or thread-local active stack.


#[cfg(test)]
mod independence_tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn two_renderer_3d_runtimes_on_two_threads_are_independent() {
        let a = Arc::new(Renderer3dRuntime::new());
        let b = Arc::new(Renderer3dRuntime::new());
        let barrier = Arc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = Arc::clone(&barrier);
        let bar2 = Arc::clone(&barrier);
        let t1 = thread::spawn(move || {
            a2.box_display_mask.store(1, Ordering::SeqCst);
            bar1.wait();
            assert_eq!(a2.box_display_mask.load(Ordering::SeqCst), 1);
        });
        let t2 = thread::spawn(move || {
            b2.box_display_mask.store(2, Ordering::SeqCst);
            bar2.wait();
            assert_eq!(b2.box_display_mask.load(Ordering::SeqCst), 2);
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(a.box_display_mask.load(Ordering::SeqCst), 1);
        assert_eq!(b.box_display_mask.load(Ordering::SeqCst), 2);
    }
}


impl std::fmt::Debug for Renderer3dRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer3dRuntime").finish_non_exhaustive()
    }
}



const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Renderer3dRuntime>();
};
