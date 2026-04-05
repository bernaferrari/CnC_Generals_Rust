//! W3DTankDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankDraw.cpp
//!
//! Extends W3DModelDraw with tank-specific rendering: tread UV scrolling, tread debris
//! particle emitters, and dual-mode tread animation (pivot vs drive).

use cgmath::{Matrix4, Point3, Vector3};

/// Maximum treads per tank (C++: MAX_TREADS_PER_TANK)
const MAX_TREADS_PER_TANK: usize = 4;

/// Tread type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreadType {
    Left,
    Right,
    Middle,
}

/// Per-tread sub-object info
#[derive(Debug, Clone)]
pub struct TreadObjectInfo {
    /// Custom UV offset for scrolling (matches C++ Material_Override)
    pub custom_uv_offset: cgmath::Vector2<f32>,
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
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // PARITY_NOTE: W3DModelDraw::doDrawModule(transformMtx)

        // PARITY_NOTE: Full tread animation logic requires:
        // - Object, PhysicsBehavior, AIUpdateInterface from GameLogic
        // - RenderObjClass with Get_Sub_Object, Get_Material_Info, etc.
        // - ParticleSystem for debris emitters
        //
        // Pivot mode: if turning AND vel/maxVel < 0.6:
        //   angle = dot(currentDir, lastDirection)
        //   scroll based on angle change direction
        // Drive mode: if motive AND vel/maxVel >= 0.3:
        //   scroll by treadAnimationRate per frame
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
        // PARITY_NOTE: if hiding, call stopMoveDebris()
    }

    pub fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadow_enabled = enable;
    }
    pub fn release_shadows(&mut self) {}
    pub fn allocate_shadows(&mut self) {}
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
        // PARITY_NOTE: if newly obscured, stop debris
    }
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
    pub fn load_post_process(&mut self) {
        // PARITY_NOTE: tossEmitters() + createEmitters()
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

        // Manually set up treads for testing
        draw.tread_count = 2;
        draw.treads[0].tread_type = TreadType::Left;
        draw.treads[0].custom_uv_offset = Vector2::new(0.5, 0.0);
        draw.treads[1].tread_type = TreadType::Right;
        draw.treads[1].custom_uv_offset = Vector2::new(0.5, 0.0);

        draw.update_tread_positions(0.1);
        assert!((draw.treads[0].custom_uv_offset.x - 0.6).abs() < 0.001);
        assert!((draw.treads[1].custom_uv_offset.x - 0.4).abs() < 0.001);

        // Test wrapping
        draw.treads[0].custom_uv_offset = Vector2::new(0.95, 0.0);
        draw.update_tread_positions(0.1);
        assert!((draw.treads[0].custom_uv_offset.x - 0.05).abs() < 0.001);
    }
}
