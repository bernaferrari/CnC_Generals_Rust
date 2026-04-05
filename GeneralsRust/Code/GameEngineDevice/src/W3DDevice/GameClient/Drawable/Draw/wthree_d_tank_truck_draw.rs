//! W3DTankTruckDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankTruckDraw.cpp
//!
//! Combines W3DTruckDraw (wheel rotation, cab/trailer bones, emitters) with
//! W3DTankDraw (tread UV scrolling, tread debris). Has 10 tire bone slots,
//! dust/dirt/powerslide particle emitters, and tread sub-object management.

use cgmath::{Matrix4, Point3, Vector3};

/// Combined module data for TankTruckDraw (extends both W3DModelDrawModuleData and W3DTankDrawModuleData)
#[derive(Debug, Clone)]
pub struct W3DTankTruckDrawModuleData {
    // From W3DTruckDrawModuleData
    pub dust_effect_name: String,
    pub dirt_effect_name: String,
    pub powerslide_effect_name: String,
    pub front_left_tire_bone_name: String,
    pub front_right_tire_bone_name: String,
    pub rear_left_tire_bone_name: String,
    pub rear_right_tire_bone_name: String,
    pub mid_front_left_tire_bone_name: String,
    pub mid_front_right_tire_bone_name: String,
    pub mid_rear_left_tire_bone_name: String,
    pub mid_rear_right_tire_bone_name: String,
    pub rotation_speed_multiplier: f32,
    pub powerslide_rotation_addition: f32,
    // From W3DTankDrawModuleData
    pub tread_debris_name_left: String,
    pub tread_debris_name_right: String,
    pub tread_animation_rate: f32,
    pub tread_pivot_speed_fraction: f32,
    pub tread_drive_speed_fraction: f32,
}

impl Default for W3DTankTruckDrawModuleData {
    fn default() -> Self {
        Self {
            dust_effect_name: String::new(),
            dirt_effect_name: String::new(),
            powerslide_effect_name: String::new(),
            front_left_tire_bone_name: String::new(),
            front_right_tire_bone_name: String::new(),
            rear_left_tire_bone_name: String::new(),
            rear_right_tire_bone_name: String::new(),
            mid_front_left_tire_bone_name: String::new(),
            mid_front_right_tire_bone_name: String::new(),
            mid_rear_left_tire_bone_name: String::new(),
            mid_rear_right_tire_bone_name: String::new(),
            rotation_speed_multiplier: 0.0,
            powerslide_rotation_addition: 0.0,
            tread_debris_name_left: "TrackDebrisDirtLeft".into(),
            tread_debris_name_right: "TrackDebrisDirtRight".into(),
            tread_animation_rate: 0.0,
            tread_pivot_speed_fraction: 0.6,
            tread_drive_speed_fraction: 0.3,
        }
    }
}

const MAX_TREADS_PER_TANK: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreadType {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone)]
struct TreadObjectInfo {
    custom_uv_offset: Vector2<f32>,
    tread_type: TreadType,
}

/// W3DTankTruckDraw implementation
///
/// Combines truck wheel rotation + tank tread UV scrolling + particle emitters.
#[derive(Debug)]
pub struct W3DTankTruckDraw {
    effects_initialized: bool,
    was_airborne: bool,
    is_powersliding: bool,
    front_wheel_rotation: f32,
    rear_wheel_rotation: f32,
    mid_front_wheel_rotation: f32,
    mid_rear_wheel_rotation: f32,
    front_left_tire_bone: i32,
    front_right_tire_bone: i32,
    rear_left_tire_bone: i32,
    rear_right_tire_bone: i32,
    mid_front_left_tire_bone: i32,
    mid_front_right_tire_bone: i32,
    mid_rear_left_tire_bone: i32,
    mid_rear_right_tire_bone: i32,
    treads: Vec<TreadObjectInfo>,
    tread_count: usize,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
}

impl W3DTankTruckDraw {
    pub fn new() -> Self {
        Self {
            effects_initialized: false,
            was_airborne: false,
            is_powersliding: false,
            front_wheel_rotation: 0.0,
            rear_wheel_rotation: 0.0,
            mid_front_wheel_rotation: 0.0,
            mid_rear_wheel_rotation: 0.0,
            front_left_tire_bone: 0,
            front_right_tire_bone: 0,
            rear_left_tire_bone: 0,
            rear_right_tire_bone: 0,
            mid_front_left_tire_bone: 0,
            mid_front_right_tire_bone: 0,
            mid_rear_left_tire_bone: 0,
            mid_rear_right_tire_bone: 0,
            treads: vec![
                TreadObjectInfo {
                    custom_uv_offset: Vector2::new(0.0, 0.0),
                    tread_type: TreadType::Middle
                };
                MAX_TREADS_PER_TANK
            ],
            tread_count: 0,
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
        }
    }

    /// Main per-frame draw: W3DModelDraw + wheel rotation + tread scrolling + emitters.
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // PARITY_NOTE: W3DModelDraw::doDrawModule(transformMtx)
        // PARITY_NOTE: Full truck+tank logic requires:
        // - All W3DTruckDraw APIs (bones, emitters, audio)
        // - All W3DTankDraw APIs (tread sub-objects, UV scrolling)
    }

    pub fn on_render_obj_recreated(&mut self) {
        // Zero all bone indices, then updateBones() + updateTreadObjects()
        self.front_left_tire_bone = 0;
        self.front_right_tire_bone = 0;
        self.rear_left_tire_bone = 0;
        self.rear_right_tire_bone = 0;
        self.mid_front_left_tire_bone = 0;
        self.mid_front_right_tire_bone = 0;
        self.mid_rear_left_tire_bone = 0;
        self.mid_rear_right_tire_bone = 0;
        self.tread_count = 0;
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
    }

    pub fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadow_enabled = enable;
    }
    pub fn release_shadows(&mut self) {}
    pub fn allocate_shadows(&mut self) {}
    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
    }
    pub fn react_to_geometry_change(&mut self) {}
    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }
    pub fn crc(&self) -> u32 {
        0
    }
    pub fn xfer(&self) -> u32 {
        1
    }
    pub fn load_post_process(&mut self) {}
}

impl Default for W3DTankTruckDraw {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wthree_d_tank_truck_draw_basic() {
        let draw = W3DTankTruckDraw::new();
        assert!(draw.is_visible());
        assert_eq!(draw.tread_count, 0);
    }
}
