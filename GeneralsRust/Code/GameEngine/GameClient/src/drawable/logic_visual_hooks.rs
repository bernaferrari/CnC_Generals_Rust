//! Live GameClient implementations of GameLogic draw-module visual hooks.
//!
//! C++ W3DModelDraw talks to `TheProjectedShadowManager` and
//! `TheTerrainTracksRenderObjClassSystem` directly. Those live in this crate,
//! so the logic-side draw modules call through these registered adapters.

use crate::radius_decal::{ShadowHandle, ShadowTypeInfo, get_projected_shadow_manager};
use crate::render_bridge::THE_RENDER_BRIDGE;
use crate::terrain::terrain_visual::THE_TERRAIN_VISUAL;
use gamelogic::common::{Coord3D, Matrix3D, ObjectID, Real};
use gamelogic::helpers::TheGameLogic;
use gamelogic::object::draw::{
    TerrainDecalClient, TerrainDecalDesc, TerrainTrackClient, register_model_bounds_hook,
    register_preload_asset_hook, register_pristine_bone_lookup_hook, register_sub_object_name_hook,
    register_terrain_decal_client, register_terrain_track_client, register_texture_aspect_hook,
};
use glam::{Mat4, Vec3};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, Once};
use ww3d_assets::prototypes::{BoxPrototype, HlodPrototype};

#[derive(Default)]
struct ProjectedDecalState {
    /// C++ `m_terrainDecal` visual resource.
    decal: Option<ShadowHandle>,
    /// C++ `m_shadow`, not `m_terrainDecal`.
    blob: Option<ShadowHandle>,
    /// Last decal opacity (0-255); drawable fade state remains elsewhere.
    opacity: i32,
    /// Objects whose decals and blobs are hidden by shroud.
    shrouded: bool,
    /// C++ `enableShadowRender(false)` for the terrain decal.
    shadow_disabled: bool,
    /// Blob `enableShadowRender(false)`, independent of the decal.
    blob_render_off: bool,
}

#[derive(Default)]
struct ProjectedDecalClient {
    /// Decal and blob lifecycle is keyed by the owning ObjectID. Keeping the
    /// related state together makes visibility updates one atomic bookkeeping
    /// operation. Lock order is state -> ShadowHandle's private decal mutex;
    /// ShadowHandle setters/release are closed field writes with no callbacks.
    state: Mutex<HashMap<ObjectID, ProjectedDecalState>>,
}

impl ProjectedDecalClient {
    fn install_decal(
        &self,
        object_id: ObjectID,
        handle: ShadowHandle,
        opacity: i32,
        shrouded: bool,
        shadow_enabled: bool,
    ) {
        let mut state = self.state.lock();
        let entry = state.entry(object_id).or_default();
        entry.opacity = opacity;
        entry.shrouded = shrouded;
        entry.shadow_disabled = !shadow_enabled;
        let opacity = if entry.shrouded || entry.shadow_disabled {
            0
        } else {
            entry.opacity
        };
        let previous = entry.decal.replace(handle);
        if let Some(previous) = previous {
            previous.release();
        }
        entry
            .decal
            .as_ref()
            .expect("just installed decal")
            .set_opacity(opacity);
    }

    fn install_blob(&self, object_id: ObjectID, handle: ShadowHandle, shrouded: bool) {
        let mut state = self.state.lock();
        let entry = state.entry(object_id).or_default();
        if shrouded {
            entry.shrouded = true;
        }
        let previous = entry.blob.replace(handle);
        if let Some(previous) = previous {
            previous.release();
        }
    }

    fn prune_empty_entry(state: &mut HashMap<ObjectID, ProjectedDecalState>, object_id: ObjectID) {
        let empty = state.get(&object_id).is_some_and(|entry| {
            entry.decal.is_none()
                && entry.blob.is_none()
                && entry.opacity == 0
                && !entry.shrouded
                && !entry.shadow_disabled
                && !entry.blob_render_off
        });
        if empty {
            state.remove(&object_id);
        }
    }
}

