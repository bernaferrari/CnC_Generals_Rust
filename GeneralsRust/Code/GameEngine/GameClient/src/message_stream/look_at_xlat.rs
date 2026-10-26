//! Look-at translator for camera movement and scrolling.

use super::game_message::{
    Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, ICoord2D,
};
use super::message_stream::{GameMessageDisposition, GameMessageTranslator};
#[cfg(test)]
use crate::display::view::{with_tactical_view, with_tactical_view_ref};
use game_engine::common::message_stream::{
    CameraInputAction, CameraInputObservation, CameraLocation,
};
type CameraInputFn<'a> = dyn FnMut(CameraInputAction, &mut crate::message_stream::ReceivingUi<'_>) -> CameraInputObservation
    + 'a;
use crate::gui::MouseCursor;

use game_engine::common::game_common::LOGICFRAMES_PER_SECOND;
use game_engine::common::global_data;
use game_engine::common::ini::ini_game_data::get_global_data;

const MAX_VIEW_LOCS: usize = 8;
const SCROLL_AMT: f32 = 100.0;
const EDGE_SCROLL_SIZE: i32 = 3;
const MMB_CLICK_DURATION_FRAMES: u32 = 5;
const MMB_CLICK_PIXEL_OFFSET: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ScrollType {
    #[default]
    None,
    Rmb,
    Key,
    ScreenEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up = 0,
    Down,
    Left,
    Right,
}

#[derive(Debug, Default, Clone, Copy)]
struct Coord2D {
    x: f32,
    y: f32,
}

impl Coord2D {
    fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    fn normalize(&mut self) {
        let len = (self.x * self.x + self.y * self.y).sqrt();
        if len > 0.0 {
            self.x /= len;
            self.y /= len;
        }
    }
}

#[derive(Default)]
pub struct LookAtTranslator {
    anchor: ICoord2D,
    original_anchor: ICoord2D,
    current_pos: ICoord2D,
    is_scrolling: bool,
    is_rotating: bool,
    is_pitching: bool,
    is_changing_fov: bool,
    timestamp: u32,
    view_location: [CameraLocation; MAX_VIEW_LOCS],
    scroll_type: ScrollType,
    scroll_dir: [bool; 4],
    last_mouse_move_frame: u32,
    rotate_left: bool,
    rotate_right: bool,
    zoom_in: bool,
    zoom_out: bool,
    /// C++ `LookAtXlat.cpp:47` static `prevCursor`.
    prev_cursor: MouseCursor,
}

fn should_emit_replay_camera(game_mode: i32, ui: &crate::message_stream::ReceivingUi<'_>) -> bool {
    if !global_data::read().save_camera_in_replay {
        return false;
    }
    if ui.recorder_playback_active() {
        return false;
    }
    // C++ LookAtXlat.cpp:459 — single-player or skirmish only.
    game_mode == 0 || game_mode == 2
}
impl LookAtTranslator {
    pub fn new() -> Self {
        Self::default()
    }

    /// C++: LookAtTranslator::hasMouseMovedRecently()
    pub fn has_mouse_moved_recently(&mut self, current_frame: u32) -> bool {
        if self.last_mouse_move_frame > current_frame {
            self.last_mouse_move_frame = 0;
        }

        self.last_mouse_move_frame + LOGICFRAMES_PER_SECOND >= current_frame
    }

    /// C++ LookAtTranslator::resetModes — drop scroll/rotate/pitch/FOV modes.
    pub fn reset_modes(&mut self) {
        self.is_scrolling = false;
        self.is_rotating = false;
        self.is_pitching = false;
        self.is_changing_fov = false;
        self.scroll_type = ScrollType::None;
    }

