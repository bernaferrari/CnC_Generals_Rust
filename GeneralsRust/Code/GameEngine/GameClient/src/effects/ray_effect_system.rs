//! C++ `RayEffectSystem` + `W3DGameClient::createRayEffectByTemplate`.
//!
//! Oracle:
//! - `GameClient/RayEffect.cpp` / `RayEffect.h` (`MAX_RAY_EFFECTS = 128`)
//! - `W3DGameClient::createRayEffectByTemplate` midpoint drawable + addRayEffect

use std::sync::{Mutex, OnceLock};

/// C++ `RayEffectSystem::MAX_RAY_EFFECTS`.
pub const MAX_RAY_EFFECTS: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct LiveRayEffect {
    pub drawable_id: u32,
    pub template_name: String,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub midpoint: [f32; 3],
    /// C++ LaserUpdate width scalar (1 at max intensity, 0 after FadeLifetime).
    pub width_scalar: f32,
    pub created_frame: u32,
    pub fade_start_frame: u32,
    /// 0 = no auto-expire (`addRayEffect` drawable-owned lifetime).
    pub expire_frame: u32,
    /// C++ W3DLaserDraw OuterBeamWidth (world units).
    pub outer_beam_width: f32,
    /// C++ W3DLaserDraw OuterColor (or InnerColor if outer is black).
    pub color: [f32; 4],
    /// C++ W3DLaserDraw Texture name.
    pub texture_name: String,
    /// True when TheThingFactory produced a real template drawable.
    pub from_template_drawable: bool,
}

/// C++ `LOGICFRAMES_PER_SECOND` (30). Leftover `RayEffectConfig` default
/// lifetime is 500ms → 15 frames; fade-out 100ms → 3 frames. Used when the
/// laser template omits MaxIntensityLifetime/FadeLifetime so FXList beams
/// cannot accumulate forever.
const DEFAULT_RAY_MAX_INTENSITY_FRAMES: u32 = 15;
const DEFAULT_RAY_FADE_FRAMES: u32 = 3;
const DEFAULT_RAY_COLOR: [f32; 4] = [0.85, 0.95, 1.0, 1.0];
const DEFAULT_RAY_WIDTH: f32 = 0.4;

fn fx_list_ray_lifetime_frames(template_name: &str) -> (u32, u32) {
    if let Some(visuals) =
        gamelogic::helpers::TheGameClient::ray_effect_template_visuals(template_name)
    {
        let max_i = if visuals.max_intensity_frames == 0 {
            DEFAULT_RAY_MAX_INTENSITY_FRAMES
        } else {
            visuals.max_intensity_frames
        };
        let fade = if visuals.fade_frames == 0 {
            DEFAULT_RAY_FADE_FRAMES
        } else {
            visuals.fade_frames
        };
        return (max_i, fade);
    }
    (DEFAULT_RAY_MAX_INTENSITY_FRAMES, DEFAULT_RAY_FADE_FRAMES)
}

/// C++ midpoint: `(end - start) * 0.5 + start`.
pub fn ray_effect_midpoint(start: [f32; 3], end: [f32; 3]) -> [f32; 3] {
    [
        (end[0] - start[0]) * 0.5 + start[0],
        (end[1] - start[1]) * 0.5 + start[1],
        (end[2] - start[2]) * 0.5 + start[2],
    ]
}

struct RayEffectStore {
    next_id: u32,
    frame: u32,
    slots: [Option<LiveRayEffect>; MAX_RAY_EFFECTS],
}

impl RayEffectStore {
    fn new() -> Self {
        Self {
            next_id: 1,
            frame: 0,
            slots: std::array::from_fn(|_| None),
        }
    }

    fn init(&mut self) {
        self.slots = std::array::from_fn(|_| None);
        self.next_id = 1;
        self.frame = 0;
    }

    fn free_index(&self) -> Option<usize> {
        self.slots.iter().position(|slot| slot.is_none())
    }

