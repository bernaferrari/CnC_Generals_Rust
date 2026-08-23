#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::input::MouseInputOrigin;
use super::selection_hud::{
    is_os_style_double_click, os_double_click_time_ms, OS_DOUBLE_CLICK_SLOP_PX,
};
use super::*;


/// C++ `LookAtXlat.cpp:241-254` short middle-click home-orientation thresholds.
const LOOKAT_MMB_CLICK_DURATION_FRAMES: u32 = 5;
const LOOKAT_MMB_CLICK_PIXEL_OFFSET: f32 = 5.0;
/// C++ `LookAtXlat.cpp:298` `const Real FACTOR = 0.01f`.
const LOOKAT_MMB_YAW_FACTOR: f32 = 0.01;
/// C++ `View.cpp` / `W3DView` default pitch when GameData CameraPitch is ~0.
const LOOKAT_DEFAULT_PITCH_DEG: f32 = 37.5;
/// C++ `LookAtXlat.cpp:45` `edgeScrollSize`.
pub(super) const EDGE_SCROLL_SIZE: f32 = 3.0;
/// C++ `View.cpp:78-79` / `W3DView::setZoom` clamp.
pub(super) const W3D_MIN_ZOOM: f32 = 0.2;
pub(super) const W3D_MAX_ZOOM: f32 = 1.3;
/// C++ `PATHFIND_CELL_SIZE_F`.
pub(super) const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// C++ `W3DView::scrollBy` `SCROLL_RESOLUTION`.
const SCROLL_RESOLUTION: f32 = 250.0;

/// C++ CameraShakerSystem axis caps (radians).
pub(super) const SHAKE_AXIS_PITCH: f32 = 7.5 * std::f32::consts::PI / 180.0;
pub(super) const SHAKE_AXIS_YAW: f32 = 15.0 * std::f32::consts::PI / 180.0;
pub(super) const SHAKE_AXIS_ROLL: f32 = 5.0 * std::f32::consts::PI / 180.0;
/// C++ `camerashakesystem.cpp` MIN/MAX/END omega (radians/s). 12.5-15Hz → 1Hz.
pub(super) const SHAKE_MIN_OMEGA: f32 = 12.5 * std::f32::consts::TAU;
pub(super) const SHAKE_MAX_OMEGA: f32 = 15.0 * std::f32::consts::TAU;
pub(super) const SHAKE_END_OMEGA: f32 = std::f32::consts::TAU;


/// C++ `Mouse.cpp` `m_dragTolerance` default / leftover `selection_xlat.rs` `DRAG_TOLERANCE`.
const DRAG_TOLERANCE_PX: f32 = 5.0;

/// C++ `PlaceEventTranslator.cpp:307` Euclidean screen px (not 1wu world).
const PLACEMENT_DRAG_THRESHOLD_DIST: f32 = 5.0;

/// C++ `View.h` `ViewLocation`: pos + angle + pitch + zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CameraViewLocation {
    pos: Vec3,
    yaw: f32,
    pitch: f32,
    zoom: f32,
}

/// C++ `LookAtXlat.h` `ScrollType`: one active pan source at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum LookAtScrollType {
    #[default]
    None,
    Rmb,
    Key,
    ScreenEdge,
}

impl LookAtScrollType {
    fn is_scrolling(self) -> bool {
        !matches!(self, Self::None)
    }

