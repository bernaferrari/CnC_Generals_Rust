// C++ ownership: ScriptActions.cpp side-effect request payloads and presentation mutations queued for the host shell.

#[derive(Debug, Clone)]
pub struct ObjectiveUpdate {
    pub name: String,
    pub description: String,
    pub completed: bool,
}

#[derive(Debug, Clone)]
pub struct ScriptEffectRequest {
    pub effect_type: String,
    pub position: Vec3,
}

#[derive(Debug, Clone)]
pub struct RadarScriptEventRequest {
    pub position: Vec3,
    pub event_type: i32,
}

#[derive(Debug, Clone)]
pub struct MilitaryCaptionRequest {
    pub text: String,
    pub duration_ms: i32,
}

#[derive(Debug, Clone)]
pub struct ScriptSoundEvent {
    pub sound_name: String,
    pub position: Option<Vec3>,
}

#[derive(Debug, Clone)]
pub struct CameraFollowRequest {
    pub object_id: u32,
    pub snap_to_unit: bool,
}

#[derive(Debug, Clone)]
pub struct CameraTetherRequest {
    pub object_id: u32,
    pub snap_to_unit: bool,
    pub play: f32,
}

#[derive(Debug, Clone)]
pub struct CameraResetRequest {
    pub position: Vec3,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraZoomRequest {
    pub zoom: f32,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraPitchRequest {
    pub pitch: f32,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraRotateRequest {
    pub rotations: f32,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraModFinalZoomRequest {
    pub zoom: f32,
    pub ease_in: f32,
    pub ease_out: f32,
}

#[derive(Debug, Clone)]
pub struct CameraModFinalPitchRequest {
    pub pitch: f32,
    pub ease_in: f32,
    pub ease_out: f32,
}

#[derive(Debug, Clone)]
pub struct CameraModFinalSpeedMultiplierRequest {
    pub multiplier: i32,
}

#[derive(Debug, Clone)]
pub struct CameraModRollingAverageRequest {
    pub frames: i32,
}

#[derive(Debug, Clone)]
pub struct VisualSpeedMultiplierRequest {
    pub multiplier: i32,
}

#[derive(Debug, Clone)]
pub struct SetFpsLimitRequest {
    pub fps: i32,
}

#[derive(Debug, Clone)]
pub struct CameraSetupRequest {
    pub position: Vec3,
    pub zoom: f32,
    pub pitch: f32,
    pub look_toward: Vec3,
}

#[derive(Debug, Clone)]
pub struct CameraLookTowardObjectRequest {
    pub object_id: u32,
    pub duration_seconds: f32,
    pub hold_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraLookTowardWaypointRequest {
    pub position: Vec3,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
    pub reverse_rotation: bool,
}

#[derive(Debug, Clone)]
pub struct CameraModLookTowardRequest {
    pub position: Vec3,
}

#[derive(Debug, Clone)]
pub struct CameraModFinalLookTowardRequest {
    pub position: Vec3,
}

#[derive(Debug, Clone)]
pub struct CameraSetDefaultRequest {
    pub pitch: f32,
    pub angle: f32,
    pub max_height: f32,
}

#[derive(Debug, Clone)]
pub struct CameraSlaveModeRequest {
    pub thing_template_name: String,
    pub bone_name: String,
}

#[derive(Debug, Clone)]
pub struct ScreenShakeRequest {
    pub intensity: i32,
}

#[derive(Debug, Clone)]
pub struct CameraAddShakerRequest {
    pub position: Vec3,
    pub amplitude: f32,
    pub duration_seconds: f32,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct CameraPathRequest {
    pub waypoint: String,
    pub seconds: f32,
    pub camera_stutter_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraMoveToRequest {
    pub position: Vec3,
    pub seconds: f32,
    pub camera_stutter_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptPopupMessageRequest {
    pub message: String,
    pub x_percent: i32,
    pub y_percent: i32,
    pub width: i32,
    pub pause: bool,
    pub pause_music: bool,
    /// Opaque live-session identity assigned by `MissionScriptHooks` when the
    /// request enters its queue. It is deliberately not presentation/save/Xfer
    /// data: Main uses it only to reject a delayed acknowledgement for a popup
    /// that C++ has already replaced.
    pub popup_generation: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewGuardbandRequest {
    pub x_bias: f32,
    pub y_bias: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraBwModeRequest {
    pub enabled: bool,
    pub frames: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraMotionBlurRequest {
    Basic { zoom_in: bool, saturate: bool },
    Jump { position: Vec3, saturate: bool },
    Follow { amount: i32 },
    EndFollow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameoFlashRequest {
    pub command_button_name: String,
    pub flash_count: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NamedTimerMutation {
    Add {
        name: String,
        text: String,
        countdown: bool,
    },
    Remove {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SuperweaponObjectDisplayMutation {
    Hide { object_id: u32 },
    Show { object_id: u32 },
}

/// C++ ScriptActions NAMED_*_SPECIAL_POWER_COUNTDOWN residual.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedSpecialPowerCountdownMutation {
    pub unit_name: String,
    pub power_name: String,
    pub op: crate::game_logic::NamedSpecialPowerCountdownOp,
    pub seconds: i32,
}
