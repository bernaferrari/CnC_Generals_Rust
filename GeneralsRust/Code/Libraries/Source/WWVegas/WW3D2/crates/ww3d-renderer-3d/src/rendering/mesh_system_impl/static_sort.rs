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
        with_sort_state(r3d, |state| state.enabled && state.flush_depth == 0)
    }

    pub fn are_decals_enabled(r3d: &crate::process_state::Renderer3dRuntime) -> bool {
        with_sort_state(r3d, |state| state.decals_enabled)
    }

    pub fn is_sorting_enabled(r3d: &crate::process_state::Renderer3dRuntime) -> bool {
        with_sort_state(r3d, |state| state.sorting_enabled)
    }

    /// Admit the original mesh handle to the receiving renderer, never a cloned mesh.
    /// C++ mesh.cpp:579-592; ww3d.cpp:1906-1913 disables admission during flush.
    pub fn try_add_mesh(
        r3d: &crate::process_state::Renderer3dRuntime,
        mesh: &Arc<MeshClass>,
    ) -> bool {
        let sort_level = mesh.get_sort_level();
        if sort_level == SORT_LEVEL_NONE {
            return false;
        }
        with_sort_state(r3d, |state| {
            if !state.enabled || state.flush_depth != 0 {
                return false;
            }
            state.entries.push(StaticSortRenderObject::from_arc(Arc::clone(mesh)));
            state.meshes.push(Some(Arc::clone(mesh)));
            state.sort_levels.push(sort_level);
            true
        })
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
            if !state.enabled || state.flush_depth != 0 {
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

    /// Consume pending entries once; a second flush cannot replay their admission.
    pub fn take_static_sort_list(
        r3d: &crate::process_state::Renderer3dRuntime,
    ) -> Vec<(StaticSortEntry, u32)> {
        with_sort_state(r3d, |state| {
            state.entries.drain(..)
                .zip(state.meshes.drain(..))
                .zip(state.sort_levels.drain(..))
                .map(|((handle, mesh), level)| (StaticSortEntry::from_handle(handle, mesh), level))
                .collect()
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

#[cfg(test)]
mod admission_tests {
    use super::*;

    #[test]
    fn static_admission_preserves_identity_and_nested_flush_is_owner_local() {
        let first = Arc::new(crate::process_state::Renderer3dRuntime::new());
        let second = Arc::new(crate::process_state::Renderer3dRuntime::new());
        let mut model = MeshModelClass::new("authored-sort");
        model.sort_level = SORT_LEVEL_BIN2;
        let mut mesh = MeshClass::new();
        mesh.model = Some(Arc::new(model));
        let mesh = Arc::new(mesh);

        assert!(StaticSortManager::try_add_mesh(&first, &mesh));
        assert!(StaticSortManager::try_add_mesh(&second, &mesh));
        let guard = StaticSortManager::begin_flush(Arc::clone(&first));
        assert!(!StaticSortManager::are_static_sort_lists_enabled(&first));
        assert!(StaticSortManager::are_static_sort_lists_enabled(&second));
        assert!(!StaticSortManager::try_add_mesh(&first, &mesh));
        let nested = StaticSortManager::begin_flush(Arc::clone(&first));
        let entries = StaticSortManager::take_static_sort_list(&first);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1, SORT_LEVEL_BIN2);
        assert!(Arc::ptr_eq(&entries[0].0.mesh_arc().unwrap(), &mesh));
        assert!(StaticSortManager::take_static_sort_list(&first).is_empty());
        drop(nested);
        assert!(!StaticSortManager::try_add_mesh(&first, &mesh));
        drop(guard);
        assert!(StaticSortManager::try_add_mesh(&first, &mesh));
        StaticSortManager::set_static_sort_lists_enabled(&first, false);
        assert!(!StaticSortManager::try_add_mesh(&first, &mesh));
        assert!(StaticSortManager::snapshot_static_sort_list(&first).is_none());
        assert_eq!(StaticSortManager::take_static_sort_list(&second).len(), 1);
    }
}
