//! Window system translator for raw input messages.

use super::game_message::{GameMessage, GameMessageType, ICoord2D};
use super::message_stream::{GameMessageDisposition, GameMessageTranslator};
use crate::core::script_action_handler::{
    is_script_display_movie_playing, stop_script_display_movie,
};
use crate::display::view::with_tactical_view_ref;
use crate::gui::game_window::{WindowInputReturnCode, WindowMessage};
use crate::gui::shell::get_shell;
use crate::gui::window_manager::with_window_manager;
use crate::helpers::TheInGameUI;
use game_engine::common::ini::get_global_data;

fn pack_legacy_mouse_data(x: i32, y: i32) -> u32 {
    ((y as u32) << 16) | ((x as u32) & 0xFFFF)
}

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u8 = 0x01;

fn raw_mouse_to_window_message(msg_type: &GameMessageType) -> Option<WindowMessage> {
    match msg_type {
        GameMessageType::RawMousePosition(_) => Some(WindowMessage::MousePos),
        GameMessageType::RawMouseLeftDoubleClick(..)
        | GameMessageType::RawMouseLeftButtonDown(..) => Some(WindowMessage::LeftDown),
        GameMessageType::RawMouseLeftButtonUp(..) => Some(WindowMessage::LeftUp),
        GameMessageType::RawMouseLeftDrag(..) => Some(WindowMessage::LeftDrag),
        GameMessageType::RawMouseMiddleDoubleClick(..)
        | GameMessageType::RawMouseMiddleButtonDown(..) => Some(WindowMessage::MiddleDown),
        GameMessageType::RawMouseMiddleButtonUp(..) => Some(WindowMessage::MiddleUp),
        GameMessageType::RawMouseMiddleDrag(..) => Some(WindowMessage::MiddleDrag),
        GameMessageType::RawMouseRightDoubleClick(..)
        | GameMessageType::RawMouseRightButtonDown(..) => Some(WindowMessage::RightDown),
        GameMessageType::RawMouseRightButtonUp(..) => Some(WindowMessage::RightUp),
        GameMessageType::RawMouseRightDrag(..) => Some(WindowMessage::RightDrag),
        GameMessageType::RawMouseWheel(delta) => {
            if *delta > 0 {
                Some(WindowMessage::WheelUp)
            } else if *delta < 0 {
                Some(WindowMessage::WheelDown)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_mouse_position(msg_type: &GameMessageType) -> Option<ICoord2D> {
    match msg_type {
        GameMessageType::RawMousePosition(pos)
        | GameMessageType::RawMouseLeftButtonDown(pos, ..)
        | GameMessageType::RawMouseLeftDoubleClick(pos, ..)
        | GameMessageType::RawMouseLeftButtonUp(pos, ..)
        | GameMessageType::RawMouseMiddleButtonDown(pos, ..)
        | GameMessageType::RawMouseMiddleDoubleClick(pos, ..)
        | GameMessageType::RawMouseMiddleButtonUp(pos, ..)
        | GameMessageType::RawMouseRightButtonDown(pos, ..)
        | GameMessageType::RawMouseRightDoubleClick(pos, ..)
        | GameMessageType::RawMouseRightButtonUp(pos, ..) => Some(pos.clone()),
        GameMessageType::RawMouseLeftDrag(_, end)
        | GameMessageType::RawMouseMiddleDrag(_, end)
        | GameMessageType::RawMouseRightDrag(_, end) => Some(end.clone()),
        _ => None,
    }
}

fn extract_mouse_delta(msg_type: &GameMessageType) -> Option<(i32, i32)> {
    match msg_type {
        GameMessageType::RawMouseLeftDrag(start, end)
        | GameMessageType::RawMouseMiddleDrag(start, end)
        | GameMessageType::RawMouseRightDrag(start, end) => {
            Some((end.x - start.x, end.y - start.y))
        }
        _ => None,
    }
}

fn is_mouse_locked() -> bool {
    with_tactical_view_ref(|view| view.is_mouse_locked())
}

fn is_legacy_left_mouse_message(msg_type: &GameMessageType) -> bool {
    matches!(
        msg_type,
        GameMessageType::RawMouseLeftButtonDown(..) | GameMessageType::RawMouseLeftButtonUp(..)
    )
}

#[derive(Default)]
pub struct WindowTranslator {
    last_mouse_pos: ICoord2D,
}

impl WindowTranslator {
    pub fn new() -> Self {
        Self {
            last_mouse_pos: ICoord2D { x: 0, y: 0 },
        }
    }
}

impl GameMessageTranslator for WindowTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        let msg_type = msg.get_type();
        let mut return_code = WindowInputReturnCode::NotUsed;
        let mut force_keep_message = false;

        if is_mouse_locked() && !is_legacy_left_mouse_message(msg_type) {
            return GameMessageDisposition::KeepMessage;
        }

        match msg_type {
            GameMessageType::MetaToggleAttackMove => {
                with_window_manager(|manager| {
                    manager.process_mouse_event(
                        WindowMessage::None,
                        self.last_mouse_pos.x,
                        self.last_mouse_pos.y,
                        0,
                    );
                });
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::RawMousePosition(..)
            | GameMessageType::RawMouseLeftButtonDown(..)
            | GameMessageType::RawMouseLeftDoubleClick(..)
            | GameMessageType::RawMouseLeftButtonUp(..)
            | GameMessageType::RawMouseMiddleButtonDown(..)
            | GameMessageType::RawMouseMiddleDoubleClick(..)
            | GameMessageType::RawMouseMiddleButtonUp(..)
            | GameMessageType::RawMouseRightButtonDown(..)
            | GameMessageType::RawMouseRightDoubleClick(..)
            | GameMessageType::RawMouseRightButtonUp(..)
            | GameMessageType::RawMouseLeftDrag(..)
            | GameMessageType::RawMouseMiddleDrag(..)
            | GameMessageType::RawMouseRightDrag(..)
            | GameMessageType::RawMouseWheel(..) => {
                if matches!(msg_type, GameMessageType::RawMouseLeftButtonUp(..))
                    && TheInGameUI::is_placement_anchored()
                {
                    force_keep_message = true;
                }

                if let Some(pos) = extract_mouse_position(msg_type) {
                    self.last_mouse_pos = pos.clone();
                }

                let Some(window_msg) = raw_mouse_to_window_message(msg_type) else {
                    return GameMessageDisposition::KeepMessage;
                };

                with_window_manager(|manager| {
                    return_code = manager.process_mouse_event_with_delta(
                        window_msg,
                        self.last_mouse_pos.x,
                        self.last_mouse_pos.y,
                        pack_legacy_mouse_data(self.last_mouse_pos.x, self.last_mouse_pos.y),
                        extract_mouse_delta(msg_type),
                    );
                });

                if get_shell().is_shell_active() {
                    return_code = WindowInputReturnCode::Used;
                }

                if TheInGameUI::get_input_enabled() == false {
                    return_code = WindowInputReturnCode::Used;
                }
            }
            GameMessageType::RawKeyDown(key) | GameMessageType::RawKeyUp(key) => {
                let key_state = match msg.get_argument(1) {
                    Some(super::game_message::GameMessageArgumentType::Integer(value)) => {
                        *value as u32
                    }
                    _ => 0,
                };
                let state = (key_state & 0xFF) as u8;
                with_window_manager(|manager| {
                    return_code = manager.process_key_event(*key as u8, state);
                });

                if return_code != WindowInputReturnCode::Used
                    && *key == KEY_ESC
                    && (state & KEY_STATE_UP) != 0
                {
                    let movie_playing = is_script_display_movie_playing();
                    let allow_exit = get_global_data()
                        .map(|global_data| global_data.read().allow_exit_out_of_movies)
                        .unwrap_or(false);

                    if movie_playing && allow_exit {
                        let _ = stop_script_display_movie();
                        return_code = WindowInputReturnCode::Used;
                    }
                }

                if return_code != WindowInputReturnCode::Used
                    && *key == KEY_ESC
                    && (state & KEY_STATE_UP) != 0
                    && TheInGameUI::get_input_enabled() == false
                {
                    return_code = WindowInputReturnCode::Used;
                }
            }
            _ => {}
        }

        if return_code == WindowInputReturnCode::Used && !force_keep_message {
            GameMessageDisposition::DestroyMessage
        } else {
            GameMessageDisposition::KeepMessage
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_legacy_mouse_data_matches_low_level_layout() {
        assert_eq!(pack_legacy_mouse_data(12, 34), (34u32 << 16) | 12u32);
    }

    #[test]
    fn raw_mouse_wheel_zero_is_ignored() {
        assert_eq!(
            raw_mouse_to_window_message(&GameMessageType::RawMouseWheel(0)),
            None
        );
    }

    #[test]
    fn raw_mouse_drag_uses_end_position_and_delta_like_cpp() {
        let msg =
            GameMessageType::RawMouseLeftDrag(ICoord2D { x: 10, y: 20 }, ICoord2D { x: 14, y: 17 });

        assert_eq!(
            extract_mouse_position(&msg),
            Some(ICoord2D { x: 14, y: 17 })
        );
        assert_eq!(extract_mouse_delta(&msg), Some((4, -3)));
    }
}
