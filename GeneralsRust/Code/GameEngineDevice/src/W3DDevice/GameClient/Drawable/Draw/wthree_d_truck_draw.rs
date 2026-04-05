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
    pub fn do_draw_module(
        &mut self,
        transform_mtx: &Matrix4<f32>,
        show_client_physics: bool,
        time_frozen: bool,
        speed: f32,
        is_motive: bool,
        turning: i32,
        is_significantly_above_terrain: bool,
        moving_backwards: bool,
        wheel_angle: f32,
        has_path: bool,
        angle_to_goal: f32,
        cab_rotation_factor: f32,
        trailer_rotation_factor: f32,
        rotation_damping_factor: f32,
        rotation_speed_multiplier: f32,
        powerslide_rotation_addition: f32,
    ) {
        // PARITY_NOTE: W3DModelDraw::doDrawModule(transformMtx)
        let _ = transform_mtx;

        if !show_client_physics || time_frozen {
            return;
        }

        const ACCEL_THRESHOLD: f32 = 0.01;
        const SIZE_CAP: f32 = 2.0;

        // PARITY_NOTE: C++ checks getRenderObject()==NULL, return
        // PARITY_NOTE: C++ checks getRenderObject() != m_prevRenderObj → updateBones()
        // PARITY_NOTE: C++ gets Object, PhysicsBehavior, TWheelInfo, AIUpdateInterface, Locomotor

        // Cab/trailer bone articulation with damping
        if self.cab_bone != 0 {
            let mut desired_angle = wheel_angle * cab_rotation_factor;

            // C++: clamp desiredAngle to [min(angleToGoal, 0), max(angleToGoal, 0)]
            if has_path {
                if angle_to_goal < 0.0 {
                    if desired_angle < angle_to_goal {
                        desired_angle = angle_to_goal;
                    }
                    if desired_angle > 0.0 {
                        desired_angle = 0.0;
                    }
                } else {
                    if desired_angle > angle_to_goal {
                        desired_angle = angle_to_goal;
                    }
                    if desired_angle < 0.0 {
                        desired_angle = 0.0;
                    }
                }
            }

            let mut delta_angle = desired_angle - self.cur_cab_rotation;
            delta_angle *= rotation_damping_factor;
            self.cur_cab_rotation += delta_angle;
            // PARITY_NOTE: Capture_Bone(cabBone), Control_Bone(cabBone, cabXfrm with Rotate_Z)

            if self.trailer_bone != 0 {
                let desired_trailer = -wheel_angle * trailer_rotation_factor;
                let mut delta_trailer = desired_trailer - self.cur_trailer_rotation;
                delta_trailer *= rotation_damping_factor;
                self.cur_trailer_rotation += delta_trailer;
                // PARITY_NOTE: Capture_Bone(trailerBone), Control_Bone(trailerBone, cabXfrm with Rotate_Z)
            }
        }

        // Wheel rotation
        if self.front_left_tire_bone != 0 || self.rear_left_tire_bone != 0 {
            let effective_speed = if moving_backwards { -speed } else { speed };
            let effective_powerslide = if moving_backwards {
                -powerslide_rotation_addition
            } else {
                powerslide_rotation_addition
            };

            self.front_wheel_rotation += rotation_speed_multiplier * effective_speed;
            if self.is_powersliding {
                self.rear_wheel_rotation +=
                    rotation_speed_multiplier * (effective_speed + effective_powerslide);
            } else {
                self.rear_wheel_rotation += rotation_speed_multiplier * effective_speed;
            }
            self.mid_front_wheel_rotation = self.front_wheel_rotation;
            self.mid_rear_wheel_rotation = self.rear_wheel_rotation;

            // PARITY_NOTE: Wheel bone transforms (Capture_Bone/Control_Bone):
            // Front tires: Z translation (heightOffset) + Z rotation (wheelAngle) + Y rotation (spin)
            // Rear tires: Y rotation (spin) + Z translation (heightOffset)
            // Mid-front/mid-rear/mid-mid: same patterns with respective bone indices
        }

        // Emitter control
        let was_powersliding = self.is_powersliding;
        self.is_powersliding = false;

        if is_motive && !is_significantly_above_terrain {
            self.effects_initialized = true;
            // PARITY_NOTE: enableEmitters(true) — createEmitters + start dust/dirt

            // PARITY_NOTE: Dust size multiplier: min(speed, SIZE_CAP)
            // PARITY_NOTE: Dirt spray on landing (framesAirborne > 3): trigger + landing sound
            // PARITY_NOTE: Powerslide detection: if turning != TURN_NONE → isPowersliding=true, start powerslide effect
            // PARITY_NOTE: Dirt stop: if !accelerating || speed > 2.0
        } else {
            // PARITY_NOTE: enableEmitters(false)
        }

        self.was_airborne = is_significantly_above_terrain;

        // PARITY_NOTE: Powerslide sound start/stop (TheAudio->addAudioEvent/removeAudioEvent)
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
