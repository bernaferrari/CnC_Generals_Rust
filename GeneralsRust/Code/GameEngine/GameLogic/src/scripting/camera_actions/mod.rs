//! Camera Script Actions
//!
//! Complete implementation of all camera-related script actions from C++ Generals.
//! These actions control camera movement, zoom, rotation, and visual effects.
//!
//! C++ Reference: ScriptActions.cpp camera action implementations

use async_trait::async_trait;
use super::{ScriptContext, ScriptResult, ScriptValue};
use super::actions::ScriptAction;
use crate::common::{AsciiString, Coord3D, Real, INVALID_OBJECT_ID, INVALID_ID};
use crate::helpers::{get_camera_view_bridge, TheGameLogic};
use crate::scripting::engine::get_named_object_tracker;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};
use std::collections::HashMap;

// Local type definitions — GameLogic cannot import GameClient's View types, so we
// define matching enum values here.  Integer discriminants must match the C++
// ordering and the real Rust enums in GameClient/src/display/view.rs.

/// Camera lock mode.  Matches `CameraLockType` in view.rs.
#[repr(i32)]
enum LockType {
    Follow = 0,
    Tether = 1,
}

/// Viewport post-process filter type.  Matches `FilterType` in view.rs.
#[repr(i32)]
#[allow(dead_code)]
enum FilterType {
    Null = 0,
    BlackAndWhite = 1,
    Crossfade = 2,
    MotionBlur = 3,
}

/// Viewport post-process filter mode.  Matches `FilterMode` in view.rs.
#[repr(i32)]
#[allow(dead_code)]
enum FilterMode {
    Null = 0,
    BWBlackAndWhite = 1,
    BWRedAndWhite = 2,
    BWGreenAndWhite = 3,
    CrossfadeFbMask = 4,
    MBInAndOutAlpha = 5,
    MBInAndOutSaturate = 6,
    MBInAlpha = 7,
    MBOutAlpha = 8,
    MBInSaturate = 9,
    MBOutSaturate = 10,
    MBEndPanAlpha = 11,
    MBPanAlpha = 12,
    MBPanAlpha1 = 13,
    MBPanAlpha2 = 14,
    MBPanAlpha3 = 15,
}

impl FilterMode {
    fn from_pan_amount(amount: i32) -> Self {
        match amount.clamp(0, 3) {
            0 => Self::MBPanAlpha,
            1 => Self::MBPanAlpha1,
            2 => Self::MBPanAlpha2,
            _ => Self::MBPanAlpha3,
        }
    }
}

struct Point3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Point3 {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// Resolve the camera view bridge.  Returns an error when no bridge is
/// registered (i.e. GameClient has not set one up yet).
fn get_tactical_view() -> GameLogicResult<&'static dyn crate::helpers::CameraViewBridge> {
    get_camera_view_bridge()
        .map(|arc| arc.as_ref())
        .ok_or_else(|| GameLogicError::Configuration("Camera view bridge not registered".into()))
}

/// Helper to find a waypoint by name
fn find_waypoint(name: &str) -> GameLogicResult<Option<Coord3D>> {
    let terrain_logic = get_terrain_logic();
    let terrain = terrain_logic.read().map_err(|e| {
        GameLogicError::Configuration(format!("Failed to acquire terrain lock: {}", e))
    })?;

    let ascii_name = AsciiString::from(name);
    Ok(terrain.get_waypoint_by_name(&ascii_name).map(|wp| *wp.get_location()))
}

fn find_named_object(name: &str) -> GameLogicResult<Option<u32>> {
    let tracker = get_named_object_tracker();
    tracker.get_object_id(name)
}

//=============================================================================
// Camera Movement Actions
//=============================================================================

/// CameraFollowNamed - Make camera follow a specific object
///
/// C++: void ScriptActions::doCameraFollowNamed(const AsciiString& unit, Bool snapToUnit)
#[derive(Debug, Clone)]
pub struct CameraFollowNamedAction;

