//! Owned renderer-3d session state (C++ WW3D2 file-scope / class-static data).
//!
//! Each display/renderer owns an [`Arc<Renderer3dRuntime>`]. Free-function bridges
//! publish the active owner via [`with_renderer_3d`] / [`install_renderer_3d`]
//! (thread-local), matching [`common::common_services`]. There is no process-wide
//! LazyLock bag: two instances on two threads never share config, texture, or id
//! counters.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32};
use std::any::Any;
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
    /// Type-erased [`crate::rendering::mesh_system`] StaticSortState (installed on first use).
    pub static_sort_state: Mutex<Option<Box<dyn Any + Send>>>,
    /// Type-erased ring mesh array (installed on first use).
    pub ring_mesh_array: Mutex<Option<Box<dyn Any + Send>>>,
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
            static_sort_state: Mutex::new(None),
            ring_mesh_array: Mutex::new(None),
        }
    }
}

thread_local! {
    static ACTIVE_STACK: RefCell<Vec<Arc<Renderer3dRuntime>>> = const { RefCell::new(Vec::new()) };
    /// Private per-thread owner when nothing is published (unit tests / free
    /// functions outside a display). Not process-shared: each thread gets its own.
    static FALLBACK: RefCell<Option<Arc<Renderer3dRuntime>>> = const { RefCell::new(None) };
}

/// Active renderer-3d services for free-function bridges.
///
/// Prefers the published stack; otherwise a private per-thread fallback Arc.
pub fn renderer_3d() -> Arc<Renderer3dRuntime> {
    ACTIVE_STACK.with(|stack| {
        if let Some(top) = stack.borrow().last().cloned() {
            return top;
        }
        FALLBACK.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(Arc::new(Renderer3dRuntime::new()));
            }
            slot.clone().expect("fallback just installed")
        })
    })
}

pub fn install_renderer_3d(services: Arc<Renderer3dRuntime>) -> Option<Arc<Renderer3dRuntime>> {
    ACTIVE_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        let prior = stack.last().cloned();
        stack.push(services);
        prior
    })
}

pub fn uninstall_renderer_3d_if_current(services: &Arc<Renderer3dRuntime>) -> bool {
    ACTIVE_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        if stack
            .last()
            .is_some_and(|top| Arc::ptr_eq(top, services))
        {
            stack.pop();
            true
        } else {
            false
        }
    })
}

pub fn with_renderer_3d<R>(services: &Arc<Renderer3dRuntime>, f: impl FnOnce() -> R) -> R {
    let prior = install_renderer_3d(Arc::clone(services));
    struct Pop {
        services: Arc<Renderer3dRuntime>,
        prior: Option<Arc<Renderer3dRuntime>>,
    }
    impl Drop for Pop {
        fn drop(&mut self) {
            if uninstall_renderer_3d_if_current(&self.services) {
                if let Some(prior) = self.prior.take() {
                    install_renderer_3d(prior);
                }
            }
        }
    }
    let _pop = Pop {
        services: Arc::clone(services),
        prior,
    };
    f()
}

#[cfg(test)]
mod independence_tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc as StdArc, Barrier};
    use std::thread;

    #[test]
    fn two_renderer_3d_runtimes_on_two_threads_are_independent() {
        let a = Arc::new(Renderer3dRuntime::new());
        let b = Arc::new(Renderer3dRuntime::new());
        a.next_logical_decal_id.store(10, Ordering::SeqCst);
        b.next_logical_decal_id.store(20, Ordering::SeqCst);
        assert!(!Arc::ptr_eq(&a, &b));

        let barrier = StdArc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = StdArc::clone(&barrier);
        let bar2 = StdArc::clone(&barrier);

        let t1 = thread::spawn(move || {
            with_renderer_3d(&a2, || {
                renderer_3d()
                    .next_logical_decal_id
                    .store(110, Ordering::SeqCst);
                bar1.wait();
                assert_eq!(
                    renderer_3d()
                        .next_logical_decal_id
                        .load(Ordering::SeqCst),
                    110
                );
            });
        });
        let t2 = thread::spawn(move || {
            with_renderer_3d(&b2, || {
                renderer_3d()
                    .next_logical_decal_id
                    .store(220, Ordering::SeqCst);
                bar2.wait();
                assert_eq!(
                    renderer_3d()
                        .next_logical_decal_id
                        .load(Ordering::SeqCst),
                    220
                );
            });
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(a.next_logical_decal_id.load(Ordering::SeqCst), 110);
        assert_eq!(b.next_logical_decal_id.load(Ordering::SeqCst), 220);
    }

    #[test]
    fn unscoped_fallback_is_per_thread_not_shared() {
        let barrier = StdArc::new(Barrier::new(2));
        let bar1 = StdArc::clone(&barrier);
        let bar2 = StdArc::clone(&barrier);
        let t1 = thread::spawn(move || {
            let a = renderer_3d();
            a.next_logical_decal_id.store(7, Ordering::SeqCst);
            bar1.wait();
            assert_eq!(renderer_3d().next_logical_decal_id.load(Ordering::SeqCst), 7);
            a
        });
        let t2 = thread::spawn(move || {
            let b = renderer_3d();
            b.next_logical_decal_id.store(9, Ordering::SeqCst);
            bar2.wait();
            assert_eq!(renderer_3d().next_logical_decal_id.load(Ordering::SeqCst), 9);
            b
        });
        let a = t1.join().unwrap();
        let b = t2.join().unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
        assert_eq!(a.next_logical_decal_id.load(Ordering::SeqCst), 7);
        assert_eq!(b.next_logical_decal_id.load(Ordering::SeqCst), 9);
    }
}
