//! W3DMOTD.cpp callback port.

use crate::gui::{
    with_window_manager, GameWindow, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
};
use game_engine::common::name_key_generator::{NameKeyGenerator, NAMEKEY_INVALID};
use std::cell::Cell;

thread_local! {
    static CLOSE_BUTTON_ID: Cell<u32> = const { Cell::new(NAMEKEY_INVALID) };
}

fn close_button_id() -> u32 {
    CLOSE_BUTTON_ID.with(Cell::get)
}

fn set_close_button_id(id: u32) {
    CLOSE_BUTTON_ID.with(|cell| cell.set(id));
}

/// Message of the day window system callback.
///
/// Mirrors C++ `MOTDSystem`: cache the close button id on create, do nothing
/// on destroy, and toggle the parent window when the close button is selected.
pub fn motd_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => {
            set_close_button_id(NameKeyGenerator::name_to_key("MOTD.wnd:CloseMOTD"));
            WindowMsgHandled::Handled
        }
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            if data1 as u32 == close_button_id() {
                let window_id = window.get_id();
                with_window_manager(|manager| {
                    if let Some(live_window) = manager.get_window_by_id(window_id) {
                        let hidden = live_window.borrow().is_hidden();
                        let _ = live_window.borrow_mut().hide(!hidden);
                    }
                });
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motd_create_caches_close_button_key() {
        let window = GameWindow::new();
        assert_eq!(
            motd_system(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );

        assert_eq!(
            close_button_id(),
            NameKeyGenerator::name_to_key("MOTD.wnd:CloseMOTD")
        );
    }

    #[test]
    fn motd_selected_close_button_toggles_window_hidden_state() {
        let live_window =
            with_window_manager(|manager| manager.create_window(None, 0, 0, 100, 100).unwrap());

        let mut window = GameWindow::new();
        window.set_id(live_window.borrow().get_id());
        window.set_status(WindowStatus::ENABLED);

        let close_id = NameKeyGenerator::name_to_key("MOTD.wnd:CloseMOTD");
        set_close_button_id(close_id);

        assert!(!live_window.borrow().is_hidden());
        assert_eq!(
            motd_system(
                &window,
                WindowMessage::GadgetSelected,
                close_id as WindowMsgData,
                0,
            ),
            WindowMsgHandled::Handled
        );
        assert!(live_window.borrow().is_hidden());

        assert_eq!(
            motd_system(
                &window,
                WindowMessage::GadgetSelected,
                close_id as WindowMsgData,
                0,
            ),
            WindowMsgHandled::Handled
        );
        assert!(!live_window.borrow().is_hidden());
    }

    #[test]
    fn motd_ignores_unknown_messages() {
        let window = GameWindow::new();
        assert_eq!(
            motd_system(&window, WindowMessage::None, 0, 0),
            WindowMsgHandled::Ignored
        );
    }
}