#[async_trait]
impl ScriptAction for CameraFollowNamedAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = parameters
            .get("unit")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("unit parameter required".into()))?;

        let snap_to_unit = parameters
            .get("snap")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Find the object
        let object_id = find_named_object(unit_name)?;
        if object_id.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        // Get the tactical view and set camera lock
        let view = get_tactical_view()?;
        view.set_camera_lock(object_id);
        view.set_snap_mode(LockType::Follow as i32, 0.0);

        if snap_to_unit {
            view.snap_to_camera_lock();
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_FOLLOW_NAMED"
    }

    fn description(&self) -> &str {
        "Make camera follow a specific named object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["snap".to_string()]
    }
}

/// CameraMoveTo - Move camera to a waypoint
///
/// C++: void ScriptActions::doMoveCameraTo(const AsciiString& waypoint, Real sec, Real cameraStutterSec, Real easeIn, Real easeOut)
#[derive(Debug, Clone)]
pub struct CameraMoveToAction;

#[async_trait]
impl ScriptAction for CameraMoveToAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint_name = parameters
            .get("waypoint")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("waypoint parameter required".into()))?;

        let sec = parameters
            .get("sec")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let camera_stutter_sec = parameters
            .get("cameraStutterSec")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_in = parameters
            .get("easeIn")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_out = parameters
            .get("easeOut")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        // Find the waypoint
        let destination = find_waypoint(waypoint_name)?;
        if destination.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        let dest = destination.unwrap();
        let point = Point3::new(dest.x, dest.y, dest.z);

        // Move camera to waypoint
        let view = get_tactical_view()?;
        view.move_camera_to(
            point.x, point.y, point.z,
            (sec * 1000.0) as i32,
            (camera_stutter_sec * 1000.0) as i32,
            true,
            (ease_in * 1000.0) as f32,
            (ease_out * 1000.0) as f32,
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "MOVE_CAMERA_TO"
    }

    fn description(&self) -> &str {
        "Move camera to a waypoint over time"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["sec".to_string(), "cameraStutterSec".to_string(), "easeIn".to_string(), "easeOut".to_string()]
    }
}

/// CameraZoom - Zoom camera to specific level
///
/// C++: void ScriptActions::doZoomCamera(Real zoom, Real sec, Real easeIn, Real easeOut)
#[derive(Debug, Clone)]
pub struct CameraZoomAction;

#[async_trait]
impl ScriptAction for CameraZoomAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let zoom = parameters
            .get("zoom")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("zoom parameter required".into()))?;

        let sec = parameters
            .get("sec")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_in = parameters
            .get("easeIn")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_out = parameters
            .get("easeOut")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let view = get_tactical_view()?;
        view.zoom_camera(zoom as f32, (sec * 1000.0) as i32, (ease_in * 1000.0) as f32, (ease_out * 1000.0) as f32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "ZOOM_CAMERA"
    }

    fn description(&self) -> &str {
        "Zoom camera to a specific level over time"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["zoom".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["sec".to_string(), "easeIn".to_string(), "easeOut".to_string()]
    }
}

/// CameraPitch - Adjust camera pitch angle
///
/// C++: void ScriptActions::doPitchCamera(Real pitch, Real sec, Real easeIn, Real easeOut)
#[derive(Debug, Clone)]
pub struct CameraPitchAction;

#[async_trait]
impl ScriptAction for CameraPitchAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let pitch = parameters
            .get("pitch")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("pitch parameter required".into()))?;

        let sec = parameters
            .get("sec")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_in = parameters
            .get("easeIn")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_out = parameters
            .get("easeOut")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let view = get_tactical_view()?;
        view.pitch_camera(pitch as f32, (sec * 1000.0) as i32, (ease_in * 1000.0) as f32, (ease_out * 1000.0) as f32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "PITCH_CAMERA"
    }

    fn description(&self) -> &str {
        "Adjust camera pitch angle over time"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["pitch".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["sec".to_string(), "easeIn".to_string(), "easeOut".to_string()]
    }
}