impl TerrainDecalClient for ProjectedDecalClient {
    fn set_decal(&self, desc: &TerrainDecalDesc) {
        if desc.texture_name.is_empty() || desc.size_x <= 0.0 || desc.size_y <= 0.0 {
            self.release(desc.object_id);
            return;
        }
        let info = ShadowTypeInfo {
            allow_updates: false,
            allow_world_align: true,
            shadow_type: gamelogic::common::SHADOW_ALPHA_DECAL,
            shadow_name: gamelogic::common::AsciiString::from(desc.texture_name.as_str()),
            size_x: desc.size_x,
            size_y: desc.size_y,
            offset_x: desc.offset_x,
            offset_y: desc.offset_y,
        };
        let mut manager = get_projected_shadow_manager().write();
        let Some(handle) = manager.add_decal(&info) else {
            return;
        };
        drop(manager);

        handle.set_position(desc.position.x, desc.position.y, desc.position.z);
        handle.set_angle(desc.angle);
        self.install_decal(
            desc.object_id,
            handle,
            (desc.opacity.clamp(0.0, 1.0) * 255.0) as i32,
            desc.shrouded,
            desc.shadow_enabled,
        );
    }

    fn set_size(&self, object_id: ObjectID, x: Real, y: Real) {
        let state = self.state.lock();
        if let Some(handle) = state.get(&object_id).and_then(|entry| entry.decal.as_ref()) {
            handle.set_size(x, y);
        }
    }

    fn set_opacity(&self, object_id: ObjectID, opacity: Real) {
        let mut state = self.state.lock();
        let entry = state.entry(object_id).or_default();
        entry.opacity = (opacity.clamp(0.0, 1.0) * 255.0) as i32;
        if let Some(handle) = &entry.decal {
            handle.set_opacity(if entry.shrouded || entry.shadow_disabled {
                0
            } else {
                entry.opacity
            });
        }
    }

    fn set_pose(&self, object_id: ObjectID, position: Coord3D, angle: Real) {
        let state = self.state.lock();
        let Some(entry) = state.get(&object_id) else {
            return;
        };
        if let Some(handle) = &entry.decal {
            handle.set_position(position.x, position.y, position.z);
            handle.set_angle(angle);
        }
        if let Some(handle) = &entry.blob {
            handle.set_position(position.x, position.y, position.z);
            handle.set_angle(angle);
        }
    }

    fn set_shrouded(&self, object_id: ObjectID, shrouded: bool) {
        let mut state = self.state.lock();
        let entry = state.entry(object_id).or_default();
        entry.shrouded = shrouded;
        if let Some(handle) = &entry.decal {
            handle.set_opacity(if entry.shadow_disabled || shrouded {
                0
            } else {
                entry.opacity
            });
        }
        if let Some(handle) = &entry.blob {
            handle.set_opacity(if shrouded || entry.blob_render_off {
                0
            } else {
                255
            });
        }
    }

    fn set_shadow_enabled(&self, object_id: ObjectID, enabled: bool) {
        let mut state = self.state.lock();
        let entry = state.entry(object_id).or_default();
        entry.shadow_disabled = !enabled;
        entry.blob_render_off = !enabled;
        if let Some(handle) = &entry.decal {
            handle.set_opacity(if entry.shrouded || !enabled {
                0
            } else {
                entry.opacity
            });
        }
        if let Some(handle) = &entry.blob {
            handle.set_opacity(if entry.shrouded || !enabled { 0 } else { 255 });
        }
    }

    fn set_blob_render(&self, object_id: ObjectID, enabled: bool) {
        let mut state = self.state.lock();
        let entry = state.entry(object_id).or_default();
        entry.blob_render_off = !enabled;
        if let Some(handle) = &entry.blob {
            handle.set_opacity(if entry.shrouded || !enabled { 0 } else { 255 });
        }
    }

