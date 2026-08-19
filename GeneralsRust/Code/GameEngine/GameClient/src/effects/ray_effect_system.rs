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
    slots: [Option<LiveRayEffect>; MAX_RAY_EFFECTS],
}

impl RayEffectStore {
    fn new() -> Self {
        Self {
            next_id: 1,
            slots: std::array::from_fn(|_| None),
        }
    }

    fn init(&mut self) {
        self.slots = std::array::from_fn(|_| None);
        self.next_id = 1;
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
    let mut store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    let idx = store.alloc_index();
    let id = store.next_id;
    store.next_id = store.next_id.wrapping_add(1).max(1);
    let midpoint = ray_effect_midpoint(start, end);
    let entry = LiveRayEffect {
        drawable_id: id,
        template_name: template_name.to_string(),
        start,
        end,
        midpoint,
    };
    store.slots[idx] = Some(entry.clone());
    Some(entry)
}

/// C++ `RayEffectSystem::addRayEffect`.
pub fn add_ray_effect(drawable_id: u32, start: [f32; 3], end: [f32; 3]) -> bool {
    let mut store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    let idx = store.alloc_index();
    store.slots[idx] = Some(LiveRayEffect {
        drawable_id,
        template_name: String::new(),
        start,
        end,
        midpoint: ray_effect_midpoint(start, end),
    });
    true
}

/// C++ `RayEffectSystem::deleteRayEffect`.
pub fn delete_ray_effect(drawable_id: u32) -> bool {
    let mut store = global_rays().lock().unwrap_or_else(|e| e.into_inner());
    for slot in &mut store.slots {
        if slot.as_ref().is_some_and(|e| e.drawable_id == drawable_id) {
            *slot = None;
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
    global_rays()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .init();
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
}
