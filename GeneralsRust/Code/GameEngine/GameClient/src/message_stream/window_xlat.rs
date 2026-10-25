//! Window system translator for raw input messages.

use super::game_message::{GameMessage, GameMessageType, ICoord2D};
use super::message_stream::{
    GameMessageDisposition, GameMessageTranslator, WindowInputDispatch, WindowInputDispatchFn,
    WindowInputDispatchResult,
};
use crate::display::view::with_tactical_view_ref;
use crate::gui::game_window::{WindowInputReturnCode, WindowMessage};
use crate::gui::shell::get_shell;
use crate::helpers::TheInGameUI;
use game_engine::common::ini::get_global_data;

fn pack_legacy_mouse_data(x: i32, y: i32) -> u32 {
    ((y as u32) << 16) | ((x as u32) & 0xFFFF)
}

fn window_message_code(message: WindowMessage) -> u32 {
    match message {
        WindowMessage::None => 0,
        WindowMessage::LeftDown => 5,
        WindowMessage::LeftUp => 6,
        WindowMessage::LeftDrag => 8,
        WindowMessage::MiddleDown => 9,
        WindowMessage::MiddleUp => 10,
        WindowMessage::MiddleDrag => 12,
        WindowMessage::RightDown => 13,
        WindowMessage::RightUp => 14,
        WindowMessage::RightDrag => 16,
        WindowMessage::WheelUp => 19,
        WindowMessage::WheelDown => 20,
        _ => unreachable!("WindowTranslator dispatches only native mouse messages"),
    }
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

fn should_keep_message_while_mouse_locked(msg_type: &GameMessageType, scrolling: bool) -> bool {
    if scrolling {
        !is_legacy_left_mouse_message(msg_type)
    } else {
        true
    }
}

fn is_legacy_left_window_message(msg: WindowMessage) -> bool {
    matches!(msg, WindowMessage::LeftDown | WindowMessage::LeftUp)
}

/// C++ `WindowTranslator::translateGameMessage` (WindowXlat.cpp:147-167):
/// while `TheTacticalView->isMouseLocked()`, keep the stream message (skip
/// `winProcessMouseEvent`) unless `TheInGameUI->isScrolling()` and the event
/// is LMB down/up so ControlBar clicks still land during keyboard/RMB scroll.
pub fn should_skip_os_window_manager_while_mouse_locked(
    msg: WindowMessage,
    scrolling: bool,
) -> bool {
    if scrolling {
        !is_legacy_left_window_message(msg)
    } else {
        true
    }
}

/// Live OS-dispatch gate for the unused WindowXlat mouse-lock pass-through.
///
/// Returns `true` when `dispatch_os_mouse_to_window_manager` must not call
/// `process_mouse_event` (C++ KEEP_MESSAGE).
pub fn os_mouse_blocked_by_mouse_lock(msg: WindowMessage) -> bool {
    is_mouse_locked()
        && should_skip_os_window_manager_while_mouse_locked(msg, TheInGameUI::is_scrolling())
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
        let mut no_dispatch = |_: WindowInputDispatch, _: &mut Vec<GameMessage>| None;
        self.translate_with_dispatch(msg, &mut no_dispatch, &mut Vec::new())
    }

    fn translate_game_message_with_context(
        &mut self,
        msg: &GameMessage,
        _client: game_engine::common::message_stream::TranslatorClientObservation,
        dispatch_window_input: &mut WindowInputDispatchFn<'_>,
        emitted_messages: &mut Vec<GameMessage>,
    ) -> GameMessageDisposition {
        self.translate_with_dispatch(msg, dispatch_window_input, emitted_messages)
    }
}