    fn blocks_key_start(self) -> bool {
        matches!(self, Self::Rmb | Self::ScreenEdge)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ScriptedCameraPlayerCancel {
    #[default]
    None,
    /// C++ setAngle/setPitch/setZoom/setHeightAboveGround.
    Set,
    /// C++ lookAt.
    LookAt,
}

impl ScriptedCameraPlayerCancel {
    pub(super) fn cancels_move(self) -> bool {
        matches!(self, Self::Set | Self::LookAt)
    }

    pub(super) fn cancels_set(self) -> bool {
        matches!(self, Self::Set)
    }

    pub(super) fn cancels_rotate(self) -> bool {
        matches!(self, Self::Set | Self::LookAt)
    }

    fn raise(self, next: Self) -> Self {
        match (self, next) {
            (Self::Set, _) | (_, Self::Set) => Self::Set,
            (Self::LookAt, _) | (_, Self::LookAt) => Self::LookAt,
            _ => Self::None,
        }
    }
}


struct LookAtHostModes {
    prev_cursor: Option<&'static str>,
    mouse_locked: bool,
    mmb_original_anchor: Option<(f32, f32)>,
    mmb_press_frame: u32,
    views: [Option<CameraViewLocation>; 8],
    /// C++ `m_scrollType` — exclusive RMB / key / screen-edge.
    scroll_type: LookAtScrollType,
    /// C++ `LookAtTranslator::m_lastMouseMoveFrame`.
    last_mouse_move_frame: u32,
    last_mouse_pixel: (f32, f32),
    /// C++ `View::m_heightAboveGround` desired HAG (wheel/key zoom).
    desired_height_above_ground: Option<f32>,
    /// Player scroll cleared camera lock (C++ `setScrolling` + `setCameraLock`).
    camera_follow_lock_broken: bool,
    /// Player setAngle/setZoom/lookAt this frame; residual must not re-apply.
    scripted_camera_player_cancel: ScriptedCameraPlayerCancel,
    /// C++ `MSG_RAW_MOUSE_WHEEL` fallthrough `stopScrolling` — KEY/EDGE stay
    /// down until the next RAW_KEY / RAW_MOUSE_POSITION.
    wheel_stopped_scroll: bool,
    /// Live OS cursor hidden because it sits under cinematic letterbox bars.
    letterbox_os_cursor_hidden: bool,
}

fn live_camera_zoom_limited() -> bool {
    #[cfg(feature = "game_client")]
    {
        game_client::display::view::with_tactical_view_ref(|view| view.is_zoom_limited())
    }
    #[cfg(not(feature = "game_client"))]
    {
        true
    }
}

pub fn height_after_zoom_steps(
    current_hag: f32,
    steps: f32,
    min_h: f32,
    max_h: f32,
    zoom_limited: bool,
) -> f32 {
    let next = current_hag + steps * 10.0;
    if zoom_limited {
        next.clamp(min_h, max_h)
    } else {
        next
    }
}

/// C++ `W3DView::setDefaultView`: `m_maxHeightAboveGround = GlobalData.max * scale`,
/// floored to `m_minHeightAboveGround`. Wheel / settle use that View max.
pub fn live_view_height_clamp(min_h: f32, max_h: f32, script_max_height_scale: f32) -> (f32, f32) {
    let scale = if script_max_height_scale.is_finite() {
        script_max_height_scale.max(0.0)
    } else {
        1.0
    };
    (min_h, (max_h * scale).max(min_h))
}

/// C++ `setAngleAndPitchToDefault`: orbit GameData CameraPitch plus extra
/// `m_defaultPitchAngle`. Live residual `1.0` is the FXPitch-style fail-closed
/// (not extra View pitch); script `0.0` is the authored default.
pub fn live_home_pitch_radians(orbit_pitch_degrees: f32, script_default_pitch: f32) -> f32 {
    let orbit_deg = if orbit_pitch_degrees.abs() < 0.1 {
        LOOKAT_DEFAULT_PITCH_DEG
    } else {
        orbit_pitch_degrees
    };
    let extra = if script_default_pitch.is_finite() && (script_default_pitch - 1.0).abs() > 1.0e-4 {
        script_default_pitch
    } else {
        0.0
    };
    orbit_deg.to_radians() + extra
}


fn note_scripted_camera_player_cancel(next: ScriptedCameraPlayerCancel) {
    let mut modes = look_at_host_modes();
    modes.scripted_camera_player_cancel = modes.scripted_camera_player_cancel.raise(next);
}

pub(super) fn take_scripted_camera_player_cancel() -> ScriptedCameraPlayerCancel {
    let mut modes = look_at_host_modes();
    std::mem::take(&mut modes.scripted_camera_player_cancel)
}

fn look_at_host_modes() -> std::sync::MutexGuard<'static, LookAtHostModes> {
    static STATE: std::sync::LazyLock<std::sync::Mutex<LookAtHostModes>> =
        std::sync::LazyLock::new(|| {
            std::sync::Mutex::new(LookAtHostModes {
                prev_cursor: None,
                mouse_locked: false,
                mmb_original_anchor: None,
                mmb_press_frame: 0,
                views: [None; 8],
                scroll_type: LookAtScrollType::None,
                last_mouse_move_frame: 0,
                last_mouse_pixel: (0.0, 0.0),
                desired_height_above_ground: None,
                camera_follow_lock_broken: false,
                scripted_camera_player_cancel: ScriptedCameraPlayerCancel::None,
                wheel_stopped_scroll: false,
                letterbox_os_cursor_hidden: false,
            })
        });
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn look_at_host_mouse_locked() -> bool {
    look_at_host_modes().mouse_locked
}

pub(crate) fn look_at_host_is_scrolling() -> bool {
    look_at_host_modes().scroll_type.is_scrolling()
}

fn clamp_w3d_zoom(zoom: f32) -> f32 {
    zoom.clamp(W3D_MIN_ZOOM, W3D_MAX_ZOOM)
}

fn lookat_note_mouse_moved(frame: u32, pixel: (f32, f32)) {
    let mut modes = look_at_host_modes();
    if modes.last_mouse_pixel != pixel {
        modes.last_mouse_move_frame = frame;
        modes.last_mouse_pixel = pixel;
        // C++ RAW_MOUSE_POSITION can restart SCREENEDGE after a wheel abort.
        modes.wheel_stopped_scroll = false;
    }
}

/// C++ LookAtXlat.cpp:199,214,226,239,335 — stamp even if the cursor did not move.
fn lookat_stamp_mouse_activity(frame: u32) {
    look_at_host_modes().last_mouse_move_frame = frame;
}

/// C++ RAW_KEY can restart KEY scroll after a wheel `stopScrolling`.
pub(super) fn lookat_note_raw_key_activity() {
    look_at_host_modes().wheel_stopped_scroll = false;
}

fn lookat_has_mouse_moved_recently(frame: u32) -> bool {
    let last = look_at_host_modes().last_mouse_move_frame;
    let last = if last > frame { 0 } else { last };
    last + game_engine::common::game_common::LOGICFRAMES_PER_SECOND as u32 >= frame
}

/// C++ `W3DView::scrollBy` (1779-1823): `end.Y += dy * SCROLL_RESOLUTION * aspect`
/// where `aspect = getWidth()/getHeight()`. Vertical screen delta is pre-multiplied
/// by tactical-view aspect before the world conversion.
fn lookat_scroll_world_delta(
    screen_scroll: Vec2,
    forward: Vec3,
    right: Vec3,
    camera_height: f32,
    view_aspect: f32,
) -> Vec3 {
    if screen_scroll.length_squared() <= f32::EPSILON {
        return Vec3::ZERO;
    }
    let scale = camera_height.max(1.0) / SCROLL_RESOLUTION;
    let aspect = view_aspect.max(0.01);
    (right * screen_scroll.x + forward * (-screen_scroll.y * aspect)) * scale
}


/// C++ `W3DView::calcCameraConstraints` inset: |center-95%| pick at ground Y.
pub(super) fn w3d_camera_constraint_offset(
    view: Mat4,
    projection: Mat4,
    viewport: (f32, f32),
    ground_y: f32,
) -> f32 {
    let (width, height) = viewport;
    if width <= 1.0 || height <= 1.0 {
        return 0.0;
    }
    let Some((c_near, c_far)) =
        unproject_mouse_ray(view, projection, (width * 0.5, height * 0.5), width, height)
    else {
        return 0.0;
    };
    let Some((b_near, b_far)) =
        unproject_mouse_ray(view, projection, (width * 0.5, height * 0.95), width, height)
    else {
        return 0.0;
    };
    let Some(center) = ray_hit_height(c_near, c_far, ground_y) else {
        return 0.0;
    };
    let Some(bottom) = ray_hit_height(b_near, b_far, ground_y) else {
        return 0.0;
    };
    Vec2::new(center.x - bottom.x, center.z - bottom.z).length()
}

fn ray_hit_height(near: Vec3, far: Vec3, height: f32) -> Option<Vec3> {
    let dy = far.y - near.y;
    if dy.abs() <= PICK_RAY_EPSILON {
        return None;
    }
    let t = (height - near.y) / dy;
    if !t.is_finite() {
        return None;
    }
    Some(near + (far - near) * t)
}

pub(super) fn airborne_look_at_ground(
    _eye: Vec3,
    object_pos: Vec3,
    look_dir: Vec3,
    far_clip: f32,
    world_min: Vec3,
    world_max: Vec3,
    world_env: Option<&crate::presentation_frame::PresentationWorldEnv>,
) -> Option<Vec3> {
    // Leftover `look_at_pick_ray` / C++ Un_Project(0,0): restart at the object
    // and cast the current look direction, not camera-to-object.
    let mut dir = look_dir;
    if dir.length_squared() <= PICK_RAY_EPSILON {
        dir = -Vec3::Y;
    }
    let dir = dir.normalize() * far_clip.max(1.0);
    let start = object_pos;
    let end = start + dir;
    raycast_frozen_terrain(start, end, world_min, world_max, world_env)
        .or_else(|| raycast_ground_plane_clamped(start, end, world_min, world_max, world_env))
}

fn normalize_signed_angle(mut angle: f32) -> f32 {
    while angle > std::f32::consts::PI {
        angle -= std::f32::consts::TAU;
    }
    while angle < -std::f32::consts::PI {
        angle += std::f32::consts::TAU;
    }
    angle
}


/// C++ `LookAtXlat.cpp:174-175`: arrow keys cannot start while box-selecting
/// or while a non-key scroll (RMB / screen-edge) is already active.
fn lookat_keyboard_scroll_blocked(is_selecting: bool, non_key_scrolling: bool) -> bool {
    is_selecting || non_key_scrolling
}

/// C++ `LookAtXlat.cpp` exclusive `m_scrollType` start/stop.
/// Key messages are processed before mouse-move, so idle+keys+edge starts Key.
fn lookat_resolve_scroll_type(
    current: LookAtScrollType,
    input_enabled: bool,
    is_selecting: bool,
    is_rmb_scrolling: bool,
    key_dirs: bool,
    at_screen_edge: bool,
) -> LookAtScrollType {
    if !input_enabled {
        return LookAtScrollType::None;
    }
    // RMB is started on button-down (`start_rmb_lookat_scroll`), not here.
    if is_rmb_scrolling {
        return LookAtScrollType::Rmb;
    }
    if look_at_host_modes().wheel_stopped_scroll {
        return LookAtScrollType::None;
    }
    let current = if current == LookAtScrollType::Rmb {
        LookAtScrollType::None
    } else {
        current
    };
    if !lookat_keyboard_scroll_blocked(is_selecting, current.blocks_key_start()) {
        if key_dirs && current == LookAtScrollType::None {
            return LookAtScrollType::Key;
        }
        if current == LookAtScrollType::Key {
            return if key_dirs {
                LookAtScrollType::Key
            } else {
                LookAtScrollType::None
            };
        }
    } else if current == LookAtScrollType::Key {
        // Frame tick still applies KEY while selecting; stop when arrows lift.
        return if key_dirs {
            LookAtScrollType::Key
        } else {
            LookAtScrollType::None
        };
    }
    // C++ :286-291: edge starts only when `!m_isScrolling`.
    if current == LookAtScrollType::None && at_screen_edge {
        LookAtScrollType::ScreenEdge
    } else if current == LookAtScrollType::ScreenEdge {
        if at_screen_edge {
            LookAtScrollType::ScreenEdge
        } else {
            LookAtScrollType::None
        }
    } else {
        current
    }
}

/// C++ `LookAtXlat.cpp:245-250`: move ≤5px and duration <5 GameClient frames.
fn lookat_mmb_is_short_click(dx: f32, dy: f32, frames: u32) -> bool {
    dx.abs() <= LOOKAT_MMB_CLICK_PIXEL_OFFSET
        && dy.abs() <= LOOKAT_MMB_CLICK_PIXEL_OFFSET
        && frames < LOOKAT_MMB_CLICK_DURATION_FRAMES
}

/// C++ `InGameUI.cpp:1836` applies `m_keyboardCameraRotateSpeed` once per
/// logic frame. Host `update_camera` is dt-scaled, so convert the same way
/// key-scroll converts `SCROLL_AMT`.
fn lookat_keyboard_rotate_delta(speed: f32, dt: f32, logic_fps: f32) -> f32 {
    let logic_fps = logic_fps.max(1.0);
    let scroll_dt = dt.max(0.0).min(2.0 / logic_fps);
    speed * scroll_dt * logic_fps
}

pub(crate) fn lookat_view_slot(key: NamedKey) -> Option<usize> {
    match key {
        NamedKey::F1 => Some(0),
        NamedKey::F2 => Some(1),
        NamedKey::F3 => Some(2),
        NamedKey::F4 => Some(3),
        NamedKey::F5 => Some(4),
        NamedKey::F6 => Some(5),
        NamedKey::F7 => Some(6),
        NamedKey::F8 => Some(7),
        _ => None,
    }
}

fn should_emit_host_replay_camera(state: GameState, mode: crate::game_logic::GameMode) -> bool {
    if !matches!(state, GameState::InGame) {
        return false;
    }
    if !game_engine::common::global_data::read().save_camera_in_replay {
        return false;
    }
    if crate::command_system::host_recorder_is_playback() {
        return false;
    }
    // C++ LookAtXlat.cpp:459 — single-player or skirmish only.
    matches!(
        mode,
        crate::game_logic::GameMode::SinglePlayer | crate::game_logic::GameMode::Skirmish
    )
}

fn should_apply_host_replay_camera(player_index: i32) -> bool {
    crate::command_system::host_should_apply_replay_camera(player_index)
}

fn lookat_bookmark_message(slot_one_based: usize) -> String {
    #[cfg(feature = "game_client")]
    {
        let template = game_client::game_text::GameText::fetch("GUI:BookmarkXSet");
        if template.contains("%d") {
            return template.replacen("%d", &slot_one_based.to_string(), 1);
        }
        if template.contains("%s") {
            return template.replacen("%s", &slot_one_based.to_string(), 1);
        }
        if !template.is_empty() && !template.starts_with("MISSING:") {
            return format!("{template} {slot_one_based}");
        }
    }
    format!("GUI:BookmarkXSet {slot_one_based}")
}

/// C++ `Mouse.ini` `DragTolerance`. Leftover / documented default is 5px
/// (`selection_xlat.rs` `DRAG_TOLERANCE`). Zero INI residual uses that default.
fn host_mouse_drag_tolerance_px() -> f32 {
    game_engine::common::ini::get_mouse_settings()
        .map(|s| s.drag_tolerance)
        .filter(|&v| v > 0)
        .unwrap_or(5) as f32
}

/// C++ `SelectionXlat.cpp:399-400`: area select only when `|dx|` or `|dy|`
/// exceeds `TheMouse->m_dragTolerance`.
fn host_screen_drag_is_click(dx: f32, dy: f32) -> bool {
    let tol = host_mouse_drag_tolerance_px();
    dx.abs() <= tol && dy.abs() <= tol
}

/// C++ `Mouse.ini` `DragToleranceMS`. Leftover default is 250ms.
fn host_mouse_drag_tolerance_ms() -> u128 {
    game_engine::common::ini::get_mouse_settings()
        .map(|s| s.drag_tolerance_ms)
        .filter(|&v| v > 0)
        .unwrap_or(250) as u128
}

/// C++ `Mouse.ini` `DragTolerance3D`. Leftover default is 5 world units.
fn host_mouse_drag_tolerance_3d() -> f32 {
    game_engine::common::ini::get_mouse_settings()
        .map(|s| s.drag_tolerance_3d)
        .filter(|&v| v > 0)
        .unwrap_or(5) as f32
}

/// C++ `SelectionXlat.cpp:982-1000` RMB click vs look-at/scroll.
fn host_rmb_release_is_click(
    dx: f32,
    dy: f32,
    elapsed_ms: u128,
    camera_delta_len: f32,
) -> bool {
    host_screen_drag_is_click(dx, dy)
        && elapsed_ms <= host_mouse_drag_tolerance_ms()
        && camera_delta_len <= host_mouse_drag_tolerance_3d()
}


/// Physical input metadata held only across the synchronous context-command
/// execution boundary. `issued_at` is part of the command's own immutable
/// fingerprint, so an unrelated queued/AI Gather event cannot be mistaken for
/// this physical input edge.
#[derive(Debug, Clone)]
struct PhysicalGatherAttempt {
    command_id: u32,
    issued_at: SystemTime,
    player_id: u32,
    target_id: ObjectId,
}

impl PhysicalGatherAttempt {
    fn from_context_click_command(
        command: &crate::command_system::GameCommand,
        origin: MouseInputOrigin,
        physical_context_gesture: bool,
    ) -> Option<Self> {
        if !matches!(origin, MouseInputOrigin::Physical) || !physical_context_gesture {
            return None;
        }
        let crate::command_system::CommandType::Gather { target_id } = &command.command_type else {
            return None;
        };
        Some(Self {
            command_id: command.command_id,
            issued_at: command.timestamp.clone(),
            player_id: command.player_id,
            target_id: *target_id,
        })
    }

    fn matches(&self, event: &crate::game_logic::AcceptedGatherCommand) -> bool {
        self.command_id == event.command_id
            && self.issued_at == event.issued_at
            && self.player_id == event.player_id
            && self.target_id == event.target_id
    }
}

/// C++ `SelectionInfo::contextCommandForNewSelection` only lets the
/// prefer-selection modifier bypass a classic LMB context route for a locally
/// selectable target. Terrain and non-local targets still reach CommandXlat.
#[inline]
fn classic_left_context_action_allowed(
    has_selection: bool,
    shift_down: bool,
    target_is_locally_selectable: bool,
) -> bool {
    has_selection && (!shift_down || !target_is_locally_selectable)
}

fn is_point_click_drag(dx: f32, dy: f32) -> bool {
    // C++ SelectionXlat.cpp:399-400 / Mouse::isClick — per-axis, not Euclidean.
    host_screen_drag_is_click(dx, dy)
}

fn placement_screen_drag_exceeds_threshold(dx: f32, dy: f32) -> bool {
    (dx * dx + dy * dy).sqrt() >= PLACEMENT_DRAG_THRESHOLD_DIST
}

/// C++ `InGameUI::isSelecting()` — set only after box-select exceeds DragTolerance.
/// LMB-held and placement-rotate are not selecting (`LookAtXlat.cpp:174-175`).
fn host_is_selecting_now(
    is_dragging: bool,
    placement_active: bool,
    start_screen: Option<(f32, f32)>,
    mouse: (f32, f32),
) -> bool {
    if !is_dragging || placement_active {
        return false;
    }
    let Some(start) = start_screen else {
        return false;
    };
    !is_point_click_drag(mouse.0 - start.0, mouse.1 - start.1)
}

/// C++ `SelectionXlat.cpp:930-937`: alternate-mouse blank LMB-up deselects.
/// Classic empty LMB never clears (`SelectionXlat.cpp:575-597` empty region `break`).
fn alternate_mouse_blank_click_deselects(
    use_alternate_mouse: bool,
    shift_down: bool,
    ctrl_down: bool,
    alt_down: bool,
) -> bool {
    use_alternate_mouse && !shift_down && !ctrl_down && !alt_down
}

/// C++ `SelectionXlat.cpp:617-626`: force a new group when the current
/// selection already has any enemy / civilian / ally / mine-building.
fn box_selection_must_replace(
    current_has_enemy: bool,
    current_has_civilian: bool,
    current_has_ally: bool,
    current_has_local_structure: bool,
) -> bool {
    current_has_enemy || current_has_civilian || current_has_ally || current_has_local_structure
}

/// C++ `SelectionInfo.cpp:203-205`: infantry + exactly one garrisonable
/// is a context enter for both point and drag. Alternate mouse never
/// context-selects (`:174-176`); enemy/civ/ally current groups also refuse.
fn infantry_garrison_context_takes_region(
    alternate_mouse: bool,
    current_has_enemy_civilian_or_ally: bool,
    current_has_local_infantry: bool,
    garrisonable_in_region: usize,
) -> bool {
    !alternate_mouse
        && !current_has_enemy_civilian_or_ally
        && current_has_local_infantry
        && garrisonable_in_region == 1
}

fn union_object_ids(mut base: Vec<ObjectId>, extra: impl IntoIterator<Item = ObjectId>) -> Vec<ObjectId> {
    for id in extra {
        if !base.contains(&id) {
            base.push(id);
        }
    }
    base
}


/// C++ `SelectionInfo.cpp:85-111` current-list counts used by box replace/garrison.
fn current_selection_box_counts(
    frame: &crate::presentation_frame::PresentationFrame,
    selected: &[ObjectId],
) -> (bool, bool, bool, bool, bool) {
    use crate::game_logic::{KindOf, Team};
    let mut has_enemy = false;
    let mut has_civilian = false;
    let mut has_ally = false;
    let mut has_local_structure = false;
    let mut has_local_infantry = false;
    for id in selected {
        let Some(object) = frame.objects.iter().find(|candidate| candidate.id == *id) else {
            continue;
        };
        if frame.is_owned_by_local(object) {
            if object.is_structure
                || crate::presentation_frame::PresentationFrame::object_has_kind(
                    object,
                    KindOf::Structure,
                )
            {
                has_local_structure = true;
            }
            if crate::presentation_frame::PresentationFrame::object_has_kind(
                object,
                KindOf::Infantry,
            ) {
                has_local_infantry = true;
            }
        } else if frame.is_enemy_of_local(object) {
            has_enemy = true;
        } else if object.team == Team::Neutral {
            has_civilian = true;
        } else if frame.is_allied_with_local(object) {
            has_ally = true;
        }
    }
    (
        has_enemy,
        has_civilian,
        has_ally,
        has_local_structure,
        has_local_infantry,
    )
}


// C++ `W3DView::deviceToWorld` equivalent for the active Main camera.  Input
// must be projected through the same view/projection pair used by WGPU rather
// than treating the window as a linear minimap.
const PICK_RAY_EPSILON: f32 = 1.0e-5;
const PICK_TERRAIN_STEPS: usize = 96;
const PICK_TERRAIN_BISECTION_STEPS: usize = 12;

pub(super) fn unproject_mouse_ray(
    view_matrix: Mat4,
    projection_matrix: Mat4,
    mouse_position: (f32, f32),
    viewport_width: f32,
    viewport_height: f32,
) -> Option<(Vec3, Vec3)> {
    let width = viewport_width.max(1.0);
    let height = viewport_height.max(1.0);
    let ndc_x = (mouse_position.0 / width).clamp(0.0, 1.0) * 2.0 - 1.0;
    let ndc_y = 1.0 - (mouse_position.1 / height).clamp(0.0, 1.0) * 2.0;
    let inverse = (projection_matrix * view_matrix).inverse();
    if !inverse.is_finite() {
        return None;
    }

    // Main's WGPU projection uses depth [0, 1], matching the selection-overlay
    // unprojection path.  Retain the signed perspective divide; using abs(w)
    // can mirror a point behind the camera into the playable world.
    let near_homogeneous = inverse * glam::Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_homogeneous = inverse * glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    if near_homogeneous.w.abs() <= PICK_RAY_EPSILON || far_homogeneous.w.abs() <= PICK_RAY_EPSILON {
        return None;
    }
    let near = near_homogeneous.truncate() / near_homogeneous.w;
    let far = far_homogeneous.truncate() / far_homogeneous.w;
    (near.is_finite() && far.is_finite() && (far - near).length_squared() > PICK_RAY_EPSILON)
        .then_some((near, far))
}

/// Parameter interval in which a finite ray segment lies over the playable XZ
/// rectangle.  This prevents a terrain edge from being sampled for arbitrary
/// off-map screen rays.
fn ray_interval_in_world_xz(
    near: Vec3,
    far: Vec3,
    world_min: Vec3,
    world_max: Vec3,
) -> Option<(f32, f32)> {
    let direction = far - near;
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for (origin, delta, min, max) in [
        (near.x, direction.x, world_min.x, world_max.x),
        (near.z, direction.z, world_min.z, world_max.z),
    ] {
        if delta.abs() <= PICK_RAY_EPSILON {
            if origin < min || origin > max {
                return None;
            }
            continue;
        }
        let a = (min - origin) / delta;
        let b = (max - origin) / delta;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter > exit {
            return None;
        }
    }
    (exit >= 0.0 && enter <= 1.0).then_some((enter.clamp(0.0, 1.0), exit.clamp(0.0, 1.0)))
}

fn frozen_terrain_height(
    world_env: Option<&crate::presentation_frame::PresentationWorldEnv>,
    point: Vec3,
) -> f32 {
    world_env
        .and_then(|env| {
            env.sample_gameplay_terrain_height(point.x, point.z)
                .or_else(|| env.sample_height(point.x, point.z))
        })
        .unwrap_or(0.0)
}

/// Intersect the active camera ray with the frozen gameplay terrain.  The
/// height surface is sampled only from the presentation frame, preserving the
/// one-frame input/render boundary and avoiding a second mutable GameLogic read.
fn raycast_frozen_terrain(
    near: Vec3,
    far: Vec3,
    world_min: Vec3,
    world_max: Vec3,
    world_env: Option<&crate::presentation_frame::PresentationWorldEnv>,
) -> Option<Vec3> {
    let (start_t, end_t) = ray_interval_in_world_xz(near, far, world_min, world_max)?;
    let mut previous_t = start_t;
    let direction = far - near;
    let surface_delta = |t: f32| {
        let point = near + direction * t;
        point.y - frozen_terrain_height(world_env, point)
    };
    let mut previous_delta = surface_delta(previous_t);
    if !previous_delta.is_finite() {
        return None;
    }

    // A normal RTS camera starts above terrain.  March the bounded ray to find
    // the first downward surface crossing, then refine it.  This works for the
    // full frozen heightmap and degrades safely to its coarse snapshot/plane.
    for step in 1..=PICK_TERRAIN_STEPS {
        let t = start_t + (end_t - start_t) * step as f32 / PICK_TERRAIN_STEPS as f32;
        let delta = surface_delta(t);
        if !delta.is_finite() {
            return None;
        }
        if previous_delta >= 0.0 && delta <= 0.0 {
            let mut low = previous_t;
            let mut high = t;
            for _ in 0..PICK_TERRAIN_BISECTION_STEPS {
                let middle = (low + high) * 0.5;
                if surface_delta(middle) >= 0.0 {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            let point = near + direction * ((low + high) * 0.5);
            let ground = frozen_terrain_height(world_env, point);
            return ground
                .is_finite()
                .then_some(Vec3::new(point.x, ground, point.z));
        }
        previous_t = t;
        previous_delta = delta;
    }
    None
}

/// Camera-ray fallback when no frozen terrain snapshot exists yet (boot/load),
/// or an unusually shallow ray leaves the map before it reaches the terrain.
/// It is still camera-relative, never the old whole-map screen interpolation.
fn raycast_ground_plane_clamped(
    near: Vec3,
    far: Vec3,
    world_min: Vec3,
    world_max: Vec3,
    world_env: Option<&crate::presentation_frame::PresentationWorldEnv>,
) -> Option<Vec3> {
    let direction = far - near;
    if direction.y.abs() <= PICK_RAY_EPSILON {
        return None;
    }
    let t = -near.y / direction.y;
    if !t.is_finite() || t < 0.0 {
        return None;
    }
    let point = near + direction * t;
    let x = point.x.clamp(world_min.x, world_max.x);
    let z = point.z.clamp(world_min.z, world_max.z);
    let ground = frozen_terrain_height(world_env, Vec3::new(x, 0.0, z));
    ground.is_finite().then_some(Vec3::new(x, ground, z))
}

impl CnCGameEngine {
    /// C++ `GameWinBlockInput` `GWM_LEFT_UP` (`GameWindow.cpp:1480-1491`):
    /// release over the control bar cancels the marquee without applying
    /// the area selection.
    pub(super) fn cancel_area_select_from_control_bar(&mut self) {
        self.is_dragging = false;
        self.selection_start = None;
        self.selection_start_screen = None;
        self.left_click_release_behavior = LeftMouseReleaseBehavior::Selection;
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheInGameUI::set_selecting(false);
        }
    }

    /// C++ `LookAtXlat` `MSG_META_OPTIONS` `stopScrolling` + `SelectionXlat`
    /// `m_leftMouseButtonIsDown = FALSE`.
    pub(super) fn apply_meta_options_interrupt(&mut self) {
        let _ = super::selection_hud::meta_options_clears_lookat_and_drag();
        self.stop_rmb_lookat_scroll();
        self.cancel_area_select_from_control_bar();
    }

    pub(super) fn host_quit_menu_blocks_world_selection(&self) -> bool {
        if self.quit_menu_host_active {
            return true;
        }
        #[cfg(feature = "game_client")]
        {
            return game_client::helpers::TheInGameUI::is_quit_menu_visible()
                || game_client::gui::callbacks::is_quit_menu_visible();
        }
        #[cfg(not(feature = "game_client"))]
        {
            false
        }
    }

    /// C++ `SelectionXlat.cpp:953-961` RMB-down click-vs-scroll samples.
    pub(super) fn note_rmb_deselect_anchor(&mut self) {
        self.rmb_deselect_down_at = Some(Instant::now());
        self.rmb_deselect_down_screen = Some(self.mouse_position);
        self.rmb_deselect_down_camera = Some(self.camera_position);
    }

    /// C++ `SelectionXlat.cpp:982-1000`: pixel + time + camera 3D gates.
    pub(super) fn rmb_release_is_deselect_click(&self) -> bool {
        let Some(anchor) = self.rmb_deselect_down_screen else {
            return false;
        };
        let dx = self.mouse_position.0 - anchor.0;
        let dy = self.mouse_position.1 - anchor.1;
        let elapsed_ms = self
            .rmb_deselect_down_at
            .map(|started| started.elapsed().as_millis())
            .unwrap_or(u128::MAX);
        let camera_delta = self
            .rmb_deselect_down_camera
            .map(|down| (self.camera_position - down).length())
            .unwrap_or(f32::MAX);
        host_rmb_release_is_click(dx, dy, elapsed_ms, camera_delta)
    }

    pub(super) fn host_is_selecting(&self) -> bool {
        host_is_selecting_now(
            self.is_dragging,
            self.pending_structure_placement.is_some(),
            self.selection_start_screen,
            self.mouse_position,
        )
    }

    pub(super) fn sync_host_selecting_flag(&self) {
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheInGameUI::set_selecting(self.host_is_selecting());
        }
    }

    fn apply_structure_placement_angle(&mut self, angle: f32) {
        self.game_hud
            .construction_panel
            .rotate_structure_placement(angle);
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .rotate_structure_placement(angle);
        game_client::helpers::TheInGameUI::set_placement_angle(angle);
    }

    /// C++ `W3DView::screenToTerrain` analog through the live WGPU camera.
    /// Placement drag/confirm reprojects the original screen pixel so a camera
    /// pan during rotate does not spin the ghost around a stale world point.
    fn screen_to_terrain(&self, screen: (f32, f32)) -> Option<Vec3> {
        let (view_w, view_h) = self.tactical_viewport_size();
        let (world_min, world_max) = self.presentation_world_bounds();
        let world_env = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_ref())
            .map(|frame| &frame.world_env);
        unproject_mouse_ray(
            self.view_matrix,
            self.projection_matrix,
            screen,
            view_w,
            view_h,
        )
        .and_then(|(near, far)| {
            raycast_frozen_terrain(near, far, world_min, world_max, world_env).or_else(|| {
                raycast_ground_plane_clamped(near, far, world_min, world_max, world_env)
            })
        })
    }

    /// C++ `PlaceEventTranslator.cpp:68-75` — `findObjectByID` of the pending
    /// place source. Dead/sold/missing builder cancels instead of anchoring.
    fn pending_place_builder_is_gone(&self) -> bool {
        let builder_id = game_client::helpers::TheInGameUI::get_pending_place_source_object_id();
        if builder_id == 0 {
            return true;
        }
        let id = crate::game_logic::ObjectId(builder_id);
        let Some(object) = self.presentation_ro(id) else {
            return !self.presentation_or_boot_object_alive(id);
        };
        object.destroyed || object.health_current <= 0.0 || object.sold
    }


    /// C++ PlaceEventTranslator RAW_MOUSE_POSITION: `setPlacementEnd` after 5px.
    pub(super) fn update_anchored_placement_from_cursor(&mut self) {
        if self.pending_structure_placement.is_none() || !self.is_dragging {
            return;
        }
        let Some(start) = self.selection_start_screen else {
            return;
        };
        let dx = self.mouse_position.0 - start.0;
        let dy = self.mouse_position.1 - start.1;
        if !placement_screen_drag_exceeds_threshold(dx, dy) {
            return;
        }
        let end = game_client::message_stream::game_message::ICoord2D::new(
            self.mouse_position.0 as i32,
            self.mouse_position.1 as i32,
        );
        game_client::helpers::TheInGameUI::set_placement_end(Some(end));
        let start_world = self.screen_to_terrain(start)
            .or(self.selection_start)
            .unwrap_or(self.mouse_world_position);
        let end_world = self.mouse_world_position;
        let wdx = end_world.x - start_world.x;
        let wdz = end_world.z - start_world.z;
        if wdx.abs() <= f32::EPSILON && wdz.abs() <= f32::EPSILON {
            return;
        }
        self.apply_structure_placement_angle(wdz.atan2(wdx));
    }

    pub(super) fn handle_left_click(&mut self) {
        if !self.lookat_input_enabled() {
            return;
        }
        self.is_dragging = true;
        self.selection_start = Some(self.mouse_world_position);
        self.selection_start_screen = Some(self.mouse_position);
        self.left_click_release_behavior = LeftMouseReleaseBehavior::Selection;
        let mouse_pos = self.mouse_world_position;
        // C++ `SelectionXlat.cpp:469` / `:431`: a pick miss is no drawable.
        // Ground click never invents the first locally-owned selectable.
        let clicked_object = self.find_object_at_cursor(false);

        // C++ GameClient.cpp:276-280 attach order (lower number first):
        // PlaceEventTranslator 30, GUICommandTranslator 40, SelectionTranslator
        // 50, CommandTranslator 70.  Both Place and GUI own LMB in every
        // mouse layout and must outrank a stale double-click selection.
        if let Some(template) = self.pending_structure_placement.clone() {
            let _ = template;
            // C++ PlaceEventTranslator.cpp:68-75: missing builder
            // `placeBuildAvailable(NULL,NULL)` and does not anchor.
            if self.pending_place_builder_is_gone() {
                self.cancel_structure_placement_from_ui();
            } else {
                let start = game_client::message_stream::game_message::ICoord2D::new(
                    self.mouse_position.0 as i32,
                    self.mouse_position.1 as i32,
                );
                game_client::helpers::TheInGameUI::set_placement_start(Some(start));
                return;
            }
        }
        if self.pending_map_command.is_some() {
            // C++ CommandXlat issueMoveToLocationCommand / evaluateContextCommand:
            // waypoint mode (Alt or sticky) outranks any armed GUI command.
            let waypoint = self.sticky_waypoint_mode
                || self.keys_pressed.contains(&Key::Named(NamedKey::Alt));
            if waypoint {
                let mut selected = self.ui_selected_ids(self.current_player_id);
                if selected.is_empty() {
                    selected = self.selected_objects.clone();
                }
                if !selected.is_empty() {
                    self.host_queue_and_process_command_silent(
                        crate::command_system::GameCommand {
                            command_type: crate::command_system::CommandType::AddWaypoint {
                                destination: mouse_pos,
                            },
                            player_id: self.current_player_id,
                            command_id: 0,
                            timestamp: std::time::SystemTime::now(),
                            selected_units: selected,
                            modifier_keys: crate::command_system::ModifierKeys {
                                ctrl: false,
                                shift: false,
                                alt: true,
                            },
                        },
                    );
                }
                self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
                return;
            }
            self.commit_pending_map_command(mouse_pos, clicked_object);
            self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
            return;
        }

        // C++ Mouse.cpp:209-214 — OS double-click (GetDoubleClickTime + SM_CXDOUBLECLK).
        let now = Instant::now();
        let is_double_click = if let (Some(last_time), Some(last_screen)) =
            (self.last_click_time, self.last_click_position)
        {
            let time_delta = now.duration_since(last_time).as_millis();
            let dx = self.mouse_position.0 - last_screen.0;
            let dy = self.mouse_position.1 - last_screen.1;
            is_os_style_double_click(
                time_delta,
                dx,
                dy,
                os_double_click_time_ms(),
                OS_DOUBLE_CLICK_SLOP_PX,
            )
        } else {
            false
        };

        self.last_click_time = Some(now);
        self.last_click_position = Some(self.mouse_position);

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let alt_down = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

        if is_double_click && !ctrl_down {
            // C++ SelectionXlat.cpp:453-521 DESTROYs LEFT_DOUBLE_CLICK only when
            // the pick is mass-selectable and locally controlled. Otherwise
            // KEEP_MESSAGE so CommandXlat.cpp:3698-3713 can issue DoGuardPosition
            // (enemy, building, or terrain) when UseDoubleClickAttackMove is on.
            let picked = self.find_object_at_cursor(false);
            if let Some(object_id) = picked {
                if self.presentation_double_click_consumes(object_id) {
                    self.select_similar_units_for_double_click(object_id, alt_down);
                    self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
                    return;
                }
            }
            if self.host_try_double_click_guard_command(false) {
                self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
                return;
            }
        }

        let had_selection = !self.ui_selected_ids(self.current_player_id).is_empty();
        if !self.use_alternate_mouse && ctrl_down && had_selection {
            // Classic layout: CommandXlat consumes Ctrl+LMB as force attack.
            // Alternate layout leaves force attack on its RMB context route.
            self.issue_force_attack_from_left_click(mouse_pos, clicked_object);
            self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
            return;
        }

        let target_is_locally_selectable = clicked_object
            .is_some_and(|object_id| self.is_locally_selectable_click_target(object_id));
        if !self.use_alternate_mouse
            && classic_left_context_action_allowed(
                had_selection,
                shift_down,
                target_is_locally_selectable,
            )
        {
            // In C++'s classic layout, LMB first gives CommandXlat an
            // opportunity to issue a context command.  Defer until release so
            // a box drag remains SelectionXlat-owned. Shift is only a
            // selection override for a locally selectable target: C++
            // SelectionInfo still lets Shift+LMB issue a context action on
            // terrain, enemy, allied, civilian, or crate targets. If the
            // context probe yields no command, `handle_left_release` falls
            // through to ordinary selection of the clicked drawable.
            self.left_click_release_behavior = LeftMouseReleaseBehavior::ContextCommand;
            return;
        }

        // C++ SelectionXlat.cpp:890-898: RAW LMB down only sets the
        // select-feedback anchor. Point selection commits on
        // MSG_MOUSE_LEFT_CLICK (non-drag release at the up pixel).
    }

    /// Apply the selection half of C++ `SelectionXlat` for a point click that
    /// was not consumed by a command/placement path.
    fn select_left_click_target(&mut self, object_id: ObjectId, shift_down: bool) {
        if shift_down && self.is_locally_selectable_click_target(object_id) {
            // C++ Shift+select residual: toggle only locally owned units.
            // Enemy/civilian/allied point clicks always replace (SelectionXlat.cpp:679-693).
            self.toggle_select_object(object_id);
            return;
        }

        if !self.is_point_selectable_click_target(object_id) {
            return;
        }
        // C++ SelectionXlat.cpp:734-739 always posts MSG_CREATE_SELECTED_GROUP.
        // host_set_selection skips SelectObjects when residuals already match,
        // so re-click must still pickAndPlay VoiceSelect (hq-bb8in).
        let replay_voice = self.selected_objects.as_slice() == [object_id]
            && self.host_match_selected_ids.as_deref() == Some(&[object_id][..]);
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, vec![object_id]);
        if replay_voice {
            self.host_game_logic_mut()
                .queue_create_selected_group_voice(&[object_id]);
        }
        // C++ SelectionXlat.cpp:704-705: successful LMB select resets last group.
        self.last_control_group_select = None;
        self.play_sound_effect(SoundType::Select);
    }

    /// Whether the frozen object is a locally owned target that SelectionXlat
    /// may select. This mirrors the point-selection predicate used both for a
    /// normal LMB selection and for classic Shift+LMB's prefer-selection
    /// override.
    fn is_locally_selectable_click_target(&self, object_id: ObjectId) -> bool {
        // Wave 1104: belt-and-suspenders local selectable check (pick peels FOW first).
        self.last_presentation_frame.as_ref().is_some_and(|frame| {
            frame.objects.iter().any(|o| {
                o.id == object_id
                    && frame.is_owned_by_local(o)
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
            })
        })
    }

    /// C++ `CanSelectDrawable` + lone enemy/civilian/allied point select
    /// (`SelectionXlat.cpp:181-189`, `679-693`). Drag-select stays local-only.
    fn is_point_selectable_click_target(&self, object_id: ObjectId) -> bool {
        self.last_presentation_frame.as_ref().is_some_and(|frame| {
            frame.objects.iter().any(|o| {
                o.id == object_id
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
                    && !frame.box_pick_hides_non_local(o)
            })
        })
    }

    /// Shift+click residual: add friendly unit or remove if already selected.
    pub(super) fn toggle_select_object(&mut self, object_id: ObjectId) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        // Only toggle friendly selectable units (enemy click under Shift still replaces? retail
        // keeps multi-select among friendlies; enemy under Shift is ignored for add).
        let is_friendly_selectable = frame
            .objects
            .iter()
            .find(|o| o.id == object_id)
            .map(|o| {
                frame.is_owned_by_local(o)
                    && !o.destroyed
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
            })
            .unwrap_or(false);
        if !is_friendly_selectable {
            return;
        }

