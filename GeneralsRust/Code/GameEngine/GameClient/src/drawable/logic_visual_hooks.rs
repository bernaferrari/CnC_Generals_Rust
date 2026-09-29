//! Live GameClient implementations of GameLogic draw-module visual hooks.
//!
//! C++ W3DModelDraw talks to `TheProjectedShadowManager` and
//! `TheTerrainTracksRenderObjClassSystem` directly. Those live in this crate,
//! so the logic-side draw modules call through these registered adapters.

use crate::radius_decal::{ShadowHandle, ShadowTypeInfo, get_projected_shadow_manager};
use crate::render_bridge::THE_RENDER_BRIDGE;
use crate::terrain::TerrainVisual;
use crate::terrain::terrain_tracks::TerrainTrackHeightProvider;
use crate::terrain::terrain_visual::THE_TERRAIN_VISUAL;
use gamelogic::common::{Coord3D, Matrix3D, ObjectID, Real};
use gamelogic::helpers::TheGameLogic;
use gamelogic::object::draw::{
    TerrainDecalClient, TerrainDecalDesc, TerrainTrackClient, register_preload_asset_hook,
    register_pristine_bone_lookup_hook, register_sub_object_name_hook, register_model_bounds_hook,
    register_terrain_decal_client,
    register_terrain_track_client, register_texture_aspect_hook,
};
use glam::{Mat4, Vec3};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Once};
use ww3d_assets::prototypes::{BoxPrototype, HlodPrototype};

struct TerrainHeight;
impl TerrainTrackHeightProvider for TerrainHeight {
    fn ground_height_and_normal(&self, x: f32, y: f32) -> (f32, Vec3) {
        if let Ok(guard) = THE_TERRAIN_VISUAL.lock() {
            if let Some(terrain) = guard.as_ref() {
                if let Ok(h) = terrain.get_height_at(x, y) {
                    return (h, Vec3::Z);
                }
            }
        }
        (0.0, Vec3::Z)
    }
}

struct ProjectedDecalClient {
    handles: Mutex<HashMap<ObjectID, ShadowHandle>>,
    /// C++ `m_shadow`, not `m_terrainDecal`.
    blobs: Mutex<HashMap<ObjectID, ShadowHandle>>,
    /// Last decal opacity (0-255) per object; the source of truth for what a
    /// visible decal shows (the drawable fades `decal_opacity` independently).
    opacities: Mutex<HashMap<ObjectID, i32>>,
    /// Objects whose decal is hidden because it is fully obscured by shroud.
    shrouded: Mutex<HashSet<ObjectID>>,
    /// Objects with shadow render disabled (C++ `enableShadowRender(false)`).
    shadow_disabled: Mutex<HashSet<ObjectID>>,
    /// Blob `enableShadowRender(false)`. Independent of the decal set and of shroud.
    blob_render_off: Mutex<HashSet<ObjectID>>,
}

impl ProjectedDecalClient {
    /// Push the tracked decal opacity to the handle, gated by shroud and
    /// shadow-render visibility. C++ fades `m_decalOpacity` regardless; shroud
    /// culling and `enableShadowRender` only gate whether the decal renders.
    fn sync_opacity(&self, object_id: ObjectID) {
        let handles = self.handles.lock();
        let Some(handle) = handles.get(&object_id) else {
            return;
        };
        let opacity = if self.shrouded.lock().contains(&object_id)
            || self.shadow_disabled.lock().contains(&object_id)
        {
            0
        } else {
            self.opacities.lock().get(&object_id).copied().unwrap_or(0)
        };
        handle.set_opacity(opacity);
    }

    fn sync_blob_opacity(&self, object_id: ObjectID) {
        let blobs = self.blobs.lock();
        let Some(handle) = blobs.get(&object_id) else {
            return;
        };
        let hidden = self.shrouded.lock().contains(&object_id)
            || self.blob_render_off.lock().contains(&object_id);
        handle.set_opacity(if hidden { 0 } else { 255 });
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
        self.opacities
            .lock()
            .insert(desc.object_id, (desc.opacity.clamp(0.0, 1.0) * 255.0) as i32);
        if desc.shrouded {
            self.shrouded.lock().insert(desc.object_id);
        } else {
            self.shrouded.lock().remove(&desc.object_id);
        }
        if desc.shadow_enabled {
            self.shadow_disabled.lock().remove(&desc.object_id);
        } else {
            self.shadow_disabled.lock().insert(desc.object_id);
        }
        if let Some(prev) = self.handles.lock().insert(desc.object_id, handle) {
            prev.release();
        }
        self.sync_opacity(desc.object_id);
    }