    fn alloc_index(&mut self) -> usize {
        if let Some(idx) = self.free_index() {
            return idx;
        }
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| slot.as_ref().map(|e| (i, e.drawable_id)))
            .min_by_key(|(_, id)| *id)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}

fn global_rays() -> &'static Mutex<RayEffectStore> {
    static STORE: OnceLock<Mutex<RayEffectStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(RayEffectStore::new()))
}

/// C++ `W3DGameClient::createRayEffectByTemplate`.
/// Empty template name matches `findTemplate` miss (no drawable).
pub fn create_ray_effect_by_template(
    start: [f32; 3],
    end: [f32; 3],
    template_name: &str,
) -> Option<LiveRayEffect> {
    if template_name.is_empty() {
        return None;
    }
    let midpoint = ray_effect_midpoint(start, end);
    let start_c = gamelogic::common::Coord3D::new(start[0], start[1], start[2]);
    let end_c = gamelogic::common::Coord3D::new(end[0], end[1], end[2]);
    let visuals = gamelogic::helpers::TheGameClient::ray_effect_template_visuals(template_name);
    let template_drawable_id = gamelogic::helpers::TheGameClient.create_ray_effect_drawable(
        template_name,
        &start_c,
        &end_c,
    );
    let mut store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    let Some(idx) = store.free_index() else {
        if let Some(id) = template_drawable_id {
            gamelogic::helpers::TheGameClient.destroy_drawable(id);
        }
        return None;
    };
    let drawable_id = template_drawable_id.unwrap_or_else(|| {
        let id = store.next_id;
        store.next_id = store.next_id.wrapping_add(1).max(1);
        id
    });
    if template_drawable_id.is_some() {
        store.next_id = store.next_id.max(drawable_id.saturating_add(1));
    }
    let now = store.frame;
    let (max_intensity, fade) = fx_list_ray_lifetime_frames(template_name);
    let fade_start_frame = now.saturating_add(max_intensity);
    let expire_frame = fade_start_frame.saturating_add(fade.max(1));
    let (outer_beam_width, color, texture_name) = match &visuals {
        Some(v) => {
            let color = if v.color[0] + v.color[1] + v.color[2] <= 0.001 {
                DEFAULT_RAY_COLOR
            } else {
                v.color
            };
            (v.outer_beam_width, color, v.texture_name.clone())
        }
        None => (DEFAULT_RAY_WIDTH, DEFAULT_RAY_COLOR, String::new()),
    };
    let entry = LiveRayEffect {
        drawable_id,
        template_name: template_name.to_string(),
        start,
        end,
        midpoint,
        width_scalar: 1.0,
        created_frame: now,
        fade_start_frame,
        expire_frame,
        outer_beam_width,
        color,
        texture_name,
        from_template_drawable: template_drawable_id.is_some(),
    };
    store.slots[idx] = Some(entry.clone());
    Some(entry)
}

/// C++ `RayEffectSystem::addRayEffect`.
pub fn add_ray_effect(drawable_id: u32, start: [f32; 3], end: [f32; 3]) -> bool {
    let mut store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    let idx = store.alloc_index();
    let now = store.frame;
    store.slots[idx] = Some(LiveRayEffect {
        drawable_id,
        template_name: String::new(),
        start,
        end,
        midpoint: ray_effect_midpoint(start, end),
        width_scalar: 1.0,
        created_frame: now,
        fade_start_frame: 0,
        expire_frame: 0,
        outer_beam_width: DEFAULT_RAY_WIDTH,
        color: DEFAULT_RAY_COLOR,
        texture_name: String::new(),
        from_template_drawable: false,
    });
    true
}