    fn add_unit_shadow(&self, desc: &TerrainDecalDesc) {
        // `addShadow`: `shadow.tga` only when the type is `SHADOW_DECAL` and
        // the name is empty or one character. A zero size is the render-object
        // box (`Extent * 2`), not `createDecalShadow`'s width of 20. No mesh
        // box is available, so a zero size is left at zero.
        const SHADOW_DECAL: u32 = 0x0000_0001;
        let shadow_name = if desc.shadow_type == SHADOW_DECAL && desc.texture_name.len() <= 1 {
            "shadow"
        } else {
            desc.texture_name.as_str()
        };
        let info = ShadowTypeInfo {
            allow_updates: false,
            allow_world_align: true,
            shadow_type: desc.shadow_type,
            shadow_name: gamelogic::common::AsciiString::from(shadow_name),
            size_x: desc.size_x,
            size_y: desc.size_y,
            offset_x: desc.offset_x,
            offset_y: desc.offset_y,
        };
        let mut manager = get_projected_shadow_manager().write();
        let Some(handle) = manager.add_shadow(&info) else {
            return;
        };
        drop(manager);
        handle.set_position(desc.position.x, desc.position.y, desc.position.z);
        handle.set_angle(desc.angle);
        self.install_blob(desc.object_id, handle, desc.shrouded);
        self.set_blob_render(desc.object_id, desc.shadow_enabled && !desc.hidden);
    }

    fn release_unit_shadow(&self, object_id: ObjectID) {
        let mut state = self.state.lock();
        if let Some(entry) = state.get_mut(&object_id) {
            if let Some(handle) = entry.blob.take() {
                handle.release();
            }
        }
        Self::prune_empty_entry(&mut state, object_id);
    }

    fn release(&self, object_id: ObjectID) {
        let mut state = self.state.lock();
        if let Some(entry) = state.get_mut(&object_id) {
            entry.opacity = 0;
            entry.shrouded = false;
            entry.shadow_disabled = false;
            if let Some(handle) = entry.decal.take() {
                handle.release();
            }
        }
        Self::prune_empty_entry(&mut state, object_id);
    }
}

struct TrackClient {
    by_object: Mutex<HashMap<ObjectID, u32>>,
}

impl TerrainTrackClient for TrackClient {
    fn bind_track(&self, object_id: ObjectID, width: Real, texture: &str) -> Option<u32> {
        let mut visual = THE_TERRAIN_VISUAL.lock().ok()?;
        let terrain = visual.as_mut()?;
        let handle = terrain
            .terrain_tracks_mut()
            .bind_track(width, width, texture)? as u32;
        self.by_object.lock().insert(object_id, handle);
        Some(handle)
    }

    fn unbind_track(&self, handle: u32) {
        if let Ok(mut visual) = THE_TERRAIN_VISUAL.lock() {
            if let Some(terrain) = visual.as_mut() {
                terrain.terrain_tracks_mut().unbind_track(handle as usize);
            }
        }
        self.by_object.lock().retain(|_, h| *h != handle);
    }

    fn add_edge(&self, handle: u32, x: Real, y: Real, sync_time: u32) {
        if let Ok(mut visual) = THE_TERRAIN_VISUAL.lock() {
            if let Some(terrain) = visual.as_mut() {
                terrain.add_track_edge(handle as usize, x, y, sync_time as i32);
            }
        }
    }

    fn add_cap(&self, handle: u32, x: Real, y: Real, sync_time: u32) {
        if let Ok(mut visual) = THE_TERRAIN_VISUAL.lock() {
            if let Some(terrain) = visual.as_mut() {
                terrain.add_track_cap(handle as usize, x, y, sync_time as i32);
            }
        }
    }

    fn set_airborne(&self, handle: u32) {
        if let Ok(mut visual) = THE_TERRAIN_VISUAL.lock() {
            if let Some(terrain) = visual.as_mut() {
                if let Some(track) = terrain.terrain_tracks_mut().track_mut(handle as usize) {
                    track.set_airborne();
                }
            }
        }
    }
}

