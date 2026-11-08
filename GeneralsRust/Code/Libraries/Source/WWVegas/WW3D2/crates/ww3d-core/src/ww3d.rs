//! High level WW3D engine façade and renderer bridge.
//!
//! Mirrors the static API exposed by the original C++ implementation while remaining agnostic of
//! the concrete rendering backend. Global state such as sorting flags is managed here and pushed
//! to the active renderer when one is registered.

use crate::errors::W3DResult;
use std::any::Any;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use crate::asset_manager::AssetManager;
use crate::class_registry::ClassRegistry;

/// Cross-backend frame statistics exposed by the registered renderer.
#[derive(Debug, Default, Clone)]
pub struct FrameStats {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub draw_calls: u32,
    pub meshes_rendered: u32,
    pub triangles_rendered: u32,
    pub material_passes: u32,
    pub texture_switches: u32,
    pub shader_switches: u32,
    pub vertex_color_passes: u32,
}

/// Native renderers may cross threads; browser GPU owners retain their actual
/// auto traits rather than pretending browser handles are transferable.
#[cfg(not(target_arch = "wasm32"))]
pub trait RenderSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: ?Sized + Send> RenderSend for T {}
#[cfg(target_arch = "wasm32")]
pub trait RenderSend {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> RenderSend for T {}

/// Native shared render objects must be thread-safe. On wasm this imposes no
/// auto trait, so an owner containing browser GPU handles stays thread-local.
#[cfg(not(target_arch = "wasm32"))]
pub trait RenderSync: Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: ?Sized + Sync> RenderSync for T {}
#[cfg(target_arch = "wasm32")]
pub trait RenderSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> RenderSync for T {}

// Keep the original allocation and concrete Arc type through deferred dispatch.
// Arc<dyn Any> cannot be downcast safely on wasm without Send + Sync.
trait StaticSortSubmission: RenderSend + RenderSync {
    fn submit(self: Arc<Self>, backend: &mut dyn RendererBackend, level: u32) -> W3DResult<()>;
}

impl<T: Any + RenderSend + RenderSync> StaticSortSubmission for T {
    fn submit(self: Arc<Self>, backend: &mut dyn RendererBackend, level: u32) -> W3DResult<()> {
        backend.add_to_static_sort_list(&self, level)
    }
}

/// Trait implemented by renderer backends. The core library does not impose any particular
/// rendering technology; instead it calls into the backend through this trait.
///
/// Each session owns its backend behind a Mutex. Native backends retain Send;
/// wasm backends may own browser GPU handles without unsafe auto-trait claims.
pub trait RendererBackend: Any + RenderSend {
    fn begin_frame(&mut self) -> W3DResult<()>;
    fn end_frame(&mut self) -> W3DResult<()>;
    fn is_ready(&self) -> bool;

    fn frame_stats(&self) -> FrameStats {
        FrameStats::default()
    }

    /// Toggle front-end sorting. Default implementation accepts the request without side effects.
    fn set_sorting_enabled(&mut self, _enabled: bool) -> W3DResult<()> {
        Ok(())
    }

    /// Query whether the backend considers render-object sorting enabled.
    fn is_sorting_enabled(&self) -> bool {
        true
    }

    /// Toggle static sort lists (transparent object buckets).
    fn set_static_sort_lists_enabled(&mut self, _enabled: bool) -> W3DResult<()> {
        Ok(())
    }

    /// Query static sort list state.
    fn are_static_sort_lists_enabled(&self) -> bool {
        true
    }

    /// Toggle decal rendering.
    fn set_decals_enabled(&mut self, _enabled: bool) -> W3DResult<()> {
        Ok(())
    }

    /// Query decal state.
    fn are_decals_enabled(&self) -> bool {
        true
    }

    /// Queue an object into the backend static sort list. `object` borrows the
    /// submitted `Arc<T>`; a backend downcasts that Arc and clones only its handle.
    fn add_to_static_sort_list(
        &mut self,
        _object: &dyn Any,
        _sort_level: u32,
    ) -> W3DResult<()> {
        Ok(())
    }