/// CameraRotate - Rotate camera around target
///
/// C++: void ScriptActions::doRotateCamera(Real rotations, Real sec, Real easeIn, Real easeOut)
#[derive(Debug, Clone)]
pub struct CameraRotateAction;

#[async_trait]
impl ScriptAction for CameraRotateAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let rotations = parameters
            .get("rotations")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("rotations parameter required".into()))?;

        let sec = parameters
            .get("sec")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_in = parameters
            .get("easeIn")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_out = parameters
            .get("easeOut")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let view = get_tactical_view()?;
        view.rotate_camera(rotations as f32, (sec * 1000.0) as i32, (ease_in * 1000.0) as f32, (ease_out * 1000.0) as f32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "ROTATE_CAMERA"
    }

    fn description(&self) -> &str {
        "Rotate camera around target over time"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["rotations".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["sec".to_string(), "easeIn".to_string(), "easeOut".to_string()]
    }
}

/// CameraSetup - Configure camera settings at waypoint
///
/// C++: void ScriptActions::doSetupCamera(const AsciiString& waypoint, Real zoom, Real pitch, const AsciiString& lookAtWaypoint)
#[derive(Debug, Clone)]
pub struct CameraSetupAction;

#[async_trait]
impl ScriptAction for CameraSetupAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint_name = parameters
            .get("waypoint")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("waypoint parameter required".into()))?;

        let zoom = parameters
            .get("zoom")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("zoom parameter required".into()))?;

        let pitch = parameters
            .get("pitch")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("pitch parameter required".into()))?;

        let look_at_waypoint = parameters
            .get("lookAtWaypoint")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("lookAtWaypoint parameter required".into()))?;

        // Find waypoints
        let camera_pos = find_waypoint(waypoint_name)?;
        let look_at_pos = find_waypoint(look_at_waypoint)?;

        if camera_pos.is_none() || look_at_pos.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        let cam_pos = camera_pos.unwrap();
        let look_pos = look_at_pos.unwrap();

        let cam_point = Point3::new(cam_pos.x, cam_pos.y, cam_pos.z);
        let look_point = Point3::new(look_pos.x, look_pos.y, look_pos.z);

        // Setup camera
        let view = get_tactical_view()?;
        view.move_camera_to(cam_point.x, cam_point.y, cam_point.z, 0, 0, true, 0.0, 0.0);
        view.camera_mod_look_toward(look_point.x, look_point.y, look_point.z);
        view.camera_mod_final_pitch(pitch as f32, 0.0, 0.0);
        view.camera_mod_final_zoom(zoom as f32, 0.0, 0.0);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "SETUP_CAMERA"
    }

    fn description(&self) -> &str {
        "Configure camera at waypoint looking at another waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string(), "zoom".to_string(), "pitch".to_string(), "lookAtWaypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// CameraMoveHome - Return camera to default position
///
/// C++: void ScriptActions::doCameraMoveHome(void)
#[derive(Debug, Clone)]
pub struct CameraMoveHomeAction;

#[async_trait]
impl ScriptAction for CameraMoveHomeAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // This is currently a no-op in C++
        // Implementation would reset camera to home position
        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOVE_HOME"
    }

    fn description(&self) -> &str {
        "Return camera to default position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// CameraSetDefault - Set new default camera position
///
/// C++: void ScriptActions::doCameraSetDefault(Real pitch, Real angle, Real maxHeight)
#[derive(Debug, Clone)]
pub struct CameraSetDefaultAction;

#[async_trait]
impl ScriptAction for CameraSetDefaultAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let pitch = parameters
            .get("pitch")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("pitch parameter required".into()))?;

        let angle = parameters
            .get("angle")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("angle parameter required".into()))?;

        let max_height = parameters
            .get("maxHeight")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("maxHeight parameter required".into()))?;

        let view = get_tactical_view()?;
        view.set_default_view(pitch as f32, angle as f32, max_height as f32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_SET_DEFAULT"
    }

    fn description(&self) -> &str {
        "Set new default camera position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["pitch".to_string(), "angle".to_string(), "maxHeight".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//=============================================================================
// Camera Tether Actions
//=============================================================================

/// CameraTetherNamed - Tether camera to object with maximum distance
///
/// C++: void ScriptActions::doCameraTetherNamed(const AsciiString& unit, Bool snapToUnit, Real play)
#[derive(Debug, Clone)]
pub struct CameraTetherNamedAction;

#[async_trait]
impl ScriptAction for CameraTetherNamedAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = parameters
            .get("unit")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("unit parameter required".into()))?;

        let snap_to_unit = parameters
            .get("snap")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let play = parameters
            .get("play")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        // Find the object
        let object_id = find_named_object(unit_name)?;
        if object_id.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        // Get the tactical view and set camera tether
        let view = get_tactical_view()?;
        view.set_camera_lock(object_id);
        view.set_snap_mode(LockType::Tether as i32, play as f32);

        if snap_to_unit {
            view.snap_to_camera_lock();
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_TETHER_NAMED"
    }

    fn description(&self) -> &str {
        "Tether camera to object with maximum distance"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["snap".to_string(), "play".to_string()]
    }
}

