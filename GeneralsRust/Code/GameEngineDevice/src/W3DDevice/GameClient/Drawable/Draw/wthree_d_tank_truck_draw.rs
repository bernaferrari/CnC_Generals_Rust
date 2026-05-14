//! W3DTankTruckDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankTruckDraw.cpp
//!
//! Combines W3DTruckDraw (wheel rotation, cab/trailer bones, emitters) with
//! W3DTankDraw (tread UV scrolling, tread debris). Has 10 tire bone slots,
//! dust/dirt/powerslide particle emitters, and tread sub-object management.

use cgmath::{Matrix4, Point3, Vector2, Vector3};

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
    last_direction: Vector3<f32>,
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
            last_direction: Vector3::new(1.0, 0.0, 0.0),
        }
    }

    /// C++ parity: W3DModelDraw::doDrawModule first, then truck+tank physics.
    pub fn do_draw_module(
        &mut self,
        transform_mtx: &Matrix4<f32>,
        show_client_physics: bool,
        time_frozen: bool,
        speed: f32,
        max_speed: f32,
        is_motive: bool,
        turning: i32,
        is_significantly_above_terrain: bool,
        moving_backwards: bool,
        wheel_angle: f32,
    ) {
        // PARITY_NOTE: W3DModelDraw::doDrawModule(transformMtx)
        let _ = transform_mtx;

        if !show_client_physics || time_frozen {
            return;
        }

        const ACCEL_THRESHOLD: f32 = 0.01;
        const SIZE_CAP: f32 = 2.0;

        // PARITY_NOTE: C++ checks getRenderObject()==NULL, return
        // PARITY_NOTE: C++ checks getRenderObject() != m_prevRenderObj → updateBones() + updateTreadObjects()

        // PARITY_NOTE: C++ gets Object, PhysicsBehavior, TWheelInfo, AIUpdateInterface
        // For now, the caller provides pre-extracted physics state.

        let rotation_factor = self.get_rotation_speed_multiplier();
        let effective_speed = if moving_backwards { -speed } else { speed };
        let powerslide_addition = self.get_powerslide_rotation_addition();

        self.front_wheel_rotation += rotation_factor * effective_speed;
        if self.is_powersliding {
            self.rear_wheel_rotation += rotation_factor * (effective_speed + powerslide_addition);
        } else {
            self.rear_wheel_rotation += rotation_factor * effective_speed;
        }
        self.mid_front_wheel_rotation = self.front_wheel_rotation;
        self.mid_rear_wheel_rotation = self.rear_wheel_rotation;

        // PARITY_NOTE: Wheel bone transforms (Capture_Bone/Control_Bone):
        // Front tires: Z translation + Z rotation (wheelAngle) + Y rotation (spin)
        // Rear/mid tires: Y rotation (spin) + Z translation (height offset)

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

        // Tread animation (C++: pivot is COMMENTED OUT for TankTruck — only drive mode)
        if self.tread_count > 0 {
            let tread_scroll_speed = self.get_tread_animation_rate();
            let safe_max_speed = if max_speed > 0.001 {
                max_speed
            } else {
                999999.0
            };

            // C++: pivot scrolling is commented out for TankTruckDraw
            // Only drive mode:
            if is_motive && speed / safe_max_speed >= self.get_tread_drive_speed_fraction() {
                for tread in &mut self.treads[..self.tread_count] {
                    let offset_u = tread.custom_uv_offset.x - tread_scroll_speed;
                    tread.custom_uv_offset.x = offset_u - offset_u.floor();
                }
            }
        }

        // PARITY_NOTE: C++ also has #ifdef SHOW_TANK_DEBRIS block for tread debris
        // (disabled in production, same as W3DTankDraw tread debris logic)
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

    /// C++ parity: Inherited from `W3DModelDraw::releaseShadows()` — releases shadow
    /// via `m_shadow->release()` and sets `m_shadow = NULL`.
    // PARITY_NOTE: Would call W3DModelDraw::releaseShadows() in C++ (removes shadow from scene).
    // This struct lacks shadow_id; when full W3DModelDraw state is composed in, delegate to parent.
    pub fn release_shadows(&mut self) {}

    /// C++ parity: Inherited from `W3DModelDraw::allocateShadows()` — creates shadow from
    /// ThingTemplate info if no shadow exists, render object exists, and shadow type != SHADOW_NONE.
    // PARITY_NOTE: Would call W3DModelDraw::allocateShadows() in C++.
    // This struct lacks shadow_id; when full W3DModelDraw state is composed in, delegate to parent.
    pub fn allocate_shadows(&mut self) {}

    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
    }

    /// C++ parity: `virtual void reactToGeometryChange() { }` — explicit empty override
    /// in W3DTankTruckDraw.h. Tank-truck geometry bounds are implicitly updated via render object transforms.
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
    /// C++ parity: `W3DTankTruckDraw::loadPostProcess()` (W3DTankTruckDraw.cpp line 754):
    ///   1. Calls `W3DModelDraw::loadPostProcess()`
    ///   2. `tossEmitters()` — releases dust/dirt/powerslide particle systems (no re-create;
    ///      emitters are created on-demand in doDrawModule via enableEmitters)
    pub fn load_post_process(&mut self) {
        // PARITY_NOTE: C++ calls tossEmitters() only (unlike TankDraw which also calls createEmitters()).
        // tossEmitters: releases m_dustEffect, m_dirtEffect, m_powerslideEffect particle systems.
        // Emitters are re-created lazily when enableEmitters(true) is called during doDrawModule.
        // Requires particle system infrastructure to be wired.
    }

    fn get_rotation_speed_multiplier(&self) -> f32 {
        0.0
    }
    fn get_powerslide_rotation_addition(&self) -> f32 {
        0.0
    }
    fn get_tread_animation_rate(&self) -> f32 {
        0.0
    }
    fn get_tread_drive_speed_fraction(&self) -> f32 {
        0.3
    }
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