    fn set_size(&self, object_id: ObjectID, x: Real, y: Real) {
        if let Some(handle) = self.handles.lock().get(&object_id) {
            handle.set_size(x, y);
        }
    }

    fn set_opacity(&self, object_id: ObjectID, opacity: Real) {
        self.opacities
            .lock()
            .insert(object_id, (opacity.clamp(0.0, 1.0) * 255.0) as i32);
        self.sync_opacity(object_id);
    }

    fn set_pose(&self, object_id: ObjectID, position: Coord3D, angle: Real) {
        if let Some(handle) = self.handles.lock().get(&object_id) {
            handle.set_position(position.x, position.y, position.z);
            handle.set_angle(angle);
        }
        if let Some(handle) = self.blobs.lock().get(&object_id) {
            handle.set_position(position.x, position.y, position.z);
            handle.set_angle(angle);
        }
    }

    fn set_shrouded(&self, object_id: ObjectID, shrouded: bool) {
        if shrouded {
            self.shrouded.lock().insert(object_id);
        } else {
            self.shrouded.lock().remove(&object_id);
        }
        self.sync_opacity(object_id);
        self.sync_blob_opacity(object_id);
    }

    fn set_shadow_enabled(&self, object_id: ObjectID, enabled: bool) {
        if enabled {
            self.shadow_disabled.lock().remove(&object_id);
        } else {
            self.shadow_disabled.lock().insert(object_id);
        }
        self.sync_opacity(object_id);
        self.set_blob_render(object_id, enabled);
    }

    fn set_blob_render(&self, object_id: ObjectID, enabled: bool) {
        if enabled {
            self.blob_render_off.lock().remove(&object_id);
        } else {
            self.blob_render_off.lock().insert(object_id);
        }
        self.sync_blob_opacity(object_id);
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
        if let Some(prev) = self.blobs.lock().insert(desc.object_id, handle) {
            prev.release();
        }
        if desc.shrouded {
            self.shrouded.lock().insert(desc.object_id);
        }
        self.set_blob_render(desc.object_id, desc.shadow_enabled && !desc.hidden);
    }

    fn release_unit_shadow(&self, object_id: ObjectID) {
        if let Some(handle) = self.blobs.lock().remove(&object_id) {
            handle.release();
        }
    }

    fn release(&self, object_id: ObjectID) {
        self.opacities.lock().remove(&object_id);
        self.shrouded.lock().remove(&object_id);
        self.shadow_disabled.lock().remove(&object_id);
        if let Some(handle) = self.handles.lock().remove(&object_id) {
            handle.release();
        }
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
                terrain.terrain_tracks_mut().add_edge_to_track(
                    handle as usize,
                    &TerrainHeight,
                    x,
                    y,
                    sync_time as i32,
                );
            }
        }
    }

    fn add_cap(&self, handle: u32, x: Real, y: Real, sync_time: u32) {
        if let Ok(mut visual) = THE_TERRAIN_VISUAL.lock() {
            if let Some(terrain) = visual.as_mut() {
                terrain.terrain_tracks_mut().add_cap_edge_to_track(
                    handle as usize,
                    &TerrainHeight,
                    x,
                    y,
                    sync_time as i32,
                );
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
    let Some(hlod) = bridge.asset_manager().get_prototype_as::<HlodPrototype>(model) else {
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
        .and_then(|hierarchy| hierarchy.bind_transforms.get(child.bone_index as usize).copied())
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
        register_terrain_decal_client(Arc::new(ProjectedDecalClient {
            handles: Mutex::new(HashMap::new()),
            blobs: Mutex::new(HashMap::new()),
            opacities: Mutex::new(HashMap::new()),
            shrouded: Mutex::new(HashSet::new()),
            shadow_disabled: Mutex::new(HashSet::new()),
            blob_render_off: Mutex::new(HashSet::new()),
        }));
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