    /// Flush all pending static sort entries.
    fn flush_static_sort_lists(&mut self) -> W3DResult<()> {
        Ok(())
    }

    /// Downcast support so higher level crates can access backend-specific state.
    fn as_any(&self) -> &dyn Any;

    /// Mutable downcast support.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}




/// C++ `WW3D::PrelitModeEnum` (`ww3d.h`). Selects which prelit material
/// wrapper (`W3D_CHUNK_PRELIT_*`) a mesh load consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrelitMode {
    Unlit = 0,
    Vertex = 1,
    #[default]
    LightmapMultiPass = 2,
    LightmapMultiTexture = 3,
}

type RendererHandle = Arc<Mutex<Box<dyn RendererBackend>>>;

/// Owned WW3D engine context (C++ W3DDisplay / session owns the WW3D façade).
///
/// Not a process singleton: construct one per display/engine instance and pass
/// it explicitly. [`WgpuMainRenderer`] owns an `Arc` of this context.
pub struct Ww3dContext {
    renderer: Mutex<Option<RendererHandle>>,
    state: Mutex<WW3DState>,
    /// C++ WW3DAssetManager lives with the WW3D session / display.
    asset_manager: std::sync::Arc<Mutex<AssetManager>>,
    class_registry: ClassRegistry,
    sync_time_ms: AtomicU32,
    previous_sync_time_ms: AtomicU32,
}

impl std::fmt::Debug for Ww3dContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ww3dContext")
            .field("sync_time_ms", &self.sync_time_ms.load(Ordering::Relaxed))
            .field(
                "previous_sync_time_ms",
                &self.previous_sync_time_ms.load(Ordering::Relaxed),
            )
            .field("has_renderer", &self.renderer.lock().map(|g| g.is_some()).unwrap_or(false))
            .field(
                "asset_cache_total",
                &self
                    .asset_manager
                    .lock()
                    .map(|g| g.cache_stats().total_count())
                    .unwrap_or(0),
            )
            .finish()
    }
}

struct WW3DState {
    is_sorting_enabled: bool,
    static_sort_lists_enabled: bool,
    decals_enabled: bool,
    decal_rejection_distance: f32,
    pending_static_sort: Vec<(Arc<dyn StaticSortSubmission>, u32)>,
    prelit_mode: PrelitMode,
}

impl Default for WW3DState {
    fn default() -> Self {
        Self {
            is_sorting_enabled: true,
            // C++ WW3D defaults this false, then W3DDisplay::init enables it
            // for the whole session. Match post-init C++ as the live default.
            static_sort_lists_enabled: true,
            decals_enabled: true,
            decal_rejection_distance: 1_000_000.0,
            pending_static_sort: Vec::new(),
            prelit_mode: PrelitMode::LightmapMultiPass,
        }
    }
}

impl Default for Ww3dContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Ww3dContext {
    pub fn new() -> Self {
        Self {
            renderer: Mutex::new(None),
            state: Mutex::new(WW3DState::default()),
            asset_manager: std::sync::Arc::new(Mutex::new(AssetManager::new())),
            class_registry: {
                let reg = ClassRegistry::new();
                reg.register_builtin_class_names();
                reg
            },
            sync_time_ms: AtomicU32::new(0),
            previous_sync_time_ms: AtomicU32::new(0),
        }
    }

    /// Register a renderer backend. Returns `true` if registered, `false` if one is active.
    pub fn register_renderer<R>(&self, renderer: R) -> bool
    where
        R: RendererBackend + 'static,
    {
        let handle: RendererHandle = Arc::new(Mutex::new(Box::new(renderer)));
        let inserted = {
            let mut slot = self.renderer.lock().unwrap_or_else(|p| p.into_inner());
            if slot.is_some() {
                false
            } else {
                *slot = Some(Arc::clone(&handle));
                true
            }
        };
        if !inserted {
            return false;
        }

        let (sorting_enabled, static_sort_enabled, decals_enabled, pending_items) = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let pending = std::mem::take(&mut state.pending_static_sort);
            (
                state.is_sorting_enabled,
                state.static_sort_lists_enabled,
                state.decals_enabled,
                pending,
            )
        };

