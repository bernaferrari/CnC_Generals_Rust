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
    register_pristine_bone_lookup_hook, register_terrain_decal_client,
    register_terrain_track_client, register_texture_aspect_hook,
};
use glam::{Mat4, Vec3};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, Once};
use ww3d_assets::prototypes::HlodPrototype;

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
}

impl TerrainDecalClient for ProjectedDecalClient {
    fn set_decal(&self, desc: &TerrainDecalDesc) {
        if desc.hidden || desc.texture_name.is_empty() || desc.size_x <= 0.0 || desc.size_y <= 0.0 {
            self.release(desc.object_id);
            return;
        }
        let info = ShadowTypeInfo {
            allow_updates: false,
            allow_world_align: true,
            shadow_type: if desc.is_unit_blob {
                gamelogic::common::SHADOW_DECAL
            } else {
                gamelogic::common::SHADOW_ALPHA_DECAL
            },
            shadow_name: gamelogic::common::AsciiString::from(desc.texture_name.as_str()),
            size_x: desc.size_x,
            size_y: desc.size_y,
        };
        let mut manager = get_projected_shadow_manager().write();
        let Some(handle) = (if desc.is_unit_blob {
            manager.add_shadow(&info)
        } else {
            manager.add_decal(&info)
        }) else {
            return;
        };
        drop(manager);

        handle.set_position(desc.position.x, desc.position.y, desc.position.z);
        handle.set_angle(desc.angle);
        handle.set_opacity((desc.opacity.clamp(0.0, 1.0) * 255.0) as i32);
        if let Some(prev) = self.handles.lock().insert(desc.object_id, handle) {
            prev.release();
        }
    }

    fn set_size(&self, object_id: ObjectID, x: Real, y: Real) {
        // ShadowHandle has no set_size; recreate from the last pose if we have a handle.
        let Some(prev) = self.handles.lock().get(&object_id).cloned() else {
            return;
        };
        let _ = (x, y, prev);
    }

    fn set_opacity(&self, object_id: ObjectID, opacity: Real) {
        if let Some(handle) = self.handles.lock().get(&object_id) {
            handle.set_opacity((opacity.clamp(0.0, 1.0) * 255.0) as i32);
        }
    }

    fn set_pose(&self, object_id: ObjectID, position: Coord3D, angle: Real) {
        if let Some(handle) = self.handles.lock().get(&object_id) {
            handle.set_position(position.x, position.y, position.z);
            handle.set_angle(angle);
        }
    }

    fn set_shrouded(&self, object_id: ObjectID, shrouded: bool) {
        if shrouded {
            if let Some(handle) = self.handles.lock().get(&object_id) {
                handle.set_opacity(0);
            }
        }
    }

    fn set_shadow_enabled(&self, object_id: ObjectID, enabled: bool) {
        if !enabled {
            if let Some(handle) = self.handles.lock().get(&object_id) {
                handle.set_opacity(0);
            }
        }
    }

    fn release(&self, object_id: ObjectID) {
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

fn preload_asset(name: &str) {
    log::debug!("W3DModelDraw preload_assets: {name}");
}

/// Install the live GameClient adapters. Safe to call more than once.
pub fn ensure_logic_draw_hooks() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        register_terrain_decal_client(Arc::new(ProjectedDecalClient {
            handles: Mutex::new(HashMap::new()),
        }));
        register_terrain_track_client(Arc::new(TrackClient {
            by_object: Mutex::new(HashMap::new()),
        }));
        register_texture_aspect_hook(texture_aspect);
        register_preload_asset_hook(preload_asset);
        register_pristine_bone_lookup_hook(Some(Arc::new(lookup_pristine_bone)));
        let _ = TheGameLogic::get_frame();
    });
}