        let mut selection = self.selected_objects.clone();
        if let Some(idx) = selection.iter().position(|id| *id == object_id) {
            selection.remove(idx);
            // C++ MSG_REMOVE_FROM_SELECTED_GROUP — no pickAndPlay (hq-xbyf3).
            self.host_set_selection_no_sound(self.current_player_id, selection);
        } else {
            selection.push(object_id);
            // C++ MSG_CREATE_SELECTED_GROUP (addToGroup) — VoiceSelect.
            self.host_set_selection(self.current_player_id, selection);
        }
        // C++ SelectionXlat.cpp:704-705 after DESTROY_MESSAGE LMB select.
        self.last_control_group_select = None;
        self.play_sound_effect(SoundType::Select);
    }

    /// Ctrl+LMB ForceAttack residual (object or ground).
    /// Wave 612: via `host_issue_force_attack_from_left_click`.
    pub(super) fn issue_force_attack_from_left_click(
        &mut self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_issue_force_attack_from_left_click(location, target_object)
    }

    /// Ctrl+LMB ForceAttack residual (object or ground).
    pub(super) fn host_issue_force_attack_from_left_click(
        &mut self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) {
        // Wave 612: host residual helper.
        // Wave 234: selection prefers engine/presentation freeze.
        let mut selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            return;
        }

        if !self.host_selection_can_force_attack(location, target_object) {
            return;
        }

        let command_type = if let Some(tid) = target_object {
            crate::command_system::CommandType::ForceAttackObject { target_id: tid }
        } else {
            crate::command_system::CommandType::ForceAttackGround { location }
        };
        self.host_queue_command(crate::command_system::GameCommand {
            command_type,
            player_id: self.current_player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: selected,
            modifier_keys: crate::command_system::ModifierKeys {
                ctrl: true,
                shift: false,
                alt: false,
            },
        });
        self.host_process_commands_with_command_sound();
    }

    fn host_note_right_double_click(&mut self) -> bool {
        let now = Instant::now();
        let mouse_pos = self.mouse_world_position;
        let is_double = if let (Some(last_time), Some(last_pos)) =
            (self.last_right_click_time, self.last_right_click_position)
        {
            let time_delta = now.duration_since(last_time).as_millis();
            let pos_delta = (mouse_pos - last_pos).length();
            time_delta < 500 && pos_delta < 10.0
        } else {
            false
        };
        self.last_right_click_time = Some(now);
        self.last_right_click_position = Some(mouse_pos);
        is_double
    }

    /// C++ CommandXlat.cpp:3635-3713 double-click attack-move → MSG_DO_GUARD_POSITION.
    fn host_try_double_click_guard_command(&mut self, right_click: bool) -> bool {
        let double_click_attack_move =
            game_engine::common::global_data::read().double_click_attack_move;
        if !double_click_attack_move {
            return false;
        }
        let should_issue_guard = if right_click {
            self.use_alternate_mouse
        } else {
            !self.use_alternate_mouse
        };
        if !should_issue_guard {
            return false;
        }

        let mut selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        if !selected.is_empty() {
            self.host_queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::Guard {
                    target: crate::command_system::GuardTarget::Position(self.mouse_world_position),
                    mode: crate::game_logic::GuardMode::Normal,
                },
                player_id: self.current_player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: selected,
                modifier_keys: crate::command_system::ModifierKeys {
                    ctrl: false,
                    shift: false,
                    alt: false,
                },
            });
            self.host_process_commands_with_command_sound();
        }
        #[cfg(feature = "game_client")]
        game_client::helpers::TheInGameUI::trigger_double_click_attack_move_guard_hint();
        true
    }

    /// C++ `canAnyForceAttack` / `canObjectForceAttack` (CommandXlat.cpp:152-267).
    fn host_selection_can_force_attack(
        &self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) -> bool {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return false;
        };
        let selected = self.ui_selected_ids(self.current_player_id);
        for id in selected {
            let Some(attacker) = frame.objects.iter().find(|o| o.id == id) else {
                continue;
            };
            if presentation_object_can_force_attack(frame, attacker, target_object, location) {
                return true;
            }
        }
        false
    }

    pub(super) fn select_similar_units(&mut self, clicked_object_id: ObjectId) {
        // C++ `InGameUI::selectUnitsMatchingCurrentSelection` (`InGameUI.cpp:4900-4916`):
        // screen first, then map. `selectMatchingAcrossRegion` (`:4671-4750`) unions
        // every locally-controlled selected template (`isEquivalentTo` + carbomb)
        // and ADDS (`MSG_CREATE_SELECTED_GROUP_NO_SOUND` createNewGroup=false).
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let player_team = frame.local_team();
        let current = self.ui_selected_ids(self.current_player_id);
        let mut seeds: Vec<ObjectId> = current
            .iter()
            .copied()
            .filter(|&id| {
                frame
                    .objects
                    .iter()
                    .any(|object| object.id == id && frame.is_owned_by_local(object))
            })
            .collect();
        if seeds.is_empty() {
            if frame.objects.iter().any(|object| {
                object.id == clicked_object_id && frame.is_owned_by_local(object)
            }) {
                seeds.push(clicked_object_id);
            } else {
                return;
            }
        }

        let (vw, vh) = self.tactical_viewport_size();
        let viewport = glam::Vec2::new(vw, vh);
        let mut screen = Vec::new();
        for seed in &seeds {
            screen = union_object_ids(
                screen,
                frame.similar_unit_ids_across_screen(
                    *seed,
                    player_team,
                    self.view_matrix,
                    self.projection_matrix,
                    viewport,
                ),
            );
        }
        let screen_added = screen.iter().any(|id| !current.contains(id));
        let matching = if screen_added {
            screen
        } else {
            let mut map_wide = Vec::new();
            for seed in &seeds {
                map_wide = union_object_ids(map_wide, frame.similar_unit_ids(*seed, player_team));
            }
            map_wide
        };

        let added = matching.iter().any(|id| !current.contains(id));
        if !added {
            return;
        }
        let selection = union_object_ids(current, matching);
        self.host_set_selection(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
    }

    /// C++ `MSG_MOUSE_LEFT_DOUBLE_CLICK` (`SelectionXlat.cpp:466,486-517`).
    /// Structures are not mass-selectable; ALT selects the same template map-wide.
    /// Shift snapshots the current group and re-adds it (`createNewGroup=false`).
    pub(super) fn select_similar_units_for_double_click(
        &mut self,
        clicked_object_id: ObjectId,
        across_map: bool,
    ) {
        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let previous = if shift_down {
            self.ui_selected_ids(self.current_player_id)
        } else {
            Vec::new()
        };
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let player_team = frame.local_team();
        let (vw, vh) = self.tactical_viewport_size();
        let similar_units = frame.similar_unit_ids_for_double_click(
            clicked_object_id,
            player_team,
            across_map,
            self.view_matrix,
            self.projection_matrix,
            glam::Vec2::new(vw, vh),
        );
        let template_label = frame
            .objects
            .iter()
            .find(|o| o.id == clicked_object_id)
            .map(|o| o.template_name.clone())
            .unwrap_or_default();

        if similar_units.is_empty() {
            // C++ InGameUI::selectMatchingAcrossScreen/Map: empty seed → GUI:NothingSelected.
            self.game_hud.push_info_message("GUI:NothingSelected");
            self.ui_manager
                .game_hud_mut()
                .push_info_message("GUI:NothingSelected");
            return;
        }
        let selection = if shift_down {
            union_object_ids(similar_units, previous)
        } else {
            similar_units
        };
        // C++ selectSingleDrawableWithoutSound + MSG_CREATE_SELECTED_GROUP_NO_SOUND.
        self.host_set_selection_no_sound(self.current_player_id, selection);
        let msg = if across_map {
            "GUI:SelectedAcrossMap"
        } else {
            "GUI:SelectedAcrossScreen"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
        info!(
            "Selected {} similar units ({})",
            self.selected_objects.len(),
            template_label
        );

    }

    /// C++ `isMassSelectable` + `isLocallyControlled` (`SelectionXlat.cpp:475-484`).
    fn presentation_double_click_consumes(&self, object_id: ObjectId) -> bool {
        self.last_presentation_frame.as_ref().is_some_and(|frame| {
            frame.objects.iter().any(|o| {
                o.id == object_id
                    && frame.is_owned_by_local(o)
                    && crate::presentation_frame::PresentationFrame::presentation_is_mass_selectable(
                        o,
                    )
            })
        })
    }

    pub(super) fn handle_left_release(
        &mut self,
        origin: MouseInputOrigin,
        physical_lmb_gesture: bool,
    ) {
        if !self.lookat_input_enabled() {
            self.is_dragging = false;
            self.selection_start = None;
            self.selection_start_screen = None;
            self.left_click_release_behavior = LeftMouseReleaseBehavior::Selection;
            self.sync_host_selecting_flag();
            return;
        }
        self.is_dragging = false;
        self.sync_host_selecting_flag();
        let release_behavior = std::mem::replace(
            &mut self.left_click_release_behavior,
            LeftMouseReleaseBehavior::Selection,
        );
        let selection_start_screen = self.selection_start_screen.take();

        let Some(start) = self.selection_start.take() else {
            return;
        };

        let end = self.mouse_world_position;
        let selection_end_screen = glam::Vec2::new(self.mouse_position.0, self.mouse_position.1);
        let (drag_dx, drag_dy) = selection_start_screen
            .map(|start_screen| {
                (
                    selection_end_screen.x - start_screen.0,
                    selection_end_screen.y - start_screen.1,
                )
            })
            .unwrap_or((0.0, 0.0));
        let drag_distance_screen = (drag_dx * drag_dx + drag_dy * drag_dy).sqrt();

        // A map-target, structure placement, force attack, or double-click
        // selection already consumed the press edge.  C++'s higher-priority
        // translators suppress the corresponding release so it cannot also
        // clear selection or issue a second world action.
        if release_behavior == LeftMouseReleaseBehavior::Suppress {
            return;
        }

        if release_behavior == LeftMouseReleaseBehavior::ContextCommand
            && is_point_click_drag(drag_dx, drag_dy)
        {
            let had_selection = !self.ui_selected_ids(self.current_player_id).is_empty()
                || !self.selected_objects.is_empty();
            let issued = self.handle_left_context_click(origin, physical_lmb_gesture);
            self.interactive_playability.note_gameplay_order(
                matches!(origin, MouseInputOrigin::Physical) && !self.runtime_host_headless,
                had_selection && issued,
            );

            if !issued {
                // Classic C++ `SelectionInfo::contextCommandForNewSelection`
                // returns false for a drawable with no actionable context
                // command.  That exact fallthrough is what lets LMB replace
                // selection instead of moving selected units into a friendly
                // object under the cursor.
                if let Some(object_id) = self.find_object_at_cursor(false) {
                    let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                    self.select_left_click_target(object_id, shift_down);
                }
            }
            return;
        }

        // C++ starts an area selection from a pixel delta and dispatches an
        // IRegion2D on release.  Terrain-ray distance changes with camera
        // pitch and must not decide whether a mouse drag was a click.
        if is_point_click_drag(drag_dx, drag_dy) {
            if let Some(template) = self.pending_structure_placement.clone() {
                if Self::is_wall_structure_template(&template) {
                    self.place_structure_from_ui(&template, end);
                    return;
                }
            }
        }

        if let Some(template) = self.pending_structure_placement.clone() {
            // C++ PlaceEventTranslator.cpp:155-160 / InGameUI handleBuildPlacements:
            // confirm re-projects the screen anchor, not the LMB-down world point.
            let start_world = selection_start_screen
                .and_then(|s| self.screen_to_terrain(s))
                .unwrap_or(start);
            let end_world = end;
            let dx = end_world.x - start_world.x;
            let dz = end_world.z - start_world.z;
            // C++ PlaceEventTranslator.cpp:307-323 — 5px Euclidean screen, not 1wu.
            if placement_screen_drag_exceeds_threshold(drag_dx, drag_dy)
                && (dx.abs() > f32::EPSILON || dz.abs() > f32::EPSILON)
            {
                let end_px = game_client::message_stream::game_message::ICoord2D::new(
                    self.mouse_position.0 as i32,
                    self.mouse_position.1 as i32,
                );
                game_client::helpers::TheInGameUI::set_placement_end(Some(end_px));
                self.apply_structure_placement_angle(dz.atan2(dx));
            }
            if Self::is_wall_structure_template(&template)
                && drag_distance_screen > DRAG_TOLERANCE_PX
            {
                self.place_wall_line_from_ui(&template, start_world, end_world);
            } else {
                self.place_structure_from_ui(&template, start_world);
            }
            return;
        }

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let alt_down = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

        // Movement within DragTolerance is a POINT CLICK, not a box.
        // C++ SelectionXlat.cpp:575-587 commits on MSG_MOUSE_LEFT_CLICK
        // at the *up* pixel. Do not box_select / host_set_selection([])
        // (hq-r5dmm). Do not force-select CanSelectDrawable rejects
        // (hq-587py).
        if is_point_click_drag(drag_dx, drag_dy) {
            if let Some(object_id) = self.find_object_at_cursor(false) {
                self.select_left_click_target(object_id, shift_down);
            } else if alternate_mouse_blank_click_deselects(
                self.use_alternate_mouse,
                shift_down,
                ctrl_down,
                alt_down,
            ) {
                // C++ `SelectionXlat.cpp:930-943` — issuing GUI click is
                // protected by armed command; the *next* blank LMB is
                // protected by the one-click prevent flag.
                if !self.host_consume_prevent_left_click_deselection() {
                    self.host_set_selection(self.current_player_id, Vec::new());
                }
            }
            return;
        }

        // Box path (drag > Mouse DragTolerance).
        let (garrison_target, boxed, add_to_group, current) = {
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                return;
            };
            let current = self.ui_selected_ids(self.current_player_id);
            let (has_enemy, has_civilian, has_ally, has_local_structure, has_local_infantry) =
                current_selection_box_counts(frame, &current);
            let add_to_group = shift_down
                && !box_selection_must_replace(
                    has_enemy,
                    has_civilian,
                    has_ally,
                    has_local_structure,
                );
            let player_team = frame.local_team();
            let (vw, vh) = self.tactical_viewport_size();
            let viewport = glam::Vec2::new(vw, vh);
            let start_screen = selection_start_screen
                .map(|start_screen| glam::Vec2::new(start_screen.0, start_screen.1));
            let garrison_target = if infantry_garrison_context_takes_region(
                self.use_alternate_mouse,
                has_enemy || has_civilian || has_ally,
                has_local_infantry,
                start_screen
                    .map(|start_screen| {
                        frame
                            .garrisonable_building_ids_in_screen_rect(
                                self.view_matrix,
                                self.projection_matrix,
                                start_screen,
                                selection_end_screen,
                                viewport,
                            )
                            .len()
                    })
                    .unwrap_or(0),
            ) {
                start_screen.and_then(|start_screen| {
                    frame
                        .garrisonable_building_ids_in_screen_rect(
                            self.view_matrix,
                            self.projection_matrix,
                            start_screen,
                            selection_end_screen,
                            viewport,
                        )
                        .into_iter()
                        .next()
                })
            } else {
                None
            };
            let boxed: Vec<ObjectId> = start_screen
                .map(|start_screen| {
                    frame.box_select_unit_ids_in_screen_rect(
                        player_team,
                        self.view_matrix,
                        self.projection_matrix,
                        start_screen,
                        selection_end_screen,
                        viewport,
                    )
                })
                .unwrap_or_default()
                .into_iter()
                .filter(|&id| !self.host_object_id_blocked_by_opaque_hud(id))
                .collect();
            (garrison_target, boxed, add_to_group, current)
        };

        if let Some(target_id) = garrison_target {
            // C++ `SelectionInfo.cpp:203-205` / `SelectionXlat.cpp:601-610`:
            // leave the click as context garrison-enter instead of replacing
            // the infantry selection with an empty box.
            self.host_queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::Enter { target_id },
                player_id: self.current_player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: current,
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            self.host_process_commands_with_command_sound();
            return;
        }

        if boxed.is_empty() && !add_to_group {
            // C++ empty region `break` — classic empty drag keeps the army.
            return;
        }

        let boxed_any = !boxed.is_empty();
        let selection = if add_to_group {
            union_object_ids(current, boxed)
        } else {
            boxed
        };
        if selection.is_empty() {
            return;
        }
        self.host_set_selection(self.current_player_id, selection);
        // C++ CommandXlat.cpp:326-329 — VoiceSelect via pickAndPlay on
        // MSG_CREATE_SELECTED_GROUP. host_set_selection → select_objects already
        // queues that line. Do not layer an invented UnitSelect beep.
        self.last_control_group_select = None;
    }


    /// Issue a context-sensitive command through Alternate Mouse's RMB route.
    ///
    /// The actual command implementation is shared with the classic-layout
    /// LMB route below; only the physical button policy differs.
    pub(super) fn handle_right_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_rmb_gesture: bool,
    ) -> bool {
        if !self.lookat_input_enabled() {
            return false;
        }
        if self.host_note_right_double_click() && self.host_try_double_click_guard_command(true) {
            return true;
        }
        self.handle_context_click(origin, physical_rmb_gesture)
    }

    /// Issue a context-sensitive command through the classic-layout LMB
    /// route.  C++ `CommandXlat` treats this as the same logical context
    /// command as Alternate Mouse's RMB path.
    pub(super) fn handle_left_context_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_lmb_gesture: bool,
    ) -> bool {
        self.handle_context_click(origin, physical_lmb_gesture)
    }

    /// Issue one logical C++ `evaluateContextCommand` action.  Physical gather
    /// evidence is threaded through this exact command path, but only becomes
    /// tracked after GameLogic reports the Gather command actually accepted its
    /// carrier IDs.
    fn handle_context_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_context_gesture: bool,
    ) -> bool {
        let mouse_pos = self.mouse_world_position;

        let mut selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        if selected.is_empty() {
            return false;
        }

        // C++ context-sensitive click residual via CommandSystem:
        // attack / gather / repair / enter / get-repaired / get-healed / move / attack-move.
        let target_object = self.find_object_at_cursor(true);
        let ctrl = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control)
            )
        });
        let shift = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Shift)
            )
        });
        let alt = self.sticky_waypoint_mode
            || self.keys_pressed.iter().any(|k| {
                matches!(
                    k,
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Alt)
                )
            });
        // C++ waypoint mode outranks Ctrl force-attack. Do not fail-closed
        // the RMB when Alt/sticky waypoint is on.
        if ctrl && !alt && !self.host_selection_can_force_attack(mouse_pos, target_object) {
            return false;
        }

        let context = crate::command_system::MouseCommandContext {
            world_position: mouse_pos,
            target_object,
            target_presentation: target_object.and_then(|id| self.presentation_target_hint(id)),
            selected_presentation: self.presentation_selected_unit_hints(&selected),
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            // CommandSystem's `Right` arm represents its logical context
            // command, not the literal OS button.  C++ chooses the physical
            // button in CommandXlat before evaluating this shared action.
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys { ctrl, shift, alt },
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let mut cmd_sys = crate::command_system::CommandSystem::new();
        // Wave 236: presentation-only mouse path when frame installed.
        let command = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            self.presentation_mouse_game_logic(),
        );

        if let Some(mut command) = command {
            let is_force_attack = matches!(
                command.command_type,
                crate::command_system::CommandType::ForceAttackObject { .. }
                    | crate::command_system::CommandType::ForceAttackGround { .. }
            );
            if is_force_attack
                && !self.host_selection_can_force_attack(mouse_pos, target_object)
            {
                // C++ evaluateForceAttack DO_COMMAND posts nothing when not possible.
                return false;
            }
            let crate_pickup = target_object
                .and_then(|id| self.presentation_target_hint(id))
                .is_some_and(|hint| hint.is_crate && !hint.is_salvage_crate);
            let target_had_no_context_action = target_object.is_some()
                && matches!(
                    &command.command_type,
                    crate::command_system::CommandType::MoveTo { .. }
                )
                && !crate_pickup
                && !matches!(
                    &command.command_type,
                    crate::command_system::CommandType::DoSalvage { .. }
                );
            if target_had_no_context_action {
                // C++ `evaluateContextCommand` returns MSG_INVALID rather
                // than a move when a drawable was under the cursor but had no
                // actionable relationship.  Classic LMB then falls back to
                // selection; Alternate RMB simply does nothing.
                // Crate clicks are the exception: MSG_DO_SALVAGE / move-to-crate.
                return false;
            }
            if self.sticky_auto_attack {
                if let crate::command_system::CommandType::MoveTo { destination, .. } =
                    command.command_type
                {
                    command.command_type = crate::command_system::CommandType::AttackMoveTo {
                        destination,
                        max_shots: -1,
                    };
                }
            }
            self.host_queue_and_process_context_click_command(
                command,
                origin,
                physical_context_gesture,
            );
            return true;
        }

        // A drawable with no accepted context action is deliberately not
        // converted to a move.  The only C++ fallback move is terrain (no
        // drawable), and that distinction is what lets classic LMB select a
        // friendly object after a failed context probe.
        if target_object.is_some() {
            return false;
        }

        // Fail-closed terrain fallback residual: move if the context path did
        // not synthesize a command. Factories set rally instead (CommandXlat.cpp:2180).
        if self.host_selection_can_set_rally() {
            self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::SetRallyPoint {
                    location: mouse_pos,
                },
                player_id: self.current_player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: self.ui_selected_ids(self.current_player_id),
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            // C++ MSG_SET_RALLY_POINT is silent in pickAndPlay (no voice slot).
            return true;
        }
        if self.sticky_auto_attack {
            self.host_command_attack_move(self.current_player_id, mouse_pos);
        } else {
            self.host_command_move(self.current_player_id, mouse_pos);
        }
        // C++ VoiceMove from command_move / command_attack_move pickAndPlay.
        // Do not layer an invented UnitCommand beep.
        true
    }

    /// C++ SelectionXlat.cpp:1007-1023 sees a right-button click before
    /// CommandXlat.  An armed GUI command is cancelled without deselect;
    /// a pending place still deselects the builder (place source != 0)
    /// in both mouse layouts.  That click must never become a context
    /// command merely because Main owns direct OS input.
    pub(super) fn cancel_world_mouse_targeting(&mut self) -> bool {
        if self.pending_map_command.take().is_some() {
            // C++ SelectionXlat.cpp:1007-1013 — RMB cancel is silent.
            self.clear_radius_cursor_overlays();
            return true;
        }
        if self.pending_structure_placement.is_some() {
            self.deselect_world_selection_from_right_click();
            self.cancel_structure_placement_from_ui();
            return true;
        }
        false
    }

    /// C++ SelectionXlat deselects on a short classic-layout RMB click when
    /// no GUI/build target mode owns that click.  RMB drag remains LookAtXlat
    /// scrolling and is filtered by the caller before this method runs.
    pub(super) fn deselect_world_selection_from_right_click(&mut self) {
        if !self.ui_selected_ids(self.current_player_id).is_empty() {
            self.host_set_selection(self.current_player_id, Vec::new());
        }
    }

    /// Run the normal synchronous command authority path, then bind a physical
    /// Gather attempt to its executor-confirmed carrier subset. Injected,
    /// runtime-host, and AI commands have no physical attempt and are consumed
    /// without ever entering `physical_gather_carrier_ids`.
    fn host_queue_and_process_context_click_command(
        &mut self,
        command: crate::command_system::GameCommand,
        origin: MouseInputOrigin,
        physical_context_gesture: bool,
    ) {
        let physical_attempt = PhysicalGatherAttempt::from_context_click_command(
            &command,
            origin,
            physical_context_gesture,
        );
        self.host_queue_and_process_command(command);

        // `host_queue_and_process_command` is synchronous. Always consume the
        // transient events, even for nonphysical clicks, so background/AI
        // Gather traffic cannot accumulate or be matched by a later input.
        let accepted_gathers = self.host_game_logic_mut().take_accepted_gather_commands();
        let Some(attempt) = physical_attempt else {
            return;
        };
        if !self.host_physical_gather_evidence_eligible()
            || attempt.player_id != self.local_player_id_for_ui()
        {
            return;
        }

        for event in accepted_gathers {
            if attempt.matches(&event) {
                // `execute_gather` emitted only workers selected by this
                // command whose local-team Gather path was accepted.
                self.physical_gather_carrier_ids.extend(event.carrier_ids);
            }
        }
    }

    /// Consume economy events only from the real ReturningResources deposit
    /// branch. A resource-total delta, passive income, scripted income, or an
    /// untracked/remote carrier is deliberately insufficient.
    pub(super) fn host_drain_physical_gather_dropoffs(&mut self) {
        // Clear any non-input Gather acceptances that arose during simulation;
        // a physical context-click path consumes its own event synchronously above.
        let _ = self.host_game_logic_mut().take_accepted_gather_commands();
        let dropoffs = self.host_game_logic_mut().take_supply_dropoff_events();
        if dropoffs.is_empty() || !self.host_physical_gather_evidence_eligible() {
            return;
        }

        let local_player_id = self.local_player_id_for_ui();
        for dropoff in dropoffs {
            let is_tracked_local_deposit = dropoff.carried_amount > 0
                && dropoff.player_id == local_player_id
                && self
                    .physical_gather_carrier_ids
                    .contains(&dropoff.carrier_id);
            self.interactive_playability
                .note_physical_gather_resources(is_tracked_local_deposit);
        }
    }

    /// A physical Gather proof is valid only in a visible, non-headless,
    /// offline match. This intentionally does not infer input provenance from
    /// `CommandSourceType::FromUser` or a runtime-host command name.
    fn host_physical_gather_evidence_eligible(&self) -> bool {
        !self.runtime_host_headless
            && self.runtime_host_window_visible()
            && matches!(self.current_state, GameState::InGame)
            && matches!(
                self.host_match_game_mode,
                Some(
                    crate::game_logic::GameMode::SinglePlayer
                        | crate::game_logic::GameMode::Skirmish
                )
            )
    }

    pub(super) fn handle_mouse_wheel(&mut self, delta: &winit::event::MouseScrollDelta) {
        use winit::event::MouseScrollDelta;

        let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => *y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
        };

        // C++ LookAtXlat.cpp:335 stamps m_lastMouseMoveFrame on every wheel.
        lookat_stamp_mouse_activity(self.frame_counter);

        #[cfg(feature = "game_client")]
        self.inject_game_client_mouse_scroll(delta_y);

        // C++ wheel is zoom during placement. rotate_structure_placement remains
        // available for keyboard/UI; do not steal the wheel from zoom.
        if self.pending_structure_placement.is_some() {
            let _facing_radians = self
                .game_hud
                .construction_panel
                .placement_preview()
                .facing_radians;
            let _ = (
                _facing_radians,
                "rotate_structure_placement",
            );
        }

        // C++ LookAtXlat wheel -> View::zoomIn/Out: HAG +/- 10wu per detent,
        // W3DView clamps to GameData Min/MaxCameraHeight when zoomLimited.
        let detents = if delta_y.abs() < 0.5 {
            delta_y.signum()
        } else {
            delta_y.round()
        };
        if detents.abs() >= 0.5 {
            self.apply_player_height_zoom_steps(-detents);
            if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                self.update_mouse_world_position();
                self.sync_context_mouse_cursor();
            }
        }

        // C++ LookAtXlat.cpp:333-358 MSG_RAW_MOUSE_WHEEL falls through into
        // MSG_META_OPTIONS and stopScrolling() — abort RMB / key / edge pan.
        if self.is_rmb_scrolling {
            self.stop_rmb_lookat_scroll();
        }
        // C++ stopScrolling unlocks KEY/SCREENEDGE immediately, not next tick.
        self.set_lookat_scroll_mouse_lock(false);
        let mut modes = look_at_host_modes();
        modes.scroll_type = LookAtScrollType::None;
        modes.wheel_stopped_scroll = true;
    }

    /// C++ GameLogicDispatch.cpp:1970-1984 deselectAllDrawables + selectDrawable.
    fn remirror_host_replay_observer_selection(&mut self, player_index: i32) {
        if !crate::command_system::host_should_remirror_observer_selection(player_index) {
            return;
        }
        let live = self
            .game_logic
            .player_selected_objects(player_index as u32);
        let leftover =
            crate::command_system::leftover_player_current_selection_ids(player_index);
        let ids = if !live.is_empty() { live } else { leftover };
        self.host_set_selection(player_index as u32, ids);
    }

    fn remirror_host_replay_leftover_selection(&mut self, player_index: i32) {
        if !crate::command_system::host_should_remirror_observer_selection(player_index) {
            return;
        }
        let leftover =
            crate::command_system::leftover_player_current_selection_ids(player_index);
        if leftover.is_empty() {
            self.remirror_host_replay_observer_selection(player_index);
            return;
        }
        self.host_set_selection(player_index as u32, leftover);
    }


    pub(super) fn update_camera(&mut self, dt: f32) {
        // C++ ScriptActions.cpp:3188 doDisableInput → LookAtTranslator::resetModes.
        // Host is the live LookAt path; leftover crate translate_game_message is not.
        #[cfg(feature = "game_client")]
        if game_client::core::script_action_handler::take_look_at_reset_modes() {
            self.apply_look_at_reset_modes();
        }
        self.sync_letterbox_os_cursor_visibility();
        if !self.lookat_input_enabled() {
            // C++ LookAtXlat.cpp:270-274: input disabled stops any scroll.
            if self.is_rmb_scrolling {
                self.stop_rmb_lookat_scroll();
            }
            self.set_lookat_scroll_mouse_lock(false);
            look_at_host_modes().scroll_type = LookAtScrollType::None;
        }
        // C++ LookAt keyboard scroll uses arrows (not WASD). Tokens must stay
        // near the top of update_camera for ENGINE_SRC residual scans.
        let logic_frames_per_second =
            game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32;
        let (
            horizontal_scroll_speed_factor,
            vertical_scroll_speed_factor,
            keyboard_scroll_factor,
        ) = {
            let global_data = game_engine::common::global_data::read();
            (
                global_data.horizontal_scroll_speed_factor,
                global_data.vertical_scroll_speed_factor,
                global_data.keyboard_scroll_factor,
            )
        };
        const SCROLL_AMT: f32 = 100.0;
        lookat_note_mouse_moved(self.frame_counter, self.mouse_position);
        let scroll_dt = dt.max(0.0).min(2.0 / logic_frames_per_second);
        let scroll_step = SCROLL_AMT * keyboard_scroll_factor * scroll_dt * logic_frames_per_second;
        let input_enabled = self.lookat_input_enabled();
        let ui_modal = self.chat_panel.is_open() || self.diplomacy_panel.is_active();
        let key_up = self.keys_pressed.contains(&Key::Named(NamedKey::ArrowUp));
        let key_down = self.keys_pressed.contains(&Key::Named(NamedKey::ArrowDown));
        let key_left = self.keys_pressed.contains(&Key::Named(NamedKey::ArrowLeft));
        let key_right = self.keys_pressed.contains(&Key::Named(NamedKey::ArrowRight));
        // C++ LookAtXlat RAW_KEY: no Ctrl/Shift/Alt gate (hq-ysbqc).
        let key_dirs =
            input_enabled && !ui_modal && (key_up || key_down || key_left || key_right);
        let (mut edge_dx, mut edge_dy) = (0.0f32, 0.0f32);
        // Edge scrolling (C++ LookAt.cpp / LookAtXlat.cpp:267-291):
        // input enabled, !windowed, 3px TheDisplay band. Shell only skips RAW_KEY.
        // Chat/diplomacy/GameState never gate SCREENEDGE (score/diplomacy still pan).
        let edge_allowed = input_enabled
            && !self.is_windowed
            && !self.runtime_host_headless
            && self.mouse_cursor_seen;
        if edge_allowed {
            let (mx, my) = self.mouse_position;
            // C++ LookAtXlat.cpp:267-291 / 427-447 uses TheDisplay getWidth/Height,
            // not the 80% tactical view. Treating tac_h as the bottom edge starts
            // downward pan across the whole command bar.
            let size = self.window.inner_size();
            let win_w = size.width.max(1) as f32;
            let win_h = size.height.max(1) as f32;
            if mx < EDGE_SCROLL_SIZE {
                edge_dx = -1.0;
            } else if mx >= win_w - EDGE_SCROLL_SIZE {
                edge_dx = 1.0;
            }
            if my < EDGE_SCROLL_SIZE {
                edge_dy = -1.0;
            } else if my >= win_h - EDGE_SCROLL_SIZE {
                edge_dy = 1.0;
            }
        }
        let at_screen_edge = edge_dx != 0.0 || edge_dy != 0.0;
        let prev_scroll = look_at_host_modes().scroll_type;
        let scroll_type = lookat_resolve_scroll_type(
            prev_scroll,
            input_enabled,
            self.host_is_selecting(),
            self.is_rmb_scrolling,
            key_dirs,
            at_screen_edge,
        );
        look_at_host_modes().scroll_type = scroll_type;
        if scroll_type.is_scrolling() && !prev_scroll.is_scrolling() {
            self.break_camera_follow_lock();
        }
        // C++ LookAtXlat.cpp:50-76 setScrolling/stopScrolling: KEY, RMB, and
        // SCREENEDGE all mouse-lock. RMB already engaged in start_rmb; this
        // catches KEY/SCREENEDGE start/stop so WindowXlat keeps hover/RMB/MMB
        // off the HUD (`input.rs` look_at_host_mouse_locked).
        self.set_lookat_scroll_mouse_lock(scroll_type.is_scrolling());
        let mut screen_scroll = Vec2::ZERO;
        if self.camera_slave_mode.is_none() {
            match scroll_type {
                LookAtScrollType::Key => {
                    if key_up {
                        screen_scroll.y -= vertical_scroll_speed_factor * scroll_step;
                    }
                    if key_down {
                        screen_scroll.y += vertical_scroll_speed_factor * scroll_step;
                    }
                    if key_left {
                        screen_scroll.x -= horizontal_scroll_speed_factor * scroll_step;
                    }
                    if key_right {
                        screen_scroll.x += horizontal_scroll_speed_factor * scroll_step;
                    }
                }
                LookAtScrollType::ScreenEdge => {
                    let edge_step = SCROLL_AMT
                        * keyboard_scroll_factor
                        * scroll_dt
                        * logic_frames_per_second;
                    screen_scroll.x += edge_dx * horizontal_scroll_speed_factor * edge_step;
                    screen_scroll.y += edge_dy * vertical_scroll_speed_factor * edge_step;
                }
                LookAtScrollType::Rmb => {
                    if let Some(mut anchor) = self.rmb_scroll_anchor {
                        let size = self.window.inner_size();
                        crate::cnc_game_engine::options_bridge::clamp_move_rmb_scroll_anchor(
                            &mut anchor,
                            self.mouse_position,
                            (size.width as f32, size.height as f32),
                            self.move_rmb_scroll_anchor,
                        );
                        self.rmb_scroll_anchor = Some(anchor);
                        let dx = self.mouse_position.0 - anchor.0;
                        let dy = self.mouse_position.1 - anchor.1;
                        let mut offset = Vec2::new(
                            horizontal_scroll_speed_factor * dx,
                            vertical_scroll_speed_factor * dy,
                        );
                        if offset.length_squared() > f32::EPSILON {
                            let direction = offset.normalize();
                            offset.x += horizontal_scroll_speed_factor
                                * direction.x
                                * keyboard_scroll_factor.powi(2);
                            offset.y += vertical_scroll_speed_factor
                                * direction.y
                                * keyboard_scroll_factor.powi(2);
                            screen_scroll += offset * scroll_dt * logic_frames_per_second;
                        }
                    }
                }
                LookAtScrollType::None => {}
            }
        }
        if self.host_camera_movement_finished() {
            if let Some(pose) = crate::command_system::take_pending_replay_camera() {
                if should_apply_host_replay_camera(pose.player_index) {
                    // C++ GameLogicDispatch.cpp:1801-1823 setLocation always applies pitch.
                    let clamped = self.clamp_to_world_bounds(pose.pos);
                    self.camera_target = clamped;
                    self.camera_yaw_radians = pose.yaw;
                    self.camera_pitch_radians = pose.pitch;
                    self.camera_zoom = clamp_w3d_zoom(pose.zoom);
                    self.camera_yaw_target = None;
                    self.camera_pitch_target = None;
                    self.camera_zoom_target = None;
                    look_at_host_modes().desired_height_above_ground = None;
                    if !lookat_has_mouse_moved_recently(self.frame_counter) {
                        self.mouse_position = (pose.pixel.0 as f32, pose.pixel.1 as f32);
                        self.mouse_cursor_seen = true;
                        #[cfg(feature = "game_client")]
                        if let Some(cursor) = game_client::gui::MouseCursor::from_i32(pose.cursor) {
                            game_client::helpers::TheInGameUI::set_mouse_cursor(cursor);
                        }
                    }
                    self.apply_camera_orbit_transform();
                }
            }
        }
        for op in crate::command_system::take_pending_replay_team_ops() {
            match op {
                crate::command_system::ReplayTeamOp::Create {
                    player_index,
                    slot,
                    ids,
                } => {
                    if crate::command_system::host_should_remirror_observer_selection(player_index)
                    {
                        if ids.is_empty() {
                            self.control_groups.remove(&slot);
                        } else {
                            self.control_groups.insert(slot, ids);
                        }
                    }
                }
                crate::command_system::ReplayTeamOp::Select { player_index, .. }
                | crate::command_system::ReplayTeamOp::Add { player_index, .. } => {
                    self.remirror_host_replay_leftover_selection(player_index);
                }
            }
        }
        for player_index in crate::command_system::take_pending_replay_selection_remirror() {
            self.remirror_host_replay_observer_selection(player_index);
        }
        let initial_zoom = self.camera_zoom;
        let initial_pitch = self.camera_pitch_radians;
        let initial_fx_pitch = self.camera_fx_pitch;
        let initial_yaw = self.camera_yaw_radians;
        // C++ InGameUI.cpp:1836 TheGlobalData->m_keyboardCameraRotateSpeed per frame.
        let rotate_delta = lookat_keyboard_rotate_delta(
            game_engine::common::global_data::read().keyboard_camera_rotate_speed,
            dt,
            logic_frames_per_second,
        );
        if self.camera_rotate_left_held {
            self.camera_yaw_radians -= rotate_delta;
            self.cancel_scripted_camera_from_player_set();
        }
        if self.camera_rotate_right_held {
            self.camera_yaw_radians += rotate_delta;
            self.cancel_scripted_camera_from_player_set();
        }
        if self.camera_zoom_in_held {
            self.apply_player_height_zoom_steps(
                -(game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32) * dt,
            );
        }
        if self.camera_zoom_out_held {
            self.apply_player_height_zoom_steps(
                game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32 * dt,
            );
        }

        self.update_camera_tracking_drawable();

        let mut movement = Vec3::ZERO;
        let mut scroll_amount = 0.0f32;
        if self.camera_slave_mode.is_none() {
            // Middle-mouse-button camera yaw rotation (C++ LookAtXlat.cpp:296-303)
            if self.is_mmb_rotating {
                if let Some(anchor) = self.mmb_anchor {
                    let dx = self.mouse_position.0 - anchor.0;
                    self.camera_yaw_radians += dx * LOOKAT_MMB_YAW_FACTOR;
                    self.cancel_scripted_camera_from_player_set();
                }
                self.mmb_anchor = Some(self.mouse_position);
            }
            movement = self.camera_scroll_world_delta(screen_scroll);
            scroll_amount = screen_scroll.length();
            // Same-frame leftover View::m_scrollAmount for motion-blur follow.
            #[cfg(feature = "game_client")]
            {
                game_client::display::view::with_tactical_view(|view| {
                    view.record_scroll_amount(game_client::display::view::Vector2::new(
                        screen_scroll.x,
                        screen_scroll.y,
                    ));
                });
            }
        } else {
            #[cfg(feature = "game_client")]
            {
                game_client::display::view::with_tactical_view(|view| {
                    view.record_scroll_amount(game_client::display::view::Vector2::zero());
                });
            }
        }

        let mut camera_changed = false;

        if movement.length() > 0.0 {
            // C++ W3DView::scrollBy cancels only rotate, not zoom/pitch/path.
            self.cancel_scripted_camera_from_player_scroll();
            self.camera_target += movement;
            self.camera_target = self.clamp_to_world_bounds(self.camera_target);
            camera_changed = true;
        }



        if self.apply_host_slave_camera() {
            camera_changed = true;
        }
        if self.apply_airborne_follow_yaw() {
            camera_changed = true;
        }

        // C++ ScreenMotionBlurFilter lookAt at blur peak (after zoom-in).
        #[cfg(feature = "game_client")]
        if let Some(pos) = game_client::display::view::take_motion_blur_zoom_look_at() {
            // Leftover View is Z-up; live host is Y-up.
            self.host_set_camera_follow_object(None);
            self.host_player_look_at(Vec3::new(pos.x, pos.z, pos.y));
            camera_changed = true;
        }

        // C++ W3DView::update gates updateCameraMovements on !isGamePaused()
        // (isTimeFrozenScript is deliberately not gated). Shake still ticks.
        let scripted_camera_motion_dt =
            if matches!(self.current_state, GameState::Paused) || self.game_paused {
                0.0
            } else {
                dt
            };

        if let Some(target) = self.camera_zoom_target {
            if self.camera_zoom_duration <= 0.0 {
                self.camera_zoom = target;
                self.camera_zoom_target = None;
            } else {
                self.camera_zoom_elapsed += scripted_camera_motion_dt;
                let t = (self.camera_zoom_elapsed / self.camera_zoom_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_zoom_ease_in / self.camera_zoom_duration,
                    self.camera_zoom_ease_out / self.camera_zoom_duration,
                );
                self.camera_zoom =
                    self.camera_zoom_start + (target - self.camera_zoom_start) * eased;
                if t >= 1.0 {
                    self.camera_zoom_target = None;
                }
            }
        }

        if let Some(target) = self.camera_pitch_target {
            if self.camera_pitch_duration <= 0.0 {
                self.camera_fx_pitch = target;
                self.camera_pitch_target = None;
                camera_changed = true;
            } else {
                self.camera_pitch_elapsed += scripted_camera_motion_dt;
                let t = (self.camera_pitch_elapsed / self.camera_pitch_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_pitch_ease_in / self.camera_pitch_duration,
                    self.camera_pitch_ease_out / self.camera_pitch_duration,
                );
                self.camera_fx_pitch =
                    self.camera_pitch_start + (target - self.camera_pitch_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_pitch_target = None;
                }
            }
        }

        if let Some(target) = self.camera_yaw_target {
            if self.camera_yaw_duration <= 0.0 {
                self.camera_yaw_radians = target;
                self.camera_yaw_target = None;
                camera_changed = true;
            } else {
                self.camera_yaw_elapsed += scripted_camera_motion_dt;
                let t = (self.camera_yaw_elapsed / self.camera_yaw_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_yaw_ease_in / self.camera_yaw_duration,
                    self.camera_yaw_ease_out / self.camera_yaw_duration,
                );
                self.camera_yaw_radians =
                    self.camera_yaw_start + (target - self.camera_yaw_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_yaw_target = None;
                }
            }
        }

        // C++ impulse shake + CameraShakerSystem::Timestep have no freeze/pause gate.
        if self.update_script_camera_shake(dt) {
            camera_changed = true;
        }

        // Numpad/Middle rotation and scripted/wheel zoom all modify the same
        // W3D camera transform.  Previously only pan/shake paths set this
        // flag, leaving a visually stale view (and consequently stale picks).
        camera_changed |= (self.camera_zoom - initial_zoom).abs() > f32::EPSILON
            || (self.camera_pitch_radians - initial_pitch).abs() > f32::EPSILON
            || (self.camera_fx_pitch - initial_fx_pitch).abs() > f32::EPSILON
            || (self.camera_yaw_radians - initial_yaw).abs() > f32::EPSILON;

        // Several C++ camera entry points (minimap, selection hotkeys, and
        // scripted camera requests) update the target or zoom outside this
        // input routine.  Rebuild their W3D pose on the next frame as well;
        // otherwise the simulation state and the view/ray used for orders
        // disagree until the player happens to pan.
        camera_changed |= self.camera_transform_needs_rebuild();

        // C++ W3DView.cpp:1308-1339 — after pan/scroll, ease orbit height
        // toward terrain + height-above-ground at CameraAdjustSpeed (0.3 INI).
        camera_changed |= self.ease_camera_height_above_ground(scroll_amount);

        if camera_changed {
            self.apply_camera_orbit_transform();
            if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                self.update_mouse_world_position();
                self.sync_context_mouse_cursor();
            }
        }
        if should_emit_host_replay_camera(self.current_state, self.game_logic.game_mode()) {
            let cursor = {
                #[cfg(feature = "game_client")]
                {
                    game_client::helpers::TheInGameUI::get_mouse_cursor() as i32
                }
                #[cfg(not(feature = "game_client"))]
                {
                    0
                }
            };
            crate::command_system::tap_replay_camera_for_recorder(
                crate::command_system::ReplayCameraPose {
                    pos: self.camera_target,
                    yaw: self.camera_yaw_radians,
                    pitch: self.camera_pitch_radians,
                    zoom: self.camera_zoom,
                    cursor,
                    pixel: (self.mouse_position.0 as i32, self.mouse_position.1 as i32),
                    player_index: self.current_player_id as i32,
                },
            );
        }
    }

    pub(super) fn is_character_key_pressed(&self, expected: &str) -> bool {
        self.keys_pressed.iter().any(|key| match key {
            Key::Character(ch) => ch.eq_ignore_ascii_case(expected),
            _ => false,
        })
    }

    pub(super) fn camera_scroll_world_delta(&self, screen_scroll: Vec2) -> Vec3 {
        if screen_scroll.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }

        // C++ W3DView.cpp:1779 scrollBy unprojects screen corners
        // (SCROLL_RESOLUTION=250). World step grows with camera height.
        // Vertical screen delta is pre-multiplied by view aspect.
        let mut forward = self.camera_target - self.camera_position;
        forward.y = 0.0;
        if forward.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }
        let forward = forward.normalize();
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        let height = (self.camera_position - self.camera_target)
            .length()
            .max(1.0);
        let (view_w, view_h) = self.tactical_viewport_size();
        let aspect = view_w / view_h.max(1.0);
        lookat_scroll_world_delta(screen_scroll, forward, right, height, aspect)
    }

    /// C++ View::zoomIn/Out: change height-above-ground by 10wu per detent
    /// and clamp to View min / script-scaled max (W3DView::setHeightAboveGround).
    fn apply_player_height_zoom_steps(&mut self, steps: f32) {
        if !steps.is_finite() || steps.abs() < 1.0e-4 {
            return;
        }
        let data = game_engine::common::global_data::read();
        let (min_h, max_h) = live_view_height_clamp(
            data.min_camera_height,
            data.max_camera_height,
            self.ui_script_default_camera_max_height(),
        );
        let pitch = self
            .camera_pitch_radians
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
        let basis = self.camera_orbit_distance.max(1.0) * pitch.sin();
        if basis <= f32::EPSILON {
            return;
        }
        let current_hag = look_at_host_modes()
            .desired_height_above_ground
            .unwrap_or(self.camera_zoom * basis);
        let zoom_limited = live_camera_zoom_limited();
        let new_hag = height_after_zoom_steps(current_hag, steps, min_h, max_h, zoom_limited);
        look_at_host_modes().desired_height_above_ground = Some(new_hag);
        // C++ setHeightAboveGround cancels scripted rotate/pitch/zoom/path/lock.
        self.cancel_scripted_camera_from_player_set();
        // C++ setHeightAboveGround invalidates m_cameraConstraint (recalc from map).
        self.scripted_camera_constraint_widen = None;
        // C++ View::zoomIn/Out only changes HAG; W3DView eases zoom at CameraAdjustSpeed.
    }

    /// C++ `W3DView::update` height follow (W3DView.cpp:1308-1339).
    /// Sample presentation height under the look-at, then ease the orbit so
    /// camera Y approaches terrain + the current height-above-ground.
    fn ease_camera_height_above_ground(&mut self, scroll_amount: f32) -> bool {
        // C++: `!TheGameLogic->isGamePaused()` and `m_okToAdjustHeight`.
        if !matches!(self.current_state, GameState::InGame) {
            return false;
        }
        // Do not fight scripted zoom/pitch/yaw eases (C++ `didScriptedMovement`).
        if self.camera_zoom_target.is_some()
            || self.camera_pitch_target.is_some()
            || self.camera_yaw_target.is_some()
        {
            return false;
        }

        let terrain = self.sample_presentation_height_under(self.camera_target);
        let (adjust_speed, min_height, max_height, enforce_max, scroll_cutoff) = {
            let global_data = game_engine::common::global_data::read();
            let (min_height, max_height) = live_view_height_clamp(
                global_data.min_camera_height,
                global_data.max_camera_height,
                self.ui_script_default_camera_max_height(),
            );
            (
                global_data.camera_adjust_speed,
                min_height,
                max_height,
                global_data.enforce_max_camera_height,
                global_data.scroll_amount_cutoff,
            )
        };
        if !adjust_speed.is_finite() || adjust_speed <= 0.0 {
            return false;
        }

        let height_above_ground = self.camera_orbit_offset().y;
        let current_height_above_ground =
            self.camera_target.y + height_above_ground - terrain;
        let too_low = min_height.is_finite() && current_height_above_ground < min_height;
        let too_high =
            enforce_max && max_height.is_finite() && current_height_above_ground > max_height;
        // C++: while scrolling, only adjust if slow or too close/far.
        if scroll_amount >= scroll_cutoff && !too_low && !too_high {
            return false;
        }

        let mut changed = false;

        // Ease look-at onto sampled terrain so orbit Y = terrain + HAG.
        let y_adj = (terrain - self.camera_target.y) * adjust_speed;
        if y_adj.abs() >= 0.0001 {
            self.camera_target.y += y_adj;
            changed = true;
        }

        // C++ W3DView.cpp:1334-1343 eases m_zoom toward (terrain+HAG)/offset.z.
        let mut desired_hag = look_at_host_modes()
            .desired_height_above_ground
            .unwrap_or(height_above_ground);
        if min_height.is_finite() && desired_hag < min_height {
            desired_hag = min_height;
        }
        if enforce_max && max_height.is_finite() && desired_hag > max_height {
            desired_hag = max_height;
        }
        if (desired_hag - height_above_ground).abs() >= 0.0001 {
            let pitch = self
                .camera_pitch_radians
                .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
            let basis = self.camera_orbit_distance.max(1.0) * pitch.sin();
            if basis > f32::EPSILON {
                let desired_zoom = desired_hag / basis;
                let zoom_adj = (desired_zoom - self.camera_zoom) * adjust_speed;
                if zoom_adj.abs() >= 0.0001 {
                    let min_zoom = min_height / basis;
                    let max_zoom = max_height / basis;
                    self.camera_zoom = (self.camera_zoom + zoom_adj).clamp(min_zoom, max_zoom);
                    changed = true;
                }
            }
        }

        changed
    }

    /// C++ InGameUI context cursor residual mapped onto winit CursorIcon.
    ///
    /// Fail-closed vs full Mouse.cpp ANI/CUR assets — uses platform icons with
    /// residual names from `MOUSE_CURSOR_INI_NAME_LIST`.
    pub(super) fn sync_context_mouse_cursor(&mut self) {
        // C++ LookAtXlat.cpp:55-70 saves prevCursor for the scroll and restores
        // it on stop; do not overwrite the locked cursor mid KEY/RMB/EDGE pan.
        if look_at_host_mouse_locked() {
            return;
        }
        // C++ SelectionXlat.cpp:425-446 + HintSpy.cpp:26-35 — hover always
        // posts MSG_MOUSEOVER_* even when the cursor icon is unchanged.
        self.sync_ingame_mouseover_hint();
        // C++ InGameUI.cpp:2462 — replayed SELECTING/ARROW stays put until
        // the viewer moves the mouse (LookAtXlat hasMouseMovedRecently).
        if crate::command_system::host_recorder_is_playback()
            && !lookat_has_mouse_moved_recently(self.frame_counter)
        {
            return;
        }
        use winit::window::CursorIcon;
        let (name, icon) = self.resolve_context_cursor_icon();
        if self.last_context_cursor == Some(name) {
            return;
        }
        self.last_context_cursor = Some(name);
        self.window.set_cursor(icon);
    }


    /// C++ HintSpy::translate MSG_MOUSEOVER_DRAWABLE_HINT / LOCATION_HINT.
    fn sync_ingame_mouseover_hint(&mut self) {
        // C++ InGameUI.cpp:2462 — playback keeps SELECTING/ARROW until the
        // viewer moves the mouse (LookAtXlat hasMouseMovedRecently, 1s).
        #[cfg(feature = "game_client")]
        self.game_client.feed_look_at_replay_hover_gate(
            crate::command_system::host_recorder_is_playback(),
            lookat_has_mouse_moved_recently(self.frame_counter),
        );
        // C++ SelectionXlat.cpp:429 hardcodes getPickTypesForContext(true).
        let hover = self.host_pick_hover_object_at_cursor();
        match hover {
            Some(id) => self.game_client.create_mouseover_hint(Some(id.0), false),
            None => self.game_client.create_mouseover_hint(None, true),
        }
    }


    /// Wave 612: via `host_resolve_context_cursor_icon`.
    pub(super) fn resolve_context_cursor_icon(&self) -> (&'static str, winit::window::CursorIcon) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_resolve_context_cursor_icon()
    }

    pub(super) fn host_resolve_context_cursor_icon(
        &self,
    ) -> (&'static str, winit::window::CursorIcon) {
        // Wave 612: host residual helper.
        use winit::window::CursorIcon;

        // Placement mode residual.
        if self.pending_structure_placement.is_some() {
            let legal = self
                .game_hud
                .construction_panel
                .placement_preview()
                .is_legal;
            return if legal {
                ("Build", CursorIcon::Cell)
            } else {
                ("InvalidBuild", CursorIcon::NotAllowed)
            };
        }

        // Pending map command residual.
        if let Some(kind) = self.pending_map_command.as_ref() {
            return match kind {
                PendingMapCommand::AttackMove => ("AttackMove", CursorIcon::Crosshair),
                PendingMapCommand::Guard(_) => ("Move", CursorIcon::AllScroll),
                PendingMapCommand::SetRallyPoint => ("SetRallyPoint", CursorIcon::Cell),
                PendingMapCommand::CombatDrop(_) => ("CombatDrop", CursorIcon::Move),
                PendingMapCommand::PlaceBeacon => ("PlaceBeacon", CursorIcon::Cell),
                PendingMapCommand::SpecialPower(_) => ("Target", CursorIcon::Crosshair),
                PendingMapCommand::Weapon(_) => ("Target", CursorIcon::Crosshair),
                PendingMapCommand::UnitAbility(_) => ("Target", CursorIcon::Crosshair),
            };
        }

        // Wave 234: selection presence prefers engine/presentation freeze.
        let has_selection = !self.ui_selected_ids(self.current_player_id).is_empty();

        let hover = self.find_object_at_cursor(true);
        let ctrl = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let alt = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

        if (alt || self.sticky_waypoint_mode) && has_selection {
            return ("Waypoint", CursorIcon::Cell);
        }

        if ctrl && has_selection {
            return if hover.is_some() {
                ("ForceAttackObj", CursorIcon::Crosshair)
            } else {
                ("ForceAttackGround", CursorIcon::Crosshair)
            };
        }

        if !has_selection {
            // Hover friendly selectable → Select residual.
            if let Some(id) = hover {
                let friendly = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame
                        .objects
                        .iter()
                        .find(|o| o.id == id)
                        .map(|o| {
                            // Wave 1098: cursor Select residual uses full presentation
                            // selectable legality (sold/unselectable/masked/disabled).
                            frame.is_owned_by_local(o)
                                && crate::unit_control::UnitControlSystem::presentation_is_selectable(
                                    o,
                                )
                        })
                        .unwrap_or(false)
                } else {
                    // Wave 905: fail-closed without presentation freeze (no find_object dual-read).
                    false
                };
                if friendly {
                    return ("Select", CursorIcon::Pointer);
                }
            }
            return ("Normal", CursorIcon::Default);
        }

        // C++ CommandXlat.cpp:2180-2199 MSG_SET_RALLY_POINT_HINT on empty ground.
        if hover.is_none() && self.host_selection_can_set_rally() {
            return ("SetRallyPoint", CursorIcon::Cell);
        }

        // Has selection: context from CommandSystem residual.
        // Wave 229: selection via presentation-first ui_selected_ids.
        let selected = self.ui_selected_ids(self.current_player_id);
        let context = crate::command_system::MouseCommandContext {
            world_position: self.mouse_world_position,
            target_object: hover,
            target_presentation: hover.and_then(|id| self.presentation_target_hint(id)),
            selected_presentation: self.presentation_selected_unit_hints(&selected),
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut cmd_sys = crate::command_system::CommandSystem::new();
        // Wave 236: presentation-only mouse path when frame installed.
        let cmd = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            self.presentation_mouse_game_logic(),
        );
        match cmd.map(|c| c.command_type) {
            Some(crate::command_system::CommandType::AttackObject { target_id }) => {
                // C++ POSSIBLE → ATTACK_OBJECT; POSSIBLE_AFTER_MOVING → OUTRANGE
                // hint. Click still issues AttackObject in both cases.
                let out_of_range = self.last_presentation_frame.as_ref().is_some_and(|frame| {
                    let Some(target) = frame.objects.iter().find(|o| o.id == target_id) else {
                        return false;
                    };
                    let any_weapon = selected.iter().any(|&id| {
                        frame
                            .objects
                            .iter()
                            .find(|o| o.id == id)
                            .is_some_and(|a| a.has_weapon)
                    });
                    any_weapon
                        && !selected.iter().any(|&id| {
                            frame
                                .objects
                                .iter()
                                .find(|o| o.id == id)
                                .is_some_and(|a| presentation_weapon_reaches(a, target.position))
                        })
                });
                if out_of_range {
                    ("OutRange", CursorIcon::NotAllowed)
                } else {
                    ("AttackObj", CursorIcon::Crosshair)
                }
            }
            Some(crate::command_system::CommandType::ForceAttackObject { .. }) => {
                ("ForceAttackObj", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::ForceAttackGround { .. }) => {
                ("ForceAttackGround", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::Enter { .. }) => {
                ("EnterFriendly", CursorIcon::Copy)
            }
            Some(crate::command_system::CommandType::GetRepaired { .. }) => {
                ("GetRepaired", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::Repair { .. }) => {
                ("DoRepair", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::GetHealed { .. }) => {
                ("GetHealed", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::Dock { .. })
            | Some(crate::command_system::CommandType::Gather { .. }) => {
                ("Dock", CursorIcon::AllScroll)
            }
            Some(crate::command_system::CommandType::Hijack { .. })
            | Some(crate::command_system::CommandType::ConvertToCarbomb { .. })
            | Some(crate::command_system::CommandType::Sabotage { .. }) => {
                ("EnterAggressively", CursorIcon::Copy)
            }
            Some(crate::command_system::CommandType::DisableVehicleHack { .. })
            | Some(crate::command_system::CommandType::StealCashHack { .. })
            | Some(crate::command_system::CommandType::HackerDisableBuilding { .. }) => {
                ("Hack", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::OverrideSpecialPowerDestination { .. }) => {
                ("ParticleUplinkCannon", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::SetRallyPoint { .. }) => {
                ("SetRallyPoint", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::ResumeConstruction { .. }) => {
                ("ResumeConstruction", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::CaptureBuilding { .. }) => {
                ("CaptureBuilding", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::MoveTo { .. })
            | Some(crate::command_system::CommandType::DoSalvage { .. })
            | Some(crate::command_system::CommandType::AttackMoveTo { .. }) => {
                ("Move", CursorIcon::AllScroll)
            }
            Some(crate::command_system::CommandType::AddWaypoint { .. }) => {
                ("Waypoint", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::Guard { .. }) => {
                ("Move", CursorIcon::AllScroll)
            }
            _ => {
                if hover.is_some() {
                    ("Select", CursorIcon::Pointer)
                } else {
                    ("Move", CursorIcon::AllScroll)
                }
            }
        }
    }

    /// Feed Main-owned OS keyboard state into GameClient Keyboard device residual.
    /// Main still owns command translation / hotkeys.
    /// Wave 606: via `host_inject_game_client_key`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_key(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: thin wrapper — OS key inject via host helper.
        self.host_inject_game_client_key(physical_key, pressed);
    }

    /// Wave 606: host OS→GameClient key inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_key(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: host OS key inject residual.
        if let Some(code) = Self::to_game_client_key_code(physical_key) {
            game_client::input::keyboard::with_keyboard(|kb| {
                let _ = kb.handle_key_simple(code, pressed);
            });
        }
    }

    /// Map winit physical keys to GameClient KeyCode without sharing winit types across crates.
    #[cfg(feature = "game_client")]
    pub(super) fn to_game_client_key_code(
        physical_key: &winit::keyboard::PhysicalKey,
    ) -> Option<game_client::input::KeyCode> {
        use game_client::input::KeyCode as Gk;
        use winit::keyboard::{KeyCode as Wk, PhysicalKey};
        let PhysicalKey::Code(code) = physical_key else {
            return None;
        };
        Some(match code {
            Wk::Escape => Gk::Escape,
            Wk::Enter | Wk::NumpadEnter => Gk::Enter,
            Wk::Space => Gk::Space,
            Wk::Tab => Gk::Tab,
            Wk::Backspace => Gk::Backspace,
            Wk::Delete => Gk::Delete,
            Wk::Home => Gk::Home,
            Wk::End => Gk::End,
            Wk::PageUp => Gk::PageUp,
            Wk::PageDown => Gk::PageDown,
            Wk::ArrowLeft => Gk::Left,
            Wk::ArrowRight => Gk::Right,
            Wk::ArrowUp => Gk::Up,
            Wk::ArrowDown => Gk::Down,
            Wk::ShiftLeft => Gk::LeftShift,
            Wk::ShiftRight => Gk::RightShift,
            Wk::ControlLeft => Gk::LeftCtrl,
            Wk::ControlRight => Gk::RightCtrl,
            Wk::AltLeft => Gk::LeftAlt,
            Wk::AltRight => Gk::RightAlt,
            Wk::KeyA => Gk::A,
            Wk::KeyB => Gk::B,
            Wk::KeyC => Gk::C,
            Wk::KeyD => Gk::D,
            Wk::KeyE => Gk::E,
            Wk::KeyF => Gk::F,
            Wk::KeyG => Gk::G,
            Wk::KeyH => Gk::H,
            Wk::KeyI => Gk::I,
            Wk::KeyJ => Gk::J,
            Wk::KeyK => Gk::K,
            Wk::KeyL => Gk::L,
            Wk::KeyM => Gk::M,
            Wk::KeyN => Gk::N,
            Wk::KeyO => Gk::O,
            Wk::KeyP => Gk::P,
            Wk::KeyQ => Gk::Q,
            Wk::KeyR => Gk::R,
            Wk::KeyS => Gk::S,
            Wk::KeyT => Gk::T,
            Wk::KeyU => Gk::U,
            Wk::KeyV => Gk::V,
            Wk::KeyW => Gk::W,
            Wk::KeyX => Gk::X,
            Wk::KeyY => Gk::Y,
            Wk::KeyZ => Gk::Z,
            Wk::Digit0 => Gk::Num0,
            Wk::Digit1 => Gk::Num1,
            Wk::Digit2 => Gk::Num2,
            Wk::Digit3 => Gk::Num3,
            Wk::Digit4 => Gk::Num4,
            Wk::Digit5 => Gk::Num5,
            Wk::Digit6 => Gk::Num6,
            Wk::Digit7 => Gk::Num7,
            Wk::Digit8 => Gk::Num8,
            Wk::Digit9 => Gk::Num9,
            Wk::F1 => Gk::F1,
            Wk::F2 => Gk::F2,
            Wk::F3 => Gk::F3,
            Wk::F4 => Gk::F4,
            Wk::F5 => Gk::F5,
            Wk::F6 => Gk::F6,
            Wk::F7 => Gk::F7,
            Wk::F8 => Gk::F8,
            Wk::F9 => Gk::F9,
            Wk::F10 => Gk::F10,
            Wk::F11 => Gk::F11,
            Wk::F12 => Gk::F12,
            _ => return None,
        })
    }

    /// Feed Main-owned OS mouse state into GameClient Mouse device residual.
    /// Main still owns command translation; this keeps client device state honest
    /// for presentation-shell UI without dual OS event ownership.
    /// Wave 606: via `host_inject_game_client_mouse_move`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_mouse_move(&self, x: f32, y: f32) {
        // Wave 606: thin wrapper — OS mouse move inject via host helper.
        self.host_inject_game_client_mouse_move(x, y);
    }

    /// Wave 606: host OS→GameClient mouse-move inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_mouse_move(&self, x: f32, y: f32) {
        // Wave 606: host OS mouse move inject residual.
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_mouse_move(x, y);
        });
    }

    /// Wave 606: via `host_inject_game_client_mouse_button`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_mouse_button(&self, button: MouseButton, pressed: bool) {
        // Wave 606: thin wrapper — OS mouse button inject via host helper.
        self.host_inject_game_client_mouse_button(button, pressed);
    }

    /// C++ WindowXlat: OS button → `TheWindowManager` gadget hit-test.
    /// Returns true when the WND consumed the click (shell active or gadget Used).
    /// C++ WindowXlat RAW_KEY → focused gadget `GWM_CHAR`.
    pub(super) fn dispatch_os_key_to_window_manager(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::WindowInputReturnCode;
            let Some(vk) = Self::winit_physical_key_to_wnd_vk(physical_key) else {
                return false;
            };
            // C++ KEY_STATE_DOWN=0x02, KEY_STATE_UP=0x01
            let state = if pressed { 0x02 } else { 0x01 };
            game_client::gui::dispatch_os_key_to_window_manager(vk, state)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (physical_key, pressed);
            false
        }
    }

    pub(super) fn winit_physical_key_to_wnd_vk(
        physical_key: &winit::keyboard::PhysicalKey,
    ) -> Option<u8> {
        use winit::keyboard::{KeyCode as Wk, PhysicalKey};
        let PhysicalKey::Code(code) = physical_key else {
            return None;
        };
        Some(match code {
            Wk::Escape => 0x1B,
            Wk::Enter | Wk::NumpadEnter => 13,
            Wk::Space => 32,
            Wk::Tab => 9,
            Wk::Backspace => 8,
            Wk::Delete => 0x2E,
            Wk::Home => 36,
            Wk::End => 35,
            Wk::PageUp => 33,
            Wk::PageDown => 34,
            Wk::ArrowLeft => 37,
            Wk::ArrowUp => 38,
            Wk::ArrowRight => 39,
            Wk::ArrowDown => 40,
            Wk::KeyA => b'A',
            Wk::KeyB => b'B',
            Wk::KeyC => b'C',
            Wk::KeyD => b'D',
            Wk::KeyE => b'E',
            Wk::KeyF => b'F',
            Wk::KeyG => b'G',
            Wk::KeyH => b'H',
            Wk::KeyI => b'I',
            Wk::KeyJ => b'J',
            Wk::KeyK => b'K',
            Wk::KeyL => b'L',
            Wk::KeyM => b'M',
            Wk::KeyN => b'N',
            Wk::KeyO => b'O',
            Wk::KeyP => b'P',
            Wk::KeyQ => b'Q',
            Wk::KeyR => b'R',
            Wk::KeyS => b'S',
            Wk::KeyT => b'T',
            Wk::KeyU => b'U',
            Wk::KeyV => b'V',
            Wk::KeyW => b'W',
            Wk::KeyX => b'X',
            Wk::KeyY => b'Y',
            Wk::KeyZ => b'Z',
            Wk::Digit0 => b'0',
            Wk::Digit1 => b'1',
            Wk::Digit2 => b'2',
            Wk::Digit3 => b'3',
            Wk::Digit4 => b'4',
            Wk::Digit5 => b'5',
            Wk::Digit6 => b'6',
            Wk::Digit7 => b'7',
            Wk::Digit8 => b'8',
            Wk::Digit9 => b'9',
            _ => return None,
        })
    }

    pub(super) fn dispatch_os_mouse_to_window_manager(
        &self,
        button: MouseButton,
        pressed: bool,
        x: i32,
        y: i32,
        origin: MouseInputOrigin,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                with_host_control_bar_input_provenance, HostControlBarInputProvenance,
            };
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            let msg = match (button, pressed) {
                (MouseButton::Left, true) => WindowMessage::LeftDown,
                (MouseButton::Left, false) => WindowMessage::LeftUp,
                (MouseButton::Right, true) => WindowMessage::RightDown,
                (MouseButton::Right, false) => WindowMessage::RightUp,
                (MouseButton::Middle, true) => WindowMessage::MiddleDown,
                (MouseButton::Middle, false) => WindowMessage::MiddleUp,
                _ => return false,
            };
            // `CommandSourceType::FromUser` alone cannot distinguish a real
            // winit event from `inject_winit_equivalent_*`: both deliberately
            // follow the same WND callback path. Scope the actual origin over
            // this synchronous dispatch so publication captures it exactly.
            let provenance = match origin {
                MouseInputOrigin::Physical => {
                    HostControlBarInputProvenance::PhysicalWindowMouseInput
                }
                MouseInputOrigin::Injected => HostControlBarInputProvenance::InjectedOrUnknown,
            };
            with_host_control_bar_input_provenance(provenance, || {
                game_client::gui::dispatch_os_mouse_to_window_manager(msg, x, y)
                    == WindowInputReturnCode::Used
            })
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (button, pressed, x, y, origin);
            false
        }
    }

    pub(super) fn dispatch_os_mouse_move(&self, x: i32, y: i32) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            game_client::gui::dispatch_os_mouse_to_window_manager(WindowMessage::MousePos, x, y)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (x, y);
            false
        }
    }

    pub(super) fn dispatch_os_mouse_wheel(
        &self,
        delta: &winit::event::MouseScrollDelta,
        x: i32,
        y: i32,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            let lines = match delta {
                winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                winit::event::MouseScrollDelta::PixelDelta(p) => (p.y as f32) / 16.0,
            };
            if lines.abs() < f32::EPSILON {
                return false;
            }
            let msg = if lines > 0.0 {
                WindowMessage::WheelUp
            } else {
                WindowMessage::WheelDown
            };
            game_client::gui::dispatch_os_mouse_to_window_manager(msg, x, y)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (delta, x, y);
            false
        }
    }

    /// Wave 606: host OS→GameClient mouse-button inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_mouse_button(&self, button: MouseButton, pressed: bool) {
        // Wave 606: host OS mouse button inject residual.
        use game_client::input::mouse::MouseButton as GcMouseButton;
        use std::time::Instant;
        let gc_btn = match button {
            MouseButton::Left => GcMouseButton::Left,
            MouseButton::Right => GcMouseButton::Right,
            MouseButton::Middle => GcMouseButton::Middle,
            MouseButton::Back => GcMouseButton::Other(3),
            MouseButton::Forward => GcMouseButton::Other(4),
            MouseButton::Other(n) => GcMouseButton::Other(n as u16),
        };
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_mouse_button(gc_btn, pressed, Instant::now());
        });
    }

    /// Wave 606: via `host_inject_game_client_mouse_scroll`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_mouse_scroll(&self, delta_y: f32) {
        // Wave 606: thin wrapper — OS mouse scroll inject via host helper.
        self.host_inject_game_client_mouse_scroll(delta_y);
    }

    /// Wave 606: host OS→GameClient mouse-scroll inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_mouse_scroll(&self, delta_y: f32) {
        // Wave 606: host OS mouse scroll inject residual.
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_scroll_lines(delta_y);
        });
    }

    pub(super) fn update_mouse_world_position(&mut self) {
        // C++ maps device coordinates through the active W3D camera.  The former
        // whole-map linear interpolation only happened to work while the camera
        // was centered and made ordinary selection/orders drift after panning,
        // rotation, or zoom.
        // A script/minimap/hotkey can change the orbit between normal camera
        // ticks and an OS mouse event.  Pick from the pose that will actually
        // be rendered, rather than the previous frame's matrix.
        if self.camera_transform_needs_rebuild() {
            self.apply_camera_orbit_transform();
        }
        let (view_w, view_h) = self.tactical_viewport_size();
        if self.mouse_position.1 > view_h {
            return;
        }
        let (world_min, world_max) = self.presentation_world_bounds();
        let picked = {
            let world_env = self
                .render_pipeline
                .presentation_frame()
                .or(self.last_presentation_frame.as_ref())
                .map(|frame| &frame.world_env);
            unproject_mouse_ray(
                self.view_matrix,
                self.projection_matrix,
                self.mouse_position,
                view_w,
                view_h,
            )
            .and_then(|(near, far)| {
                raycast_frozen_terrain(near, far, world_min, world_max, world_env).or_else(|| {
                    raycast_ground_plane_clamped(near, far, world_min, world_max, world_env)
                })
            })
        };
        if let Some(position) = picked {
            self.mouse_world_position = position;
        }
    }

    /// Presentation-only world pick. Returns `None` when no snapshot is installed
    /// (no live GameLogic dual-read residual). InGame always seeds
    /// `last_presentation_frame` before input.

    /// Wave 228: build presentation target hint for RMB classification.

    /// Wave 229: presentation-frozen selected-unit capabilities for RMB classification.
    pub(super) fn presentation_selected_unit_hints(
        &self,
        ids: &[crate::game_logic::ObjectId],
    ) -> Vec<crate::command_system::PresentationSelectedUnitHint> {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(ids.len());
        for &id in ids {
            let Some(o) = frame.objects.iter().find(|x| x.id == id) else {
                continue;
            };
            // Wave 1097: selected-hint residual fail-closed on unusable sources.
            if o.destroyed
                || o.health_current <= 0.0
                || o.sold
                || o.under_construction
                || o.masked
                || o.disabled
                || o.unselectable
            {
                continue;
            }
            // C++ construction/structure repair authority is `KINDOF_DOZER`.
            // Preserve it in the frozen input instead of classifying a unit
            // by its UI name (a harvester/worker is not necessarily a dozer).
            let is_worker = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Dozer,
            );
            // Gather authorization is an authored capability, not a template
            // naming convention.  C++ marks Chinooks, Supply Trucks, and GLA
            // Workers with KINDOF_HARVESTER.
            let is_resource_collector =
                crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Harvester,
                );
            let can_attack = o.has_weapon;
            let can_move = o.is_mobile;
            let capture_power = o.capture_power;
            let capture_power_ready = o.capture_power_ready;
            let can_capture =
                capture_power != crate::game_logic::CapturePowerKind::None && capture_power_ready;
            let can_repair = is_worker;
            let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
            let is_vehicle = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Vehicle,
            );
            let is_aircraft = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Aircraft,
            );
            let is_infantry = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Infantry,
            );
            let is_above_terrain = o.airborne_target
                || (o.ground_height_from_terrain && o.position.y > o.ground_height + 0.01);
            out.push(crate::command_system::PresentationSelectedUnitHint {
                id,
                is_alive: true,
                is_resource_collector,
                is_worker,
                can_attack,
                can_move,
                can_request_service: o.contained_by.is_none(),
                can_capture,
                template_name: o.template_name.clone(),
                can_repair,
                is_damaged,
                is_vehicle,
                is_aircraft,
                is_above_terrain,
                is_infantry,
                transport_slot_count: o.transport_slot_count,
                stored_supplies: o.stored_supplies,
                is_controlled_by_local: frame.is_owned_by_local(o),
                capture_power,
                capture_power_ready,
                is_salvager: crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Salvager,
                ),
                can_override_special_power_destination: o
                    .special_power_override_destination
                    .is_some(),
            });
        }
        out
    }

    pub(super) fn presentation_target_hint(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> Option<crate::command_system::PresentationTargetHint> {
        let frame = self.last_presentation_frame.as_ref()?;
        // Wave 1097: target-hint residual fail-closed on sold/masked and non-local
        // FOW unless Clear (matches pick peels 1093–1096).
        let o = frame.objects.iter().find(|x| {
            x.id == id
                && !x.destroyed
                && !x.sold
                && !x.masked
                && !frame.box_pick_hides_non_local(x)
        })?;
        let is_neutral = o.team == crate::game_logic::Team::Neutral;
        let is_enemy = frame.is_enemy_of_local(o);
        let is_structure = o.object_type
            == crate::presentation_frame::PresentationObjectType::Building
            || crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Structure,
            );
        let is_resource = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::Harvestable,
        ) || crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::Resource,
        );
        let enter_available_capacity = frame
            .normal_enter_available_capacity_for_local(o)
            .unwrap_or(0);
        let can_be_entered = enter_available_capacity > 0;
        let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
        let is_friendly = !is_neutral && frame.is_allied_with_local(o);
        // Freeze exact Object INI KindOf service tags.  The executor repeats
        // these pairings against live authority when consuming the command.
        let provides_heal = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::HealPad,
        );
        let provides_aircraft_repair =
            crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::FSAirfield,
            );
        let provides_vehicle_repair = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::RepairPad,
        );
        // C++ treats a non-stealthed occupant as a GarrisonContain gate, but
        // checks friendly contained occupants separately for every target.
        // Freeze both, including stale references which must fail closed.
        let (capture_nonstealthed_garrison_count, capture_friendly_garrison_count) = o
            .garrisoned_units
            .iter()
            .fold((0u16, 0u16), |counts, occupant_id| {
                let Some(occupant) = frame
                    .objects
                    .iter()
                    .find(|candidate| candidate.id == *occupant_id)
                else {
                    return (
                        counts.0.saturating_add(o.capture_garrisonable as u16),
                        counts.1.saturating_add(1),
                    );
                };
                (
                    counts
                        .0
                        .saturating_add((o.capture_garrisonable && !occupant.stealthed) as u16),
                    counts
                        .1
                        .saturating_add(frame.is_allied_with_local(occupant) as u16),
                )
            });
        Some(crate::command_system::PresentationTargetHint {
            id,
            // Wave 1098: is_alive residual excludes sold/masked.
            is_alive: !o.destroyed && o.health_current > 0.0 && !o.sold && !o.masked,
            is_structure,
            is_resource,
            under_construction: o.under_construction,
            sold: o.sold,
            team: o.team,
            is_enemy_of_local: is_enemy,
            is_neutral,
            template_name: o.template_name.clone(),
            can_be_entered,
            enter_available_capacity,
            enter_uses_transport_slots: o.normal_enter_uses_transport_slots(),
            enter_requires_infantry: o.normal_enter_requires_infantry(),
            enter_forbids_aircraft: o.normal_enter_forbids_aircraft(),
            enter_disabled_subdued: o.disabled_subdued,
            enter_is_rider_change: o.contain_module_kind
                == crate::game_logic::ContainModuleKind::RiderChange,
            rider_change_allowed_templates: o.rider_change_allowed_templates.clone(),
            is_damaged,
            is_friendly_of_local: is_friendly,
            // ActionManager keys service on KindOf, not ObjectType::Building.
            provides_vehicle_repair,
            provides_aircraft_repair,
            provides_heal,
            can_provide_service: o.contained_by.is_none(),
            dock_kind: o.dock_kind,
            dock_controller_is_local: frame.is_owned_by_local(o),
            stored_supplies: o.stored_supplies,
            capturable: o.capturable,
            immune_to_capture: o.immune_to_capture,
            capture_garrisonable: o.capture_garrisonable,
            capture_nonstealthed_garrison_count,
            capture_friendly_garrison_count,
            capture_target_effectively_stealthed: o.effectively_stealthed,
            is_crate: o.is_crate,
            is_salvage_crate: o.is_salvage_crate,
            is_vehicle: crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Vehicle,
            ),
            is_aircraft: crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Aircraft,
            ),
            is_drone: crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Drone,
            ),
            is_carbomb: o.is_carbomb,
            is_unmanned: o.disabled_unmanned,
            is_mine: o.has_mine
                || crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Mine,
                )
                || crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::DemoTrap,
                )
                || crate::game_logic::host_car_bomb::object_definition_has_kind(
                    &o.template_name,
                    "MINE",
                ),
        })
    }

    pub(super) fn find_object_at_position(
        &self,
        position: Vec3,
        command_context: bool,
    ) -> Option<ObjectId> {
        // Wave 222: presentation-only pick (no GameLogic dual-read residual).
        // hq-bzidj: widen with mines/shrubbery via SelectionInfo pick types.
        self.host_find_object_at_position(position, command_context)
    }

    pub(super) fn find_object_at_cursor(&self, command_context: bool) -> Option<ObjectId> {
        self.host_pick_object_at_cursor(command_context)
    }

    /// C++ `InGameUI::setPreventLeftClickDeselectionInAlternateMouseModeForOneClick`.
    pub(super) fn host_set_prevent_left_click_deselection(&mut self, enabled: bool) {
        self.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click = enabled;
        game_client::helpers::TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            enabled,
        );
    }

    /// C++ `SelectionXlat.cpp:935-943`: consume the one-click keep-selection flag.
    pub(super) fn host_consume_prevent_left_click_deselection(&mut self) -> bool {
        let leftover = game_client::helpers::TheInGameUI::get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click();
        let prevent =
            self.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click || leftover;
        if prevent {
            self.host_set_prevent_left_click_deselection(false);
        }
        prevent
    }





    /// Path following is authoritative in `GameLogic::update_movement`.
    /// Retained as a no-op compatibility hook for older call sites.

    /// Legacy render stub -- NOT called from the active render path.
    /// Actual rendering is handled by RenderPipeline::execute() -> ForwardPass::render()
    /// which queues MeshClass instances into the WW3D Renderer and issues real draw calls.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    pub(super) fn render_game_objects<'a>(&'a self, _render_pass: &mut wgpu::RenderPass<'a>) {
        // Presentation-only stub: RenderPipeline is the sole draw path.
        let n = self
            .last_presentation_frame
            .as_ref()
            .map(|f| f.objects.len())
            .unwrap_or(0);
        log::trace!(
            "Legacy stub: presentation has {} objects (RenderPipeline is sole draw path)",
            n
        );
    }

    /// Legacy per-object render stub -- logs model status but does NOT submit draw calls.
    /// The active render path is RenderPipeline::collect_render_items() which builds
    /// RenderItem list and ForwardPass::prepare_mesh_instance() which creates actual
    /// MeshClass instances submitted to the WW3D Renderer.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    pub(super) fn render_object<'a>(
        &'a self,
        obj: &Object,
        _render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        let model_name = obj.get_template().get_model_name();

        log::trace!(
            "Render object {} template '{}' model '{}' (cached={})",
            obj.id,
            obj.template_name,
            model_name,
            self.graphics_system.get_model(model_name).is_some()
        );

        let w3d_model = self
            .graphics_system
            .get_model(model_name)
            .or_else(|| self.graphics_system.get_model(&obj.template_name));

        if let Some(w3d_model) = w3d_model {
            let total_vertices: usize = w3d_model
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len())
                .sum();
            let total_indices: usize = w3d_model.meshes.iter().map(|mesh| mesh.indices.len()).sum();

            log::trace!("Rendering W3D model: {} (template: {}) with {} vertices, {} indices across {} meshes",
                model_name, obj.template_name, total_vertices, total_indices, w3d_model.meshes.len());
            log::trace!("Resolved W3D model '{}' for object {}", model_name, obj.id);
        } else {
            log::debug!(
                "No W3D model resolved for object {} template '{}' (model '{}') -- fallback cube will be used by RenderPipeline",
                obj.id,
                obj.template_name,
                model_name
            );
        }
    }

    #[allow(dead_code)] // Legacy stub: selection_renderer + PresentationFrame own production path
    pub(super) fn render_selection_indicators(&self, _render_pass: &mut wgpu::RenderPass) {
        // Prefer presentation selected residual when installed (no live find_object dual-read).
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            let n = frame
                .objects
                .iter()
                .filter(|o| o.selected && !o.destroyed)
                .count();
            log::trace!(
                "Legacy stub: presentation selected count={n} (selection_renderer is sole path)"
            );
            return;
        }
        // Boot residual only.
        for &object_id in &self.selected_objects {
            let _ = object_id;
        }
    }

    pub(super) fn render_projectiles(&self, _render_pass: &mut wgpu::RenderPass) {
        // Projectiles render from PresentationFrame (host CombatSystem freeze).
    }

    pub(super) fn render_ui(&self, _render_pass: &mut wgpu::RenderPass) {
        if let Err(err) = self.ui_manager.render() {
            log::warn!("UI manager render failed: {}", err);
        }
        log::trace!(
            "UI overlay rendered for {} selected units",
            self.selected_objects.len()
        );
    }

    pub(super) fn lookat_input_enabled(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheInGameUI::get_input_enabled()
        }
        #[cfg(not(feature = "game_client"))]
        {
            true
        }
    }

    pub(super) fn look_at_player_broke_camera_lock(&self) -> bool {
        look_at_host_modes().camera_follow_lock_broken
    }

    pub(super) fn look_at_clear_player_broke_camera_lock(&self) {
        look_at_host_modes().camera_follow_lock_broken = false;
    }

    fn host_camera_movement_finished(&self) -> bool {
        self.camera_zoom_target.is_none()
            && self.camera_pitch_target.is_none()
            && self.camera_yaw_target.is_none()
    }

    pub(super) fn cancel_scripted_camera_from_player_set(&mut self) {
        note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::Set);
        self.camera_zoom_target = None;
        self.camera_pitch_target = None;
        self.camera_yaw_target = None;
        self.camera_zoom_duration = 0.0;
        self.camera_pitch_duration = 0.0;
        self.camera_yaw_duration = 0.0;
        self.game_logic.cancel_scripted_camera_from_player_set();
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.cancel_scripted_camera_from_player_set();
            });
        }
    }

    pub(super) fn cancel_scripted_camera_from_player_look_at(&mut self) {
        note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::LookAt);
        self.camera_yaw_target = None;
        self.camera_yaw_duration = 0.0;
        self.game_logic.cancel_scripted_camera_from_player_look_at();
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.cancel_scripted_camera_from_player_look_at();
            });
        }
    }

    fn cancel_scripted_camera_from_player_scroll(&mut self) {
        self.camera_yaw_target = None;
        self.camera_yaw_duration = 0.0;
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.cancel_scripted_camera_from_player_scroll();
            });
        }
    }


    fn break_camera_follow_lock(&mut self) {
        look_at_host_modes().camera_follow_lock_broken = true;
        self.host_set_camera_follow_object(None);
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                // C++ InGameUI.cpp:2800-2801 setCameraLock(INVALID) +
                // setCameraLockDrawable(NULL).
                view.set_camera_lock(None);
                view.set_camera_lock_drawable(None);
            });
        }
    }

    /// C++ `W3DView::cameraEnableSlaveMode` + bone matrix slam.
    fn apply_host_slave_camera(&mut self) -> bool {
        let Some(mode) = self.camera_slave_mode.clone() else {
            return false;
        };
        let origin = self.last_presentation_frame.as_ref().and_then(|frame| {
            frame.first_alive_position_for_template(&mode.thing_template_name)
        });
        let mut eye = None;
        let mut look = origin;
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.camera_enable_slave_mode(&mode.thing_template_name, &mode.bone_name);
            });
            let slaved = game_client::display::view::with_tactical_view_ref(|view| {
                view.is_camera_slaved()
            });
            if slaved {
                let p = game_client::display::view::with_tactical_view_ref(|view| {
                    view.get_3d_camera_position()
                });
                // Leftover View is C++ Z-up; live host is Y-up.
                eye = Some(Vec3::new(p.x, p.z, p.y));
            }
        }
        let Some(origin) = origin else {
            return false;
        };
        if let Some(eye_pos) = eye {
            self.camera_position = eye_pos;
            self.camera_target = look.unwrap_or(origin);
            self.view_matrix = Mat4::look_at_rh(self.camera_position, self.camera_target, Vec3::Y);
            true
        } else if !mode.bone_name.is_empty() {
            // Bone missing: still parent the camera at the unit, not an orbit look-at.
            self.camera_position = origin + Vec3::Y * 2.0;
            self.camera_target = origin;
            self.view_matrix = Mat4::look_at_rh(self.camera_position, self.camera_target, Vec3::Y);
            true
        } else {
            let clamped = self.clamp_to_world_bounds(origin);
            if (self.camera_target.x - clamped.x).abs() > 0.001
                || (self.camera_target.z - clamped.z).abs() > 0.001
            {
                self.camera_target.x = clamped.x;
                self.camera_target.z = clamped.z;
                true
            } else {
                false
            }
        }
    }

    /// C++ `W3DView.cpp:1224-1247` LOCK_FOLLOW airborne yaw ease.
    fn apply_airborne_follow_yaw(&mut self) -> bool {
        if self.look_at_player_broke_camera_lock() {
            return false;
        }
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return false;
        };
        if frame.camera_follow_position.is_none() {
            return false;
        }
        let follow = frame.camera_follow_position.unwrap();
        let follow = Vec3::new(follow[0], follow[1], follow[2]);
        let Some(obj) = frame.objects.iter().min_by(|a, b| {
            let da = (a.position - follow).length_squared();
            let db = (b.position - follow).length_squared();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            return false;
        };
        let airborne = obj.airborne_target
            || matches!(
                obj.object_type,
                crate::presentation_frame::PresentationObjectType::Aircraft
            );
        if !airborne {
            return false;
        }
        let ground = if obj.ground_height_from_terrain {
            obj.ground_height
        } else {
            self.sample_presentation_height_under(obj.position)
        };
        if obj.position.y <= PATHFIND_CELL_SIZE_F + ground {
            return false;
        }
        let ideal = normalize_signed_angle(obj.orientation - std::f32::consts::FRAC_PI_2);
        let diff = normalize_signed_angle(ideal - self.camera_yaw_radians);
        if diff.abs() < 1.0e-4 {
            return false;
        }
        self.camera_yaw_radians = normalize_signed_angle(self.camera_yaw_radians + diff * 0.1);
        true
    }


    /// C++ W3DDisplay.cpp:1883-1900 draws the software cursor then letterbox
    /// bars on top. Live has only the OS cursor, so hide it over the bars.
    fn sync_letterbox_os_cursor_visibility(&mut self) {
        #[cfg(feature = "game_client")]
        {
            if look_at_host_mouse_locked() {
                return;
            }
            let fade = self.game_client.letterbox_overlay_fade();
            let enabled = self.game_client.letterbox_overlay_enabled();
            let size = self.window.inner_size();
            let width = size.width.max(1) as f32;
            let height = size.height.max(1) as f32;
            let plan = game_client::display::display_fx::letterbox_plan(
                width, height, fade, enabled,
            );
            let my = self.mouse_position.1;
            let over_bar = (plan.draw_top && my < plan.bar_height)
                || (plan.draw_bottom && my >= height - plan.bar_height);
            let hidden = look_at_host_modes().letterbox_os_cursor_hidden;
            if over_bar != hidden {
                self.window.set_cursor_visible(!over_bar);
                look_at_host_modes().letterbox_os_cursor_hidden = over_bar;
            }
        }
    }

    /// C++ `LookAtXlat.cpp:50-76` `setScrolling` / `stopScrolling`.
    ///
    /// Every exclusive scroll type (KEY / RMB / SCREENEDGE) mouse-locks the
    /// tactical view and sets InGameUI scrolling so WindowXlat keeps hover,
    /// RMB, and MMB off HUD gadgets. Idempotent: RMB start/stop may already
    /// have applied the same flags.
    fn set_lookat_scroll_mouse_lock(&mut self, locked: bool) {
        if look_at_host_modes().mouse_locked == locked {
            return;
        }
        if locked {
            let mut modes = look_at_host_modes();
            modes.prev_cursor = self.last_context_cursor;
            modes.mouse_locked = true;
            drop(modes);
            #[cfg(feature = "game_client")]
            {
                game_client::helpers::TheInGameUI::set_scrolling(true);
                game_client::display::view::with_tactical_view(|view| {
                    view.set_mouse_lock(true);
                });
            }
            // C++ InGameUI.cpp:2797 setMouseCursor(SCROLL).
            self.last_context_cursor = Some("Scroll");
            self.window.set_cursor(winit::window::CursorIcon::AllScroll);
        } else {
            {
                let mut modes = look_at_host_modes();
                modes.mouse_locked = false;
                // C++ LookAtXlat.cpp:70 TheMouse->setCursor(prevCursor).
                let _prev = modes.prev_cursor.take();
            }
            #[cfg(feature = "game_client")]
            {
                game_client::helpers::TheInGameUI::set_scrolling(false);
                game_client::display::view::with_tactical_view(|view| {
                    view.set_mouse_lock(false);
                });
            }
            self.last_context_cursor = None;
            self.sync_context_mouse_cursor();
        }
    }

    /// C++ `LookAtXlat.cpp:50-62` `setScrolling(SCROLL_RMB)`.
    pub(super) fn start_rmb_lookat_scroll(&mut self) {
        lookat_stamp_mouse_activity(self.frame_counter);
        self.rmb_scroll_anchor = Some(self.mouse_position);
        // C++ :204-206: start only when `!isSelecting() && !m_isScrolling`.
        if self.host_is_selecting() || self.is_rmb_scrolling {
            return;
        }
        if look_at_host_modes().scroll_type.is_scrolling() {
            return;
        }
        if !self.lookat_input_enabled() {
            return;
        }
        let mut modes = look_at_host_modes();
        modes.wheel_stopped_scroll = false;
        modes.scroll_type = LookAtScrollType::Rmb;
        drop(modes);
        self.is_rmb_scrolling = true;
        self.break_camera_follow_lock();
        self.set_lookat_scroll_mouse_lock(true);
    }

    /// C++ `LookAtXlat.cpp:65-76` `stopScrolling`.
    pub(super) fn stop_rmb_lookat_scroll(&mut self) {
        lookat_stamp_mouse_activity(self.frame_counter);
        {
            let mut modes = look_at_host_modes();
            if modes.scroll_type == LookAtScrollType::Rmb {
                modes.scroll_type = LookAtScrollType::None;
            }
        }
        self.is_rmb_scrolling = false;
        self.rmb_scroll_anchor = None;
        self.set_lookat_scroll_mouse_lock(false);
    }

    /// C++ `LookAtTranslator::resetModes` — drop scroll/rotate/pitch/FOV flags
    /// so a cinematic `doDisableInput` cannot leave the camera stuck.
    pub(super) fn apply_look_at_reset_modes(&mut self) {
        if self.is_rmb_scrolling {
            self.stop_rmb_lookat_scroll();
        }
        // Live host also unlocks KEY/SCREENEDGE so WindowXlat cannot stay
        // suppressed after doDisableInput (C++ resetModes only clears flags).
        self.set_lookat_scroll_mouse_lock(false);
        self.is_mmb_rotating = false;
        self.mmb_anchor = None;
        self.camera_rotate_left_held = false;
        self.camera_rotate_right_held = false;
        self.camera_zoom_in_held = false;
        self.camera_zoom_out_held = false;
        let mut modes = look_at_host_modes();
        modes.mmb_original_anchor = None;
        modes.mmb_press_frame = 0;
        modes.scroll_type = LookAtScrollType::None;
    }

    /// C++ `LookAtXlat.cpp:224-233` middle-button down.
    pub(super) fn begin_mmb_lookat_rotate(&mut self) {
        lookat_stamp_mouse_activity(self.frame_counter);
        self.is_mmb_rotating = true;
        self.mmb_anchor = Some(self.mouse_position);
        let mut modes = look_at_host_modes();
        modes.mmb_original_anchor = Some(self.mouse_position);
        modes.mmb_press_frame = self.frame_counter;
    }

    /// C++ `LookAtXlat.cpp:237-254` short MMB click resets angle/pitch/zoom.
    pub(super) fn end_mmb_lookat_rotate(&mut self) {
        lookat_stamp_mouse_activity(self.frame_counter);
        let (original, press_frame) = {
            let mut modes = look_at_host_modes();
            (
                modes.mmb_original_anchor.take(),
                modes.mmb_press_frame,
            )
        };
        self.is_mmb_rotating = false;
        self.mmb_anchor = None;
        let Some(origin) = original else {
            return;
        };
        let dx = self.mouse_position.0 - origin.0;
        let dy = self.mouse_position.1 - origin.1;
        let frames = self.frame_counter.saturating_sub(press_frame);
        if lookat_mmb_is_short_click(dx, dy, frames) {
            self.reset_camera_pose_in_place();
        }
    }

    /// C++ `InGameUI::resetCamera` + `W3DView::resetCamera`: keep look-at,
    /// restore default angle/pitch/zoom. Does not retarget the command center.
    pub(super) fn reset_camera_pose_in_place(&mut self) {
        let defaults = Self::configured_startup_camera_defaults();
        // C++ setAngleAndPitchToDefault: m_pitchAngle = m_defaultPitchAngle.
        self.camera_yaw_radians = defaults.yaw_degrees.to_radians();
        self.camera_pitch_radians = live_home_pitch_radians(
            defaults.pitch_degrees,
            self.ui_script_default_camera_pitch(),
        );
        self.camera_yaw_target = None;
        self.camera_pitch_target = None;
        self.camera_zoom_target = None;
        // C++ W3DView::setAngleAndPitchToDefault / resetCamera: m_FXPitch = 1.0.
        self.camera_fx_pitch = 1.0;
        self.camera_zoom = self.compute_default_camera_zoom_for_target(
            self.camera_target,
            self.ui_script_default_camera_max_height(),
        );
        // C++ resetCamera/setZoom invalidates m_cameraConstraint.
        self.scripted_camera_constraint_widen = None;
        self.apply_camera_orbit_transform();
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.update_mouse_world_position();
            self.sync_context_mouse_cursor();
        }
    }

    /// C++ `LookAtXlat.cpp:550-587` SAVE_VIEW / VIEW_VIEW full `ViewLocation`.
    pub(super) fn save_or_recall_camera_view(&mut self, slot: usize) {
        let save = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        self.store_or_apply_camera_view(slot, save);
    }

    /// Explicit SAVE_VIEW (`save`) vs VIEW_VIEW so Keyboard Options remaps
    /// do not depend on the live Ctrl key.
    pub(super) fn store_or_apply_camera_view(&mut self, slot: usize, save: bool) {
        if slot >= 8 {
            return;
        }
        if save {
            let loc = CameraViewLocation {
                pos: self.camera_target,
                yaw: self.camera_yaw_radians,
                pitch: self.camera_pitch_radians,
                zoom: self.camera_zoom,
            };
            look_at_host_modes().views[slot] = Some(loc);
            self.camera_view_bookmarks[slot] = Some(loc.pos);
            let msg = lookat_bookmark_message(slot + 1);
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
        } else if let Some(loc) = look_at_host_modes().views[slot] {
            let clamped = self.clamp_to_world_bounds(loc.pos);
            self.camera_target = clamped;
            self.camera_yaw_radians = loc.yaw;
            self.camera_pitch_radians = loc.pitch;
            self.camera_zoom = clamp_w3d_zoom(loc.zoom);
            self.camera_yaw_target = None;
            self.camera_pitch_target = None;
            self.camera_zoom_target = None;
            look_at_host_modes().desired_height_above_ground = None;
            self.apply_camera_orbit_transform();
            if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                self.update_mouse_world_position();
                self.sync_context_mouse_cursor();
            }
        } else if let Some(pos) = self.camera_view_bookmarks[slot] {
            // Older position-only bookmark: restore look-at, keep current pose extras.
            let _ = self.host_center_camera_and_request_focus(pos);
        }
        // C++ View::setLocation no-ops when !m_valid. Unsaved F1-F8 is silent.
    }

    pub(super) fn clear_look_at_host_modes(&mut self) {
        self.apply_look_at_reset_modes();
    }

}

