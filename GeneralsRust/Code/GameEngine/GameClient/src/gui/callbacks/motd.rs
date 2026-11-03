//! W3DMOTD.cpp callback port.

use crate::gui::{GameWindow, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus};
use game_engine::common::name_key_generator::{NAMEKEY_INVALID, NameKeyGenerator};
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static TEST_CLOSE_BUTTON_ID: Cell<u32> = const { Cell::new(NAMEKEY_INVALID) };
}

fn close_button_id_on_owner(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) -> u32 {
    if let Some(owner) = owner {
        return owner.motd_close_button_id();
    }
    if let Some(id) = crate::core::game_client::with_live_game_client_mut(|client| {
        client.window_manager_owner_ref().motd_close_button_id()
    }) {
        return id;
    }
    #[cfg(test)]
    {
        return TEST_CLOSE_BUTTON_ID.with(Cell::get);
    }
    #[cfg(not(test))]
    {
        NAMEKEY_INVALID
    }
}

fn set_close_button_id_on_owner(
    owner: Option<&crate::gui::window_manager::WindowManagerOwner>,
    id: u32,
) {
    if let Some(owner) = owner {
        owner.set_motd_close_button_id(id);
        return;
    }
    if let Some(()) = crate::core::game_client::with_live_game_client_mut(|client| {
        client
            .window_manager_owner_ref()
            .set_motd_close_button_id(id);
    }) {
        return;
    }
    #[cfg(test)]
    {
        TEST_CLOSE_BUTTON_ID.with(|cell| cell.set(id));
        return;
    }
    #[cfg(not(test))]
    {
        let _ = id;
    }
}

fn close_button_id() -> u32 {
    close_button_id_on_owner(None)
}

fn set_close_button_id(id: u32) {
    set_close_button_id_on_owner(None, id);
}

/// Message of the day window system callback.
///
/// Mirrors C++ `MOTDSystem`: cache the close button id on create, do nothing
/// on destroy, and toggle the parent window when the close button is selected.
pub fn motd_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
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
                (|manager: &mut crate::gui::window_manager::WindowManager| {
                    if let Some(live_window) = manager.get_window_by_id(window_id) {
                        let hidden = live_window.borrow().is_hidden();
                        let _ = live_window.borrow_mut().hide(!hidden, manager);
                    }
                })(receiving_manager);
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
        let mut window = GameWindow::new();
        assert_eq!(
            motd_system(&mut window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );

        assert_eq!(
            close_button_id(),
            NameKeyGenerator::name_to_key("MOTD.wnd:CloseMOTD")
        );
    }

    #[test]
    fn motd_selected_close_button_toggles_window_hidden_state() {
        let owner = crate::gui::window_manager::WindowManagerOwner::new();
        { // was owner.with_scope
            let live_window =
                owner.with_manager(|manager| manager.test_create_window(None, 0, 0, 100, 100).unwrap());

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
    }

    #[test]
    fn motd_ignores_unknown_messages() {
        let mut window = GameWindow::new();
        assert_eq!(
            motd_system(&mut window, WindowMessage::None, 0, 0),
            WindowMsgHandled::Ignored
        );
    }
}
