//! Window system translator for raw input messages.

use super::game_message::{GameMessage, GameMessageType, ICoord2D};
use super::message_stream::{GameMessageDisposition, GameMessageTranslator};
use crate::gui::game_window::{WindowInputReturnCode, WindowMessage};
use crate::gui::shell::get_shell;
use crate::gui::window_manager::with_window_manager;

fn pack_legacy_mouse_data(x: i32, y: i32) -> u32 {
    ((y as u32) << 16) | ((x as u32) & 0xFFFF)
}

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
        GameMessageType::RawMouseLeftDrag(start, ..)
        | GameMessageType::RawMouseMiddleDrag(start, ..)
        | GameMessageType::RawMouseRightDrag(start, ..) => Some(start.clone()),
        _ => None,
    }
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
                if let Some(pos) = extract_mouse_position(msg_type) {
                    self.last_mouse_pos = pos.clone();
                }

                let Some(window_msg) = raw_mouse_to_window_message(msg_type) else {
                    return GameMessageDisposition::KeepMessage;
                };

                with_window_manager(|manager| {
                    return_code = manager.process_mouse_event(
                        window_msg,
                        self.last_mouse_pos.x,
                        self.last_mouse_pos.y,
                        pack_legacy_mouse_data(self.last_mouse_pos.x, self.last_mouse_pos.y),
                    );
                });

                if get_shell().is_shell_active() {
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
            }
            _ => {}
        }

        if return_code == WindowInputReturnCode::Used {
            GameMessageDisposition::DestroyMessage
        } else {
            GameMessageDisposition::KeepMessage
        }
    }
}