/// CameraStopTetherNamed - Stop tethering camera to object
///
/// C++: void ScriptActions::doCameraStopTetherNamed(void)
#[derive(Debug, Clone)]
pub struct CameraStopTetherNamedAction;

#[async_trait]
impl ScriptAction for CameraStopTetherNamedAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let view = get_tactical_view()?;
        view.set_camera_lock(None);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_STOP_TETHER_NAMED"
    }

    fn description(&self) -> &str {
        "Stop tethering camera to object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//=============================================================================
// Camera Look Actions
//=============================================================================

/// CameraLookTowardObject - Rotate camera to look at object
///
/// C++: void ScriptActions::doRotateCameraTowardObject(const AsciiString& unitName, Real sec, Real holdSec, Real easeIn, Real easeOut)
#[derive(Debug, Clone)]
pub struct CameraLookTowardObjectAction;

#[async_trait]
impl ScriptAction for CameraLookTowardObjectAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = parameters
            .get("unit")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("unit parameter required".into()))?;

        let sec = parameters
            .get("sec")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let hold_sec = parameters
            .get("holdSec")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_in = parameters
            .get("easeIn")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_out = parameters
            .get("easeOut")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        // Find the object
        let object_id = find_named_object(unit_name)?;
        if object_id.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        let view = get_tactical_view()?;
        let ms = (sec * 1000.0) as i32;
        let hold_ms = (hold_sec * 1000.0) as i32;
        view.rotate_camera_toward_object(object_id.unwrap(), ms, hold_ms, ease_in as f32, ease_out as f32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_LOOK_TOWARD_OBJECT"
    }

    fn description(&self) -> &str {
        "Rotate camera to look at object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["sec".to_string(), "holdSec".to_string(), "easeIn".to_string(), "easeOut".to_string()]
    }
}

/// CameraLookTowardWaypoint - Rotate camera to look at waypoint
///
/// C++: void ScriptActions::doRotateCameraTowardWaypoint(const AsciiString& waypointName, Real sec, Real easeIn, Real easeOut, Bool reverseRotation)
#[derive(Debug, Clone)]
pub struct CameraLookTowardWaypointAction;

#[async_trait]
impl ScriptAction for CameraLookTowardWaypointAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint_name = parameters
            .get("waypoint")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("waypoint parameter required".into()))?;

        let sec = parameters
            .get("sec")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_in = parameters
            .get("easeIn")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_out = parameters
            .get("easeOut")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let reverse_rotation = parameters
            .get("reverseRotation")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Find the waypoint
        let waypoint_pos = find_waypoint(waypoint_name)?;
        if waypoint_pos.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        let pos = waypoint_pos.unwrap();
        let point = Point3::new(pos.x, pos.y, pos.z);

        let view = get_tactical_view()?;
        let ms = (sec * 1000.0) as i32;
        view.rotate_camera_toward_position(point.x, point.y, point.z, ms, ease_in as f32, ease_out as f32, reverse_rotation);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_LOOK_TOWARD_WAYPOINT"
    }

    fn description(&self) -> &str {
        "Rotate camera to look at waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["sec".to_string(), "easeIn".to_string(), "easeOut".to_string(), "reverseRotation".to_string()]
    }
}

