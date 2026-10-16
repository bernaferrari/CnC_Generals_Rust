//! Native continuation of the session camera and native input adapter. Original View v1
//! is unchanged; this payload retains state that original Xfer did not promise.
use super::camera_height::CameraHeightContinuation;
use super::mouse::LookAtContinuation;
use super::native_display::{NativeDisplayContinuation, NativeDisplayState};
use super::{CnCGameEngine, ScriptCameraShaker};
use crate::presentation_frame::camera::SessionCamera;
use crate::save_load::camera_codec::CameraCodec;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const MAX_BYTES: usize = 1_048_576;

/// Private validated fields prevent mutation between detached decoding and commit.
pub(crate) struct NativeCameraContinuation(
    CameraState,
    #[cfg(feature = "game_client")] Option<(glam::Vec2, Vec3)>,
);

#[derive(Serialize, Deserialize)]
struct CameraState {
    completed_eye: Vec3,
    completed_forward: Vec3,
    completed_up: Vec3,
    height: CameraHeightContinuation,
    look_at: LookAtContinuation,
    display: NativeDisplayContinuation,
    slave: Option<(String, String)>,
    cursor: Option<String>,
    control_group_click: Option<(u8, u64)>,
    camera_position: Vec3,
    camera_target: Vec3,
    scripted_camera_constraint_widen: Option<(f32, f32, f32, f32)>,
    camera_zoom: f32,
    camera_zoom_target: Option<f32>,
    camera_zoom_start: f32,
    camera_zoom_duration: f32,
    camera_zoom_frame: u32,
    camera_zoom_ease_in: f32,
    camera_zoom_ease_out: f32,
    camera_pitch_radians: f32,
    camera_pitch_target: Option<f32>,
    camera_pitch_start: f32,
    camera_pitch_duration: f32,
    camera_pitch_frame: u32,
    camera_pitch_ease_in: f32,
    camera_pitch_ease_out: f32,
    camera_fx_pitch: f32,
    camera_yaw_radians: f32,
    camera_yaw_target: Option<f32>,
    camera_yaw_start: f32,
    camera_yaw_duration: f32,
    camera_yaw_frame: u32,
    camera_yaw_ease_in: f32,
    camera_yaw_ease_out: f32,
    camera_shake_offset: Vec3,
    camera_shake_rotation: Vec3,
    screen_shake_intensity: f32,
    screen_shake_angle_cos: f32,
    screen_shake_angle_sin: f32,
    script_camera_shakers: Vec<ScriptCameraShaker>,
    script_fps_limit: Option<u32>,
    camera_follow_factor: f32,
    host_match_camera_follow_active: Option<bool>,
    host_match_camera_follow_position: Option<[f32; 3]>,
    host_match_camera_follow_id: Option<Option<crate::game_logic::ObjectId>>,
    // Historical Native85 observation slots: keep their exact positions and
    // types, but never admit them as another mutable camera-default owner.
    host_match_script_camera_max_height: Option<f32>,
    host_match_script_camera_pitch: Option<f32>,
    keys_pressed: std::collections::HashSet<winit::keyboard::Key>,
    mouse_position: (f32, f32),
    mouse_cursor_seen: bool,
    mouse_world_position: Vec3,
    sticky_waypoint_mode: bool,
    sticky_auto_attack: bool,
    use_alternate_mouse: bool,
    is_dragging: bool,
    selection_start: Option<Vec3>,
    selection_start_screen: Option<(f32, f32)>,
    last_click_position: Option<(f32, f32)>,
    last_right_click_screen: Option<(f32, f32)>,
    left_click_release_behavior: super::LeftMouseReleaseBehavior,
    displayed_max_selection_warning: bool,
    lmb_context_started_physically: bool,
    rmb_scroll_anchor: Option<(f32, f32)>,
    is_rmb_scrolling: bool,
    move_rmb_scroll_anchor: bool,
    draw_rmb_scroll_anchor: bool,
    rmb_scroll_started_physically: bool,
    rmb_deselect_down_screen: Option<(f32, f32)>,
    rmb_deselect_down_camera: Option<Vec3>,
    is_mmb_rotating: bool,
    mmb_anchor: Option<(f32, f32)>,
    selected_objects: Vec<crate::game_logic::ObjectId>,
    camera_view_bookmarks: [Option<Vec3>; 8],
    camera_rotate_left_held: bool,
    camera_rotate_right_held: bool,
    camera_zoom_in_held: bool,
    camera_zoom_out_held: bool,
    camera_tracking_selection: bool,
    replay_fast_forward: bool,
    prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click: bool,
    frame_counter: u32,
    script_fps_limit_last_tick_age: Option<u64>,
    last_click_time_age: Option<u64>,
    last_right_click_time_age: Option<u64>,
    rmb_deselect_down_at_age: Option<u64>,
}