impl WindowTranslator {
    fn translate_with_dispatch(
        &mut self,
        msg: &GameMessage,
        dispatch_window_input: &mut WindowInputDispatchFn<'_>,
        _emitted_messages: &mut Vec<GameMessage>,
    ) -> GameMessageDisposition {
        let msg_type = msg.get_type();
        let mut return_code = WindowInputReturnCode::NotUsed;
        let mut force_keep_message = false;

        if is_mouse_locked()
            && should_keep_message_while_mouse_locked(msg_type, TheInGameUI::is_scrolling())
        {
            return GameMessageDisposition::KeepMessage;
        }

        match msg_type {
            GameMessageType::MetaToggleAttackMove => {
                let _ = dispatch_window_input(
                    WindowInputDispatch::Mouse {
                        message: window_message_code(WindowMessage::None),
                        x: self.last_mouse_pos.x,
                        y: self.last_mouse_pos.y,
                        data: 0,
                        delta: None,
                    },
                    _emitted_messages,
                );
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

                return_code = match dispatch_window_input(
                    WindowInputDispatch::Mouse {
                        message: window_message_code(window_msg),
                        x: self.last_mouse_pos.x,
                        y: self.last_mouse_pos.y,
                        data: pack_legacy_mouse_data(self.last_mouse_pos.x, self.last_mouse_pos.y),
                        delta: extract_mouse_delta(msg_type),
                    },
                    _emitted_messages,
                ) {
                    Some(WindowInputDispatchResult::Used) => WindowInputReturnCode::Used,
                    Some(WindowInputDispatchResult::Display(_)) => {
                        unreachable!("display response returned for a window input request")
                    }
                    Some(WindowInputDispatchResult::NotUsed) | None => {
                        WindowInputReturnCode::NotUsed
                    }
                };

                if get_shell().is_shell_active() {
                    return_code = WindowInputReturnCode::Used;
                }

                if !TheInGameUI::get_input_enabled() {
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
                return_code = match dispatch_window_input(
                    WindowInputDispatch::Key {
                        key: *key as u8,
                        state,
                    },
                    _emitted_messages,
                ) {
                    Some(WindowInputDispatchResult::Used) => WindowInputReturnCode::Used,
                    Some(WindowInputDispatchResult::Display(_)) => {
                        unreachable!("display response returned for a window input request")
                    }
                    Some(WindowInputDispatchResult::NotUsed) | None => {
                        WindowInputReturnCode::NotUsed
                    }
                };

                if return_code != WindowInputReturnCode::Used
                    && *key == KEY_ESC
                    && (state & KEY_STATE_UP) != 0
                {
                    let movie_playing = matches!(dispatch_window_input(
                        WindowInputDispatch::Display(game_engine::common::message_stream::DisplayInputAction::IsMoviePlaying),
                        _emitted_messages,
                    ), Some(WindowInputDispatchResult::Display(true)));
                    let allow_exit = get_global_data()
                        .map(|global_data| global_data.read().allow_exit_out_of_movies)
                        .unwrap_or(false);

                    if movie_playing && allow_exit {
                        let _ = dispatch_window_input(
                            WindowInputDispatch::Display(
                                game_engine::common::message_stream::DisplayInputAction::StopMovie,
                            ),
                            _emitted_messages,
                        );
                        return_code = WindowInputReturnCode::Used;
                    }
                }

                if return_code != WindowInputReturnCode::Used
                    && *key == KEY_ESC
                    && (state & KEY_STATE_UP) != 0
                    && !TheInGameUI::get_input_enabled()
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

    #[test]
    fn locked_mouse_without_scrolling_keeps_left_clicks_like_cpp() {
        let left_down = GameMessageType::RawMouseLeftButtonDown(ICoord2D { x: 1, y: 2 }, 0, 0);
        let left_up = GameMessageType::RawMouseLeftButtonUp(ICoord2D { x: 1, y: 2 }, 0, 0);
        let position = GameMessageType::RawMousePosition(ICoord2D { x: 1, y: 2 });

        assert!(should_keep_message_while_mouse_locked(&left_down, false));
        assert!(should_keep_message_while_mouse_locked(&left_up, false));
        assert!(should_keep_message_while_mouse_locked(&position, false));
    }

    #[test]
    fn locked_mouse_while_scrolling_only_passes_left_clicks_like_cpp() {
        let left_down = GameMessageType::RawMouseLeftButtonDown(ICoord2D { x: 1, y: 2 }, 0, 0);
        let left_up = GameMessageType::RawMouseLeftButtonUp(ICoord2D { x: 1, y: 2 }, 0, 0);
        let right_down = GameMessageType::RawMouseRightButtonDown(ICoord2D { x: 1, y: 2 }, 0, 0);
        let drag =
            GameMessageType::RawMouseLeftDrag(ICoord2D { x: 1, y: 2 }, ICoord2D { x: 2, y: 3 });

        assert!(!should_keep_message_while_mouse_locked(&left_down, true));
        assert!(!should_keep_message_while_mouse_locked(&left_up, true));
        assert!(should_keep_message_while_mouse_locked(&right_down, true));
        assert!(should_keep_message_while_mouse_locked(&drag, true));
    }

    #[test]
    fn locked_os_dispatch_while_scrolling_only_passes_left_clicks_like_cpp() {
        // C++ WindowXlat.cpp:147-167 — mouse-locked + isScrolling lets LMB
        // reach winProcessMouseEvent so ControlBar clicks are not issued
        // through the HUD.
        assert!(!should_skip_os_window_manager_while_mouse_locked(
            WindowMessage::LeftDown,
            true
        ));
        assert!(!should_skip_os_window_manager_while_mouse_locked(
            WindowMessage::LeftUp,
            true
        ));
        assert!(should_skip_os_window_manager_while_mouse_locked(
            WindowMessage::RightDown,
            true
        ));
        assert!(should_skip_os_window_manager_while_mouse_locked(
            WindowMessage::LeftDrag,
            true
        ));
        assert!(should_skip_os_window_manager_while_mouse_locked(
            WindowMessage::MousePos,
            true
        ));
        assert!(should_skip_os_window_manager_while_mouse_locked(
            WindowMessage::LeftDown,
            false
        ));
    }

    #[test]
    fn live_os_dispatch_uses_window_xlat_mouse_lock_pass_through() {
        let dispatch = include_str!("../gui/window_manager/reentry.rs");
        let start = dispatch
            .find("pub fn dispatch_os_mouse_to_window_manager")
            .expect("live OS mouse dispatch");
        let body = &dispatch[start..];
        let end = body
            .find("pub fn dispatch_os_key_to_window_manager")
            .unwrap_or(body.len());
        let body = &body[..end];
        assert!(body.contains("dispatch_os_mouse_to_window_manager_with_facts"));
        assert!(body.contains("let mouse_locked = crate::display::view::with_tactical_view_ref"));
        let native = body
            .split_once("pub fn dispatch_os_mouse_to_window_manager_with_facts(")
            .unwrap()
            .1
            .split_once("fn dispatch_mouse_with_facts(")
            .unwrap()
            .0;
        assert!(native.contains("dispatch_mouse_with_facts("));
        assert!(native.contains("manager.process_mouse_event(msg, x, y, data)"));
        assert!(!native.contains("with_tactical_view"));
        let gate = body.split_once("fn dispatch_mouse_with_facts(").unwrap().1;
        assert!(gate.contains("facts.mouse_locked") && gate.contains("facts.scrolling"));
        assert!(
            gate.find("should_skip_os_window_manager_while_mouse_locked")
                .expect("source lock gate")
                < gate
                    .find("let rc = dispatch(")
                    .expect("actual callback dispatch"),
            "receiving lock gate must precede callback dispatch"
        );
    }
}