    fn set_scrolling(
        &mut self,
        scroll_type: ScrollType,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
        camera: &mut CameraInputFn<'_>,
    ) {
        if !ui.get_input_enabled() {
            return;
        }
        // C++ LookAtXlat.cpp:55-61: save cursor, setScrolling, setMouseLock.
        self.prev_cursor = ui.get_mouse_cursor();
        self.is_scrolling = true;
        self.scroll_type = scroll_type;
        if ui.set_scrolling(true) {
            camera(CameraInputAction::ClearTracking, ui);
        }
        camera(CameraInputAction::SetMouseLock(true), ui);
    }

    fn stop_scrolling(
        &mut self,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
        camera: &mut CameraInputFn<'_>,
    ) {
        self.is_scrolling = false;
        self.scroll_type = ScrollType::None;
        ui.set_scrolling(false);
        camera(CameraInputAction::SetMouseLock(false), ui);
        ui.set_mouse_cursor(self.prev_cursor);
    }

    fn update_scroll_dir(&mut self, key: u32, pressed: bool) {
        match key {
            0x26 => self.scroll_dir[Direction::Up as usize] = pressed,
            0x28 => self.scroll_dir[Direction::Down as usize] = pressed,
            0x25 => self.scroll_dir[Direction::Left as usize] = pressed,
            0x27 => self.scroll_dir[Direction::Right as usize] = pressed,
            _ => {}
        }
    }

    fn get_global_scroll_factors(&self) -> (f32, f32, f32, bool, f32) {
        if let Some(data) = get_global_data() {
            let guard = data.read();
            (
                guard.horizontal_scroll_speed_factor,
                guard.vertical_scroll_speed_factor,
                guard.keyboard_scroll_factor,
                guard.windowed,
                guard.scroll_amount_cutoff,
            )
        } else {
            (1.0, 1.0, 1.0, false, 0.0)
        }
    }

    fn handle_frame_tick(
        &mut self,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
        camera: &mut CameraInputFn<'_>,
        game_mode: i32,
    ) {
        // Host Main update_camera drains take_look_at_reset_modes (doDisableInput).
        let dimensions = camera(CameraInputAction::Observe, ui);
        let (display_width, display_height) = (dimensions.width, dimensions.height);
        let (h_factor, v_factor, key_factor, _windowed, cutoff) = self.get_global_scroll_factors();

        let mut offset = Coord2D::zero();
        if self.is_scrolling && !ui.is_scrolling() {
            ui.set_scroll_amount(0.0, 0.0);
            self.stop_scrolling(ui, camera);
        } else if self.is_scrolling {
            match self.scroll_type {
                ScrollType::Rmb => {
                    offset.x = h_factor * (self.current_pos.x - self.anchor.x) as f32;
                    offset.y = v_factor * (self.current_pos.y - self.anchor.y) as f32;
                    let mut vec = offset;
                    vec.normalize();
                    offset.x += h_factor * vec.x * key_factor * key_factor;
                    offset.y += v_factor * vec.y * key_factor * key_factor;
                }
                ScrollType::Key => {
                    if self.scroll_dir[Direction::Up as usize] {
                        offset.y -= v_factor * SCROLL_AMT * key_factor;
                    }
                    if self.scroll_dir[Direction::Down as usize] {
                        offset.y += v_factor * SCROLL_AMT * key_factor;
                    }
                    if self.scroll_dir[Direction::Left as usize] {
                        offset.x -= h_factor * SCROLL_AMT * key_factor;
                    }
                    if self.scroll_dir[Direction::Right as usize] {
                        offset.x += h_factor * SCROLL_AMT * key_factor;
                    }
                }
                ScrollType::ScreenEdge => {
                    if self.current_pos.y < EDGE_SCROLL_SIZE {
                        offset.y -= v_factor * SCROLL_AMT * key_factor;
                    }
                    if self.current_pos.y >= display_height - EDGE_SCROLL_SIZE {
                        offset.y += v_factor * SCROLL_AMT * key_factor;
                    }
                    if self.current_pos.x < EDGE_SCROLL_SIZE {
                        offset.x -= h_factor * SCROLL_AMT * key_factor;
                    }
                    if self.current_pos.x >= display_width - EDGE_SCROLL_SIZE {
                        offset.x += h_factor * SCROLL_AMT * key_factor;
                    }
                }
                ScrollType::None => {}
            }

            if cutoff > 0.0 {
                if offset.x.abs() < cutoff {
                    offset.x = 0.0;
                }
                if offset.y.abs() < cutoff {
                    offset.y = 0.0;
                }
            }

            ui.set_scroll_amount(offset.x, offset.y);
            camera(
                CameraInputAction::Scroll {
                    x: offset.x,
                    y: offset.y,
                },
                ui,
            );
        } else {
            ui.set_scroll_amount(0.0, 0.0);
        }

        if self.rotate_left {
            camera(CameraInputAction::Rotate(-0.02), ui);
        }
        if self.rotate_right {
            camera(CameraInputAction::Rotate(0.02), ui);
        }
        if self.zoom_in {
            camera(CameraInputAction::ZoomIn, ui);
        }
        if self.zoom_out {
            camera(CameraInputAction::ZoomOut, ui);
        }

        // C++ LookAtXlat.cpp:459-470 — record camera for skirmish/SP replays.
        if should_emit_replay_camera(game_mode, ui) {
            let loc = camera(CameraInputAction::Observe, ui).location;
            let pos = loc.position;
            let coord = Coord3D::new(pos[0], pos[1], pos[2]);
            let mut msg = GameMessage::new(GameMessageType::SetReplayCamera(
                coord.clone(),
                loc.angle,
                loc.zoom,
            ));
            msg.append_location_argument(coord);
            msg.append_real_argument(loc.angle);
            msg.append_real_argument(loc.pitch);
            msg.append_real_argument(loc.zoom);
            ui.emit(msg);
        }
    }
}