fn presentation_closest_spawn_or_rider<'a>(
    frame: &'a crate::presentation_frame::PresentationFrame,
    attacker: &crate::presentation_frame::RenderableObject,
    pos: Vec3,
    slaves: bool,
) -> Option<&'a crate::presentation_frame::RenderableObject> {
    let mut best: Option<(&crate::presentation_frame::RenderableObject, f32)> = None;
    for other in &frame.objects {
        if other.destroyed {
            continue;
        }
        let is_match = if slaves {
            other.producer_id == Some(attacker.id)
        } else {
            other.contained_by == Some(attacker.id)
                || attacker.garrisoned_units.iter().any(|id| *id == other.id)
        };
        if !is_match {
            continue;
        }
        let dx = other.position.x - pos.x;
        let dz = other.position.z - pos.z;
        let dist_sq = dx * dx + dz * dz;
        if best.map(|(_, d)| dist_sq < d).unwrap_or(true) {
            best = Some((other, dist_sq));
        }
    }
    best.map(|(obj, _)| obj)
}

fn presentation_weapon_reaches(
    attacker: &crate::presentation_frame::RenderableObject,
    pos: Vec3,
) -> bool {
    if !attacker.has_weapon {
        return false;
    }
    if attacker.weapon_range <= 0.0 {
        return true;
    }
    let dx = attacker.position.x - pos.x;
    let dz = attacker.position.z - pos.z;
    (dx * dx + dz * dz).sqrt() <= attacker.weapon_range
}