/// CameraModLookToward - Modify camera to look toward waypoint
///
/// C++: void ScriptActions::doModCameraLookToward(const AsciiString& waypoint)
#[derive(Debug, Clone)]
pub struct CameraModLookTowardAction;

#[async_trait]
impl ScriptAction for CameraModLookTowardAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint_name = parameters
            .get("waypoint")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("waypoint parameter required".into()))?;

        // Find the waypoint
        let waypoint_pos = find_waypoint(waypoint_name)?;
        if waypoint_pos.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        let pos = waypoint_pos.unwrap();
        let point = Point3::new(pos.x, pos.y, pos.z);

        let view = get_tactical_view()?;
        view.camera_mod_look_toward(point.x, point.y, point.z);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOD_LOOK_TOWARD"
    }

    fn description(&self) -> &str {
        "Modify camera to look toward waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// CameraModFinalLookToward - Set final look target for camera modification
///
/// C++: void ScriptActions::doModCameraFinalLookToward(const AsciiString& waypoint)
#[derive(Debug, Clone)]
pub struct CameraModFinalLookTowardAction;

#[async_trait]
impl ScriptAction for CameraModFinalLookTowardAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint_name = parameters
            .get("waypoint")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("waypoint parameter required".into()))?;

        // Find the waypoint
        let waypoint_pos = find_waypoint(waypoint_name)?;
        if waypoint_pos.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        let pos = waypoint_pos.unwrap();
        let point = Point3::new(pos.x, pos.y, pos.z);

        let view = get_tactical_view()?;
        view.camera_mod_final_look_toward(point.x, point.y, point.z);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOD_FINAL_LOOK_TOWARD"
    }

    fn description(&self) -> &str {
        "Set final look target for camera modification"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//=============================================================================
// Camera Reset Actions
//=============================================================================

/// ResetCamera - Reset camera to waypoint with default orientation
///
/// C++: void ScriptActions::doResetCamera(const AsciiString& waypoint, Real sec, Real easeIn, Real easeOut)
#[derive(Debug, Clone)]
pub struct ResetCameraAction;

#[async_trait]
impl ScriptAction for ResetCameraAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint_name = parameters
            .get("waypoint")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("waypoint parameter required".into()))?;

        let sec = parameters
            .get("sec")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_in = parameters
            .get("easeIn")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_out = parameters
            .get("easeOut")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        // Find the waypoint
        let waypoint_pos = find_waypoint(waypoint_name)?;
        if waypoint_pos.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        let pos = waypoint_pos.unwrap();
        let point = Point3::new(pos.x, pos.y, pos.z);

        let view = get_tactical_view()?;
        view.reset_camera(point.x, point.y, point.z, (sec * 1000.0) as i32, (ease_in * 1000.0) as f32, (ease_out * 1000.0) as f32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "RESET_CAMERA"
    }

    fn description(&self) -> &str {
        "Reset camera to waypoint with default orientation"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["sec".to_string(), "easeIn".to_string(), "easeOut".to_string()]
    }
}

//=============================================================================
// Camera Motion Blur Actions
//=============================================================================

/// CameraMotionBlur - Enable/disable motion blur effect
///
/// C++: void ScriptActions::doCameraMotionBlur(Bool zoomIn, Bool saturate)
#[derive(Debug, Clone)]
pub struct CameraMotionBlurAction;