        let mut restore_pending: Option<Vec<(Arc<dyn StaticSortSubmission>, u32)>> = None;
        if let Some(result) = self.with_renderer(|backend| {
            backend.set_sorting_enabled(sorting_enabled)?;
            backend.set_static_sort_lists_enabled(static_sort_enabled)?;
            backend.set_decals_enabled(decals_enabled)?;
            for (object, level) in pending_items.iter().cloned() {
                object.submit(backend, level)?;
            }
            Ok(())
        }) {
            if let Err(err) = result {
                restore_pending = Some(pending_items);
                log::warn!("Ww3dContext::register_renderer: backend rejected init: {err:?}");
            }
        } else {
            restore_pending = Some(pending_items);
        }
        if let Some(pending) = restore_pending {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.pending_static_sort.extend(pending);
        }
        true
    }

    pub fn unregister_renderer(&self) {
        self.renderer.lock().unwrap_or_else(|p| p.into_inner()).take();
    }

    pub fn get_current_renderer(&self) -> Option<W3DRenderer> {
        self.renderer
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .cloned()
            .map(W3DRenderer)
    }

    pub fn with_renderer<F, T>(&self, mut f: F) -> Option<W3DResult<T>>
    where
        F: FnMut(&mut dyn RendererBackend) -> W3DResult<T>,
    {
        let handle = self.get_current_renderer()?;
        let binding = handle.handle();
        let mut guard = binding.lock().unwrap_or_else(|p| p.into_inner());
        Some(f(guard.as_mut()))
    }

    pub fn current_frame_stats(&self) -> Option<FrameStats> {
        let handle = self.get_current_renderer()?;
        let binding = handle.handle();
        let guard = binding.lock().unwrap_or_else(|p| p.into_inner());
        Some(guard.frame_stats())
    }

    pub fn sync(&self, sync_time_ms: u32) {
        let previous = self.sync_time_ms.swap(sync_time_ms, Ordering::SeqCst);
        self.previous_sync_time_ms.store(previous, Ordering::SeqCst);
    }

    pub fn sync_time(&self) -> u32 {
        self.sync_time_ms.load(Ordering::SeqCst)
    }

    pub fn previous_sync_time(&self) -> u32 {
        self.previous_sync_time_ms.load(Ordering::SeqCst)
    }

    pub fn class_registry(&self) -> &ClassRegistry {
        &self.class_registry
    }

    pub fn asset_manager(&self) -> std::sync::Arc<Mutex<AssetManager>> {
        std::sync::Arc::clone(&self.asset_manager)
    }

    pub fn with_asset_manager_mut<R>(&self, f: impl FnOnce(&mut AssetManager) -> R) -> R {
        let mut mgr = self.asset_manager.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut mgr)
    }

    pub fn with_asset_manager_ref<R>(&self, f: impl FnOnce(&AssetManager) -> R) -> R {
        let mgr = self.asset_manager.lock().unwrap_or_else(|p| p.into_inner());
        f(&mgr)
    }

    /// Enable sorting on this context and its registered renderer.
    pub fn enable_sorting(&self, enable: bool) -> W3DResult<()> {
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.is_sorting_enabled = enable;
        }
        if let Some(Err(err)) = self.with_renderer(|backend| backend.set_sorting_enabled(enable)) {
            return Err(err);
        }
        Ok(())
    }

    pub fn is_sorting_enabled(&self) -> bool {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).is_sorting_enabled
    }

    pub fn set_static_sort_lists_enabled(&self, enable: bool) -> W3DResult<()> {
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.static_sort_lists_enabled = enable;
        }
        if let Some(Err(err)) =
            self.with_renderer(|backend| backend.set_static_sort_lists_enabled(enable))
        {
            return Err(err);
        }
        Ok(())
    }

    pub fn are_static_sort_lists_enabled(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .static_sort_lists_enabled
    }

    pub fn set_decals_enabled(&self, enable: bool) -> W3DResult<()> {
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.decals_enabled = enable;
        }
        if let Some(Err(err)) = self.with_renderer(|backend| backend.set_decals_enabled(enable)) {
            return Err(err);
        }
        Ok(())
    }

    pub fn are_decals_enabled(&self) -> bool {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).decals_enabled
    }

    pub fn set_decal_rejection_distance(&self, distance: f32) {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).decal_rejection_distance = distance;
    }

    pub fn decal_rejection_distance(&self) -> f32 {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .decal_rejection_distance
    }

    pub fn add_to_static_sort_list<T>(&self, object: Arc<T>, sort_level: u32) -> W3DResult<()>
    where
        T: Any + RenderSend + RenderSync + 'static,
    {
        let object: Arc<dyn StaticSortSubmission> = object;
        if let Some(result) = self.with_renderer(|backend| {
            Arc::clone(&object).submit(backend, sort_level)
        }) {
            return result;
        }
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pending_static_sort
            .push((object, sort_level));
        Ok(())
    }

    pub fn flush_static_sort_lists(&self) -> W3DResult<()> {
        if let Some(result) = self.with_renderer(|backend| backend.flush_static_sort_lists()) {
            return result;
        }
        Ok(())
    }

    pub fn set_prelit_mode(&self, mode: PrelitMode) {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).prelit_mode = mode;
    }

    pub fn get_prelit_mode(&self) -> PrelitMode {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).prelit_mode
    }
}