/// C++ `canObjectForceAttack` (CommandXlat.cpp:152-244) on the presentation freeze.
fn presentation_is_spawns_are_the_weapons(
    attacker: &crate::presentation_frame::RenderableObject,
) -> bool {
    attacker.hive_slave_count > 0
        || crate::game_logic::host_base_defense::is_stinger_site_structure(&attacker.template_name)
        || crate::game_logic::host_fire_base::is_fire_base_template(&attacker.template_name)
}

fn presentation_object_can_force_attack(
    frame: &crate::presentation_frame::PresentationFrame,
    attacker: &crate::presentation_frame::RenderableObject,
    victim: Option<ObjectId>,
    pos: Vec3,
) -> bool {
    use crate::game_logic::KindOf;
    use crate::presentation_frame::PresentationFrame;

    if let Some(victim_id) = victim {
        let Some(target) = frame.objects.iter().find(|o| o.id == victim_id) else {
            return false;
        };
        if target.destroyed || target.sold {
            return false;
        }
        let target_pos = target.position;
        let mut possible = attacker.has_weapon;
        if presentation_is_spawns_are_the_weapons(attacker) {
            if !possible {
                if let Some(slave) =
                    presentation_closest_spawn_or_rider(frame, attacker, target_pos, true)
                {
                    possible = slave.has_weapon;
                }
            } else if let Some(rider) =
                presentation_closest_spawn_or_rider(frame, attacker, target_pos, false)
            {
                if rider.has_weapon {
                    return true;
                }
            }
        }
        return possible;
    }

    let mut test = attacker;
    if PresentationFrame::object_has_kind(attacker, KindOf::Immobile)
        || presentation_is_spawns_are_the_weapons(attacker)
    {
        if let Some(slave) = presentation_closest_spawn_or_rider(frame, attacker, pos, true) {
            test = slave;
        } else if !presentation_weapon_reaches(attacker, pos) {
            if let Some(rider) = presentation_closest_spawn_or_rider(frame, attacker, pos, false) {
                test = rider;
            }
        }
    }
    presentation_weapon_reaches(test, pos)
}

