//! W3DTruckDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTruckDraw.cpp
//!
//! Extends W3DModelDraw with wheel rotation, cab/trailer bone articulation,
//! dust/dirt/powerslide particle emitters, and landing/powerslide audio.
//!
//! C++ author: John Ahlquist, March 2002

use cgmath::{Matrix4, Point3, Vector3};

/// Acceleration threshold for "is accelerating" check (C++: ACCEL_THRESHOLD)
const ACCEL_THRESHOLD: f32 = 0.01;
/// Max dust size multiplier (C++: SIZE_CAP)
const SIZE_CAP: f32 = 2.0;

/// W3DTruckDrawModuleData fields (C++ parity)
#[derive(Debug, Clone)]
pub struct W3DTruckDrawModuleData {
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
    pub mid_mid_left_tire_bone_name: String,
    pub mid_mid_right_tire_bone_name: String,
    pub rotation_speed_multiplier: f32,
    pub powerslide_rotation_addition: f32,
    pub cab_bone_name: String,
    pub trailer_bone_name: String,
    pub cab_rotation_factor: f32,
    pub trailer_rotation_factor: f32,
    pub rotation_damping_factor: f32,
}

impl Default for W3DTruckDrawModuleData {
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
            mid_mid_left_tire_bone_name: String::new(),
            mid_mid_right_tire_bone_name: String::new(),
            rotation_speed_multiplier: 0.0,
            powerslide_rotation_addition: 0.0,
            cab_bone_name: String::new(),
            trailer_bone_name: String::new(),
            cab_rotation_factor: 0.0,
            trailer_rotation_factor: 0.0,
            rotation_damping_factor: 0.0,
        }
    }
}

/// W3DTruckDraw implementation
#[derive(Debug)]
pub struct W3DTruckDraw {
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
    mid_mid_left_tire_bone: i32,
    mid_mid_right_tire_bone: i32,
    cab_bone: i32,
    cur_cab_rotation: f32,
    trailer_bone: i32,
    cur_trailer_rotation: f32,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
}

impl W3DTruckDraw {
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
            mid_mid_left_tire_bone: 0,
            mid_mid_right_tire_bone: 0,
            cab_bone: 0,
            cur_cab_rotation: 0.0,
            trailer_bone: 0,
            cur_trailer_rotation: 0.0,
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
        }
    }

    /// Main per-frame draw.
    /// 1. W3DModelDraw::doDrawModule()
    /// 2. Early exit if !showClientPhysics or time frozen
    /// 3. Update bone indices if render object changed
    /// 4. Cab/trailer articulation with damping
    /// 5. Wheel rotation based on speed
    /// 6. Front tire: suspension height + steering angle + spin
    /// 7. Rear/mid tires: spin + height offset
    /// 8. Emitter control: dust, dirt, powerslide
    /// 9. Audio: powerslide start/stop, landing sound
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // PARITY_NOTE: W3DModelDraw::doDrawModule(transformMtx)
        // PARITY_NOTE: Full implementation requires:
        // - RenderObjClass::Get_Bone_Index, Capture_Bone, Control_Bone
        // - PhysicsBehavior for velocity, speed, isMotive, isAirborne, getTurning
        // - TWheelInfo for suspension offsets and steering angle
        // - AIUpdateInterface for path/goal angle
        // - ThePartitionManager for angle calculation
        // - ParticleSystem for dust/dirt/powerslide emitters
        // - AudioEventRTS for sounds
    }

    pub fn on_render_obj_recreated(&mut self) {
        self.front_left_tire_bone = 0;
        self.front_right_tire_bone = 0;
        self.rear_left_tire_bone = 0;
        self.rear_right_tire_bone = 0;
        self.mid_front_left_tire_bone = 0;
        self.mid_front_right_tire_bone = 0;
        self.mid_rear_left_tire_bone = 0;
        self.mid_rear_right_tire_bone = 0;
        self.mid_mid_left_tire_bone = 0;
        self.mid_mid_right_tire_bone = 0;
        self.cab_bone = 0;
        self.trailer_bone = 0;
        // PARITY_NOTE: updateBones()
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        // PARITY_NOTE: if hiding, enableEmitters(false)
    }

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
        // PARITY_NOTE: if newly obscured, tossEmitters(); if revealed, createEmitters()
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
    pub fn load_post_process(&mut self) {
        // PARITY_NOTE: tossEmitters()
    }
}

impl Default for W3DTruckDraw {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wthree_d_truck_draw_basic() {
        let draw = W3DTruckDraw::new();
        assert!(draw.is_visible());
        assert_eq!(draw.front_wheel_rotation, 0.0);
    }
}
