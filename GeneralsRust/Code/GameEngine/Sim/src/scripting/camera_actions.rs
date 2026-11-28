//! Defines authored camera actions from ScriptActions.cpp at the simulation boundary.

use super::interpreter::ScriptInterpreter;
use super::program::{ParameterType, ScriptAction, ScriptActionType};
use super::world::ScriptCameraRequest;
use super::{ScriptActionResult, ScriptError};
use crate::GameLogicResult;
use glam::Vec3;

/// The world driving an authored camera action supplies lookups and presentation.
pub trait ScriptCameraWorld {
    fn waypoint_position(&mut self, name: &str) -> GameLogicResult<Option<Vec3>>;
    fn named_object_id(&mut self, name: &str, interpreter: &ScriptInterpreter) -> Option<u32>;
    fn camera(&mut self, request: ScriptCameraRequest<'_>) -> GameLogicResult<()>;
    fn oversize_terrain(&mut self, amount: i32) -> GameLogicResult<()>;
}

/// Decode and execute an authored camera action at its original script slot.
pub fn try_execute_camera_action(
    action: &ScriptAction,
    interpreter: &ScriptInterpreter,
    world: &mut (impl ScriptCameraWorld + ?Sized),
) -> Result<Option<ScriptActionResult>, ScriptError> {
    use ScriptActionType::*;
    use ScriptCameraRequest as Request;

    let parameter = |index| {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::missing_parameter(index))
    };
    let name = |index| parameter(index).map(|value| value.get_string());
    let int = |index| parameter(index).map(|value| value.get_int());
    let real = |index| parameter(index).map(|value| value.get_real());
    let optional_real = |index| {
        action
            .get_parameter(index)
            .map_or(0.0, |value| value.get_real())
    };
    let optional_int = |index| {
        action
            .get_parameter(index)
            .map_or(0, |value| value.get_int())
    };
    let bool_flag = |index| optional_int(index) != 0;
    let kind = action.get_action_type();
    let request = match kind {
        MoveCameraTo => {
            let target = parameter(0)?;
            let position = if target.get_parameter_type() == ParameterType::Coord3D {
                Some(target.get_coord())
            } else {
                waypoint(world, target.get_string(), kind)
            };
            let Some(position) = position else {
                return Ok(Some(ScriptActionResult::Success));
            };
            Request::MoveTo {
                x: position.x,
                y: position.y,
                z: position.z,
                seconds: optional_real(1),
                camera_stutter_seconds: optional_real(2),
                ease_in_seconds: optional_real(3),
                ease_out_seconds: optional_real(4),
            }
        }
        MoveCameraAlongWaypointPath => Request::WaypointPath {
            waypoint_path: name(0)?,
            seconds: optional_real(1),
            camera_stutter_seconds: optional_real(2),
            ease_in_seconds: optional_real(3),
            ease_out_seconds: optional_real(4),
        },
        RotateCamera => Request::Rotate {
            rotations: real(0)?,
            seconds: optional_real(1),
            ease_in_seconds: optional_real(2),
            ease_out_seconds: optional_real(3),
        },
        MoveCameraToSelection => Request::MoveToSelection,
        CameraMoveHome => Request::MoveHome,
        ZoomCamera => Request::Zoom {
            zoom: real(0)?,
            seconds: optional_real(1),
            ease_in_seconds: optional_real(2),
            ease_out_seconds: optional_real(3),
        },
        PitchCamera => Request::Pitch {
            pitch: real(0)?,
            seconds: optional_real(1),
            ease_in_seconds: optional_real(2),
            ease_out_seconds: optional_real(3),
        },
        CameraModFreezeTime => Request::FreezeTime,
        CameraModFreezeAngle => Request::FreezeAngle,
        CameraModSetFinalZoom => Request::FinalZoom {
            zoom: real(0)?,
            ease_in: optional_real(1),
            ease_out: optional_real(2),
        },
        CameraModSetFinalPitch => Request::FinalPitch {
            pitch: real(0)?,
            ease_in: optional_real(1),
            ease_out: optional_real(2),
        },
        CameraModSetFinalSpeedMultiplier => Request::FinalSpeedMultiplier {
            multiplier: int(0)?,
        },
        CameraModSetRollingAverage => Request::RollingAverage { frames: int(0)? },
        CameraFollowNamed | CameraTetherNamed | CameraLookTowardObject => {
            let name = name(0)?;
            let Some(object_id) = world.named_object_id(name, interpreter) else {
                log::warn!("Script camera action {kind:?}: object '{name}' not found");
                return Ok(Some(ScriptActionResult::Success));
            };
            match kind {
                CameraFollowNamed => Request::FollowObject {
                    object_id,
                    snap_to_unit: bool_flag(1),
                },
                CameraTetherNamed => Request::TetherObject {
                    object_id,
                    snap_to_unit: bool_flag(1),
                    play: optional_real(2),
                },
                _ => Request::LookTowardObject {
                    object_id,
                    seconds: optional_real(1),
                    hold_seconds: optional_real(2),
                    ease_in_seconds: optional_real(3),
                    ease_out_seconds: optional_real(4),
                },
            }
        }
        CameraStopFollow | CameraStopTetherNamed => Request::StopFollow,
        ResetCamera => {
            let name = name(0)?;
            let duration_seconds = real(1)?;
            let Some(position) = waypoint(world, name, kind) else {
                return Ok(Some(ScriptActionResult::Success));
            };
            Request::ResetTo {
                x: position.x,
                y: position.y,
                z: position.z,
                duration_seconds,
                ease_in_seconds: optional_real(2),
                ease_out_seconds: optional_real(3),
            }
        }
        SetupCamera => {
            let position = name(0)?;
            let zoom = real(1)?;
            let pitch = real(2)?;
            let look_toward = name(3)?;
            // doSetupCamera queries the initial position before the look target.
            let Some(position) = waypoint(world, position, kind) else {
                return Ok(Some(ScriptActionResult::Success));
            };
            let Some(look_toward) = waypoint(world, look_toward, kind) else {
                return Ok(Some(ScriptActionResult::Success));
            };
            Request::Setup {
                x: position.x,
                y: position.y,
                z: position.z,
                zoom,
                pitch,
                look_toward_x: look_toward.x,
                look_toward_y: look_toward.y,
                look_toward_z: look_toward.z,
            }
        }
        CameraLookTowardWaypoint
        | CameraModLookToward
        | CameraModFinalLookToward
        | CameraMotionBlurJump => {
            let name = name(0)?;
            let Some(Vec3 { x, y, z }) = waypoint(world, name, kind) else {
                return Ok(Some(ScriptActionResult::Success));
            };
            match kind {
                CameraLookTowardWaypoint => Request::LookTowardWaypoint {
                    x,
                    y,
                    z,
                    seconds: optional_real(1),
                    ease_in_seconds: optional_real(2),
                    ease_out_seconds: optional_real(3),
                    reverse_rotation: bool_flag(4),
                },
                CameraModLookToward => Request::LookToward { x, y, z },
                CameraModFinalLookToward => Request::FinalLookToward { x, y, z },
                _ => Request::MotionBlurJump {
                    x,
                    y,
                    z,
                    saturate: bool_flag(1),
                },
            }
        }
        CameraLetterboxBegin => Request::LetterboxBegin,
        CameraLetterboxEnd => Request::LetterboxEnd,
        CameraSetDefault => Request::SetDefault {
            pitch: real(0)?,
            angle: real(1)?,
            max_height: real(2)?,
        },
        CameraEnableSlaveMode => Request::EnableSlaveMode {
            thing_template_name: name(0)?,
            bone_name: name(1)?,
        },
        CameraDisableSlaveMode => Request::DisableSlaveMode,
        ScreenShake => Request::ScreenShake { intensity: int(0)? },
        CameraAddShakerAt => {
            let name = name(0)?;
            let amplitude = real(1)?;
            let duration_seconds = real(2)?;
            let radius = real(3)?;
            let Some(Vec3 { x, y, z }) = waypoint(world, name, kind) else {
                return Ok(Some(ScriptActionResult::Success));
            };
            Request::AddShakerAt {
                x,
                y,
                z,
                amplitude,
                duration_seconds,
                radius,
            }
        }
        ResizeViewGuardband => Request::ViewGuardband {
            gbx: real(0)?,
            gby: real(1)?,
        },
        CameraBwModeBegin | CameraBwModeEnd => Request::BwMode {
            enabled: kind == CameraBwModeBegin,
            frames: optional_int(0),
        },
        DrawSkyboxBegin | DrawSkyboxEnd => Request::SkyboxEnabled {
            enabled: kind == DrawSkyboxBegin,
        },
        CameraMotionBlur => Request::MotionBlur {
            zoom_in: bool_flag(0),
            saturate: bool_flag(1),
        },
        CameraMotionBlurFollow => Request::MotionBlurFollow { amount: int(0)? },
        CameraMotionBlurEndFollow => Request::MotionBlurEndFollow,
        CameraSetAudibleDistance => {
            // doCameraSetAudibleDistance is a no-op in ScriptActions.cpp.
            real(0)?;
            return Ok(Some(ScriptActionResult::Success));
        }
        OversizeTerrain => {
            if let Err(error) = world.oversize_terrain(int(0)?) {
                log::warn!("Script action handler oversize_terrain failed: {error}");
            }
            return Ok(Some(ScriptActionResult::Success));
        }
        _ => return Ok(None),
    };

    // A receiving presentation error still lets the script continue to its next
    // action. The world already chose the applicable presentation callback.
    if let Err(error) = world.camera(request) {
        log::warn!("Script camera action {request:?} failed: {error}");
    }
    Ok(Some(ScriptActionResult::Success))
}

fn waypoint(
    world: &mut (impl ScriptCameraWorld + ?Sized),
    name: &str,
    kind: ScriptActionType,
) -> Option<Vec3> {
    match world.waypoint_position(name) {
        Ok(Some(position)) => Some(position),
        Ok(None) => {
            log::warn!("Script camera action {kind:?}: waypoint '{name}' not found");
            None
        }
        Err(error) => {
            log::warn!("Script camera action {kind:?}: waypoint '{name}' query failed: {error}");
            None
        }
    }
}
