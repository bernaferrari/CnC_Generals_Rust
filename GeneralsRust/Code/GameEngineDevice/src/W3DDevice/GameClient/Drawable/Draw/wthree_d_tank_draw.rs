//! W3DTankDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankDraw.cpp
//!
//! Extends W3DModelDraw with tank-specific rendering: tread UV scrolling, tread debris
//! particle emitters, and dual-mode tread animation (pivot vs drive).

use cgmath::{Matrix4, Point3, Vector2, Vector3};

/// Maximum treads per tank (C++: MAX_TREADS_PER_TANK)
const MAX_TREADS_PER_TANK: usize = 4;

/// Tread type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreadType {
    Left,
    Right,
    Middle,
}

/// Physics turning type (C++: PhysicsTurningType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsTurningType {
    None = 0,
    Positive = 1,
    Negative = 2,
}

/// Per-tread sub-object info
#[derive(Debug, Clone)]
pub struct TreadObjectInfo {
    /// Custom UV offset for scrolling (matches C++ Material_Override)
    pub custom_uv_offset: Vector2<f32>,
    pub tread_type: TreadType,
}

/// W3DTankDrawModuleData fields
#[derive(Debug, Clone)]
pub struct W3DTankDrawModuleData {
    /// Tread debris particle name left (INI: "TreadDebrisLeft", default: "TrackDebrisDirtLeft")
    pub tread_debris_name_left: String,
    /// Tread debris particle name right (INI: "TreadDebrisRight", default: "TrackDebrisDirtRight")
    pub tread_debris_name_right: String,
    /// UV scroll speed (INI: "TreadAnimationRate", default: 0.0)
    pub tread_animation_rate: f32,
    /// Speed fraction for pivot tread scroll (INI: "TreadPivotSpeedFraction", default: 0.6)
    pub tread_pivot_speed_fraction: f32,
    /// Speed fraction for drive tread scroll (INI: "TreadDriveSpeedFraction", default: 0.3)
    pub tread_drive_speed_fraction: f32,
}

impl Default for W3DTankDrawModuleData {
    fn default() -> Self {
        Self {
            tread_debris_name_left: "TrackDebrisDirtLeft".into(),
            tread_debris_name_right: "TrackDebrisDirtRight".into(),
            tread_animation_rate: 0.0,
            tread_pivot_speed_fraction: 0.6,
            tread_drive_speed_fraction: 0.3,
        }
    }
}

/// W3DTankDraw implementation
///
/// Extends W3DModelDraw with tread UV scrolling and debris particle emitters.
/// Treads are sub-objects whose names contain ".TREADS" with a LinearOffsetTextureMapper.
#[derive(Debug)]
pub struct W3DTankDraw {
    module_data: W3DTankDrawModuleData,
    /// Discovered tread sub-objects
    treads: Vec<TreadObjectInfo>,
    /// Number of active tread objects found
    tread_count: usize,
    /// Last direction for pivot angle calculation (default: (1,0,0))
    last_direction: Vector3<f32>,
    /// Whether hidden
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
    debris_active: bool,
    time_frozen: bool,
}

impl W3DTankDraw {
    pub fn new(module_data: W3DTankDrawModuleData) -> Self {
        Self {
            module_data,
            treads: vec![
                TreadObjectInfo {
                    custom_uv_offset: Vector2::new(0.0, 0.0),
                    tread_type: TreadType::Middle
                };
                MAX_TREADS_PER_TANK
            ],
            tread_count: 0,
            last_direction: Vector3::new(1.0, 0.0, 0.0),
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
            debris_active: false,
            time_frozen: false,
        }
    }

    pub fn new_default() -> Self {
        Self::new(W3DTankDrawModuleData::default())
    }

    /// Scroll tread UV offsets.
    /// TREAD_LEFT/TREAD_MIDDLE: add uvDelta. TREAD_RIGHT: subtract.
    /// Wraps UV into [0,1] via offset - floor(offset).
    fn update_tread_positions(&mut self, uv_delta: f32) {
        for tread in &mut self.treads[..self.tread_count] {
            match tread.tread_type {
                TreadType::Left | TreadType::Middle => {
                    tread.custom_uv_offset.x += uv_delta;
                }
                TreadType::Right => {
                    tread.custom_uv_offset.x -= uv_delta;
                }
            }
            // Wrap to [0,1]
            tread.custom_uv_offset.x -= tread.custom_uv_offset.x.floor();
        }
    }