fn client_frame(engine: &CnCGameEngine) -> u32 {
    #[cfg(feature = "game_client")]
    {
        engine.game_client.get_frame()
    }
    #[cfg(not(feature = "game_client"))]
    {
        engine.host_match_logic_frame.unwrap_or(0)
    }
}

fn codec() -> CameraCodec<MAX_BYTES> {
    CameraCodec
}
fn age(now: Instant, then: Option<Instant>) -> Result<Option<u64>, String> {
    then.map(|value| {
        u64::try_from(now.saturating_duration_since(value).as_nanos())
            .map_err(|_| "camera input age exceeds native range".to_owned())
    })
    .transpose()
}
fn restore_age(now: Instant, age: Option<u64>) -> Option<Instant> {
    age.map(|nanos| {
        now.checked_sub(Duration::from_nanos(nanos))
            .expect("validated monotonic camera input age")
    })
}
impl CameraState {
    /// One projection for live admission and detached cross-block validation.
    /// All values were validated before admission; no callbacks or clocks run.
    fn admit_session_camera_fields(&self, camera: &mut SessionCamera) {
        camera.native_camera.admit_continuation(self.height.clone());
        camera.camera_slave_mode = self.slave.as_ref().map(|(thing_template_name, bone_name)| {
            crate::game_logic::mission_scripts::CameraSlaveModeRequest {
                thing_template_name: thing_template_name.clone(),
                bone_name: bone_name.clone(),
            }
        });
        camera.camera_position = self.camera_position;
        camera.scripted_camera_constraint_widen = self.scripted_camera_constraint_widen;
        camera.camera_zoom = self.camera_zoom;
        camera.camera_zoom_target = self.camera_zoom_target;
        camera.camera_zoom_start = self.camera_zoom_start;
        camera.camera_zoom_duration = self.camera_zoom_duration;
        camera.camera_zoom_frame = self.camera_zoom_frame;
        camera.camera_zoom_ease_in = self.camera_zoom_ease_in;
        camera.camera_zoom_ease_out = self.camera_zoom_ease_out;
        camera.camera_pitch_radians = self.camera_pitch_radians;
        camera.camera_pitch_target = self.camera_pitch_target;
        camera.camera_pitch_start = self.camera_pitch_start;
        camera.camera_pitch_duration = self.camera_pitch_duration;
        camera.camera_pitch_frame = self.camera_pitch_frame;
        camera.camera_pitch_ease_in = self.camera_pitch_ease_in;
        camera.camera_pitch_ease_out = self.camera_pitch_ease_out;
        camera.camera_fx_pitch = self.camera_fx_pitch;
        camera.camera_yaw_radians = self.camera_yaw_radians;
        camera.camera_yaw_target = self.camera_yaw_target;
        camera.camera_yaw_start = self.camera_yaw_start;
        camera.camera_yaw_duration = self.camera_yaw_duration;
        camera.camera_yaw_frame = self.camera_yaw_frame;
        camera.camera_yaw_ease_in = self.camera_yaw_ease_in;
        camera.camera_yaw_ease_out = self.camera_yaw_ease_out;
        camera.camera_shake_rotation = self.camera_shake_rotation;
        camera.script_camera_shakers = self.script_camera_shakers.clone();
        camera.camera_follow_factor = self.camera_follow_factor;
    }

    fn completed_view(&self) -> glam::Mat4 {
        glam::Mat4::look_at_rh(
            self.completed_eye,
            self.completed_eye + self.completed_forward,
            self.completed_up,
        )
    }
}