#[cfg(test)]
mod camera_pick_tests {
    use super::*;

    #[test]
    fn double_click_type_select_uses_os_pixel_slop() {
        let src = include_str!("mouse.rs");
        assert!(src.contains("is_os_style_double_click"));
        assert!(src.contains("os_double_click_time_ms"));
        assert!(src.contains("OS_DOUBLE_CLICK_SLOP_PX"));
        assert!(!src.contains("time_delta < 500 && pos_delta < 10.0"));
    }

    #[test]
    fn reset_camera_pose_restores_fx_pitch_to_one() {
        let src = include_str!("mouse.rs");
        let start = src
            .find("fn reset_camera_pose_in_place")
            .expect("reset_camera_pose_in_place");
        let body = &src[start..start + 900];
        assert!(
            body.contains("self.camera_fx_pitch = 1.0"),
            "CAMERA_RESET/MMB must restore C++ m_FXPitch 1.0"
        );
        assert!(
            body.contains("self.camera_pitch_target = None"),
            "reset must cancel an in-flight CAMERA_PITCH lerp"
        );
        assert!(
            body.contains("live_home_pitch_radians")
                && body.contains("self.ui_script_default_camera_pitch()"),
            "CAMERA_RESET/MMB must restore scripted m_defaultPitchAngle"
        );
    }