fn texture_aspect(name: &str) -> Option<f32> {
    if name.is_empty() {
        return None;
    }
    let candidates = [
        name.to_string(),
        format!("{name}.tga"),
        format!("{name}.png"),
        format!("Art/Textures/{name}"),
        format!("Art/Textures/{name}.tga"),
        format!("Art/Textures/{name}.png"),
        format!("Data/English/Art/Textures/{name}.tga"),
    ];
    for path in candidates {
        if let Ok((w, h)) = image::image_dimensions(&path) {
            if h > 0 {
                return Some(w as f32 / h as f32);
            }
        }
    }
    None
}

fn pivot_name(bytes: &[u8; 16]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// C++ `Create_Render_Obj` + `Get_Bone_Index` / `Get_Bone_Transform` at identity×scale.
/// Used for both pristine cache fill and current-client bone queries when the
/// live HTree pose is the bind pose (or the only available W3D pose).
pub fn lookup_w3d_client_bone(
    model: &str,
    scale: Real,
    _frame: i32,
    bone: &str,
) -> Option<(i32, Matrix3D)> {
    let guard = THE_RENDER_BRIDGE.lock().ok()?;
    let bridge = guard.as_ref()?;
    let assets = bridge.asset_manager();
    let hierarchy = assets.get_hierarchy_prototype(model).or_else(|| {
        assets
            .get_prototype_as::<HlodPrototype>(model)
            .and_then(|hlod| assets.get_hierarchy_prototype(&hlod.hierarchy_name))
    })?;
    let idx = hierarchy
        .pivots
        .iter()
        .position(|pivot| pivot_name(&pivot.name).eq_ignore_ascii_case(bone))?;
    let mut mtx = hierarchy
        .bind_transforms
        .get(idx)
        .copied()
        .unwrap_or(Mat4::IDENTITY);
    if scale.is_finite() && (scale - 1.0).abs() > f32::EPSILON {
        mtx = Mat4::from_scale(Vec3::splat(scale)) * mtx;
    }
    Some((idx as i32, mtx))
}

fn lookup_pristine_bone(
    model: &str,
    scale: Real,
    frame: i32,
    bone: &str,
) -> Option<(i32, Matrix3D)> {
    lookup_w3d_client_bone(model, scale, frame, bone)
}

fn lookup_sub_object_names(model: &str) -> Vec<String> {
    let Ok(guard) = THE_RENDER_BRIDGE.lock() else {
        return Vec::new();
    };
    let Some(bridge) = guard.as_ref() else {
        return Vec::new();
    };
    let Some(hlod) = bridge
        .asset_manager()
        .get_prototype_as::<HlodPrototype>(model)
    else {
        return Vec::new();
    };
    hlod.lods
        .iter()
        .flat_map(|lod| lod.models.iter().map(|child| child.name.clone()))
        .collect()
}

fn lookup_model_obj_bounds(model: &str) -> Option<([f32; 3], [f32; 3])> {
    let guard = THE_RENDER_BRIDGE.lock().ok()?;
    let bridge = guard.as_ref()?;
    let assets = bridge.asset_manager();
    let Some(proto) = assets.get_prototype_as::<HlodPrototype>(model) else {
        return Some(([0.0; 3], [0.0; 3]));
    };
    let Some(lod) = proto.lods.last() else {
        return Some(([0.0; 3], [0.0; 3]));
    };
    let Some(child) = lod.models.iter().find(|child| {
        child
            .name
            .rsplit('.')
            .next()
            .unwrap_or(child.name.as_str())
            .eq_ignore_ascii_case("BOUNDINGBOX")
    }) else {
        return Some(([0.0; 3], [0.0; 3]));
    };
    let Some(obbox) = assets.get_prototype_as::<BoxPrototype>(&child.name) else {
        return Some(([0.0; 3], [0.0; 3]));
    };
    let center = glam::Vec3::new(obbox.center.x, obbox.center.y, obbox.center.z);
    let extent = glam::Vec3::new(obbox.extent.x, obbox.extent.y, obbox.extent.z);
    let bind = assets
        .get_hierarchy_prototype(&proto.hierarchy_name)
        .and_then(|hierarchy| {
            hierarchy
                .bind_transforms
                .get(child.bone_index as usize)
                .copied()
        })
        .unwrap_or(glam::Mat4::IDENTITY);
    let placed = bind.transform_point3(center);
    let placed_extent = bind.x_axis.truncate().abs() * extent.x
        + bind.y_axis.truncate().abs() * extent.y
        + bind.z_axis.truncate().abs() * extent.z;
    Some((placed.to_array(), placed_extent.to_array()))
}

fn preload_asset(name: &str) {
    log::debug!("W3DModelDraw preload_assets: {name}");
}

/// Install the live GameClient adapters. Safe to call more than once.
pub fn ensure_logic_draw_hooks() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        register_terrain_decal_client(Arc::new(ProjectedDecalClient::default()));
        register_terrain_track_client(Arc::new(TrackClient {
            by_object: Mutex::new(HashMap::new()),
        }));
        register_texture_aspect_hook(texture_aspect);
        register_preload_asset_hook(preload_asset);
        register_pristine_bone_lookup_hook(Some(Arc::new(lookup_pristine_bone)));
        register_sub_object_name_hook(Some(Arc::new(lookup_sub_object_names)));
        register_model_bounds_hook(Some(Arc::new(lookup_model_obj_bounds)));
        let _ = TheGameLogic::get_frame();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::radius_decal::ProjectedShadowManager;
    use gamelogic::common::SHADOW_ALPHA_DECAL;

    fn decal(manager: &mut ProjectedShadowManager, name: &str) -> ShadowHandle {
        manager
            .add_decal(&ShadowTypeInfo {
                allow_updates: false,
                allow_world_align: true,
                shadow_type: SHADOW_ALPHA_DECAL,
                shadow_name: gamelogic::common::AsciiString::from(name),
                size_x: 10.0,
                size_y: 8.0,
                offset_x: 0.0,
                offset_y: 0.0,
            })
            .expect("valid test decal")
    }

    fn only_item(manager: &ProjectedShadowManager) -> crate::effects::decals::DecalRenderItem {
        let items = manager.collect_render_items();
        assert_eq!(items.len(), 1);
        items.into_iter().next().unwrap()
    }

    #[test]
    fn track_callbacks_sample_ground_without_relocking_the_terrain_owner() {
        use crate::terrain::terrain_tracks::{MAX_TRACK_EDGE_COUNT, TerrainTracksConfig};
        use crate::terrain::terrain_visual::TerrainVisualImpl;

        // This is the actual GameLogic -> GameClient adapter, not a mock height
        // provider. The old callback locks this owner again and never returns.
        let mut visual = TerrainVisualImpl::new();
        visual.set_terrain_tracks_detail_with_config(TerrainTracksConfig::default());
        let handle = visual
            .terrain_tracks_mut()
            .bind_track(4.0, 4.0, "track")
            .unwrap() as u32;
        let previous = THE_TERRAIN_VISUAL.lock().unwrap().replace(visual);
        struct RestoreTerrain(Option<TerrainVisualImpl>);
        impl Drop for RestoreTerrain {
            fn drop(&mut self) {
                *THE_TERRAIN_VISUAL.lock().unwrap() = self.0.take();
            }
        }
        let _restore = RestoreTerrain(previous);
        let client = TrackClient {
            by_object: Mutex::new(HashMap::new()),
        };
        client.add_edge(handle, 0.0, 0.0, 10);
        client.add_edge(handle, 20.0, 0.0, 20);
        client.add_edge(handle, 40.0, 0.0, 30);
        client.add_cap(handle, 60.0, 0.0, 40);
        let owner = THE_TERRAIN_VISUAL.lock().unwrap();
        let track = owner
            .as_ref()
            .unwrap()
            .terrain_tracks()
            .track(handle as usize)
            .unwrap();
        let edges = track.active_edges(MAX_TRACK_EDGE_COUNT);
        assert_eq!(edges.len(), 3);
        assert_eq!(
            edges.iter().map(|edge| edge.time_added).collect::<Vec<_>>(),
            [20, 30, 40]
        );
        assert!(track.have_cap());
        assert!(!track.have_anchor());
        assert_eq!(edges.last().unwrap().alpha, 0.0);
    }

    #[test]
    fn decal_state_updates_visibility_pose_and_releases_replaced_handles() {
        let object_id = 71;
        let mut manager = ProjectedShadowManager::new();
        let client = ProjectedDecalClient::default();
        let first = decal(&mut manager, "first");

        client.install_decal(object_id, first, 204, false, true);
        let item = only_item(&manager);
        assert_eq!(item.color[3], 204.0 / 255.0);

        client.set_size(object_id, 12.0, 9.0);
        client.set_opacity(object_id, 0.5);
        assert_eq!(only_item(&manager).size_x, 12.0);
        assert_eq!(only_item(&manager).color[3], 127.0 / 255.0);

        client.set_pose(object_id, Coord3D::new(3.0, 4.0, 5.0), 0.75);
        client.set_shrouded(object_id, true);
        assert!(manager.collect_render_items().is_empty());

        client.set_shrouded(object_id, false);
        client.set_shadow_enabled(object_id, false);
        assert!(manager.collect_render_items().is_empty());
        client.set_shadow_enabled(object_id, true);

        let item = only_item(&manager);
        assert_eq!(item.position, Vec3::new(3.0, 4.0, 5.0));
        assert_eq!(item.rotation, 0.75);

        let replacement = decal(&mut manager, "replacement");
        client.install_decal(object_id, replacement, 128, false, true);
        let item = only_item(&manager);
        assert_eq!(item.texture_name, "replacement");
        assert_eq!(item.color[3], 128.0 / 255.0);

        client.release(object_id);
        assert!(manager.collect_render_items().is_empty());
        assert!(!client.state.lock().contains_key(&object_id));
    }

    #[test]
    fn independent_clients_keep_same_object_id_handles_and_pose_separate() {
        let object_id = 9;
        let mut manager_a = ProjectedShadowManager::new();
        let mut manager_b = ProjectedShadowManager::new();
        let client_a = ProjectedDecalClient::default();
        let client_b = ProjectedDecalClient::default();

        client_a.install_decal(object_id, decal(&mut manager_a, "a"), 255, false, true);
        client_b.install_decal(object_id, decal(&mut manager_b, "b"), 255, false, true);
        client_a.set_pose(object_id, Coord3D::new(11.0, 12.0, 13.0), 0.25);

        let item_a = only_item(&manager_a);
        let item_b = only_item(&manager_b);
        assert_eq!(item_a.texture_name, "a");
        assert_eq!(item_a.position, Vec3::new(11.0, 12.0, 13.0));
        assert_eq!(item_b.texture_name, "b");
        assert_eq!(item_b.position, Vec3::ZERO);

        client_a.release(object_id);
        assert!(manager_a.collect_render_items().is_empty());
        assert!(!client_a.state.lock().contains_key(&object_id));
        assert_eq!(only_item(&manager_b).texture_name, "b");
    }

    #[test]
    fn teardown_keeps_pending_blob_disable_until_explicitly_cleared() {
        let object_id = 18;
        let client = ProjectedDecalClient::default();

        client.set_blob_render(object_id, false);
        client.release(object_id);
        assert!(client.state.lock()[&object_id].blob_render_off);

        client.set_blob_render(object_id, true);
        client.release_unit_shadow(object_id);
        assert!(!client.state.lock().contains_key(&object_id));
    }
}