    /// Main per-frame draw.
    ///
    /// Key logic (after W3DModelDraw::doDrawModule):
    /// 1. Get velocity from PhysicsBehavior
    /// 2. Start/stop debris based on velMag > DEBRIS_THRESHOLD
    /// 3. Set velocityMultiplier and burstCountMultiplier on debris emitters
    /// 4. Tread animation (if treadCount > 0):
    ///    a. PIVOT mode: if turning AND velocityMagnitude/maxSpeed < pivotSpeedFraction,
    ///       scroll treads based on angle change from lastDirection
    ///    b. DRIVE mode: if isMotive AND velocityMagnitude/maxSpeed >= driveSpeedFraction,
    ///       scroll treads by treadAnimationRate per frame
    pub fn do_draw_module(
        &mut self,
        transform_mtx: &Matrix4<f32>,
        time_frozen: bool,
        velocity_x: f32,
        velocity_y: f32,
        velocity_mag: f32,
        max_speed: f32,
        is_motive: bool,
        turning: PhysicsTurningType,
        dir_x: f32,
        dir_y: f32,
    ) {
        const DEBRIS_THRESHOLD: f32 = 0.00001;

        if time_frozen {
            self.time_frozen = true;
            return;
        }
        self.time_frozen = false;

        // PARITY_NOTE: C++ checks getRenderObject()==NULL, return
        // PARITY_NOTE: C++ checks getRenderObject() != m_prevRenderObj → updateTreadObjects()

        // C++: velMag = vel->x*vel->x + vel->y*vel->y (only ground plane movement)
        let vel_mag_sq = velocity_x * velocity_x + velocity_y * velocity_y;

        // Debris start/stop based on velocity threshold
        if vel_mag_sq > DEBRIS_THRESHOLD && !self.hidden && !self.fully_obscured_by_shroud {
            if !self.debris_active {
                self.debris_active = true;
                // PARITY_NOTE: m_treadDebrisLeft->start(); m_treadDebrisRight->start();
            }
        } else {
            if self.debris_active {
                self.debris_active = false;
                // PARITY_NOTE: m_treadDebrisLeft->stop(); m_treadDebrisRight->stop();
            }
        }

        // PARITY_NOTE: velocity multiplier for debris emitters:
        // velMult.x = 0.5f * velMag + 0.1f; cap at 1.0
        // velMult.y = velMult.x;
        // velMult.z = velMag + 0.1f; cap at 1.0
        // m_treadDebrisLeft->setVelocityMultiplier(&velMult)
        // m_treadDebrisRight->setVelocityMultiplier(&velMult)
        // m_treadDebrisLeft->setBurstCountMultiplier(velMult.z)
        // m_treadDebrisRight->setBurstCountMultiplier(velMult.z)

        // Tread animation (C++: only runs if m_treadCount > 0)
        if self.tread_count > 0 {
            let tread_scroll_speed = self.module_data.tread_animation_rate;
            // PARITY_NOTE: C++ uses maxSpeed from obj->getAIUpdateInterface()->getCurLocomotorSpeed()
            let safe_max_speed = if max_speed > 0.001 {
                max_speed
            } else {
                999999.0
            };

            // Pivot mode: turning while mostly stationary
            if turning != PhysicsTurningType::None
                && velocity_mag / safe_max_speed < self.module_data.tread_pivot_speed_fraction
            {
                // C++: dot product of current 2D direction with last direction
                let angle_to_goal = dir_x * self.last_direction.x + dir_y * self.last_direction.y;
                if (1.0 - angle_to_goal).abs() > 0.00001 {
                    if turning == PhysicsTurningType::Negative {
                        self.update_tread_positions(-tread_scroll_speed);
                    } else {
                        self.update_tread_positions(tread_scroll_speed);
                    }
                }
                self.last_direction.x = dir_x;
                self.last_direction.y = dir_y;
                self.last_direction.z = 0.0;
            }
            // Drive mode: motive and above drive speed fraction
            else if is_motive
                && velocity_mag / safe_max_speed >= self.module_data.tread_drive_speed_fraction
            {
                // C++ scrolls ALL treads by subtracting treadScrollSpeed (note: different from
                // updateTreadPositions which adds for LEFT/MIDDLE and subtracts for RIGHT).
                // In drive mode, all treads scroll in the same direction.
                for tread in &mut self.treads[..self.tread_count] {
                    let offset_u = tread.custom_uv_offset.x - tread_scroll_speed;
                    tread.custom_uv_offset.x = offset_u - offset_u.floor();
                }
            }
        }

        // PARITY_NOTE: W3DModelDraw::doDrawModule(transformMtx) called here
        let _ = transform_mtx;
    }

    /// Called when render object is recreated. Re-discovers tread sub-objects.
    pub fn on_render_obj_recreated(&mut self) {
        self.update_tread_objects();
    }