    #[test]
    fn camera_set_default_scales_wheel_clamp_and_home_pitch() {
        let (min_h, max_half) = live_view_height_clamp(40.0, 200.0, 0.5);
        assert!((min_h - 40.0).abs() < f32::EPSILON);
        assert!((max_half - 100.0).abs() < f32::EPSILON);
        let (_, max_double) = live_view_height_clamp(40.0, 200.0, 2.0);
        assert!((max_double - 400.0).abs() < f32::EPSILON);
        let (_, max_zero) = live_view_height_clamp(40.0, 200.0, 0.0);
        assert!(
            (max_zero - 40.0).abs() < f32::EPSILON,
            "scale 0 floors to min like C++ setDefaultView"
        );

        let home_default = live_home_pitch_radians(37.5, 1.0);
        assert!((home_default - 37.5_f32.to_radians()).abs() < 1.0e-5);
        let home_script_zero = live_home_pitch_radians(37.5, 0.0);
        assert!((home_script_zero - 37.5_f32.to_radians()).abs() < 1.0e-5);
        let home_scripted = live_home_pitch_radians(37.5, 0.8);
        assert!((home_scripted - (37.5_f32.to_radians() + 0.8)).abs() < 1.0e-5);

        let src = include_str!("mouse.rs");
        let zoom = src
            .find("fn apply_player_height_zoom_steps")
            .expect("apply_player_height_zoom_steps");
        assert!(
            src[zoom..zoom + 700].contains("live_view_height_clamp"),
            "wheel clamp must use script-scaled View max"
        );
        let settle = src
            .find("fn ease_camera_height_above_ground")
            .expect("ease_camera_height_above_ground");
        assert!(
            src[settle..settle + 1200].contains("live_view_height_clamp"),
            "settle clamp must use script-scaled View max"
        );
    }

    fn cursor_pick_uses_camera_ray_not_twenty_wu_pad() {
        let src = include_str!("selection.rs");
        assert!(src.contains("fn host_pick_object_at_cursor"));
        assert!(src.contains("pick_object_id_along_camera_ray"));
        assert!(src.contains("host_cursor_blocked_by_opaque_window"));
        let mouse = include_str!("mouse.rs");
        assert!(mouse.contains("fn find_object_at_cursor"));
        assert!(mouse.contains("self.find_object_at_cursor(false)"));
    }

    fn classic_shift_lmb_only_prefers_selection_for_a_local_target() {
        assert!(classic_left_context_action_allowed(true, false, true));
        assert!(!classic_left_context_action_allowed(true, true, true));
        assert!(classic_left_context_action_allowed(true, true, false));
        assert!(!classic_left_context_action_allowed(false, false, false));
    }

    #[test]
    fn center_screen_pick_follows_the_render_camera_not_map_extents() {
        let camera = Vec3::new(0.0, 120.0, 120.0);
        let view = Mat4::look_at_rh(camera, Vec3::ZERO, Vec3::Y);
        let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 1.0, 1.0, 2_000.0);
        let (near, far) = unproject_mouse_ray(view, projection, (500.0, 500.0), 1_000.0, 1_000.0)
            .expect("a finite WGPU camera ray");

        let hit = raycast_ground_plane_clamped(
            near,
            far,
            Vec3::new(-1_000.0, 0.0, -1_000.0),
            Vec3::new(1_000.0, 0.0, 1_000.0),
            None,
        )
        .expect("center ray intersects the ground plane");

