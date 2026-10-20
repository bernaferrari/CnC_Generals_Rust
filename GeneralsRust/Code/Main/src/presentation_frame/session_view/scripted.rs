//! Scripted movement state belongs to the same SessionView as native pose.
//! Transport queues and positional save records remain separate adapters.
use super::waypoint::WaypointMotion;
use crate::game_logic::ObjectId;
use glam::Vec3;

/// Detached historical tracking observation used only by frozen DTO adapters.
/// The live source payload belongs to SessionCamera.rotation_registers.
#[derive(Debug, Clone)]
pub(crate) struct CameraObjectTracking {
    pub(crate) hold_frames: u32,
    pub(crate) last_position: Vec3,
}
impl CameraObjectTracking {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.hold_frames > i32::MAX as u32 / 33 || !self.last_position.is_finite() {
            Err("invalid camera object tracking continuation".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub(crate) struct ScriptCameraState {
    pub(crate) motion: WaypointMotion,
    pub(crate) path_arrived: bool,
    pub(crate) camera_follow_target: Option<ObjectId>,
    pub(crate) camera_tether_play: Option<f32>,
    pub(crate) script_default_camera_pitch: f32,
    pub(crate) script_default_camera_angle: f32,
    pub(crate) script_default_camera_max_height: f32,
    pub(crate) script_camera_freeze_time: bool,
}

impl Default for ScriptCameraState {
    fn default() -> Self {
        Self {
            motion: WaypointMotion::default(),
            path_arrived: false,
            camera_follow_target: None,
            camera_tether_play: None,
            script_default_camera_pitch: 0.0,
            script_default_camera_angle: 0.0,
            script_default_camera_max_height: 1.0,
            script_camera_freeze_time: false,
        }
    }
}