impl GameMessageTranslator for LookAtTranslator {
    fn requires_receiving_camera_context(&self) -> bool {
        true
    }
    fn requires_receiving_window_context(&self) -> bool {
        true
    }
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        log::error!("GUI message translator requires receiving UI context");
        GameMessageDisposition::KeepMessage
    }
    fn translate_game_message_with_context(
        &mut self,
        msg: &GameMessage,
        client: game_engine::common::message_stream::TranslatorClientObservation,
        _dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,
        _emitted: &mut Vec<GameMessage>,
        client_ui: &mut dyn std::any::Any,
    ) -> GameMessageDisposition {
        let ui = &mut crate::message_stream::ReceivingUi::new(
            client_ui
                .downcast_mut::<crate::core::subsystems::InGameUISubsystem>()
                .expect("message translator requires its receiving UI"),
            client.logic_frame,
            client.frame,
        )
        .with_emitter(_emitted);

        self.translate_with_client(msg, client, ui, &mut |action, ui| match ui.dispatch(
            _dispatch,
            game_engine::common::message_stream::WindowInputDispatch::Camera(action),
        ) {
            Some(game_engine::common::message_stream::WindowInputDispatchResult::Camera(
                observation,
            )) => observation,
            _ => panic!("camera translator requires receiving camera"),
        })
    }
}
impl LookAtTranslator {
    fn translate_with_client(
        &mut self,
        msg: &GameMessage,
        client: game_engine::common::message_stream::TranslatorClientObservation,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
        camera: &mut CameraInputFn<'_>,
    ) -> GameMessageDisposition {
        // Live drain is Main update_camera → apply_look_at_reset_modes.
        let msg_type = msg.get_type();
        match msg_type {
            GameMessageType::RawKeyDown(key) | GameMessageType::RawKeyUp(key) => {
                if client.shell_active {
                    return GameMessageDisposition::KeepMessage;
                }
                let key_state = match msg.get_argument(1) {
                    Some(GameMessageArgumentType::Integer(value)) => *value as u32,
                    _ => 0,
                };
                let pressed = (key_state & 0x0001) == 0;
                self.update_scroll_dir(*key, pressed);

                if ui.is_selecting() || (self.is_scrolling && self.scroll_type != ScrollType::Key) {
                    return GameMessageDisposition::KeepMessage;
                }

                let num_dirs = self.scroll_dir.iter().filter(|&&dir| dir).count();
                if num_dirs > 0 && !self.is_scrolling {
                    self.set_scrolling(ScrollType::Key, ui, camera);
                } else if num_dirs == 0 && self.is_scrolling {
                    self.stop_scrolling(ui, camera);
                }
            }
            GameMessageType::RawMouseRightButtonDown(pos, ..) => {
                self.last_mouse_move_frame = ui.logic_frame;
                self.anchor = pos.clone();
                self.current_pos = pos.clone();
                if !ui.is_selecting() && !self.is_scrolling {
                    self.set_scrolling(ScrollType::Rmb, ui, camera);
                }
            }
            GameMessageType::RawMouseRightButtonUp(..) => {
                self.last_mouse_move_frame = ui.logic_frame;
                if self.scroll_type == ScrollType::Rmb {
                    self.stop_scrolling(ui, camera);
                }
            }
            GameMessageType::RawMouseMiddleButtonDown(pos, ..) => {
                self.last_mouse_move_frame = ui.logic_frame;
                self.is_rotating = true;
                self.anchor = pos.clone();
                self.original_anchor = pos.clone();
                self.current_pos = pos.clone();
                self.timestamp = ui.logic_frame;
            }
            GameMessageType::RawMouseMiddleButtonUp(..) => {
                self.last_mouse_move_frame = ui.logic_frame;
                self.is_rotating = false;
                let dx = (self.current_pos.x - self.original_anchor.x).abs();
                let dy = (self.current_pos.y - self.original_anchor.y).abs();
                let did_move = dx > MMB_CLICK_PIXEL_OFFSET || dy > MMB_CLICK_PIXEL_OFFSET;
                let is_short_click =
                    ui.logic_frame.wrapping_sub(self.timestamp) < MMB_CLICK_DURATION_FRAMES;
                if !did_move && is_short_click {
                    camera(CameraInputAction::ResetPose, ui);
                }
            }
            GameMessageType::RawMousePosition(pos) => {
                if self.current_pos.x != pos.x || self.current_pos.y != pos.y {
                    self.last_mouse_move_frame = ui.logic_frame;
                }
                self.current_pos = pos.clone();

                let dimensions = camera(CameraInputAction::Observe, ui);
                let (display_width, display_height) = (dimensions.width, dimensions.height);
                let (_, _, _, windowed, _) = self.get_global_scroll_factors();
                if !ui.get_input_enabled() {
                    if self.is_scrolling {
                        self.stop_scrolling(ui, camera);
                    }
                    return GameMessageDisposition::KeepMessage;
                }

                if !windowed {
                    if self.is_scrolling {
                        if self.scroll_type == ScrollType::ScreenEdge
                            && self.current_pos.x >= EDGE_SCROLL_SIZE
                            && self.current_pos.y >= EDGE_SCROLL_SIZE
                            && self.current_pos.y < display_height - EDGE_SCROLL_SIZE
                            && self.current_pos.x < display_width - EDGE_SCROLL_SIZE
                        {
                            self.stop_scrolling(ui, camera);
                        }
                    } else if self.current_pos.x < EDGE_SCROLL_SIZE
                        || self.current_pos.y < EDGE_SCROLL_SIZE
                        || self.current_pos.y >= display_height - EDGE_SCROLL_SIZE
                        || self.current_pos.x >= display_width - EDGE_SCROLL_SIZE
                    {
                        self.set_scrolling(ScrollType::ScreenEdge, ui, camera);
                    }
                }

                if self.is_rotating {
                    let delta = (self.current_pos.x - self.anchor.x) as f32 * 0.01;
                    camera(CameraInputAction::Rotate(delta), ui);
                    self.anchor = pos.clone();
                }
                if self.is_pitching {
                    let delta = (self.current_pos.y - self.anchor.y) as f32 * 0.01;
                    camera(CameraInputAction::Pitch(delta), ui);
                    self.anchor = pos.clone();
                }
            }
            GameMessageType::RawMouseWheel(spin) => {
                self.last_mouse_move_frame = ui.logic_frame;
                if *spin > 0 {
                    for _ in 0..*spin {
                        camera(CameraInputAction::ZoomIn, ui);
                    }
                } else if *spin < 0 {
                    for _ in 0..(-*spin) {
                        camera(CameraInputAction::ZoomOut, ui);
                    }
                }
                // C++ LookAtXlat.cpp:333-358 falls through into MSG_META_OPTIONS.
                self.stop_scrolling(ui, camera);
            }
            GameMessageType::MetaOptions => {
                self.stop_scrolling(ui, camera);
            }
            GameMessageType::FrameTick(..) => {
                self.handle_frame_tick(ui, camera, client.game_mode);
            }
            GameMessageType::MetaBeginCameraRotateLeft => {
                self.rotate_left = true;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndCameraRotateLeft => {
                self.rotate_left = false;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginCameraRotateRight => {
                self.rotate_right = true;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndCameraRotateRight => {
                self.rotate_right = false;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginCameraZoomIn => {
                self.zoom_in = true;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndCameraZoomIn => {
                self.zoom_in = false;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginCameraZoomOut => {
                self.zoom_out = true;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndCameraZoomOut => {
                self.zoom_out = false;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaCameraReset => {
                // C++ InGameUI::resetCamera keeps look-at and restores default pose.
                camera(CameraInputAction::ResetPose, ui);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaSaveView(slot) => {
                let index = (*slot as usize).saturating_sub(1);
                if index < self.view_location.len() {
                    self.view_location[index] = camera(CameraInputAction::Observe, ui).location;
                    let template = crate::game_text::GameText::fetch("GUI:BookmarkXSet");
                    let msg = if template.contains("%d") {
                        template.replacen("%d", &(*slot).to_string(), 1)
                    } else {
                        format!("{template} {slot}")
                    };
                    ui.message(&msg);
                }
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaViewView(slot) => {
                let index = (*slot as usize).saturating_sub(1);
                if index < self.view_location.len() && self.view_location[index].valid {
                    let location = self.view_location[index].clone();
                    camera(CameraInputAction::SetLocation(location), ui);
                }
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaToggleCameraTracking => {
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::SetReplayCamera(pos, angle, zoom) => {
                // C++ GameLogicDispatch.cpp:1801-1825 applies only during playback.
                let playback = ui.recorder_playback_active();
                let use_camera = global_data::read().use_camera_in_replay;
                if playback && use_camera {
                    let observed = camera(CameraInputAction::Observe, ui);
                    if observed.movement_finished {
                        let location = CameraLocation {
                            position: [pos.x, pos.y, pos.z],
                            angle: *angle,
                            pitch: observed.location.pitch,
                            zoom: *zoom,
                            valid: true,
                        };
                        camera(CameraInputAction::SetLocation(location), ui);
                    };
                }
            }
            _ => {}
        }

        GameMessageDisposition::KeepMessage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_move_recency_matches_cpp_one_second_window() {
        let mut translator = LookAtTranslator::new();
        translator.last_mouse_move_frame = 100;

        assert!(translator.has_mouse_moved_recently(130));
        assert!(!translator.has_mouse_moved_recently(131));
    }

    #[test]
    fn mouse_move_recency_resets_when_frame_counter_rewinds() {
        let mut translator = LookAtTranslator::new();
        translator.last_mouse_move_frame = 100;

        assert!(translator.has_mouse_moved_recently(10));
        assert_eq!(translator.last_mouse_move_frame, 0);
    }

    #[test]
    fn reset_modes_clears_scroll_rotate_pitch_fov() {
        let mut translator = LookAtTranslator::new();
        translator.is_scrolling = true;
        translator.is_rotating = true;
        translator.is_pitching = true;
        translator.is_changing_fov = true;
        translator.scroll_type = ScrollType::Rmb;
        translator.reset_modes();
        assert!(!translator.is_scrolling);
        assert!(!translator.is_rotating);
        assert!(!translator.is_pitching);
        assert!(!translator.is_changing_fov);
        assert_eq!(translator.scroll_type, ScrollType::None);
    }

    #[test]
    fn set_scrolling_locks_mouse_and_stop_restores_prev_cursor() {
        TheInGameUI::set_scrolling(false);
        TheInGameUI::set_mouse_cursor(MouseCursor::Waypoint);
        with_tactical_view(|view| {
            view.set_mouse_lock(false);
            view.set_camera_lock(Some(42));
            view.set_camera_lock_drawable(Some(7));
        });

        let mut translator = LookAtTranslator::new();
        translator.set_scrolling(ScrollType::Rmb);

        assert!(translator.is_scrolling);
        assert_eq!(translator.scroll_type, ScrollType::Rmb);
        assert!(TheInGameUI::is_scrolling());
        assert!(with_tactical_view_ref(|view| view.is_mouse_locked()));
        assert_eq!(translator.prev_cursor, MouseCursor::Waypoint);
        // C++ InGameUI::setScrolling(TRUE) clears lock + drawable.
        assert_eq!(with_tactical_view_ref(|view| view.camera_lock_id()), None);
        assert_eq!(
            with_tactical_view_ref(|view| view.camera_lock_drawable_id()),
            None
        );

        TheInGameUI::set_mouse_cursor(MouseCursor::Scroll);
        translator.stop_scrolling();

        assert!(!translator.is_scrolling);
        assert_eq!(translator.scroll_type, ScrollType::None);
        assert!(!TheInGameUI::is_scrolling());
        assert!(!with_tactical_view_ref(|view| view.is_mouse_locked()));
        assert_eq!(TheInGameUI::get_mouse_cursor(), MouseCursor::Waypoint);
    }

    #[test]
    fn mmb_click_thresholds_match_cpp() {
        assert_eq!(MMB_CLICK_DURATION_FRAMES, 5);
        assert_eq!(MMB_CLICK_PIXEL_OFFSET, 5);
    }

    #[test]
    fn wheel_falls_through_to_stop_scrolling() {
        TheInGameUI::set_mouse_cursor(MouseCursor::Selecting);
        let mut translator = LookAtTranslator::new();
        translator.set_scrolling(ScrollType::Rmb);
        let _ =
            translator.translate_game_message(&GameMessage::new(GameMessageType::RawMouseWheel(1)));
        assert!(!translator.is_scrolling);
        assert_eq!(translator.scroll_type, ScrollType::None);
    }

    #[test]
    fn frame_tick_emits_set_replay_camera_for_skirmish_when_save_flag() {
        // C++ LookAtXlat.cpp:459-467 appends camera to this source pass only.
        let previous = global_data::read().save_camera_in_replay;
        global_data::write().save_camera_in_replay = true;
        for mode in [0, 2, 3] {
            let mut state = crate::core::subsystems::InGameUISubsystem::default();
            let mut view = crate::display::view::View::new();
            let mut emitted = Vec::new();
            let mut ui = crate::message_stream::ReceivingUi::new(&mut state, 1, 1)
                .with_emitter(&mut emitted);
            let mut translator = LookAtTranslator::new();
            translator.handle_frame_tick(
                &mut ui,
                &mut |action, _| {
                    crate::gui::callbacks::control_bar_callbacks::apply_camera_input(
                        &mut view, action,
                    )
                },
                mode,
            );
            drop(ui);
            assert_eq!(
                emitted
                    .iter()
                    .any(|msg| matches!(msg.get_type(), GameMessageType::SetReplayCamera(..))),
                mode == 0 || mode == 2
            );
        }
        global_data::write().save_camera_in_replay = previous;
    }
}
