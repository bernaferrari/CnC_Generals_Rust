#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

/// Static sort list for transparent object sorting
#[derive(Debug, Default)]
pub(super) struct StaticSortState {
    entries: Vec<Arc<StaticSortRenderObject>>,
    meshes: Vec<Option<Arc<MeshClass>>>,
    sort_levels: Vec<u32>,
    enabled: bool,
    decals_enabled: bool,
    flush_depth: u32,
}

pub(super) static STATIC_SORT_STATE: OnceLock<Mutex<StaticSortState>> = OnceLock::new();

pub(super) fn static_sort_state() -> &'static Mutex<StaticSortState> {
    STATIC_SORT_STATE.get_or_init(|| {
        Mutex::new(StaticSortState {
            entries: Vec::new(),
            meshes: Vec::new(),
            sort_levels: Vec::new(),
            enabled: true,
            decals_enabled: true,
            flush_depth: 0,
        })
    })
}

#[derive(Clone)]
pub struct StaticSortEntry {
    handle: Arc<StaticSortRenderObject>,
    mesh: Option<Arc<MeshClass>>,
}

impl StaticSortEntry {
    fn from_handle(handle: Arc<StaticSortRenderObject>, mesh: Option<Arc<MeshClass>>) -> Self {
        Self { handle, mesh }
    }

    pub fn mesh_arc(&self) -> Option<Arc<MeshClass>> {
        self.mesh.clone()
    }

    pub fn render_object(&self) -> Arc<dyn RenderObjClass> {
        self.handle.render_obj()
    }
}

pub struct StaticSortFlushGuard;

impl Drop for StaticSortFlushGuard {
    fn drop(&mut self) {
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        if state.flush_depth > 0 {
            state.flush_depth -= 1;
            if state.flush_depth == 0 {
                state.entries.clear();
                state.meshes.clear();
                state.sort_levels.clear();
            }
        }
    }
}

pub struct StaticSortManager;

impl StaticSortManager {
    pub fn set_static_sort_lists_enabled(enabled: bool) {
        let _ = ww3d_core::WW3D::set_static_sort_lists_enabled(enabled);
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        state.enabled = enabled;
        if !enabled {
            state.entries.clear();
            state.meshes.clear();
            state.sort_levels.clear();
        }
    }

    pub fn set_decals_enabled(enabled: bool) {
        let _ = ww3d_core::WW3D::set_decals_enabled(enabled);
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        state.decals_enabled = enabled;
    }

    pub fn add_to_static_sort_list(handle: Arc<StaticSortRenderObject>, sort_level: u32) {
        Self::add_to_static_sort_list_with_mesh(handle, sort_level, None);
    }

    pub fn add_to_static_sort_list_with_mesh(
        handle: Arc<StaticSortRenderObject>,
        sort_level: u32,
        mesh: Option<Arc<MeshClass>>,
    ) {
        if !ww3d_core::WW3D::are_static_sort_lists_enabled() {
            return;
        }
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        if !state.enabled {
            return;
        }
        state.entries.push(handle);
        state.meshes.push(mesh);
        state.sort_levels.push(sort_level);
    }

    pub fn snapshot_static_sort_list() -> Option<(Vec<StaticSortEntry>, Vec<u32>)> {
        let state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        if state.entries.is_empty() {
            return None;
        }
        let entries = state
            .entries
            .iter()
            .cloned()
            .zip(state.meshes.iter().cloned())
            .map(|(handle, mesh)| StaticSortEntry::from_handle(handle, mesh))
            .collect::<Vec<_>>();
        Some((entries, state.sort_levels.clone()))
    }

    pub fn begin_flush() -> StaticSortFlushGuard {
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        state.flush_depth = state.flush_depth.saturating_add(1);
        StaticSortFlushGuard
    }

    pub fn flush_static_sort_list() {
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        state.entries.clear();
        state.meshes.clear();
        state.sort_levels.clear();
    }
}
