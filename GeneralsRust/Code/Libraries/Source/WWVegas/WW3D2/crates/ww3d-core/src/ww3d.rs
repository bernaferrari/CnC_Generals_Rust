//! High level WW3D engine façade and renderer bridge.
//!
//! Mirrors the static API exposed by the original C++ implementation while remaining agnostic of
//! the concrete rendering backend. Global state such as sorting flags is managed here and pushed
//! to the active renderer when one is registered.

use crate::errors::{W3DError, W3DResult};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

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

/// Trait implemented by renderer backends. The core library does not impose any particular
/// rendering technology; instead it calls into the backend through this trait.
///
/// Backends are owned by the game thread (stored thread-local alongside the
/// rest of the WW3D statics), so no `Send`/`Sync` bound is imposed.
pub trait RendererBackend: Any {
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

    /// Queue an object into the backend static sort list.
    fn add_to_static_sort_list(
        &mut self,
        _object: Arc<dyn Any + Send + Sync>,
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

type RendererHandle = Rc<RefCell<Box<dyn RendererBackend>>>;

thread_local! {
    /// C++ kept the registered renderer in a plain static driven from the game
    /// thread; the port mirrors that with thread-local ownership.
    static RENDERER_STORAGE: RefCell<Option<RendererHandle>> = const { RefCell::new(None) };
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

struct WW3DState {
    is_sorting_enabled: bool,
    static_sort_lists_enabled: bool,
    decals_enabled: bool,
    decal_rejection_distance: f32,
    pending_static_sort: Vec<(Arc<dyn Any + Send + Sync>, u32)>,
    prelit_mode: PrelitMode,
}

thread_local! {
    /// C++ kept these toggles as plain statics on the game thread.
    static WW3D_STATE: RefCell<WW3DState> = RefCell::new(WW3DState {
        is_sorting_enabled: true,
        // C++ WW3D defaults this false, then W3DDisplay::init enables it
        // for the whole session. No Device/Main boot hook exists here, so
        // the live default must match post-init C++.
        static_sort_lists_enabled: true,
        decals_enabled: true,
        decal_rejection_distance: 1_000_000.0,
        pending_static_sort: Vec::new(),
        prelit_mode: PrelitMode::LightmapMultiPass,
    });
}

#[derive(Default)]
struct SyncState {
    sync_time_ms: AtomicU32,
    previous_sync_time_ms: AtomicU32,
}

fn sync_state() -> &'static SyncState {
    static STATE: OnceLock<SyncState> = OnceLock::new();
    STATE.get_or_init(SyncState::default)
}

/// Wrapper object used for backwards compatibility with the historic API.
#[derive(Clone)]
pub struct W3DRenderer(RendererHandle);

impl W3DRenderer {
    pub fn handle(&self) -> RendererHandle {
        self.0.clone()
    }
}

/// Main WW3D engine façade.
pub struct WW3D;

impl WW3D {
    /// Register a renderer backend. Returns `true` if the renderer was registered, `false` if a
    /// renderer is already active.
    pub fn register_renderer<R>(renderer: R) -> bool
    where
        R: RendererBackend + 'static,
    {
        let handle: RendererHandle = Rc::new(RefCell::new(Box::new(renderer)));
        let inserted = RENDERER_STORAGE.with_borrow_mut(|slot| {
            if slot.is_some() {
                false
            } else {
                *slot = Some(handle.clone());
                true
            }
        });
        if !inserted {
            return false;
        }

        let (sorting_enabled, static_sort_enabled, decals_enabled, pending_items) =
            WW3D_STATE.with_borrow_mut(|state| {
                let pending = std::mem::take(&mut state.pending_static_sort);
                (
                    state.is_sorting_enabled,
                    state.static_sort_lists_enabled,
                    state.decals_enabled,
                    pending,
                )
            });

        let mut restore_pending: Option<Vec<(Arc<dyn Any + Send + Sync>, u32)>> = None;

        if let Some(result) = WW3D::with_renderer(|backend| {
            backend.set_sorting_enabled(sorting_enabled)?;
            backend.set_static_sort_lists_enabled(static_sort_enabled)?;
            backend.set_decals_enabled(decals_enabled)?;

            for (object, level) in pending_items.iter().cloned() {
                backend.add_to_static_sort_list(object, level)?;
            }
            // C++ Render_And_Clear runs from the frame, not from renderer init.
            // A flush here has no pass and only drops the objects.
            Ok(())
        }) {
            if let Err(err) = result {
                restore_pending = Some(pending_items);
                log::warn!("WW3D::register_renderer: backend rejected initialization: {err:?}");
            }
        } else {
            restore_pending = Some(pending_items);
        }

        if let Some(pending) = restore_pending {
            WW3D_STATE.with_borrow_mut(|state| state.pending_static_sort.extend(pending));
        }

        true
    }

    /// Remove the currently registered renderer.
    pub fn unregister_renderer() {
        RENDERER_STORAGE.with_borrow_mut(|slot| {
            slot.take();
        });
    }

    /// Borrow the current renderer handle if one is registered.
    pub fn get_current_renderer() -> Option<W3DRenderer> {
        RENDERER_STORAGE
            .with_borrow(|slot| slot.as_ref().cloned())
            .map(W3DRenderer)
    }

    /// Convenience helper: run the provided closure with the current renderer if present.
    pub fn with_renderer<F, T>(mut f: F) -> Option<W3DResult<T>>
    where
        F: FnMut(&mut dyn RendererBackend) -> W3DResult<T>,
    {
        let handle = WW3D::get_current_renderer()?;
        let binding = handle.handle();
        let mut guard = binding.borrow_mut();
        Some(f(guard.as_mut()))
    }

    /// Retrieve the latest frame statistics from the active renderer, if any.
    pub fn current_frame_stats() -> Option<FrameStats> {
        let handle = WW3D::get_current_renderer()?;
        let binding = handle.handle();
        let guard = binding.borrow_mut();
        Some(guard.frame_stats())
    }

    /// Mirror the C++ WW3D::Sync call, storing the latest engine tick in milliseconds.
    pub fn sync(sync_time_ms: u32) {
        let state = sync_state();
        let previous = state.sync_time_ms.swap(sync_time_ms, Ordering::SeqCst);
        state
            .previous_sync_time_ms
            .store(previous, Ordering::SeqCst);
    }

    /// Retrieve the most recent sync time in milliseconds.
    pub fn sync_time() -> u32 {
        sync_state().sync_time_ms.load(Ordering::Relaxed)
    }

    /// Retrieve the previous sync time in milliseconds.
    pub fn previous_sync_time() -> u32 {
        sync_state().previous_sync_time_ms.load(Ordering::Relaxed)
    }

    /// Globally enable or disable render-object sorting.
    pub fn enable_sorting(enable: bool) -> W3DResult<()> {
        let already = WW3D_STATE.with_borrow_mut(|state| {
            if state.is_sorting_enabled == enable {
                true
            } else {
                state.is_sorting_enabled = enable;
                false
            }
        });
        if already {
            return Ok(());
        }

        if let Some(Err(err)) = WW3D::with_renderer(|backend| backend.set_sorting_enabled(enable)) {
            WW3D_STATE.with_borrow_mut(|state| state.is_sorting_enabled = !enable);
            return Err(err);
        }
        Ok(())
    }

    /// Query whether render-object sorting is enabled.
    pub fn is_sorting_enabled() -> bool {
        WW3D_STATE.with_borrow(|state| state.is_sorting_enabled)
    }

    /// Enable or disable the static sort lists.
    pub fn set_static_sort_lists_enabled(enable: bool) -> W3DResult<()> {
        let already = WW3D_STATE.with_borrow_mut(|state| {
            if state.static_sort_lists_enabled == enable {
                true
            } else {
                state.static_sort_lists_enabled = enable;
                false
            }
        });
        if already {
            return Ok(());
        }

        if let Some(Err(err)) =
            WW3D::with_renderer(|backend| backend.set_static_sort_lists_enabled(enable))
        {
            WW3D_STATE.with_borrow_mut(|state| state.static_sort_lists_enabled = !enable);
            return Err(err);
        }
        Ok(())
    }

    /// Query whether static sort lists are enabled.
    pub fn are_static_sort_lists_enabled() -> bool {
        WW3D_STATE.with_borrow(|state| state.static_sort_lists_enabled)
    }

    /// Enable or disable decals globally.
    pub fn set_decals_enabled(enable: bool) -> W3DResult<()> {
        let already = WW3D_STATE.with_borrow_mut(|state| {
            if state.decals_enabled == enable {
                true
            } else {
                state.decals_enabled = enable;
                false
            }
        });
        if already {
            return Ok(());
        }

        if let Some(Err(err)) = WW3D::with_renderer(|backend| backend.set_decals_enabled(enable)) {
            WW3D_STATE.with_borrow_mut(|state| state.decals_enabled = !enable);
            return Err(err);
        }
        Ok(())
    }

    /// Query whether decals are enabled.
    pub fn are_decals_enabled() -> bool {
        WW3D_STATE.with_borrow(|state| state.decals_enabled)
    }

    /// Set the global decal rejection distance.
    pub fn set_decal_rejection_distance(distance: f32) {
        WW3D_STATE.with_borrow_mut(|state| state.decal_rejection_distance = distance.max(0.0));
    }

    /// Retrieve the decal rejection distance.
    pub fn decal_rejection_distance() -> f32 {
        WW3D_STATE.with_borrow(|state| state.decal_rejection_distance)
    }

    /// Queue an object into the static sort list. If the renderer is not yet available the object
    /// is held and replayed once a renderer registers.
    pub fn add_to_static_sort_list<T>(object: Arc<T>, sort_level: u32) -> W3DResult<()>
    where
        T: Any + Send + Sync + 'static,
    {
        let rejected = WW3D_STATE.with_borrow(|state| {
            if !state.is_sorting_enabled {
                Some("sorting is currently disabled".to_string())
            } else if !state.static_sort_lists_enabled {
                Some("static sort lists are disabled".to_string())
            } else {
                None
            }
        });
        if let Some(reason) = rejected {
            return Err(W3DError::FeatureDisabled(reason));
        }

        let arc_any: Arc<dyn Any + Send + Sync> = object;

        if let Some(result) = WW3D::with_renderer(|backend| {
            backend.add_to_static_sort_list(arc_any.clone(), sort_level)
        }) {
            match result {
                Ok(_) => Ok(()),
                Err(err) => {
                    WW3D_STATE
                        .with_borrow_mut(|state| state.pending_static_sort.push((arc_any, sort_level)));
                    Err(err)
                }
            }
        } else {
            WW3D_STATE.with_borrow_mut(|state| state.pending_static_sort.push((arc_any, sort_level)));
            Err(W3DError::RendererUnavailable)
        }
    }

    /// Flush all queued static sort objects through the active renderer.
    pub fn flush_static_sort_lists() -> W3DResult<()> {
        if let Some(result) = WW3D::with_renderer(|backend| backend.flush_static_sort_lists()) {
            result
        } else {
            Err(W3DError::RendererUnavailable)
        }
    }

    /// C++ `WW3D::Set_Prelit_Mode`.
    pub fn set_prelit_mode(mode: PrelitMode) {
        WW3D_STATE.with_borrow_mut(|state| state.prelit_mode = mode);
    }

    /// C++ `WW3D::Get_Prelit_Mode`. Defaults to lightmap multi-pass.
    pub fn get_prelit_mode() -> PrelitMode {
        WW3D_STATE.with_borrow(|state| state.prelit_mode)
    }
}

// Re-export for compatibility with legacy naming.
pub use self::WW3D as WW3DClass;