impl NativeCameraContinuation {
    /// Decode/capture validates each record independently; this detects two
    /// individually valid records that refer to different camera continuations.
    pub(crate) fn validate_logic_continuation(
        &self,
        logic: &crate::game_logic::LogicCameraContinuation,
    ) -> Result<(), String> {
        let Some(owned) = logic.owned_camera_state() else {
            // Raw/V1 inactive Rust records have no complete camera to compare.
            return Ok(());
        };
        let (eye, forward, up) = owned.completed_pose()?;
        let same =
            |a: Vec3, b: Vec3| a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits);
        if !same(eye, self.0.completed_eye)
            || !same(forward, self.0.completed_forward)
            || !same(up, self.0.completed_up)
        {
            return Err("logic/native completed camera pose disagrees".into());
        }
        let mut camera = SessionCamera::default();
        self.0.admit_session_camera_fields(&mut camera);
        camera.view_matrix = self.0.completed_view();
        #[cfg(feature = "game_client")]
        {
            let (scroll, previous_target) = self.1.unwrap_or_default();
            camera.native_filter_scroll = scroll;
            camera.native_filter_previous_target = previous_target;
        }
        logic.validate_native_projection(&camera, self.0.camera_target)
    }

    pub(crate) fn camera_persist(&self) -> crate::save_load::snapshot::CameraPersist {
        crate::save_load::snapshot::CameraPersist {
            angle: self.0.camera_yaw_radians,
            position: self.0.camera_position.to_array(),
            target: self.0.camera_target.to_array(),
            zoom: self.0.camera_zoom,
        }
    }
}
impl CnCGameEngine {
    pub(crate) fn capture_camera_continuation(&self) -> Result<Vec<u8>, String> {
        let now = Instant::now();
        let inverse_view = self.game_logic.session_view.camera.view_matrix.inverse();
        let view = self.game_logic.session_view.capture();
        let state = CameraState {
            completed_eye: inverse_view.transform_point3(Vec3::ZERO),
            completed_forward: inverse_view.transform_vector3(Vec3::NEG_Z),
            completed_up: inverse_view.transform_vector3(Vec3::Y),
            height: self
                .game_logic
                .session_view
                .camera
                .native_camera
                .capture_continuation(),
            look_at: self.look_at_modes.capture_continuation(),
            display: self
                .native_display
                .capture_continuation(client_frame(self))?,
            slave: self
                .game_logic
                .session_view
                .camera
                .camera_slave_mode
                .as_ref()
                .map(|mode| (mode.thing_template_name.clone(), mode.bone_name.clone())),
            cursor: self.last_context_cursor.map(str::to_owned),
            control_group_click: self
                .last_control_group_select
                .map(|(group, time)| age(now, Some(time)).map(|value| (group, value.unwrap())))
                .transpose()?,
            camera_position: self.game_logic.session_view.camera.camera_position.clone(),
            camera_target: view.target,
            scripted_camera_constraint_widen: self
                .game_logic
                .session_view
                .camera
                .scripted_camera_constraint_widen
                .clone(),
            camera_zoom: self.game_logic.session_view.camera.camera_zoom.clone(),
            camera_zoom_target: self
                .game_logic
                .session_view
                .camera
                .camera_zoom_target
                .clone(),
            camera_zoom_start: self
                .game_logic
                .session_view
                .camera
                .camera_zoom_start
                .clone(),
            camera_zoom_duration: self
                .game_logic
                .session_view
                .camera
                .camera_zoom_duration
                .clone(),
            camera_zoom_frame: self
                .game_logic
                .session_view
                .camera
                .camera_zoom_frame
                .clone(),
            camera_zoom_ease_in: self
                .game_logic
                .session_view
                .camera
                .camera_zoom_ease_in
                .clone(),
            camera_zoom_ease_out: self
                .game_logic
                .session_view
                .camera
                .camera_zoom_ease_out
                .clone(),
            camera_pitch_radians: self
                .game_logic
                .session_view
                .camera
                .camera_pitch_radians
                .clone(),
            camera_pitch_target: self
                .game_logic
                .session_view
                .camera
                .camera_pitch_target
                .clone(),
            camera_pitch_start: self
                .game_logic
                .session_view
                .camera
                .camera_pitch_start
                .clone(),
            camera_pitch_duration: self
                .game_logic
                .session_view
                .camera
                .camera_pitch_duration
                .clone(),
            camera_pitch_frame: self
                .game_logic
                .session_view
                .camera
                .camera_pitch_frame
                .clone(),
            camera_pitch_ease_in: self
                .game_logic
                .session_view
                .camera
                .camera_pitch_ease_in
                .clone(),
            camera_pitch_ease_out: self
                .game_logic
                .session_view
                .camera
                .camera_pitch_ease_out
                .clone(),
            camera_fx_pitch: self.game_logic.session_view.camera.camera_fx_pitch.clone(),
            camera_yaw_radians: self
                .game_logic
                .session_view
                .camera
                .camera_yaw_radians
                .clone(),
            camera_yaw_target: self
                .game_logic
                .session_view
                .camera
                .camera_yaw_target
                .clone(),
            camera_yaw_start: self.game_logic.session_view.camera.camera_yaw_start.clone(),
            camera_yaw_duration: self
                .game_logic
                .session_view
                .camera
                .camera_yaw_duration
                .clone(),
            camera_yaw_frame: self.game_logic.session_view.camera.camera_yaw_frame.clone(),
            camera_yaw_ease_in: self
                .game_logic
                .session_view
                .camera
                .camera_yaw_ease_in
                .clone(),
            camera_yaw_ease_out: self
                .game_logic
                .session_view
                .camera
                .camera_yaw_ease_out
                .clone(),
            camera_shake_offset: view.offset,
            camera_shake_rotation: self
                .game_logic
                .session_view
                .camera
                .camera_shake_rotation
                .clone(),
            screen_shake_intensity: view.intensity,
            screen_shake_angle_cos: view.cosine,
            screen_shake_angle_sin: view.sine,
            script_camera_shakers: self
                .game_logic
                .session_view
                .camera
                .script_camera_shakers
                .clone(),
            script_fps_limit: self.script_fps_limit.clone(),
            camera_follow_factor: self
                .game_logic
                .session_view
                .camera
                .camera_follow_factor
                .clone(),
            host_match_camera_follow_active: self.host_match_camera_follow_active.clone(),
            host_match_camera_follow_position: self.host_match_camera_follow_position.clone(),
            host_match_camera_follow_id: self.host_match_camera_follow_id.clone(),
            host_match_script_camera_max_height: Some(
                self.game_logic.script_default_camera_max_height(),
            ),
            host_match_script_camera_pitch: Some(self.game_logic.script_default_camera_pitch()),
            keys_pressed: self.keys_pressed.clone(),
            mouse_position: self.mouse_position.clone(),
            mouse_cursor_seen: self.mouse_cursor_seen.clone(),
            mouse_world_position: self.mouse_world_position.clone(),
            sticky_waypoint_mode: self.sticky_waypoint_mode.clone(),
            sticky_auto_attack: self.sticky_auto_attack.clone(),
            use_alternate_mouse: self.use_alternate_mouse.clone(),
            is_dragging: self.is_dragging.clone(),
            selection_start: self.selection_start.clone(),
            selection_start_screen: self.selection_start_screen.clone(),
            last_click_position: self.last_click_position.clone(),
            last_right_click_screen: self.last_right_click_screen.clone(),
            left_click_release_behavior: self.left_click_release_behavior.clone(),
            displayed_max_selection_warning: self.displayed_max_selection_warning.clone(),
            lmb_context_started_physically: self.lmb_context_started_physically.clone(),
            rmb_scroll_anchor: self.rmb_scroll_anchor.clone(),
            is_rmb_scrolling: self.is_rmb_scrolling.clone(),
            move_rmb_scroll_anchor: self.move_rmb_scroll_anchor.clone(),
            draw_rmb_scroll_anchor: self.draw_rmb_scroll_anchor.clone(),
            rmb_scroll_started_physically: self.rmb_scroll_started_physically.clone(),
            rmb_deselect_down_screen: self.rmb_deselect_down_screen.clone(),
            rmb_deselect_down_camera: self.rmb_deselect_down_camera.clone(),
            is_mmb_rotating: self.is_mmb_rotating.clone(),
            mmb_anchor: self.mmb_anchor.clone(),
            selected_objects: self.selected_objects.clone(),
            camera_view_bookmarks: self.camera_view_bookmarks.clone(),
            camera_rotate_left_held: self.camera_rotate_left_held.clone(),
            camera_rotate_right_held: self.camera_rotate_right_held.clone(),
            camera_zoom_in_held: self.camera_zoom_in_held.clone(),
            camera_zoom_out_held: self.camera_zoom_out_held.clone(),
            camera_tracking_selection: self.camera_tracking_selection.clone(),
            replay_fast_forward: self.replay_fast_forward.clone(),
            prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click: self
                .prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click,
            frame_counter: self.frame_counter.clone(),
            script_fps_limit_last_tick_age: age(now, self.script_fps_limit_last_tick)?,
            last_click_time_age: age(now, self.last_click_time)?,
            last_right_click_time_age: age(now, self.last_right_click_time)?,
            rmb_deselect_down_at_age: age(now, self.rmb_deselect_down_at)?,
        };
        state.validate()?;
        encode_native_camera(self, &state)
    }
    pub(crate) fn decode_camera_continuation(
        bytes: &[u8],
    ) -> Result<NativeCameraContinuation, String> {
        if bytes.len() > MAX_BYTES {
            return Err("camera continuation exceeds size limit".into());
        }
        decode_native_camera(bytes)
    }
    pub(crate) fn admit_camera_continuation(&mut self, continuation: NativeCameraContinuation) {
        #[cfg(feature = "game_client")]
        {
            let (scroll, previous_target) = continuation.1.unwrap_or_default();
            self.game_logic.session_view.camera.native_filter_scroll = scroll;
            self.game_logic
                .session_view
                .camera
                .native_filter_previous_target = previous_target;
        }
        let state = continuation.0;
        let completed_view = state.completed_view();
        let now = Instant::now();
        state.admit_session_camera_fields(&mut self.game_logic.session_view.camera);
        self.look_at_modes.admit_continuation(state.look_at);
        self.native_display
            .admit_continuation(state.display, client_frame(self));

        self.last_context_cursor = state.cursor.as_deref().and_then(cursor_name);
        self.last_control_group_select = state
            .control_group_click
            .map(|(group, nanos)| (group, restore_age(now, Some(nanos)).unwrap()));

        self.game_logic
            .session_view
            .admit(crate::presentation_frame::SessionViewState {
                target: state.camera_target,
                intensity: state.screen_shake_intensity,
                cosine: state.screen_shake_angle_cos,
                sine: state.screen_shake_angle_sin,
                offset: state.camera_shake_offset,
            })
            .expect("native CameraState was validated before commitment");
        self.script_fps_limit = state.script_fps_limit;

        self.host_match_camera_follow_active = state.host_match_camera_follow_active;
        self.host_match_camera_follow_position = state.host_match_camera_follow_position;
        self.host_match_camera_follow_id = state.host_match_camera_follow_id;
        // Historical default observations were validated during detached decode.
        // The Logic continuation already admitted the canonical script defaults;
        // old presentation/cache observations must never overwrite them.
        self.keys_pressed = state.keys_pressed;
        self.mouse_position = state.mouse_position;
        self.mouse_cursor_seen = state.mouse_cursor_seen;
        self.mouse_world_position = state.mouse_world_position;
        self.sticky_waypoint_mode = state.sticky_waypoint_mode;
        self.sticky_auto_attack = state.sticky_auto_attack;
        self.use_alternate_mouse = state.use_alternate_mouse;
        self.is_dragging = state.is_dragging;
        self.selection_start = state.selection_start;
        self.selection_start_screen = state.selection_start_screen;
        self.last_click_position = state.last_click_position;
        self.last_right_click_screen = state.last_right_click_screen;
        self.left_click_release_behavior = state.left_click_release_behavior;
        self.displayed_max_selection_warning = state.displayed_max_selection_warning;
        self.lmb_context_started_physically = state.lmb_context_started_physically;
        self.rmb_scroll_anchor = state.rmb_scroll_anchor;
        self.is_rmb_scrolling = state.is_rmb_scrolling;
        self.move_rmb_scroll_anchor = state.move_rmb_scroll_anchor;
        self.draw_rmb_scroll_anchor = state.draw_rmb_scroll_anchor;
        self.rmb_scroll_started_physically = state.rmb_scroll_started_physically;
        self.rmb_deselect_down_screen = state.rmb_deselect_down_screen;
        self.rmb_deselect_down_camera = state.rmb_deselect_down_camera;
        self.is_mmb_rotating = state.is_mmb_rotating;
        self.mmb_anchor = state.mmb_anchor;
        self.selected_objects = state.selected_objects;
        self.camera_view_bookmarks = state.camera_view_bookmarks;
        self.camera_rotate_left_held = state.camera_rotate_left_held;
        self.camera_rotate_right_held = state.camera_rotate_right_held;
        self.camera_zoom_in_held = state.camera_zoom_in_held;
        self.camera_zoom_out_held = state.camera_zoom_out_held;
        self.camera_tracking_selection = state.camera_tracking_selection;
        self.replay_fast_forward = state.replay_fast_forward;
        self.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click =
            state.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click;
        self.frame_counter = state.frame_counter;
        self.script_fps_limit_last_tick = restore_age(now, state.script_fps_limit_last_tick_age);
        self.last_click_time = restore_age(now, state.last_click_time_age);
        self.last_right_click_time = restore_age(now, state.last_right_click_time_age);
        self.rmb_deselect_down_at = restore_age(now, state.rmb_deselect_down_at_age);
        self.reconcile_camera_cursor_after_restore();
        self.rebuild_tactical_projection();
        // Fast camera steps can change axes without committing the visible view.
        // Restore the completed semantic pose independently; never infer timers
        // or the fixed map basis from the saved eye.
        self.game_logic.session_view.camera.view_matrix = completed_view;
    }
}
pub(super) fn cursor_name(name: &str) -> Option<&'static str> {
    cursor_presentation(name).map(|(name, _)| name)
}
pub(super) fn cursor_presentation(name: &str) -> Option<(&'static str, winit::window::CursorIcon)> {
    use winit::window::CursorIcon;
    match name {
        "Attack" => Some(("Attack", CursorIcon::Crosshair)),
        "AttackMove" => Some(("AttackMove", CursorIcon::Crosshair)),
        "AttackObj" => Some(("AttackObj", CursorIcon::Crosshair)),
        "Build" => Some(("Build", CursorIcon::Cell)),
        "CaptureBuilding" => Some(("CaptureBuilding", CursorIcon::Cell)),
        "CombatDrop" => Some(("CombatDrop", CursorIcon::Move)),
        "DoRepair" => Some(("DoRepair", CursorIcon::Progress)),
        "Dock" => Some(("Dock", CursorIcon::AllScroll)),
        "EnterAggressively" => Some(("EnterAggressively", CursorIcon::Copy)),
        "EnterFriendly" => Some(("EnterFriendly", CursorIcon::Copy)),
        "ForceAttackGround" => Some(("ForceAttackGround", CursorIcon::Crosshair)),
        "ForceAttackObj" => Some(("ForceAttackObj", CursorIcon::Crosshair)),
        "GenericInvalid" => Some(("GenericInvalid", CursorIcon::NotAllowed)),
        "GetHealed" => Some(("GetHealed", CursorIcon::Progress)),
        "GetRepaired" => Some(("GetRepaired", CursorIcon::Progress)),
        "Hack" => Some(("Hack", CursorIcon::Cell)),
        "InvalidBuild" => Some(("InvalidBuild", CursorIcon::NotAllowed)),
        "Move" => Some(("Move", CursorIcon::AllScroll)),
        "Normal" => Some(("Normal", CursorIcon::Default)),
        "OutRange" => Some(("OutRange", CursorIcon::NotAllowed)),
        "ParticleUplinkCannon" => Some(("ParticleUplinkCannon", CursorIcon::Crosshair)),
        "PlaceBeacon" => Some(("PlaceBeacon", CursorIcon::Cell)),
        "ResumeConstruction" => Some(("ResumeConstruction", CursorIcon::Progress)),
        "Scroll" => Some(("Scroll", CursorIcon::AllScroll)),
        "Select" => Some(("Select", CursorIcon::Pointer)),
        "SetRallyPoint" => Some(("SetRallyPoint", CursorIcon::Cell)),
        "Target" => Some(("Target", CursorIcon::Crosshair)),
        "Waypoint" => Some(("Waypoint", CursorIcon::Cell)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
mod validation;

mod filter;
use filter::{decode_native_camera, encode_native_camera};

#[cfg(test)]
#[path = "camera_continuation/owner_consistency_tests.rs"]
mod owner_consistency_tests;

#[cfg(test)]
#[path = "camera_continuation/default_owner_tests.rs"]
mod default_owner_tests;
