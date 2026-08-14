//! W3D-specific ghost object snapshot layer.
//!
//! CPU-side parity port for `W3DDevice/GameLogic/W3DGhostObject.cpp`.

use game_engine::common::game_common::MAX_PLAYER_COUNT;
use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};

pub const INVALID_OBJECT_ID: u32 = u32::MAX;
pub const INVALID_DRAWABLE_ID: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix3x4 {
    pub rows: [[f32; 4]; 3],
}

impl Matrix3x4 {
    pub const IDENTITY: Self = Self {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    };
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSubObjectSnapshot {
    pub name: String,
    pub visible: bool,
    pub transform: Matrix3x4,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderObjectState {
    pub name: String,
    pub scale: f32,
    pub color: u32,
    pub transform: Matrix3x4,
    pub sub_objects: Vec<RenderSubObjectSnapshot>,
    pub class_id: RenderObjectClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderObjectClass {
    Mesh,
    HLod,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct W3DRenderObjectSnapshot {
    pub render_object: RenderObjectState,
    pub uv_animations_disabled: bool,
    pub muzzle_fx_hidden: bool,
}

impl W3DRenderObjectSnapshot {
    pub fn new(render_object: RenderObjectState) -> Self {
        let mut snapshot = Self {
            render_object,
            uv_animations_disabled: false,
            muzzle_fx_hidden: false,
        };
        snapshot.disable_fog_animations();
        snapshot
    }

    pub fn update(&mut self, render_object: RenderObjectState) {
        self.render_object = render_object;
        self.disable_fog_animations();
    }

    fn disable_fog_animations(&mut self) {
        if self.render_object.class_id == RenderObjectClass::HLod {
            self.uv_animations_disabled = true;
        }
        self.muzzle_fx_hidden = self
            .render_object
            .sub_objects
            .iter_mut()
            .filter(|sub| sub.name.contains("MUZZLEFX"))
            .map(|sub| {
                sub.visible = false;
            })
            .count()
            > 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhostSceneEvent {
    RemoveParentObject(u32),
    RestoreParentObject(u32),
    AddSnapshot {
        player_index: usize,
        snapshot: usize,
    },
    RemoveSnapshot {
        player_index: usize,
        snapshot: usize,
    },
}

/// Stable identity for one pooled W3D ghost. Vector positions are not identities:
/// the C++ manager inserts at the list head and recycles modules from a free list.
pub type W3DGhostSceneId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct W3DGhostSnapshotKey {
    pub ghost_id: W3DGhostSceneId,
    pub player_index: usize,
    pub snapshot_index: usize,
}

/// Exact immutable payload handed from the logic-owned ghost manager to the
/// client scene boundary. It deliberately carries DrawableInfo and geometry;
/// neither ownership nor shroud alpha is inferred downstream.
#[derive(Debug, Clone, PartialEq)]
pub struct FrozenW3DGhostSnapshot {
    pub key: W3DGhostSnapshotKey,
    pub parent_object_id: Option<u32>,
    pub drawable_info: W3DDrawableInfo,
    pub parent_geometry: Option<ParentGeometrySnapshot>,
    pub render_object: W3DRenderObjectSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrozenW3DGhostSceneEvent {
    RemoveParentObject {
        ghost_id: W3DGhostSceneId,
        parent_object_id: u32,
    },
    RestoreParentObject {
        ghost_id: W3DGhostSceneId,
        parent_object_id: u32,
    },
    UpsertSnapshot(FrozenW3DGhostSnapshot),
    RemoveSnapshot(W3DGhostSnapshotKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct W3DDrawableInfo {
    pub drawable_id: u32,
    pub flags: i32,
    pub shroud_status_object_id: u32,
}

impl Default for W3DDrawableInfo {
    fn default() -> Self {
        Self {
            drawable_id: INVALID_DRAWABLE_ID,
            flags: 0,
            shroud_status_object_id: INVALID_OBJECT_ID,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParentGeometrySnapshot {
    pub geometry_type: u32,
    pub is_small: bool,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub position: [f32; 3],
    pub angle: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct W3DGhostObject {
    scene_id: W3DGhostSceneId,
    parent_object_id: Option<u32>,
    partition_data_attached: bool,
    parent_snapshots: Vec<Vec<W3DRenderObjectSnapshot>>,
    drawable_info: W3DDrawableInfo,
    parent_geometry: Option<ParentGeometrySnapshot>,
    parent_fully_obscured: bool,
    scene_events: Vec<GhostSceneEvent>,
    previous_shroudedness: Vec<Option<u8>>,
}

impl Default for W3DGhostObject {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DGhostObject {
    pub fn new() -> Self {
        Self {
            scene_id: 0,
            parent_object_id: None,
            partition_data_attached: false,
            parent_snapshots: vec![Vec::new(); MAX_PLAYER_COUNT],
            drawable_info: W3DDrawableInfo::default(),
            parent_geometry: None,
            parent_fully_obscured: false,
            scene_events: Vec::new(),
            previous_shroudedness: vec![None; MAX_PLAYER_COUNT],
        }
    }

    pub fn scene_id(&self) -> W3DGhostSceneId {
        self.scene_id
    }

    pub fn update_parent_object(&mut self, object_id: Option<u32>, has_partition_data: bool) {
        self.parent_object_id = object_id;
        self.partition_data_attached = has_partition_data;
    }

    pub fn parent_object_id(&self) -> Option<u32> {
        self.parent_object_id
    }

    pub fn drawable_info(&self) -> W3DDrawableInfo {
        self.drawable_info
    }

    pub fn set_drawable_info(&mut self, info: W3DDrawableInfo) {
        self.drawable_info = info;
    }

    pub fn snapshots(&self, player_index: usize) -> &[W3DRenderObjectSnapshot] {
        self.parent_snapshots
            .get(player_index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn scene_events(&self) -> &[GhostSceneEvent] {
        &self.scene_events
    }

    fn drain_frozen_scene_events(&mut self) -> Vec<FrozenW3DGhostSceneEvent> {
        let events = std::mem::take(&mut self.scene_events);
        events
            .into_iter()
            .filter_map(|event| match event {
                GhostSceneEvent::RemoveParentObject(parent_object_id) => {
                    Some(FrozenW3DGhostSceneEvent::RemoveParentObject {
                        ghost_id: self.scene_id,
                        parent_object_id,
                    })
                }
                GhostSceneEvent::RestoreParentObject(parent_object_id) => {
                    Some(FrozenW3DGhostSceneEvent::RestoreParentObject {
                        ghost_id: self.scene_id,
                        parent_object_id,
                    })
                }
                GhostSceneEvent::AddSnapshot {
                    player_index,
                    snapshot,
                } => self
                    .parent_snapshots
                    .get(player_index)
                    .and_then(|snapshots| snapshots.get(snapshot))
                    .cloned()
                    .map(|render_object| {
                        FrozenW3DGhostSceneEvent::UpsertSnapshot(FrozenW3DGhostSnapshot {
                            key: W3DGhostSnapshotKey {
                                ghost_id: self.scene_id,
                                player_index,
                                snapshot_index: snapshot,
                            },
                            parent_object_id: self.parent_object_id,
                            drawable_info: self.drawable_info,
                            parent_geometry: self.parent_geometry,
                            render_object,
                        })
                    }),
                GhostSceneEvent::RemoveSnapshot {
                    player_index,
                    snapshot,
                } => Some(FrozenW3DGhostSceneEvent::RemoveSnapshot(
                    W3DGhostSnapshotKey {
                        ghost_id: self.scene_id,
                        player_index,
                        snapshot_index: snapshot,
                    },
                )),
            })
            .collect()
    }

    pub fn parent_fully_obscured(&self) -> bool {
        self.parent_fully_obscured
    }

    pub fn parent_geometry(&self) -> Option<ParentGeometrySnapshot> {
        self.parent_geometry
    }

    pub fn previous_shroudedness(&self, player_index: usize) -> Option<u8> {
        self.previous_shroudedness
            .get(player_index)
            .copied()
            .flatten()
    }

    /// C++ `snapShot`; retail builds only snapshot the local player.
    pub fn snapshot(
        &mut self,
        player_index: usize,
        local_player_index: usize,
        drawable_effectively_hidden: bool,
        render_objects: &[RenderObjectState],
        geometry: ParentGeometrySnapshot,
    ) {
        if player_index != local_player_index || drawable_effectively_hidden {
            return;
        }
        if player_index >= MAX_PLAYER_COUNT {
            return;
        }

        let snapshots = &mut self.parent_snapshots[player_index];
        for (idx, render_object) in render_objects.iter().cloned().enumerate() {
            if let Some(snapshot) = snapshots.get_mut(idx) {
                snapshot.update(render_object);
            } else {
                snapshots.push(W3DRenderObjectSnapshot::new(render_object));
            }
            self.scene_events.push(GhostSceneEvent::RemoveParentObject(
                self.parent_object_id.unwrap_or(INVALID_OBJECT_ID),
            ));
            self.scene_events.push(GhostSceneEvent::AddSnapshot {
                player_index,
                snapshot: idx,
            });
        }

        if !render_objects.is_empty() {
            self.parent_geometry = Some(geometry);
        }
    }

    pub fn remove_parent_object(&mut self) {
        let Some(parent) = self.parent_object_id else {
            return;
        };
        self.parent_fully_obscured = true;
        self.scene_events
            .push(GhostSceneEvent::RemoveParentObject(parent));
    }

    pub fn restore_parent_object(&mut self) {
        let Some(parent) = self.parent_object_id else {
            return;
        };
        self.parent_fully_obscured = false;
        self.scene_events
            .push(GhostSceneEvent::RestoreParentObject(parent));
    }

    pub fn add_to_scene(&mut self, player_index: usize) {
        for i in 0..self.snapshots(player_index).len() {
            self.scene_events.push(GhostSceneEvent::AddSnapshot {
                player_index,
                snapshot: i,
            });
        }
    }

    pub fn remove_from_scene(&mut self, player_index: usize) {
        for i in 0..self.snapshots(player_index).len() {
            self.scene_events.push(GhostSceneEvent::RemoveSnapshot {
                player_index,
                snapshot: i,
            });
        }
    }

    pub fn free_snapshot(&mut self, player_index: usize, local_player_index: usize) {
        if player_index != local_player_index || player_index >= MAX_PLAYER_COUNT {
            return;
        }
        if self.parent_snapshots[player_index].is_empty() {
            return;
        }
        self.remove_from_scene(player_index);
        if self.parent_object_id.is_some() {
            self.restore_parent_object();
        }
        self.parent_snapshots[player_index].clear();
    }

    pub fn free_all_snapshots(&mut self, local_player_index: usize) {
        if local_player_index >= MAX_PLAYER_COUNT {
            return;
        }
        if !self.parent_snapshots[local_player_index].is_empty() {
            self.remove_from_scene(local_player_index);
            if self.parent_object_id.is_some() {
                self.restore_parent_object();
            }
            self.parent_snapshots[local_player_index].clear();
        }
    }

    pub fn release(&mut self) {
        for snapshots in &mut self.parent_snapshots {
            snapshots.clear();
        }
        self.parent_object_id = None;
        self.partition_data_attached = false;
        self.parent_geometry = None;
        self.parent_fully_obscured = false;
        self.previous_shroudedness.fill(None);
    }

    pub fn set_previous_shroudedness(&mut self, player_index: usize, status: u8) {
        if let Some(slot) = self.previous_shroudedness.get_mut(player_index) {
            *slot = Some(status);
        }
    }

    pub fn has_snapshot(&self, player_index: usize) -> bool {
        self.parent_snapshots
            .get(player_index)
            .is_some_and(|snapshots| !snapshots.is_empty())
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct W3DGhostObjectManager {
    free_modules: Vec<W3DGhostObject>,
    used_modules: Vec<W3DGhostObject>,
    local_player: usize,
    lock_ghost_objects: bool,
    save_lock_ghost_objects: bool,
    next_scene_id: W3DGhostSceneId,
    pending_scene_events: Vec<FrozenW3DGhostSceneEvent>,
}

impl W3DGhostObjectManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn local_player_index(&self) -> usize {
        self.local_player
    }

    pub fn used_count(&self) -> usize {
        self.used_modules.len()
    }

    pub fn free_count(&self) -> usize {
        self.free_modules.len()
    }

    pub fn used(&self) -> &[W3DGhostObject] {
        &self.used_modules
    }

    pub fn used_mut(&mut self) -> &mut [W3DGhostObject] {
        &mut self.used_modules
    }

    pub fn set_lock_ghost_objects(&mut self, locked: bool) {
        self.lock_ghost_objects = locked;
    }

    pub fn set_save_lock_ghost_objects(&mut self, locked: bool) {
        self.save_lock_ghost_objects = locked;
    }

    pub fn add_ghost_object(
        &mut self,
        object_id: Option<u32>,
        has_partition_data: bool,
    ) -> Option<usize> {
        if self.lock_ghost_objects || self.save_lock_ghost_objects {
            return None;
        }
        let mut ghost = self.free_modules.pop().unwrap_or_else(W3DGhostObject::new);
        ghost.release();
        self.next_scene_id = self.next_scene_id.wrapping_add(1).max(1);
        ghost.scene_id = self.next_scene_id;
        ghost.parent_object_id = object_id;
        ghost.partition_data_attached = has_partition_data;
        ghost.drawable_info = W3DDrawableInfo {
            drawable_id: INVALID_DRAWABLE_ID,
            flags: 0,
            shroud_status_object_id: INVALID_OBJECT_ID,
        };
        self.used_modules.insert(0, ghost);
        Some(0)
    }

    pub fn remove_ghost_object(&mut self, index: usize) {
        if index >= self.used_modules.len() {
            return;
        }
        let mut ghost = self.used_modules.remove(index);
        ghost.free_all_snapshots(self.local_player);
        self.pending_scene_events
            .extend(ghost.drain_frozen_scene_events());
        ghost.release();
        self.free_modules.push(ghost);
    }

    /// Consume the scene mutations produced since the previous frame freeze.
    /// Returned values own every string/vector and cannot alias later logic
    /// updates or free-list reuse.
    pub fn drain_scene_events(&mut self) -> Vec<FrozenW3DGhostSceneEvent> {
        let mut result = std::mem::take(&mut self.pending_scene_events);
        for ghost in &mut self.used_modules {
            result.extend(ghost.drain_frozen_scene_events());
        }
        result
    }

    pub fn reset(&mut self) {
        let mut i = 0;
        while i < self.used_modules.len() {
            if self.used_modules[i].parent_object_id.is_none() {
                self.remove_ghost_object(i);
            } else {
                i += 1;
            }
        }
    }

    /// C++ `setLocalPlayerIndex` scene replacement rules.
    pub fn set_local_player_index(&mut self, index: usize) {
        let old = self.local_player;
        for ghost in &mut self.used_modules {
            ghost.remove_from_scene(old);
            if ghost.has_snapshot(index) {
                if !ghost.has_snapshot(old) && ghost.parent_object_id.is_some() {
                    ghost.remove_parent_object();
                }
                ghost.add_to_scene(index);
            } else if ghost.has_snapshot(old) && ghost.parent_object_id.is_some() {
                ghost.restore_parent_object();
            }
        }
        self.local_player = index;
    }

    pub fn update_orphaned_objects(&mut self, player_index_list: &[usize]) {
        let mut i = 0;
        while i < self.used_modules.len() {
            if self.used_modules[i].parent_object_id.is_some() {
                i += 1;
                continue;
            }
            let mut stored = usize::from(self.used_modules[i].has_snapshot(self.local_player));
            for &player in player_index_list {
                stored += usize::from(self.used_modules[i].has_snapshot(player));
            }
            if stored == 0 {
                self.remove_ghost_object(i);
            } else {
                i += 1;
            }
        }
    }

    pub fn release_partition_data(&mut self) {
        for ghost in &mut self.used_modules {
            ghost.partition_data_attached = false;
        }
    }

    pub fn restore_partition_data(&mut self) {
        for ghost in &mut self.used_modules {
            ghost.partition_data_attached = true;
            for player in 0..MAX_PLAYER_COUNT {
                if ghost.has_snapshot(player) {
                    ghost.set_previous_shroudedness(player, OBJECTSHROUD_FOGGED);
                }
            }
        }
    }
}

pub const OBJECTSHROUD_FOGGED: u8 = 2;

/// Active non-network W3D ghost scene owner used by the runtime hooks and Main
/// render ingress. Keeping its scene queue separate from the generic
/// Snapshotable manager preserves the C++ render payload without downcasts.
pub static THE_W3D_GHOST_OBJECT_MANAGER: Lazy<Arc<RwLock<W3DGhostObjectManager>>> =
    Lazy::new(|| Arc::new(RwLock::new(W3DGhostObjectManager::new())));
