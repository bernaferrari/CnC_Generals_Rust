//! Look-at translator for camera movement and scrolling.

use super::game_message::{
    Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, ICoord2D,
};
use super::message_stream::{emit_message, GameMessageDisposition, GameMessageTranslator};
use crate::display::view::{with_tactical_view, with_tactical_view_ref, ViewLocation};
use crate::gui::get_shell;
use crate::helpers::TheInGameUI;
use game_engine::common::game_common::LOGICFRAMES_PER_SECOND;
use game_engine::common::ini::ini_game_data::get_global_data;
use gamelogic::helpers::TheGameLogic;

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
    view_location: [ViewLocation; MAX_VIEW_LOCS],
    scroll_type: ScrollType,
    scroll_dir: [bool; 4],
    last_mouse_move_frame: u32,
    rotate_left: bool,
    rotate_right: bool,
    zoom_in: bool,
    zoom_out: bool,
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

    fn set_scrolling(&mut self, scroll_type: ScrollType) {
        if !TheInGameUI::get_input_enabled() {
            return;
        }
        self.is_scrolling = true;
        self.scroll_type = scroll_type;
        TheInGameUI::set_scrolling(true);
    }

    fn stop_scrolling(&mut self) {
        self.is_scrolling = false;
        self.scroll_type = ScrollType::None;
        TheInGameUI::set_scrolling(false);
        TheInGameUI::set_cursor_arrow();
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

    fn handle_frame_tick(&mut self) {
        let (display_width, display_height) =
            with_tactical_view_ref(|view| (view.width(), view.height()));
        let (h_factor, v_factor, key_factor, _windowed, cutoff) = self.get_global_scroll_factors();

        let mut offset = Coord2D::zero();
        if self.is_scrolling && !TheInGameUI::is_scrolling() {
            TheInGameUI::set_scroll_amount(0.0, 0.0);
            self.stop_scrolling();
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

            TheInGameUI::set_scroll_amount(offset.x, offset.y);
            with_tactical_view(|view| {
                view.scroll_by(&crate::display::view::Vector2::new(offset.x, offset.y));
            });
        } else {
            TheInGameUI::set_scroll_amount(0.0, 0.0);
        }

        with_tactical_view(|view| {
            if self.rotate_left {
                view.set_angle(view.angle() - 0.02);
            }
            if self.rotate_right {
                view.set_angle(view.angle() + 0.02);
            }
            if self.zoom_in {
                view.zoom_in();
            }
            if self.zoom_out {
                view.zoom_out();
            }
        });
    }
}

