//! Live W3DDisplay shadow / occlusion pass.
//!
//! C++ `DoShadows` + `flushOccludedObjectsIntoStencil` + `renderStencilShadows`.
//! Projected unit decals are queued onto terrain; volumetric volumes write
//! stencil then a 0x7fa0a0a0 fill quad. Occluded units get a player-color pass.

use crate::drawable::drawable_manager::with_drawable_manager;
use crate::effects::decals::DecalRenderItem;
use crate::radius_decal::get_projected_shadow_manager;
use crate::terrain::terrain_visual::THE_TERRAIN_VISUAL;
use crate::terrain::TerrainVisual;

use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct UnitShadowCaster {
    pub position: [f32; 3],
    pub size_x: f32,
    pub size_y: f32,
    pub angle: f32,
    pub player_color: u32,
    pub occluded: bool,
    pub heat_vision: bool,
    pub volume: bool,
}

static UNIT_CASTERS: Mutex<Vec<UnitShadowCaster>> = Mutex::new(Vec::new());
static SHADOW_REBUILD_SERIAL: Mutex<u32> = Mutex::new(0);

/// C++ `W3DShadowManager::invalidateCachedLightPositions` / GameLOD hook.
pub fn rebuild_shadows() {
    if let Ok(mut serial) = SHADOW_REBUILD_SERIAL.lock() {
        *serial = serial.wrapping_add(1);
    }
    if let Ok(mut casters) = UNIT_CASTERS.lock() {
        casters.clear();
    }
}

pub fn shadow_rebuild_serial() -> u32 {
    SHADOW_REBUILD_SERIAL.lock().map(|g| *g).unwrap_or(0)
}

pub fn register_unit_shadow(caster: UnitShadowCaster) {
    if let Ok(mut list) = UNIT_CASTERS.lock() {
        list.push(caster);
    }
}

pub fn clear_unit_shadows() {
    if let Ok(mut list) = UNIT_CASTERS.lock() {
        list.clear();
    }
}

/// C++ `W3DProjectedShadowManager::flushDecals` / `queueDecal`.
/// Only allocated projected decals (`addDecal` / `addShadow`). C++ has no
/// per-drawable fallback blob — inventing one double-draws once real decals land.
pub fn collect_unit_decal_items() -> Vec<DecalRenderItem> {
    get_projected_shadow_manager()
        .read()
        .collect_render_items()
}