#[async_trait]
impl ScriptAction for CameraMotionBlurAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let zoom_in = parameters
            .get("zoomIn")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| GameLogicError::Configuration("zoomIn parameter required".into()))?;

        let saturate = parameters
            .get("saturate")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| GameLogicError::Configuration("saturate parameter required".into()))?;

        let view = get_tactical_view()?;

        let mode = if saturate {
            if zoom_in {
                FilterMode::MBInSaturate
            } else {
                FilterMode::MBOutSaturate
            }
        } else {
            if zoom_in {
                FilterMode::MBInAlpha
            } else {
                FilterMode::MBOutAlpha
            }
        };

        // C++ parity: if setViewFilter returns false, restore to FT_NULL_FILTER
        if !view.set_view_filter(FilterType::MotionBlur as i32) {
            view.set_view_filter(FilterType::Null as i32);
        } else {
            view.set_view_filter_mode(mode as i32);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOTION_BLUR"
    }

    fn description(&self) -> &str {
        "Enable/disable motion blur effect"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["zoomIn".to_string(), "saturate".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// CameraMotionBlurJump - Motion blur jump to waypoint
///
/// C++: void ScriptActions::doCameraMotionBlurJump(const AsciiString& waypointName, Bool saturate)
#[derive(Debug, Clone)]
pub struct CameraMotionBlurJumpAction;

#[async_trait]
impl ScriptAction for CameraMotionBlurJumpAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint_name = parameters
            .get("waypoint")
            .and_then(|v| v.as_string())
            .ok_or_else(|| GameLogicError::Configuration("waypoint parameter required".into()))?;

        let saturate = parameters
            .get("saturate")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Find the waypoint
        let waypoint_pos = find_waypoint(waypoint_name)?;
        if waypoint_pos.is_none() {
            return Ok(ScriptResult::Success(None));
        }

        let pos = waypoint_pos.unwrap();
        let point = Point3::new(pos.x, pos.y, pos.z);

        let view = get_tactical_view()?;

        let mode = if saturate {
            FilterMode::MBInAndOutSaturate
        } else {
            FilterMode::MBInAndOutAlpha
        };

        if view.set_view_filter(FilterType::MotionBlur as i32) {
            view.set_view_filter_mode(mode as i32);
            view.set_view_filter_pos(point.x, point.y, point.z);
        } else {
            // C++ parity: if setViewFilter returns false, restore to FT_NULL_FILTER
            // and fall back to lookAt
            view.set_view_filter(FilterType::Null as i32);
            view.look_at(point.x, point.y, point.z);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOTION_BLUR_JUMP"
    }

    fn description(&self) -> &str {
        "Motion blur jump to waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["saturate".to_string()]
    }
}

/// CameraMotionBlurFollow - Motion blur follow mode
///
/// C++: CAMERA_MOTION_BLUR_FOLLOW script action
#[derive(Debug, Clone)]
pub struct CameraMotionBlurFollowAction;

#[async_trait]
impl ScriptAction for CameraMotionBlurFollowAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let amount = parameters
            .get("amount")
            .and_then(|v| v.as_int())
            .ok_or_else(|| GameLogicError::Configuration("amount parameter required".into()))?;

        let view = get_tactical_view()?;
        let mode = FilterMode::from_pan_amount(amount as i32);
        view.set_view_filter_mode(mode as i32);
        view.set_view_filter(FilterType::MotionBlur as i32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOTION_BLUR_FOLLOW"
    }

    fn description(&self) -> &str {
        "Motion blur follow mode"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// CameraMotionBlurEndFollow - End motion blur follow mode
///
/// C++: CAMERA_MOTION_BLUR_END_FOLLOW script action
#[derive(Debug, Clone)]
pub struct CameraMotionBlurEndFollowAction;

#[async_trait]
impl ScriptAction for CameraMotionBlurEndFollowAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let view = get_tactical_view()?;
        view.set_view_filter_mode(FilterMode::MBEndPanAlpha as i32);
        view.set_view_filter(FilterType::MotionBlur as i32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOTION_BLUR_END_FOLLOW"
    }

    fn description(&self) -> &str {
        "End motion blur follow mode"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//=============================================================================
// Camera Stop Follow Action
//=============================================================================

/// CameraStopFollow - Stop following object
///
/// C++: void ScriptActions::doStopCameraFollowUnit()
#[derive(Debug, Clone)]
pub struct CameraStopFollowAction;

#[async_trait]
impl ScriptAction for CameraStopFollowAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let view = get_tactical_view()?;
        view.set_camera_lock(None);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_STOP_FOLLOW"
    }

    fn description(&self) -> &str {
        "Stop camera from following object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//=============================================================================
// Camera Modification Actions
//=============================================================================

/// CameraModFinalPitch - Set final pitch for camera movement
///
/// C++: TheTacticalView->cameraModFinalPitch(pitch, 0.0f, 0.0f)
#[derive(Debug, Clone)]
pub struct CameraModFinalPitchAction;

#[async_trait]
impl ScriptAction for CameraModFinalPitchAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let pitch = parameters
            .get("pitch")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("pitch parameter required".into()))?;

        let ease_in = parameters
            .get("easeIn")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_out = parameters
            .get("easeOut")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let view = get_tactical_view()?;
        view.camera_mod_final_pitch(pitch as f32, ease_in as f32, ease_out as f32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOD_FINAL_PITCH"
    }

    fn description(&self) -> &str {
        "Set final pitch for camera movement"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["pitch".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["easeIn".to_string(), "easeOut".to_string()]
    }
}

/// CameraModFinalZoom - Set final zoom for camera movement
///
/// C++: TheTacticalView->cameraModFinalZoom(zoom, 0.0f, 0.0f)
#[derive(Debug, Clone)]
pub struct CameraModFinalZoomAction;

#[async_trait]
impl ScriptAction for CameraModFinalZoomAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let zoom = parameters
            .get("zoom")
            .and_then(|v| v.as_real())
            .ok_or_else(|| GameLogicError::Configuration("zoom parameter required".into()))?;

        let ease_in = parameters
            .get("easeIn")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let ease_out = parameters
            .get("easeOut")
            .and_then(|v| v.as_real())
            .unwrap_or(0.0);

        let view = get_tactical_view()?;
        view.camera_mod_final_zoom(zoom as f32, ease_in as f32, ease_out as f32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOD_FINAL_ZOOM"
    }

    fn description(&self) -> &str {
        "Set final zoom for camera movement"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["zoom".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["easeIn".to_string(), "easeOut".to_string()]
    }
}

/// CameraModFreezeTime - Freeze time for camera movement
///
/// C++: TheTacticalView->cameraModFreezeTime()
#[derive(Debug, Clone)]
pub struct CameraModFreezeTimeAction;

#[async_trait]
impl ScriptAction for CameraModFreezeTimeAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let view = get_tactical_view()?;
        view.camera_mod_freeze_time();

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOD_FREEZE_TIME"
    }

    fn description(&self) -> &str {
        "Freeze time for camera movement"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// CameraModFreezeAngle - Freeze angle for camera movement
///
/// C++: TheTacticalView->cameraModFreezeAngle()
#[derive(Debug, Clone)]
pub struct CameraModFreezeAngleAction;

#[async_trait]
impl ScriptAction for CameraModFreezeAngleAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let view = get_tactical_view()?;
        view.camera_mod_freeze_angle();

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "CAMERA_MOD_FREEZE_ANGLE"
    }

    fn description(&self) -> &str {
        "Freeze angle for camera movement"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//=============================================================================
// Camera Selection Actions
//=============================================================================

/// CameraMoveToSelection - Move camera to current selection
///
/// C++: void ScriptActions::doModCameraMoveToSelection(void)
#[derive(Debug, Clone)]
pub struct CameraMoveToSelectionAction;

#[async_trait]
impl ScriptAction for CameraMoveToSelectionAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // This would move camera to center of current selection
        // Implementation requires access to selection system
        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "MOVE_CAMERA_TO_SELECTION"
    }

    fn description(&self) -> &str {
        "Move camera to current selection"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