impl GameMessageTranslator for LookAtTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        let msg_type = msg.get_type();
        match msg_type {
            GameMessageType::RawKeyDown(key) | GameMessageType::RawKeyUp(key) => {
                if get_shell().is_shell_active() {
                    return GameMessageDisposition::KeepMessage;
                }
                let key_state = match msg.get_argument(1) {
                    Some(GameMessageArgumentType::Integer(value)) => *value as u32,
                    _ => 0,
                };
                let pressed = (key_state & 0x0001) == 0;
                self.update_scroll_dir(*key, pressed);

                if TheInGameUI::is_selecting()
                    || (self.is_scrolling && self.scroll_type != ScrollType::Key)
                {
                    return GameMessageDisposition::KeepMessage;
                }

                let num_dirs = self.scroll_dir.iter().filter(|&&dir| dir).count();
                if num_dirs > 0 && !self.is_scrolling {
                    self.set_scrolling(ScrollType::Key);
                } else if num_dirs == 0 && self.is_scrolling {
                    self.stop_scrolling();
                }
            }
            GameMessageType::RawMouseRightButtonDown(pos, ..) => {
                self.last_mouse_move_frame = TheGameLogic::get_frame();
                self.anchor = pos.clone();
                self.current_pos = pos.clone();
                if !TheInGameUI::is_selecting() && !self.is_scrolling {
                    self.set_scrolling(ScrollType::Rmb);
                }
            }
            GameMessageType::RawMouseRightButtonUp(..) => {
                self.last_mouse_move_frame = TheGameLogic::get_frame();
                if self.scroll_type == ScrollType::Rmb {
                    self.stop_scrolling();
                }
            }
            GameMessageType::RawMouseMiddleButtonDown(pos, ..) => {
                self.last_mouse_move_frame = TheGameLogic::get_frame();
                self.is_rotating = true;
                self.anchor = pos.clone();
                self.original_anchor = pos.clone();
                self.current_pos = pos.clone();
                self.timestamp = TheGameLogic::get_frame();
            }
            GameMessageType::RawMouseMiddleButtonUp(..) => {
                self.last_mouse_move_frame = TheGameLogic::get_frame();
                self.is_rotating = false;
                let dx = (self.current_pos.x - self.original_anchor.x).abs();
                let dy = (self.current_pos.y - self.original_anchor.y).abs();
                let did_move = dx > MMB_CLICK_PIXEL_OFFSET || dy > MMB_CLICK_PIXEL_OFFSET;
                let is_short_click = TheGameLogic::get_frame().wrapping_sub(self.timestamp)
                    < MMB_CLICK_DURATION_FRAMES;
                if !did_move && is_short_click {
                    with_tactical_view(|view| {
                        view.set_angle_and_pitch_to_default();
                        view.set_zoom_to_default();
                    });
                }
            }
            GameMessageType::RawMousePosition(pos) => {
                if self.current_pos.x != pos.x || self.current_pos.y != pos.y {
                    self.last_mouse_move_frame = TheGameLogic::get_frame();
                }
                self.current_pos = pos.clone();

                let (display_width, display_height) =
                    with_tactical_view_ref(|view| (view.width(), view.height()));
                let (_, _, _, windowed, _) = self.get_global_scroll_factors();
                if !TheInGameUI::get_input_enabled() {
                    if self.is_scrolling {
                        self.stop_scrolling();
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
                            self.stop_scrolling();
                        }
                    } else if self.current_pos.x < EDGE_SCROLL_SIZE
                        || self.current_pos.y < EDGE_SCROLL_SIZE
                        || self.current_pos.y >= display_height - EDGE_SCROLL_SIZE
                        || self.current_pos.x >= display_width - EDGE_SCROLL_SIZE
                    {
                        self.set_scrolling(ScrollType::ScreenEdge);
                    }
                }

                if self.is_rotating {
                    let delta = (self.current_pos.x - self.anchor.x) as f32 * 0.01;
                    with_tactical_view(|view| view.set_angle(view.angle() + delta));
                    self.anchor = pos.clone();
                }
                if self.is_pitching {
                    let delta = (self.current_pos.y - self.anchor.y) as f32 * 0.01;
                    with_tactical_view(|view| view.set_pitch(view.pitch() + delta));
                    self.anchor = pos.clone();
                }
            }
            GameMessageType::RawMouseWheel(spin) => {
                self.last_mouse_move_frame = TheGameLogic::get_frame();
                if *spin > 0 {
                    for _ in 0..*spin {
                        with_tactical_view(|view| view.zoom_in());
                    }
                } else if *spin < 0 {
                    for _ in 0..(-*spin) {
                        with_tactical_view(|view| view.zoom_out());
                    }
                }
            }
            GameMessageType::FrameTick(..) => {
                self.handle_frame_tick();
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
                with_tactical_view(|view| view.set_angle_and_pitch_to_default());
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaSaveView(slot) => {
                let index = (*slot as usize).saturating_sub(1);
                if index < self.view_location.len() {
                    self.view_location[index] = with_tactical_view_ref(|view| view.get_location());
                }
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaViewView(slot) => {
                let index = (*slot as usize).saturating_sub(1);
                if index < self.view_location.len() && self.view_location[index].is_valid() {
                    let location = self.view_location[index].clone();
                    with_tactical_view(|view| view.set_location(&location));
                }
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaToggleCameraTracking => {
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::SetReplayCamera(pos, angle, zoom) => {
                with_tactical_view(|view| {
                    let mut location = ViewLocation::new();
                    location.init(pos.x, pos.y, pos.z, *angle, view.pitch(), *zoom);
                    view.set_location(&location);
                });
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
}