/// C++ LaserUpdate width decay + drawable death → `removeFromRayEffects`.
/// `expire_frame == 0` is drawable-owned (no FXList auto-expire).
pub fn update_ray_effects(current_frame: u32) {
    let mut store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    if store.frame == 0 && current_frame > 1 {
        for slot in &mut store.slots {
            if let Some(ray) = slot.as_mut() {
                if ray.created_frame == 0 && ray.expire_frame != 0 {
                    let life = ray.expire_frame;
                    let fade = ray.expire_frame.saturating_sub(ray.fade_start_frame);
                    let max_i = life.saturating_sub(fade);
                    ray.created_frame = current_frame;
                    ray.fade_start_frame = current_frame.saturating_add(max_i);
                    ray.expire_frame = ray.fade_start_frame.saturating_add(fade.max(1));
                }
            }
        }
    }
    store.frame = current_frame;
    for slot in &mut store.slots {
        let Some(ray) = slot.as_mut() else {
            continue;
        };
        if ray.expire_frame == 0 {
            ray.width_scalar = 1.0;
            continue;
        }
        if current_frame >= ray.expire_frame {
            let id = ray.drawable_id;
            let from_template = ray.from_template_drawable;
            *slot = None;
            if from_template {
                drop(store);
                gamelogic::helpers::TheGameClient.destroy_drawable(id);
                return update_ray_effects(current_frame);
            }
            continue;
        }
        if current_frame < ray.fade_start_frame {
            ray.width_scalar = 1.0;
        } else {
            let fade_len = ray.expire_frame.saturating_sub(ray.fade_start_frame).max(1);
            let elapsed = current_frame.saturating_sub(ray.fade_start_frame);
            ray.width_scalar = (1.0 - elapsed as f32 / fade_len as f32).clamp(0.0, 1.0);
        }
    }
}

/// C++ `RayEffectSystem::deleteRayEffect`.
pub fn delete_ray_effect(drawable_id: u32) -> bool {
    let mut store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    for slot in &mut store.slots {
        if slot.as_ref().is_some_and(|e| e.drawable_id == drawable_id) {
            let from_template = slot.as_ref().is_some_and(|e| e.from_template_drawable);
            *slot = None;
            drop(store);
            if from_template {
                gamelogic::helpers::TheGameClient.destroy_drawable(drawable_id);
            }
            return true;
        }
    }
    false
}

/// C++ `RayEffectSystem::getRayEffectData`.
pub fn get_ray_effect_data(drawable_id: u32) -> Option<LiveRayEffect> {
    let store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    store.slots.iter().find_map(|slot| {
        slot.as_ref()
            .filter(|e| e.drawable_id == drawable_id)
            .cloned()
    })
}

pub fn live_ray_effects() -> Vec<LiveRayEffect> {
    let store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    store.slots.iter().filter_map(|s| s.clone()).collect()
}

/// C++ `RayEffectSystem::reset` / `init`.
pub fn reset_ray_effects() {
    let mut store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    let ids: Vec<u32> = store
        .slots
        .iter()
        .filter_map(|s| {
            s.as_ref()
                .filter(|e| e.from_template_drawable)
                .map(|e| e.drawable_id)
        })
        .collect();
    store.init();
    drop(store);
    for id in ids {
        gamelogic::helpers::TheGameClient.destroy_drawable(id);
    }
}

/// GPU line endpoints for the registered ray (start → end).
pub fn bake_ray_effect_gpu_endpoints(effect: &LiveRayEffect) -> ([f32; 3], [f32; 3], [f32; 3]) {
    (effect.start, effect.midpoint, effect.end)
}

/// wgpu line-list for `W3DGameClient::createRayEffectByTemplate`:
/// drawable at C++ midpoint, beam from registered start → end.
#[derive(Debug, Clone, PartialEq)]
pub struct RayEffectGpuMesh {
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u16>,
    pub midpoint: [f32; 3],
}