/// Wrapper object used for backwards compatibility with the historic API.
#[derive(Clone)]
pub struct W3DRenderer(RendererHandle);

impl W3DRenderer {
    pub fn handle(&self) -> RendererHandle {
        self.0.clone()
    }
}

/// Historic empty façade type. Prefer [`Ww3dContext`] owned by the display.
#[derive(Debug, Default)]
pub struct WW3D;
pub use self::WW3D as WW3DClass;




#[cfg(test)]
mod independence_tests {
    use super::*;

    #[test]
    fn two_ww3d_contexts_on_two_threads_do_not_share_asset_manager() {
        let a = Ww3dContext::new();
        let b = Ww3dContext::new();
        assert!(!std::sync::Arc::ptr_eq(&a.asset_manager(), &b.asset_manager()));
    }

    #[derive(Default)]
    struct SortBackend {
        entries: Vec<(Arc<u32>, u32)>,
    }

    impl RendererBackend for SortBackend {
        fn begin_frame(&mut self) -> W3DResult<()> { Ok(()) }
        fn end_frame(&mut self) -> W3DResult<()> { Ok(()) }
        fn is_ready(&self) -> bool { true }
        fn as_any(&self) -> &dyn Any { self }
        fn as_any_mut(&mut self) -> &mut dyn Any { self }

        fn add_to_static_sort_list(&mut self, object: &dyn Any, level: u32) -> W3DResult<()> {
            let object = object.downcast_ref::<Arc<u32>>().expect("original concrete Arc");
            self.entries.push((Arc::clone(object), level));
            Ok(())
        }
    }

    #[test]
    fn static_sort_preserves_identity_order_and_levels_across_registration() {
        let context = Ww3dContext::new();
        let first = Arc::new(11_u32);
        let second = Arc::new(22_u32);
        context.add_to_static_sort_list(Arc::clone(&first), 7).unwrap();
        assert!(context.register_renderer(SortBackend::default()));
        context.add_to_static_sort_list(Arc::clone(&second), 2).unwrap();
        context.with_renderer(|backend| {
            let backend = backend.as_any().downcast_ref::<SortBackend>().unwrap();
            assert_eq!(backend.entries.len(), 2);
            assert!(Arc::ptr_eq(&backend.entries[0].0, &first));
            assert!(Arc::ptr_eq(&backend.entries[1].0, &second));
            assert_eq!(backend.entries[0].1, 7);
            assert_eq!(backend.entries[1].1, 2);
            Ok(())
        }).unwrap().unwrap();
    }
}