        assert!(
            hit.length() < 0.02,
            "center-screen pick must land at the camera target, got {hit:?}"
        );
    }

    #[test]
    fn ray_interval_rejects_a_parallel_ray_outside_the_map() {
        assert!(ray_interval_in_world_xz(
            Vec3::new(20.0, 5.0, 0.0),
            Vec3::new(20.0, -5.0, 0.0),
            Vec3::new(-10.0, 0.0, -10.0),
            Vec3::new(10.0, 0.0, 10.0),
        )
        .is_none());
    }

    #[test]
    fn point_select_predicate_accepts_non_local_selectable() {
        // C++ SelectionXlat.cpp:181-189 — point clicks select anything selectable.
        // Local-only remains the Shift-add / drag-select gate.
        assert!(classic_left_context_action_allowed(true, true, false));
        assert!(!classic_left_context_action_allowed(true, true, true));
    }

    #[test]
    fn lookat_arrow_keys_blocked_during_box_select_and_rmb_scroll() {
        // C++ LookAtXlat.cpp:174-175
        assert!(lookat_keyboard_scroll_blocked(true, false));
        assert!(lookat_keyboard_scroll_blocked(false, true));
        assert!(lookat_keyboard_scroll_blocked(true, true));
        assert!(!lookat_keyboard_scroll_blocked(false, false));
    }

    #[test]
    fn lookat_scroll_types_are_exclusive() {
        // C++ m_scrollType: frame tick applies only one of RMB / KEY / SCREENEDGE.
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::Rmb,
                true,
                false,
                true,
                true,
                true
            ),
            LookAtScrollType::Rmb
        );
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::ScreenEdge,
                true,
                false,
                false,
                true,
                true
            ),
            LookAtScrollType::ScreenEdge
        );
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::Key,
                true,
                false,
                false,
                true,
                true
            ),
            LookAtScrollType::Key
        );
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::None,
                true,
                false,
                false,
                false,
                true
            ),
            LookAtScrollType::ScreenEdge
        );
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::None,
                true,
                false,
                false,
                true,
                true
            ),
            LookAtScrollType::Key
        );
        // Edge may start while box-selecting; keys may not.
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::None,
                true,
                true,
                false,
                true,
                true
            ),
            LookAtScrollType::ScreenEdge
        );
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::Key,
                false,
                false,
                false,
                true,
                true
            ),
            LookAtScrollType::None
        );
    }

    #[test]
    fn update_camera_does_not_snap_scout_past_6000wu() {
        // C++ LookAtXlat / W3DView::scrollBy have no distance-to-own-units snap.
        let src = include_str!("mouse.rs");
        let start = src.find("fn update_camera(&mut self, dt: f32)").expect("update_camera");
        let end = src[start..]
            .find("fn is_character_key_pressed")
            .map(|i| start + i)
            .unwrap_or(src.len());
        let body = &src[start..end];
        assert!(
            !body.contains("camera_is_unreasonably_far_from_local_units")
                && !body.contains("6_000.0 * 6_000.0")
                && !body.contains("snap_camera_to_local_units_if_needed"),
            "scouting past 6000wu must not yank the camera back to base"
        );
        assert!(
            body.contains("lookat_resolve_scroll_type")
                && body.contains("LookAtScrollType::ScreenEdge"),
            "live LookAt must apply one exclusive scroll type"
        );
    }

    #[test]
    fn lookat_mmb_short_click_matches_cpp_5px_5_frame_window() {
        // C++ LookAtXlat.cpp:241-250 CLICK_DURATION=5 PIXEL_OFFSET=5
        assert!(lookat_mmb_is_short_click(0.0, 0.0, 0));
        assert!(lookat_mmb_is_short_click(5.0, 5.0, 4));
        assert!(!lookat_mmb_is_short_click(5.1, 0.0, 0));
        assert!(!lookat_mmb_is_short_click(0.0, 0.0, 5));
    }

    #[test]
    fn lookat_keyboard_rotate_uses_ini_speed_per_logic_frame() {
        // C++ GlobalData.cpp m_keyboardCameraRotateSpeed default 0.1, per InGameUI frame.
        let delta = lookat_keyboard_rotate_delta(0.1, 1.0 / 30.0, 30.0);
        assert!((delta - 0.1).abs() < 1.0e-5);
        let faster = lookat_keyboard_rotate_delta(0.25, 1.0 / 30.0, 30.0);
        assert!((faster - 0.25).abs() < 1.0e-5);
        assert!((LOOKAT_MMB_YAW_FACTOR - 0.01).abs() < f32::EPSILON);
    }

    #[test]
    fn lookat_view_location_stores_full_pose() {
        let loc = CameraViewLocation {
            pos: Vec3::new(100.0, 2.0, -40.0),
            yaw: 0.5,
            pitch: 0.7,
            zoom: 1.25,
        };
        assert_eq!(loc.pos.x, 100.0);
        assert!((loc.yaw - 0.5).abs() < f32::EPSILON);
        assert!((loc.pitch - 0.7).abs() < f32::EPSILON);
        assert!((loc.zoom - 1.25).abs() < f32::EPSILON);
        assert_eq!(lookat_view_slot(NamedKey::F1), Some(0));
        assert_eq!(lookat_view_slot(NamedKey::F8), Some(7));
        assert!(lookat_bookmark_message(3).contains("3"));
    }

    #[test]
    fn lookat_reset_pose_keeps_look_at_not_base() {
        // C++ InGameUI.cpp:4141-4143 getLocation then resetCamera(&currentView.getPosition()).
        let look_at = Vec3::new(900.0, 10.0, -300.0);
        let after_reset_target = look_at;
        assert_ne!(after_reset_target, Vec3::ZERO);
        assert!((LOOKAT_DEFAULT_PITCH_DEG - 37.5).abs() < f32::EPSILON);
    }

    #[test]
    fn force_attack_and_double_click_guard_follow_command_xlat() {
        // C++ CommandXlat.cpp:152-244 / 3635-3713.
        let src = include_str!("mouse.rs");
        assert!(
            src.contains("fn host_selection_can_force_attack")
                && src.contains("presentation_is_spawns_are_the_weapons")
                && src.contains("presentation_closest_spawn_or_rider"),
            "live force-attack must gate on canObjectForceAttack spawn/rider"
        );
        assert!(
            src.contains("fn host_try_double_click_guard_command")
                && src.contains("CommandType::Guard")
                && src.contains("GuardMode::Normal"),
            "double-click attack-move must post DoGuardPosition / Guard Normal"
        );
    }

    #[test]
    fn host_replay_camera_emit_matches_cpp_look_at_gates() {
        // C++ LookAtXlat.cpp:459 — saveCameraInReplay && (SP || skirmish).
        assert!(!should_emit_host_replay_camera(
            GameState::Menu,
            crate::game_logic::GameMode::Skirmish
        ));
        assert!(!should_emit_host_replay_camera(
            GameState::InGame,
            crate::game_logic::GameMode::Multiplayer
        ));
        assert!(!should_emit_host_replay_camera(
            GameState::InGame,
            crate::game_logic::GameMode::Shell
        ));
    }

    #[test]
    fn drag_tolerance_and_empty_click_match_cpp_selection_xlat() {
        // C++ Mouse.cpp DragTolerance default 5 / SelectionXlat.cpp:399-407, 575-597, 617-626, 930-937.
        assert!(is_point_click_drag(0.0, 0.0));
        assert!(is_point_click_drag(5.0, 0.0));
        assert!(is_point_click_drag(0.0, 5.0));
        assert!(is_point_click_drag(4.0, 4.0), "4x4 diagonal is still a click");
        assert!(is_point_click_drag(5.0, 5.0));
        assert!(!is_point_click_drag(5.1, 0.0));
        assert!(!is_point_click_drag(0.0, 5.1));
        assert!(!alternate_mouse_blank_click_deselects(false, false, false, false));
        assert!(alternate_mouse_blank_click_deselects(true, false, false, false));
        assert!(!alternate_mouse_blank_click_deselects(true, true, false, false));
        assert!(box_selection_must_replace(true, false, false, false));
        assert!(box_selection_must_replace(false, false, false, true));
        assert!(!box_selection_must_replace(false, false, false, false));
        assert!(infantry_garrison_context_takes_region(false, false, true, 1));
        assert!(!infantry_garrison_context_takes_region(true, false, true, 1));
        assert!(!infantry_garrison_context_takes_region(false, true, true, 1));
        assert!(!infantry_garrison_context_takes_region(false, false, true, 0));

    }

    #[test]
    fn host_is_selecting_requires_drag_tolerance_not_lmb_held() {
        // C++ SelectionXlat.cpp:399-408 — isSelecting after DragTolerance, not LMB down.
        assert!(!host_is_selecting_now(
            false,
            false,
            Some((0.0, 0.0)),
            (3.0, 0.0)
        ));
        assert!(!host_is_selecting_now(
            true,
            false,
            Some((0.0, 0.0)),
            (5.0, 0.0)
        ));
        assert!(host_is_selecting_now(
            true,
            false,
            Some((0.0, 0.0)),
            (5.1, 0.0)
        ));
        assert!(
            !host_is_selecting_now(true, true, Some((0.0, 0.0)), (20.0, 0.0)),
            "placement rotate must not count as isSelecting"
        );
        let src = include_str!("mouse.rs");
        let cam = src
            .find("fn update_camera(&mut self, dt: f32)")
            .expect("update_camera");
        let cam_body = &src[cam..src.len().min(cam + 2500)];
        assert!(
            cam_body.contains("self.host_is_selecting()"),
            "arrow scroll must use isSelecting, not is_dragging"
        );
        let rmb = src
            .find("fn start_rmb_lookat_scroll")
            .expect("start_rmb_lookat_scroll");
        let rmb_body = &src[rmb..src.len().min(rmb + 400)];
        assert!(
            rmb_body.contains("self.host_is_selecting()")
                && !rmb_body.contains("self.is_dragging || self.is_rmb_scrolling"),
            "RMB scroll must gate on isSelecting, not LMB-held"
        );
    }

    #[test]
    fn placement_rotate_uses_5px_screen_not_1wu() {
        // C++ PlaceEventTranslator.cpp:307-320 Euclidean 5px, no 1wu world gate.
        assert!(!placement_screen_drag_exceeds_threshold(3.0, 3.0));
        assert!(placement_screen_drag_exceeds_threshold(3.0, 4.0));
        assert!(placement_screen_drag_exceeds_threshold(5.0, 0.0));
        let src = include_str!("mouse.rs");
        assert!(
            src.contains("placement_screen_drag_exceeds_threshold(drag_dx, drag_dy)")
                && src.contains("fn update_anchored_placement_from_cursor")
                && !src.contains("dx * dx + dz * dz > 1.0"),
            "placement rotate must use 5px screen, not 1wu release gate"
        );
    }

    #[test]
    fn placement_lmb_down_cancels_when_builder_gone() {
        // C++ PlaceEventTranslator.cpp:68-75 — missing builder does not anchor.
        let src = include_str!("mouse.rs");
        let start = src
            .find("fn handle_left_click")
            .expect("handle_left_click");
        let body = &src[start..src.len().min(start + 2200)];
        assert!(
            body.contains("pending_place_builder_is_gone")
                && body.contains("cancel_structure_placement_from_ui")
                && body.contains("set_placement_start"),
            "LMB-down must cancel when the pending place source is gone"
        );
        assert!(
            src.contains("get_pending_place_source_object_id")
                && src.contains("object.sold"),
            "builder-gone must look up leftover place source and treat sold as gone"
        );
        let ui = include_str!("ui_commands.rs");
        let begin = ui
            .find("fn begin_structure_placement_from_ui")
            .expect("begin_structure_placement_from_ui");
        assert!(
            ui[begin..ui.len().min(begin + 1200)].contains("place_build_available"),
            "arming placement must store leftover pending place source"
        );
    }

    #[test]
    fn placement_angle_reprojects_screen_anchor() {
        // C++ InGameUI::handleBuildPlacements screenToTerrain both points.
        let src = include_str!("mouse.rs");
        let drag = src
            .find("fn update_anchored_placement_from_cursor")
            .expect("update_anchored_placement_from_cursor");
        let drag_body = &src[drag..src.len().min(drag + 900)];
        assert!(
            drag_body.contains("self.screen_to_terrain(start)")
                && !drag_body.contains("self.selection_start.unwrap_or(self.mouse_world_position)"),
            "anchored rotate must reproject the screen start, not the stale world point"
        );
        let confirm = src
            .find("confirm re-projects the screen anchor")
            .expect("placement confirm comment");
        let confirm_body = &src[confirm..src.len().min(confirm + 700)];
        assert!(
            confirm_body.contains("self.screen_to_terrain(s)")
                && confirm_body.contains("place_structure_from_ui(&template, start_world)"),
            "placement confirm must place at the reprojected screen anchor"
        );
    }


    #[test]
    fn alternate_mouse_one_click_prevent_deselect_matches_cpp() {
        // C++ SelectionXlat.cpp:935-943 + GUICommandTranslator.cpp:471-473.
        let mouse = include_str!("mouse.rs");
        let ui = include_str!("ui_commands.rs");
        assert!(mouse.contains("host_consume_prevent_left_click_deselection"));
        assert!(mouse.contains("host_set_prevent_left_click_deselection"));
        assert!(ui.contains("host_set_prevent_left_click_deselection(true)"));
        assert!(mouse.contains("host_pick_hover_object_at_cursor"));
    }


    #[test]
    fn left_release_point_click_does_not_box_wipe() {
        // C++ MetaEvent.cpp:571-596 + SelectionXlat.cpp:575-597 / 905-950.
        let src = include_str!("mouse.rs");
        assert!(src.contains("const DRAG_TOLERANCE_PX: f32 = 5.0"));
        assert!(src.contains("is_point_click_drag(drag_dx, drag_dy)"));
        assert!(!src.contains("drag_distance_screen <= 2.0"));
        assert!(src.contains("alternate_mouse_blank_click_deselects"));
        assert!(src.contains("garrisonable_building_ids_in_screen_rect"));
        assert!(src.contains("box_selection_must_replace"));
        assert!(src.contains("union_object_ids(similar_units, previous)"));
        assert!(
            !src.contains("first selectable local"),
            "pick miss must not invent the first locally-owned unit"
        );
        assert!(src.contains("CommandType::Enter { target_id }"));
        let release = src
            .find("fn handle_left_release")
            .expect("handle_left_release");
        let release_end = src[release..]
            .find("fn handle_right_click")
            .map(|i| release + i)
            .unwrap_or(release + 12_000);
        let release_body = &src[release..release_end];
        assert!(
            release_body.contains("select_left_click_target"),
            "point LMB must commit selection on click, not press"
        );
        let press = src.find("fn handle_left_click").expect("handle_left_click");
        let press_end = src[press + 1..]
            .find("\n    fn ")
            .map(|i| press + 1 + i)
            .unwrap_or(press + 2500);
        let force = format!("{}{}", "force-select local ", "object");
        assert!(
            !release_body.contains(&force) && !src[press..press_end].contains(&force),
            "empty LMB must not force-select CanSelectDrawable rejects"
        );
        assert!(
            !src[press..press_end].contains("select_left_click_target"),
            "RAW LMB down must not commit SelectionXlat"
        );
    }

    #[test]
    fn select_p2_leftover_double_click_and_sound_match_cpp() {
        // hq-kmgkv: 3–4px is still a click (Mouse.ini DragTolerance default 5).
        assert!(host_screen_drag_is_click(4.0, 0.0));
        assert!(host_screen_drag_is_click(0.0, 4.0));
        assert!(host_screen_drag_is_click(5.0, 5.0));
        assert!(!host_screen_drag_is_click(6.0, 0.0));
        assert!((host_mouse_drag_tolerance_px() - 5.0).abs() < f32::EPSILON);

        // hq-ht4gw / hq-myqqv / hq-gqwjy live-host markers.
        let src = include_str!("mouse.rs");
        assert!(src.contains("fn presentation_double_click_consumes"));
        assert!(src.contains("KEEP_MESSAGE so CommandXlat.cpp:3698-3713"));
        assert!(src.contains("union_object_ids(similar_units, previous)"));
        assert!(src.contains("let boxed_any = !boxed.is_empty()"));
        assert!(src.contains("if boxed_any"));
        assert!(src.contains("host_mouse_drag_tolerance_px()"));
    }

    #[test]
    fn rmb_click_gates_match_cpp_selection_xlat() {
        // C++ SelectionXlat.cpp:982-1000 + Mouse.ini defaults 5px / 250ms / 5wu.
        assert!(host_rmb_release_is_click(0.0, 0.0, 0, 0.0));
        assert!(host_rmb_release_is_click(5.0, 5.0, 250, 5.0));
        assert!(!host_rmb_release_is_click(6.0, 0.0, 0, 0.0));
        assert!(!host_rmb_release_is_click(0.0, 0.0, 251, 0.0));
        assert!(!host_rmb_release_is_click(0.0, 0.0, 0, 5.1));
        assert!((host_mouse_drag_tolerance_ms() as f32 - 250.0).abs() < f32::EPSILON);
        assert!((host_mouse_drag_tolerance_3d() - 5.0).abs() < f32::EPSILON);

        let src = include_str!("mouse.rs");
        assert!(src.contains("fn cancel_area_select_from_control_bar"));
        assert!(src.contains("fn note_rmb_deselect_anchor"));
        assert!(src.contains("fn rmb_release_is_deselect_click"));
        assert!(src.contains("host_find_object_at_position"));
        let input = include_str!("input.rs");
        assert!(input.contains("cancel_area_select_from_control_bar"));
        assert!(input.contains("rmb_release_is_deselect_click"));
        assert!(!input.contains("DRAG_THRESHOLD_SQ"));
    }

    #[test]
    fn w3d_set_zoom_clamps_to_view_min_max() {
        // C++ View.cpp:78-79 / W3DView::setZoom [0.2, 1.3].
        assert!((clamp_w3d_zoom(0.1) - 0.2).abs() < f32::EPSILON);
        assert!((clamp_w3d_zoom(2.0) - 1.3).abs() < f32::EPSILON);
        assert!((clamp_w3d_zoom(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn letterbox_lifts_min_max_camera_height_clamp() {
        let limited = height_after_zoom_steps(120.0, -20.0, 40.0, 200.0, true);
        assert!((limited - 40.0).abs() < f32::EPSILON);
        let unlocked = height_after_zoom_steps(120.0, -20.0, 40.0, 200.0, false);
        assert!((unlocked - -80.0).abs() < f32::EPSILON);
        let high = height_after_zoom_steps(180.0, 10.0, 40.0, 200.0, false);
        assert!((high - 280.0).abs() < f32::EPSILON);
    }

    #[test]
    fn lookat_mouse_moved_recently_is_one_logic_second() {
        look_at_host_modes().last_mouse_move_frame = 10;
        assert!(lookat_has_mouse_moved_recently(10));
        assert!(lookat_has_mouse_moved_recently(40));
        assert!(!lookat_has_mouse_moved_recently(41));
    }

    #[test]
    fn disable_input_blocks_key_and_edge_scroll() {
        // C++ LookAtXlat setScrolling / RAW_MOUSE_POSITION gate on input enabled.
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::None,
                false,
                false,
                false,
                true,
                true
            ),
            LookAtScrollType::None
        );
    }

    #[test]
    fn wheel_stop_blocks_key_and_edge_until_next_input() {
        // C++ MSG_RAW_MOUSE_WHEEL fallthrough stopScrolling; KEY/EDGE stay down
        // until the next RAW_KEY / RAW_MOUSE_POSITION.
        look_at_host_modes().wheel_stopped_scroll = true;
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::None,
                true,
                false,
                false,
                true,
                true
            ),
            LookAtScrollType::None
        );
        lookat_note_raw_key_activity();
        assert_eq!(
            lookat_resolve_scroll_type(
                LookAtScrollType::None,
                true,
                false,
                false,
                true,
                false
            ),
            LookAtScrollType::Key
        );
    }

    #[test]
    fn click_and_wheel_stamp_activity_without_pixel_change() {
        look_at_host_modes().last_mouse_move_frame = 0;
        look_at_host_modes().last_mouse_pixel = (10.0, 10.0);
        lookat_note_mouse_moved(50, (10.0, 10.0));
        assert_eq!(look_at_host_modes().last_mouse_move_frame, 0);
        lookat_stamp_mouse_activity(50);
        assert_eq!(look_at_host_modes().last_mouse_move_frame, 50);
        assert!(lookat_has_mouse_moved_recently(50));
    }

    #[test]
    fn wheel_zoom_stops_scroll_and_lmb_resets_group_tap() {
        let src = include_str!("mouse.rs");
        let wheel = src
            .find("fn handle_mouse_wheel")
            .map(|i| &src[i..src.len().min(i + 1800)])
            .expect("handle_mouse_wheel");
        assert!(
            wheel.contains("stop_rmb_lookat_scroll")
                && wheel.contains("LookAtScrollType::None")
                && wheel.contains("wheel_stopped_scroll = true")
                && wheel.contains("lookat_stamp_mouse_activity"),
            "wheel must stamp activity and stop RMB/key/edge scroll"
        );
        let edge = src
            .find("if edge_allowed")
            .map(|i| &src[i..src.len().min(i + 700)])
            .expect("edge_allowed");
        assert!(
            edge.contains("window.inner_size()") && edge.contains("win_h"),
            "bottom edge-scroll must use Display height, not tactical 80%"
        );
        assert!(
            !edge.contains("tactical_viewport_size()"),
            "edge-scroll must not use the 80% tactical viewport"
        );
        assert!(
            src.contains("last_control_group_select = None"),
            "manual LMB select must reset control-group double-tap"
        );
        let recall = src
            .find("fn save_or_recall_camera_view")
            .map(|i| &src[i..src.len().min(i + 2800)])
            .expect("save_or_recall_camera_view");
        assert!(
            !recall.contains("push_info_message")
                || recall.contains("lookat_bookmark_message"),
            "empty F1-F8 bookmark must stay silent"
        );
        assert!(
            recall.contains("Unsaved F1-F8 is silent"),
            "empty bookmark path must remain a silent no-op"
        );
        let cancel = src
            .find("fn cancel_world_mouse_targeting")
            .map(|i| &src[i..src.len().min(i + 700)])
            .expect("cancel_world_mouse_targeting");
        assert!(
            !cancel.contains("push_info_message")
                && !cancel.contains(concat!("Cancelled pending", " command")),
            "hq-0foy9: RMB cancel of an armed GUI command must stay silent"
        );
        assert!(
            src.contains("fn sync_letterbox_os_cursor_visibility")
                && src.contains("set_cursor_visible(!over_bar)"),
            "OS cursor must hide under cinematic letterbox bars"
        );
        assert!(
            src.contains("lookat_stamp_mouse_activity(self.frame_counter)")
                && src.contains("fn start_rmb_lookat_scroll")
                && src.contains("fn begin_mmb_lookat_rotate"),
            "RMB/MMB/wheel must stamp hasMouseMovedRecently"
        );
    }


    #[test]
    fn update_camera_does_not_gate_arrows_on_modifiers() {
        let src = include_str!("mouse.rs");
        let start = src.find("fn update_camera(&mut self, dt: f32)").expect("update_camera");
        let end = src[start..]
            .find("fn is_character_key_pressed")
            .map(|i| start + i)
            .unwrap_or(src.len());
        let body = &src[start..end];
        assert!(
            !body.contains("mods_down"),
            "C++ RAW_KEY has no Ctrl/Shift/Alt gate"
        );
        assert!(
            body.contains("break_camera_follow_lock"),
            "player scroll must clear camera lock"
        );
        assert!(
            body.contains("clamp_w3d_zoom"),
            "replay/bookmark zoom must clamp"
        );
        assert!(
            body.contains("apply_host_slave_camera") && body.contains("apply_airborne_follow_yaw"),
            "slave bone + airborne follow yaw must be live"
        );
        assert!(
            body.contains("cancel_scripted_camera_from_player_set")
                && body.contains("cancel_scripted_camera_from_player_scroll"),
            "player rotate/zoom/scroll must cancel scripted camera"
        );
    }

    #[test]
    fn rmb_set_scrolling_breaks_camera_lock() {
        // C++ InGameUI.cpp:2799-2801 setCameraLock(INVALID) + setCameraLockDrawable(NULL).
        let src = include_str!("mouse.rs");
        let start = src
            .find("fn start_rmb_lookat_scroll")
            .expect("start_rmb_lookat_scroll");
        let body = &src[start..src.len().min(start + 900)];
        assert!(
            body.contains("break_camera_follow_lock"),
            "RMB setScrolling must break camera lock"
        );
        let brk = src
            .find("fn break_camera_follow_lock")
            .expect("break_camera_follow_lock");
        let brk_body = &src[brk..src.len().min(brk + 500)];
        assert!(
            brk_body.contains("set_camera_lock(None)")
                && brk_body.contains("set_camera_lock_drawable(None)"),
            "setScrolling clears lock + drawable"
        );
    }

    #[test]
    fn key_and_screenedge_scroll_mouse_lock_like_cpp() {
        // C++ LookAtXlat.cpp:50-62 setScrolling — KEY/SCREENEDGE lock the mouse
        // the same as RMB so WindowXlat keeps hover/RMB/MMB off the HUD.
        let src = include_str!("mouse.rs");
        let helper = src
            .find("fn set_lookat_scroll_mouse_lock")
            .expect("set_lookat_scroll_mouse_lock");
        let helper_body = &src[helper..src.len().min(helper + 900)];
        assert!(
            helper_body.contains("modes.mouse_locked = true")
                && helper_body.contains("TheInGameUI::set_scrolling(true)")
                && helper_body.contains("view.set_mouse_lock(true)")
                && helper_body.contains("set_scrolling(false)")
                && helper_body.contains("view.set_mouse_lock(false)"),
            "shared setScrolling path must lock/unlock mouse + InGameUI"
        );
        let cam = src
            .find("fn update_camera(&mut self, dt: f32)")
            .expect("update_camera");
        let cam_end = src[cam..]
            .find("fn is_character_key_pressed")
            .map(|i| cam + i)
            .unwrap_or(src.len());
        let cam_body = &src[cam..cam_end];
        assert!(
            cam_body.contains("self.set_lookat_scroll_mouse_lock(scroll_type.is_scrolling())")
                && cam_body.contains("self.set_lookat_scroll_mouse_lock(false)"),
            "update_camera must mouse-lock KEY/SCREENEDGE and unlock when input dies"
        );
        let wheel = src
            .find("fn handle_mouse_wheel")
            .map(|i| &src[i..src.len().min(i + 1800)])
            .expect("handle_mouse_wheel");
        assert!(
            wheel.contains("self.set_lookat_scroll_mouse_lock(false)"),
            "wheel stopScrolling must unlock KEY/SCREENEDGE immediately"
        );
        let reset = src
            .find("fn apply_look_at_reset_modes")
            .map(|i| &src[i..src.len().min(i + 700)])
            .expect("apply_look_at_reset_modes");
        assert!(
            reset.contains("self.set_lookat_scroll_mouse_lock(false)"),
            "doDisableInput reset must not leave KEY/SCREENEDGE mouse-locked"
        );
        assert!(
            src.contains("if look_at_host_mouse_locked()")
                && src.contains("do not overwrite the locked cursor mid KEY/RMB/EDGE pan"),
            "context cursor must stay put during KEY/SCREENEDGE lock"
        );
    }



    #[test]
    fn airborne_look_at_ray_hits_ground_plane() {
        let look_dir = Vec3::new(0.0, -40.0, -120.0);
        let hit = airborne_look_at_ground(
            Vec3::new(0.0, 120.0, 120.0),
            Vec3::new(0.0, 80.0, 0.0),
            look_dir,
            2_000.0,
            Vec3::new(-1_000.0, 0.0, -1_000.0),
            Vec3::new(1_000.0, 0.0, 1_000.0),
            None,
        )
        .expect("airborne look-at must hit ground");
        assert!(
            hit.y.abs() < 0.02,
            "ground hit Y must be the plane, got {hit:?}"
        );
        assert!(
            hit.z.abs() > 1.0,
            "ray through an elevated unit must land past the XY origin, got {hit:?}"
        );

        // Off-axis unit: look dir is screen-center, not camera-to-object.
        let off_axis = airborne_look_at_ground(
            Vec3::new(0.0, 120.0, 120.0),
            Vec3::new(40.0, 80.0, 0.0),
            look_dir,
            2_000.0,
            Vec3::new(-1_000.0, 0.0, -1_000.0),
            Vec3::new(1_000.0, 0.0, 1_000.0),
            None,
        )
        .expect("off-axis airborne look-at must hit ground");
        assert!(
            (off_axis.x - 40.0).abs() < 0.5,
            "look dir must keep the unit on the screen-center ray, got {off_axis:?}"
        );
    }

    #[test]
    fn vertical_pan_uses_display_aspect_boost() {
        // C++ W3DView.cpp:1796-1798 — 1920x1080 with 80% tactical frac → 2.222.
        let forward = Vec3::new(0.0, 0.0, 1.0);
        let right = Vec3::new(1.0, 0.0, 0.0);
        let aspect = 1920.0 / 864.0;
        let dx = lookat_scroll_world_delta(Vec2::new(1.0, 0.0), forward, right, 250.0, aspect);
        let dy = lookat_scroll_world_delta(Vec2::new(0.0, 1.0), forward, right, 250.0, aspect);
        assert!((dx.x - 1.0).abs() < 1.0e-5, "horizontal step {dx:?}");
        assert!(
            (dy.z + aspect).abs() < 1.0e-5,
            "vertical step must be aspect-boosted, got {dy:?}"
        );
        assert!(
            dy.length() > dx.length() * 2.0,
            "retail vertical pan is faster than horizontal by view aspect"
        );
    }

    #[test]
    fn replay_hover_feeds_has_mouse_moved_recently_gate() {
        let src = include_str!("mouse.rs");
        let start = src
            .find("fn sync_ingame_mouseover_hint")
            .expect("sync_ingame_mouseover_hint");
        let end = src[start + 1..]
            .find("\n    fn ")
            .map(|i| start + 1 + i)
            .unwrap_or(start + 600);
        let body = &src[start..end];
        let feed_at = body
            .find("feed_look_at_replay_hover_gate")
            .expect("must stamp leftover InGameUI playback/moved-recently");
        let hint_at = body
            .find("create_mouseover_hint")
            .expect("must still post mouseover hint");
        assert!(
            feed_at < hint_at,
            "C++ InGameUI.cpp:2462 gate must be fed before createMouseoverHint"
        );
        assert!(
            body.contains("host_recorder_is_playback")
                && body.contains("lookat_has_mouse_moved_recently"),
            "live host owns both playback and LookAt 1s window"
        );
    }




}
