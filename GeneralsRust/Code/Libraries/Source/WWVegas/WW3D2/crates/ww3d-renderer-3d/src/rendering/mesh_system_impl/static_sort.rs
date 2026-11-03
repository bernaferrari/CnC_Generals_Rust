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
use std::sync::Arc;

/// Static sort list for transparent object sorting
#[derive(Debug, Default)]
pub(crate) struct StaticSortState {
    entries: Vec<Arc<StaticSortRenderObject>>,
    meshes: Vec<Option<Arc<MeshClass>>>,
    sort_levels: Vec<u32>,
    enabled: bool,
    decals_enabled: bool,
    sorting_enabled: bool,
    flush_depth: u32,
}

pub(crate) fn default_static_sort_state() -> StaticSortState {
    StaticSortState {
        entries: Vec::new(),
        meshes: Vec::new(),
        sort_levels: Vec::new(),
        enabled: true,
        decals_enabled: true,
        sorting_enabled: true,
        flush_depth: 0,
    }
}

pub(super) fn with_sort_state<R>(
    r3d: &crate::process_state::Renderer3dRuntime,
    f: impl FnOnce(&mut StaticSortState) -> R,
) -> R {
    f(&mut r3d
        .static_sort_state
        .lock()
        .unwrap_or_else(|p| p.into_inner()))
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

pub struct StaticSortFlushGuard {
    r3d: Arc<crate::process_state::Renderer3dRuntime>,
}

impl Drop for StaticSortFlushGuard {
    fn drop(&mut self) {
        with_sort_state(&self.r3d, |state| {
            if state.flush_depth > 0 {
                state.flush_depth -= 1;
                if state.flush_depth == 0 {
                    state.entries.clear();
                    state.meshes.clear();
                    state.sort_levels.clear();
                }
            }
        });
    }
}

pub struct StaticSortManager;

impl StaticSortManager {
    pub fn set_static_sort_lists_enabled(r3d: &crate::process_state::Renderer3dRuntime, enabled: bool) {
        with_sort_state(r3d, |state| {
            state.enabled = enabled;
            if !enabled {
                state.entries.clear();
                state.meshes.clear();
                state.sort_levels.clear();
            }
        });
    }

    pub fn set_decals_enabled(r3d: &crate::process_state::Renderer3dRuntime, enabled: bool) {
        with_sort_state(r3d, |state| state.decals_enabled = enabled);
    }

    pub fn set_sorting_enabled(r3d: &crate::process_state::Renderer3dRuntime, enabled: bool) {
        with_sort_state(r3d, |state| state.sorting_enabled = enabled);
    }

    pub fn are_static_sort_lists_enabled(r3d: &crate::process_state::Renderer3dRuntime) -> bool {
        with_sort_state(r3d, |state| state.enabled)
    }

    pub fn are_decals_enabled(r3d: &crate::process_state::Renderer3dRuntime) -> bool {
        with_sort_state(r3d, |state| state.decals_enabled)
    }

    pub fn is_sorting_enabled(r3d: &crate::process_state::Renderer3dRuntime) -> bool {
        with_sort_state(r3d, |state| state.sorting_enabled)
    }

    pub fn add_to_static_sort_list(r3d: &crate::process_state::Renderer3dRuntime, handle: Arc<StaticSortRenderObject>, sort_level: u32) {
        Self::add_to_static_sort_list_with_mesh(r3d, handle, sort_level, None);
    }

    pub fn add_to_static_sort_list_with_mesh(
        r3d: &crate::process_state::Renderer3dRuntime,
        handle: Arc<StaticSortRenderObject>,
        sort_level: u32,
        mesh: Option<Arc<MeshClass>>,
    ) {
        with_sort_state(r3d, |state| {
            if !state.enabled {
                return;
            }
            state.entries.push(handle);
            state.meshes.push(mesh);
            state.sort_levels.push(sort_level);
        });
    }

    pub fn snapshot_static_sort_list(r3d: &crate::process_state::Renderer3dRuntime) -> Option<(Vec<StaticSortEntry>, Vec<u32>)> {
        with_sort_state(r3d, |state| {
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
        })
    }

    pub fn begin_flush(r3d: Arc<crate::process_state::Renderer3dRuntime>) -> StaticSortFlushGuard {
        with_sort_state(&r3d, |state| state.flush_depth = state.flush_depth.saturating_add(1));
        StaticSortFlushGuard { r3d }
    }

    pub fn flush_static_sort_list(r3d: &crate::process_state::Renderer3dRuntime) {
        with_sort_state(r3d, |state| {
            state.entries.clear();
            state.meshes.clear();
            state.sort_levels.clear();
        });
    }
}