pub fn bake_ray_effect_gpu_mesh(effect: &LiveRayEffect) -> RayEffectGpuMesh {
    let (start, midpoint, end) = bake_ray_effect_gpu_endpoints(effect);
    RayEffectGpuMesh {
        vertices: vec![start, end],
        indices: vec![0, 1],
        midpoint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::fxlist_integration::{FXContext, FXNugget, RayEffectFXNugget};
    use crate::effects::particle_manager::ParticleSystemManager;
    use nalgebra::{Point3, Vector3};

    #[test]
    fn fxlist_ray_effect_nugget_gpu_mesh_matches_cpp_midpoint_and_offsets() {
        reset_ray_effects();

        let primary = [10.0_f32, 20.0, 30.0];
        let secondary = [50.0_f32, 60.0, 70.0];
        let primary_offset = [1.0_f32, 2.0, 3.0];
        let secondary_offset = [-1.0_f32, 0.0, 1.0];

        let mut nugget = RayEffectFXNugget::new("GenericLaser".to_string());
        nugget.primary_offset =
            Vector3::new(primary_offset[0], primary_offset[1], primary_offset[2]);
        nugget.secondary_offset = Vector3::new(
            secondary_offset[0],
            secondary_offset[1],
            secondary_offset[2],
        );

        let mut manager = ParticleSystemManager::new();
        let mut ctx = FXContext {
            particle_manager: &mut manager,
            ray_effect_manager: None,
            decal_manager: None,
            bone_query: None,
            current_frame: 0,
            local_player_index: 0,
        };
        FXNugget::do_fx_pos(
            &nugget,
            Point3::new(primary[0], primary[1], primary[2]),
            None,
            0.0,
            Some(Point3::new(secondary[0], secondary[1], secondary[2])),
            0.0,
            &mut ctx,
        );

        let rays = live_ray_effects();
        assert_eq!(
            rays.len(),
            1,
            "RayEffectFXNugget must call create_ray_effect_by_template"
        );

        let exp_start = [
            primary[0] + primary_offset[0],
            primary[1] + primary_offset[1],
            primary[2] + primary_offset[2],
        ];
        let exp_end = [
            secondary[0] + secondary_offset[0],
            secondary[1] + secondary_offset[1],
            secondary[2] + secondary_offset[2],
        ];
        let exp_mid = [
            (exp_end[0] - exp_start[0]) * 0.5 + exp_start[0],
            (exp_end[1] - exp_start[1]) * 0.5 + exp_start[1],
            (exp_end[2] - exp_start[2]) * 0.5 + exp_start[2],
        ];
        assert_eq!(rays[0].start, exp_start);
        assert_eq!(rays[0].end, exp_end);
        assert_eq!(rays[0].midpoint, exp_mid);
        assert_eq!(rays[0].midpoint, ray_effect_midpoint(exp_start, exp_end));

        let mesh = bake_ray_effect_gpu_mesh(&rays[0]);
        assert_eq!(mesh.vertices, vec![exp_start, exp_end]);
        assert_eq!(mesh.indices, vec![0, 1]);
        assert_eq!(mesh.midpoint, exp_mid);

        assert!(get_ray_effect_data(rays[0].drawable_id).is_some());
        assert!(delete_ray_effect(rays[0].drawable_id));
        assert!(get_ray_effect_data(rays[0].drawable_id).is_none());
        reset_ray_effects();
    }

    #[test]
    fn fxlist_ray_effect_expires_after_max_intensity_plus_fade() {
        reset_ray_effects();
        update_ray_effects(10);
        let ray = create_ray_effect_by_template([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], "GenericLaser")
            .expect("template present");
        assert_eq!(live_ray_effects().len(), 1);
        assert!((ray.width_scalar - 1.0).abs() < 1e-5);
        assert!(ray.expire_frame > ray.created_frame);

        update_ray_effects(ray.fade_start_frame);
        let mid = live_ray_effects();
        assert_eq!(mid.len(), 1);
        assert!(mid[0].width_scalar <= 1.0);

        update_ray_effects(ray.expire_frame);
        assert!(
            live_ray_effects().is_empty(),
            "FXList RayEffect must expire after MaxIntensityLifetime+FadeLifetime"
        );
        reset_ray_effects();
    }
}