/// Collect occluded-unit player-color and heat-vision drawables.
pub fn collect_occlusion_overlays() -> Vec<OcclusionOverlay> {
    let mut out = Vec::new();
    with_drawable_manager(|manager| {
        for id in manager.get_all_drawable_ids() {
            let Some(drawable) = manager.get_drawable(id) else {
                continue;
            };
            if !drawable.is_visible() {
                continue;
            }
            let pos = drawable.get_position();
            out.push(OcclusionOverlay {
                position: [pos.x, pos.y, pos.z],
                color: [1.0, 0.35, 0.05, 0.0],
                kind: OverlayKind::HeatVision,
            });
        }
    });
    let casters = UNIT_CASTERS.lock().map(|g| g.clone()).unwrap_or_default();
    for caster in casters {
        if caster.occluded {
            let a = ((caster.player_color >> 24) & 0xff) as f32 / 255.0;
            let r = ((caster.player_color >> 16) & 0xff) as f32 / 255.0;
            let g = ((caster.player_color >> 8) & 0xff) as f32 / 255.0;
            let b = (caster.player_color & 0xff) as f32 / 255.0;
            out.push(OcclusionOverlay {
                position: caster.position,
                color: [r, g, b, a.max(0.55)],
                kind: OverlayKind::PlayerColor,
            });
        }
        if caster.heat_vision {
            out.push(OcclusionOverlay {
                position: caster.position,
                color: [1.0, 0.35, 0.05, 0.85],
                kind: OverlayKind::HeatVision,
            });
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    PlayerColor,
    HeatVision,
}

#[derive(Debug, Clone)]
pub struct OcclusionOverlay {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub kind: OverlayKind,
}

/// Terrain-and-sky AABB used by particle cull (C++ getMaximumVisibleBox(..., TRUE)).
#[derive(Debug, Clone, Copy)]
pub struct VisibleBox {
    pub center: [f32; 3],
    pub extent: [f32; 3],
}

impl VisibleBox {
    pub fn contains_expanded(&self, pos: [f32; 3], size: f32) -> bool {
        (pos[0] - self.center[0]).abs() <= self.extent[0] + size
            && (pos[1] - self.center[1]).abs() <= self.extent[1] + size
            && (pos[2] - self.center[2]).abs() <= self.extent[2] + size
    }
}

/// Clip frustum corners to the terrain min-height plane.
pub fn maximum_visible_box(
    camera: [f32; 3],
    target: [f32; 3],
    near_z: f32,
    far_z: f32,
    fov: f32,
    aspect: f32,
    min_height: f32,
) -> VisibleBox {
    let mut forward = [
        target[0] - camera[0],
        target[1] - camera[1],
        target[2] - camera[2],
    ];
    let fl = (forward[0] * forward[0] + forward[1] * forward[1] + forward[2] * forward[2]).sqrt();
    if fl > 1e-5 {
        forward[0] /= fl;
        forward[1] /= fl;
        forward[2] /= fl;
    } else {
        forward = [0.0, 1.0, 0.0];
    }
    let mut up = [0.0, 0.0, 1.0];
    if forward[2].abs() > 0.99 {
        up = [0.0, 1.0, 0.0];
    }
    let mut right = [
        forward[1] * up[2] - forward[2] * up[1],
        forward[2] * up[0] - forward[0] * up[2],
        forward[0] * up[1] - forward[1] * up[0],
    ];
    let rl = (right[0] * right[0] + right[1] * right[1] + right[2] * right[2]).sqrt().max(1e-5);
    right[0] /= rl;
    right[1] /= rl;
    right[2] /= rl;
    up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];

    let tan_half = (fov * 0.5).tan();
    let mut corners = [[0.0f32; 3]; 8];
    for (i, &dist) in [near_z, far_z].iter().enumerate() {
        let hh = dist * tan_half;
        let hw = hh * aspect;
        let c = [
            camera[0] + forward[0] * dist,
            camera[1] + forward[1] * dist,
            camera[2] + forward[2] * dist,
        ];
        let signs = [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];
        for (j, (sx, sy)) in signs.iter().enumerate() {
            corners[i * 4 + j] = [
                c[0] + right[0] * hw * sx + up[0] * hh * sy,
                c[1] + right[1] * hw * sx + up[1] * hh * sy,
                c[2] + right[2] * hw * sx + up[2] * hh * sy,
            ];
        }
    }
    // Clip far corners down to the ground plane (C++ ignoreMaxHeight = TRUE).
    for i in 0..4 {
        let a = corners[i];
        let b = corners[i + 4];
        let dz = b[2] - a[2];
        if dz.abs() > 1e-5 {
            let t = (min_height - a[2]) / dz;
            if (0.0..=1.0).contains(&t) {
                corners[i + 4] = [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    min_height,
                ];
            }
        }
    }
    let mut min = corners[0];
    let mut max = corners[0];
    for c in &corners {
        for k in 0..3 {
            min[k] = min[k].min(c[k]);
            max[k] = max[k].max(c[k]);
        }
    }
    VisibleBox {
        center: [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ],
        extent: [
            (max[0] - min[0]) * 0.5,
            (max[1] - min[1]) * 0.5,
            (max[2] - min[2]) * 0.5,
        ],
    }
}

pub fn terrain_min_height() -> f32 {
    if let Ok(guard) = THE_TERRAIN_VISUAL.lock() {
        if let Some(terrain) = guard.as_ref() {
            if let Ok(h) = terrain.get_height_at(0.0, 0.0) {
                return h.min(0.0);
            }
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_box_contains_camera_target() {
        let box_ = maximum_visible_box(
            [0.0, 0.0, 100.0],
            [0.0, 100.0, 0.0],
            1.0,
            500.0,
            1.0,
            1.0,
            0.0,
        );
        assert!(box_.contains_expanded([0.0, 50.0, 20.0], 5.0));
    }

    #[test]
    fn rebuild_increments_serial() {
        let before = shadow_rebuild_serial();
        rebuild_shadows();
        assert!(shadow_rebuild_serial() > before);
    }

    #[test]
    fn collect_unit_decal_items_does_not_invent_caster_blobs() {
        let before = collect_unit_decal_items().len();
        register_unit_shadow(UnitShadowCaster {
            position: [10.0, 20.0, 0.0],
            size_x: 12.0,
            size_y: 12.0,
            angle: 0.0,
            player_color: 0,
            occluded: false,
            heat_vision: false,
            volume: false,
        });
        let after = collect_unit_decal_items();
        clear_unit_shadows();
        assert_eq!(
            after.len(),
            before,
            "C++ flushDecals draws allocated addDecal/addShadow only"
        );
    }
}