    /// Discover tread sub-objects from render object.
    /// Iterates sub-objects, finds meshes whose names contain ".TREADS",
    /// checks for LinearOffsetTextureMapperClass, stores them.
    fn update_tread_objects(&mut self) {
        self.tread_count = 0;
        // PARITY_NOTE: Full implementation:
        // for each sub-object of render object:
        //   if CLASSID_MESH && name contains ".TREADS":
        //     check for LinearOffsetTextureMapperClass
        //     disable auto-scroll: mapper->Set_UV_Offset_Delta(Vector2(0,0))
        //     store tread with custom Material_Override
        //     determine TREAD_LEFT/TREAD_RIGHT from 7th char after "TREADS"
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        if hidden {
            self.debris_active = false;
        }
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

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        if fully_obscured {
            self.debris_active = false;
        }
        self.fully_obscured_by_shroud = fully_obscured;
    }
    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
    }

    /// C++ parity: Inherited from `W3DModelDraw::reactToGeometryChange() { }` — no override
    /// in W3DTankDraw.h. Tank geometry bounds are implicitly updated via render object transforms.
    pub fn react_to_geometry_change(&mut self) {}

    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }
    pub fn get_module_data(&self) -> &W3DTankDrawModuleData {
        &self.module_data
    }
    pub fn crc(&self) -> u32 {
        0
    }
    /// C++ parity: Version 1, no additional data saved
    pub fn xfer(&self) -> u32 {
        1
    }
    /// C++ parity: `W3DTankDraw::loadPostProcess()` (W3DTankDraw.cpp line 418):
    ///   1. Calls `W3DModelDraw::loadPostProcess()`
    ///   2. `tossEmitters()` — releases tread debris particle systems
    ///   3. `createEmitters()` — re-creates tread debris from moduleData names
    pub fn load_post_process(&mut self) {
        // PARITY_NOTE: C++ calls tossEmitters() then createEmitters().
        // tossEmitters: releases m_treadDebrisLeft and m_treadDebrisRight particle systems
        // createEmitters: creates new particle systems from moduleData:
        //   m_treadDebrisLeft = TheParticleSystem->createSystem(moduleData->m_treadDebrisNameLeft)
        //   m_treadDebrisRight = TheParticleSystem->createSystem(moduleData->m_treadDebrisNameRight)
        // Requires particle system infrastructure to be wired.
    }

    pub fn get_tread_count(&self) -> usize {
        self.tread_count
    }
}

impl Default for W3DTankDraw {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wthree_d_tank_draw_tread_scroll() {
        let mut draw = W3DTankDraw::new_default();
        assert_eq!(draw.get_tread_count(), 0);

        draw.tread_count = 2;
        draw.treads[0].tread_type = TreadType::Left;
        draw.treads[0].custom_uv_offset = Vector2::new(0.5, 0.0);
        draw.treads[1].tread_type = TreadType::Right;
        draw.treads[1].custom_uv_offset = Vector2::new(0.5, 0.0);

        draw.update_tread_positions(0.1);
        assert!((draw.treads[0].custom_uv_offset.x - 0.6).abs() < 0.001);
        assert!((draw.treads[1].custom_uv_offset.x - 0.4).abs() < 0.001);

        draw.treads[0].custom_uv_offset = Vector2::new(0.95, 0.0);
        draw.update_tread_positions(0.1);
        assert!((draw.treads[0].custom_uv_offset.x - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_wthree_d_tank_draw_do_draw_frozen() {
        let mut draw = W3DTankDraw::new_default();
        draw.do_draw_module(
            &Matrix4::identity(),
            true,
            0.0,
            0.0,
            0.0,
            10.0,
            false,
            PhysicsTurningType::None,
            1.0,
            0.0,
        );
        assert!(draw.time_frozen);
    }

    #[test]
    fn test_wthree_d_tank_draw_drive_mode() {
        let mut draw = W3DTankDraw::new(W3DTankDrawModuleData {
            tread_animation_rate: 0.05,
            tread_drive_speed_fraction: 0.3,
            ..Default::default()
        });
        draw.tread_count = 1;
        draw.treads[0].custom_uv_offset = Vector2::new(0.0, 0.0);
        draw.do_draw_module(
            &Matrix4::identity(),
            false,
            1.0,
            0.0,
            5.0,
            10.0,
            true,
            PhysicsTurningType::None,
            1.0,
            0.0,
        );
        assert!((draw.treads[0].custom_uv_offset.x - (-0.05)).abs() < 0.001);
    }

    #[test]
    fn test_wthree_d_tank_draw_pivot_mode() {
        let mut draw = W3DTankDraw::new(W3DTankDrawModuleData {
            tread_animation_rate: 0.05,
            tread_pivot_speed_fraction: 0.6,
            ..Default::default()
        });
        draw.tread_count = 1;
        draw.treads[0].tread_type = TreadType::Middle;
        draw.treads[0].custom_uv_offset = Vector2::new(0.0, 0.0);
        draw.do_draw_module(
            &Matrix4::identity(),
            false,
            0.0,
            0.0,
            0.1,
            10.0,
            true,
            PhysicsTurningType::Positive,
            0.0,
            1.0,
        );
        assert!((draw.treads[0].custom_uv_offset.x - 0.05).abs() < 0.001);
    }
}
