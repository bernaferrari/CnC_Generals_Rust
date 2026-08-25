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
    let Some((b_near, b_far)) = unproject_mouse_ray(
        view,
        projection,
        (width * 0.5, height * 0.95),
        width,
        height,
    ) else {
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
fn host_rmb_release_is_click(dx: f32, dy: f32, elapsed_ms: u128, camera_delta_len: f32) -> bool {
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

fn union_object_ids(
    mut base: Vec<ObjectId>,
    extra: impl IntoIterator<Item = ObjectId>,
) -> Vec<ObjectId> {
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

mod camera;
mod selection_input;
mod ui_dispatch;
mod world_pick;

#[cfg(test)]
mod camera_pick_tests;

// Source aggregate for source-honesty tests after this module's structural split.
const MOUSE_SOURCE: &str = concat!(
    include_str!("mouse.rs"),
    include_str!("mouse/selection_input.rs"),
    include_str!("mouse/camera.rs"),
    include_str!("mouse/ui_dispatch.rs"),
    include_str!("mouse/world_pick.rs"),
);
