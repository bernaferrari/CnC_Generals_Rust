//! W3DTreeDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTreeDraw.cpp
//!
//! Like W3DPropDraw, W3DTreeDraw does NOT manage its own RenderObjClass. It registers
//! the tree with the terrain system which handles all rendering, swaying, toppling,
//! and sinking. The draw module is a one-shot registration.
//!
//! The W3DTreeDrawModuleData contains all the topple/sink parameters consumed by
//! the terrain system.

use cgmath::{Matrix4, Point3};

/// Default topple speed
const DEFAULT_MINIMUM_TOPPLE_SPEED: f32 = 0.5;
/// Default initial velocity percent
const DEFAULT_INITIAL_VELOCITY_PERCENT: f32 = 0.2;
/// Default initial acceleration percent
const DEFAULT_INITIAL_ACCEL_PERCENT: f32 = 0.01;
/// Default bounce velocity percent
const DEFAULT_BOUNCE_VELOCITY_PERCENT: f32 = 0.3;
/// Default sink frames (10 seconds at 30 FPS)
const DEFAULT_SINK_FRAMES: u32 = 300;
/// Default sink distance
const DEFAULT_SINK_DISTANCE: f32 = 20.0;

/// W3DTreeDrawModuleData - INI-parsed configuration for tree behavior
///
/// All fields match the C++ W3DTreeDrawModuleData struct.
#[derive(Debug, Clone)]
pub struct W3DTreeDrawModuleData {
    /// Tree model name (INI: "ModelName")
    pub model_name: String,
    /// Tree texture (INI: "TextureName")
    pub texture_name: String,
    /// Frames to push tree outward (INI: "MoveOutwardTime", default: 1)
    pub frames_to_move_outward: u32,
    /// Frames to push tree inward (INI: "MoveInwardTime", default: 1)
    pub frames_to_move_inward: u32,
    /// Max push distance factor (INI: "MoveOutwardDistanceFactor", default: 1.0)
    pub max_outward_movement: f32,
    /// Darkening factor (INI: "DarkeningFactor", default: 0.0)
    pub darkening: f32,
    /// FX when tree topples (INI: "ToppleFX")
    pub topple_fx: String,
    /// FX when tree bounces (INI: "BounceFX")
    pub bounce_fx: String,
    /// Stump model name after toppling (INI: "StumpName")
    pub stump_name: String,
    /// Topple start velocity (INI: "InitialVelocityPercent", default: 0.2)
    pub initial_velocity_percent: f32,
    /// Topple start acceleration (INI: "InitialAccelPercent", default: 0.01)
    pub initial_accel_percent: f32,
    /// Velocity retained after bounce (INI: "BounceVelocityPercent", default: 0.3)
    pub bounce_velocity_percent: f32,
    /// Minimum topple speed (INI: "MinimumToppleSpeed", default: 0.5)
    pub minimum_topple_speed: f32,
    /// Destroy tree after toppling (INI: "KillWhenFinishedToppling", default: true)
    pub kill_when_toppled: bool,
    /// Whether tree can topple (INI: "DoTopple", default: false)
    pub do_topple: bool,
    /// Frames to sink after toppling (INI: "SinkTime", default: 300)
    pub sink_frames: u32,
    /// How far tree sinks (INI: "SinkDistance", default: 20.0)
    pub sink_distance: f32,
    /// Whether tree casts shadow (INI: "DoShadow", default: false)
    pub do_shadow: bool,
}

impl Default for W3DTreeDrawModuleData {
    fn default() -> Self {
        Self {
            model_name: String::new(),
            texture_name: String::new(),
            frames_to_move_outward: 1,
            frames_to_move_inward: 1,
            max_outward_movement: 1.0,
            darkening: 0.0,
            topple_fx: String::new(),
            bounce_fx: String::new(),
            stump_name: String::new(),
            initial_velocity_percent: DEFAULT_INITIAL_VELOCITY_PERCENT,
            initial_accel_percent: DEFAULT_INITIAL_ACCEL_PERCENT,
            bounce_velocity_percent: DEFAULT_BOUNCE_VELOCITY_PERCENT,
            minimum_topple_speed: DEFAULT_MINIMUM_TOPPLE_SPEED,
            kill_when_toppled: true,
            do_topple: false,
            sink_frames: DEFAULT_SINK_FRAMES,
            sink_distance: DEFAULT_SINK_DISTANCE,
            do_shadow: false,
        }
    }
}

/// W3DTreeDraw implementation
///
/// Like W3DPropDraw, this is a one-shot registration with the terrain system.
/// `doDrawModule()` is a no-op. Trees are rendered by the terrain system.
#[derive(Debug)]
pub struct W3DTreeDraw {
    /// One-shot flag: whether the tree has been registered with terrain system
    tree_added: bool,
    /// Module data with all tree behavior parameters
    module_data: W3DTreeDrawModuleData,
    /// Whether hidden
    hidden: bool,
}

impl W3DTreeDraw {
    pub fn new(module_data: W3DTreeDrawModuleData) -> Self {
        Self {
            tree_added: false,
            module_data,
            hidden: false,
        }
    }

    pub fn new_default() -> Self {
        Self::new(W3DTreeDrawModuleData::default())
    }

    /// No-op. Trees are rendered by the terrain system.
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {}

    /// One-shot registration with terrain system.
    /// On first call after position is non-zero, registers via TheTerrainRenderObject->addTree().
    /// Note: scaleRandomness is forced to 0.0f (randomization done in WorldBuilder).
    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
        if self.tree_added {
            return;
        }
        // PARITY_NOTE: TheTerrainRenderObject->addTree(
        //   drawID, position, scale, orientation, 0.0f, moduleData)
        self.tree_added = true;
    }

    pub fn set_shadows_enabled(&mut self, _enable: bool) {}
    pub fn release_shadows(&mut self) {}
    pub fn allocate_shadows(&mut self) {}
    pub fn set_fully_obscured_by_shroud(&mut self, _fully_obscured: bool) {}
    pub fn react_to_geometry_change(&mut self) {}

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden
    }

    pub fn get_module_data(&self) -> &W3DTreeDrawModuleData {
        &self.module_data
    }

    pub fn crc(&self) -> u32 {
        0
    }
    pub fn xfer(&self) -> u32 {
        1
    }
    pub fn load_post_process(&mut self) {}
}

impl Default for W3DTreeDraw {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_tree_draw_basic() {
        let draw = W3DTreeDraw::new_default();
        assert_eq!(
            draw.get_module_data().minimum_topple_speed,
            DEFAULT_MINIMUM_TOPPLE_SPEED
        );
        assert_eq!(draw.get_module_data().sink_frames, DEFAULT_SINK_FRAMES);
        assert!(draw.get_module_data().kill_when_toppled);
        assert!(!draw.get_module_data().do_topple);
    }
}
